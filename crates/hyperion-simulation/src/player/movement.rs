use bevy_ecs::{
    entity::Entity,
    lifecycle::Insert,
    message::MessageReader,
    observer::On,
    query::Changed,
    system::{Commands, ParamSet, Query, Res},
    world::World,
};
use glam::{DVec3, Vec3};
use hyperion_entity::{
    EntitySize, Flight, PendingTeleportation, Pitch, Position, Yaw, metadata::entity::Pose,
};
use hyperion_net::{
    Compose,
    packet::{self, OrderedPacketRef},
};
use hyperion_proxy_proto::ConnectionId;
use hyperion_utils::next_lowest;
use hyperion_world::Blocks;
use tracing::{error, warn};
use valence_protocol::{
    VarInt,
    packets::play::{
        self, PlayerAbilitiesS2c, client_command_c2s::ClientCommand,
        player_abilities_s2c::PlayerAbilitiesFlags,
        player_position_look_s2c::PlayerPositionLookFlags,
    },
};
use valence_text::IntoText;

use crate::player::MovementTracking;

pub(crate) fn send_pending_teleportation(
    now_teleporting: On<'_, '_, Insert, PendingTeleportation>,
    query: Query<'_, '_, (&PendingTeleportation, &Yaw, &Pitch, &ConnectionId)>,
    compose: Res<'_, Compose>,
) {
    let (pending_teleportation, yaw, pitch, &connection) = match query.get(now_teleporting.entity) {
        Ok(data) => data,
        Err(e) => {
            error!("failed to send pending teleportation: query failed: {e}");
            return;
        }
    };

    let pkt = play::PlayerPositionLookS2c {
        position: pending_teleportation.destination.as_dvec3(),
        yaw: **yaw,
        pitch: **pitch,
        flags: PlayerPositionLookFlags::default(),
        teleport_id: VarInt(pending_teleportation.teleport_id),
    };

    compose.unicast(&pkt, connection).unwrap();
}

pub(crate) fn update_flight(
    compose: Res<'_, Compose>,
    query: Query<'_, '_, (&ConnectionId, &Flight), Changed<Flight>>,
) {
    for (&connection_id, flight) in &query {
        let pkt = PlayerAbilitiesS2c {
            flags: PlayerAbilitiesFlags::default()
                .with_allow_flying(flight.allow)
                .with_flying(flight.is_flying),
            flying_speed: flight.speed,
            fov_modifier: 0.0,
        };

        compose.unicast(&pkt, connection_id).unwrap();
    }
}

#[expect(
    clippy::cognitive_complexity,
    reason = "This cannot be split into different systems because the events across the various \
              MessageReaders must be processed in order. Splitting each packet handler to \
              different functions would add complexity because most parameters will still need to \
              be passed manually."
)]
pub(crate) fn position_and_look_updates(
    mut full_reader: MessageReader<'_, '_, packet::play::Full>,
    mut position_reader: MessageReader<'_, '_, packet::play::PositionAndOnGround>,
    mut look_reader: MessageReader<'_, '_, packet::play::LookAndOnGround>,
    mut teleport_reader: MessageReader<'_, '_, packet::play::TeleportConfirm>,
    mut queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&EntitySize, &mut MovementTracking, &mut Position, &Yaw)>,
            Query<'_, '_, (&mut Yaw, &mut Pitch)>,
            Query<'_, '_, &mut Position>,
        ),
    >,
    teleport_query: Query<'_, '_, &PendingTeleportation>,
    blocks: Res<'_, Blocks>,
    compose: Res<'_, Compose>,
    mut commands: Commands<'_, '_>,
) {
    let mut full_reader = full_reader.read().map(OrderedPacketRef::from).peekable();
    let mut position_reader = position_reader
        .read()
        .map(OrderedPacketRef::from)
        .peekable();
    let mut look_reader = look_reader.read().map(OrderedPacketRef::from).peekable();
    let mut teleport_reader = teleport_reader
        .read()
        .map(OrderedPacketRef::from)
        .peekable();
    let blocks = blocks.into_inner();
    let compose = compose.into_inner();

    loop {
        // next_lowest is used to process the packet which was sent first. It is important to
        // process position packets of different types in the order they were sent by the client
        // so the client is at the correct final position after processing all packets.
        let result = next_lowest! {
            packet in full_reader => {
                change_position_or_correct_client(
                    packet.sender(),
                    packet.connection_id(),
                    queries.p0(),
                    blocks,
                    compose,
                    &mut commands,
                    packet.position.as_vec3(),
                    packet.on_ground,
                );

                let mut query = queries.p1();
                let (mut yaw, mut pitch) = match query.get_mut(packet.sender()) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("failed to handle full packet: query failed: {e}");
                        continue;
                    }
                };

                yaw.yaw = packet.yaw;
                pitch.pitch = packet.pitch;
            },
            packet in position_reader => {
                change_position_or_correct_client(
                    packet.sender(),
                    packet.connection_id(),
                    queries.p0(),
                    blocks,
                    compose,
                    &mut commands,
                    packet.position.as_vec3(),
                    packet.on_ground,
                );
            },
            packet in look_reader => {
                let mut query = queries.p1();
                let (mut yaw, mut pitch) = match query.get_mut(packet.sender()) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("failed to handle look and on ground: query failed: {e}");
                        continue;
                    }
                };

                yaw.yaw = packet.yaw;
                pitch.pitch = packet.pitch;
            },
            packet in teleport_reader => {
                let client = packet.sender();
                let Ok(pending_teleport) = teleport_query.get(client) else {
                    warn!("failed to confirm teleportation: client is not pending teleportation, so there is nothing to confirm");
                    continue;
                };

                let pending_teleport_id = pending_teleport.teleport_id;

                if VarInt(pending_teleport_id) != packet.teleport_id {
                    // If this is reached and the client is behaving correctly, the client has been
                    // teleported again (with teleport id `pending_teleport_id`) since the initial teleport
                    // (with teleport id `packet.teleport_id`). The current teleport confirmation
                    // can be ignored; the client will need to send a new one for the newer
                    // teleport.
                    continue;
                }

                let mut query = queries.p2();
                let mut position  = match query.get_mut(client) {
                    Ok(position) => position,
                    Err(e) => {
                        error!("failed to confirm teleportation: query failed: {e}");
                        continue;
                    }
                };

                **position = pending_teleport.destination;

                commands.queue(move |world: &mut World| {
                    let Ok(mut entity) = world.get_entity_mut(client) else {
                        error!("failed to confirm teleportation: client entity has despawned");
                        return;
                    };

                    let Some(pending_teleport) = entity.get::<PendingTeleportation>() else {
                        error!(
                            "failed to confirm teleportation: client is missing PendingTeleportation \
                             component"
                        );
                        return;
                    };

                    if pending_teleport.teleport_id != pending_teleport_id {
                        // A new pending teleport must have started between the time that this
                        // command was queued and the time that this command was ran. Therefore,
                        // this should not remove the PendingTeleportation component.
                        return;
                    }

                    entity.remove::<PendingTeleportation>();
                });
            }
        };
        if result.is_none() {
            break;
        }
    }
}

// TODO: I think this might be the place that introduces desync
fn change_position_or_correct_client(
    client: Entity,
    connection_id: ConnectionId,
    mut query: Query<'_, '_, (&EntitySize, &mut MovementTracking, &mut Position, &Yaw)>,
    blocks: &Blocks,
    compose: &Compose,
    commands: &mut Commands<'_, '_>,
    proposed: Vec3,
    on_ground: bool,
) {
    let (&size, mut tracking, mut pose, yaw) = match query.get_mut(client) {
        Ok(data) => data,
        Err(e) => {
            error!("change_position_or_correct_client failed: query failed: {e}");
            return;
        }
    };

    if let Err(e) = try_change_position(proposed, &pose, size, blocks) {
        // Send error message to player
        let msg = format!("§c{e}");
        let pkt = play::GameMessageS2c {
            chat: msg.into_cow_text(),
            overlay: false,
        };

        if let Err(e) = compose.unicast(&pkt, connection_id) {
            warn!("Failed to send error message to player: {e}");
        }

        commands
            .entity(client)
            .insert(PendingTeleportation::new(pose.position));
    }

    tracking.received_movement_packets = tracking.received_movement_packets.saturating_add(1);
    let y_delta = proposed.y - pose.y;

    if y_delta > 0. && tracking.was_on_ground && !on_ground {
        tracking.server_velocity.y = 0.419_999_986_886_978_15;

        if tracking.sprinting {
            let smth = yaw.yaw * 0.017_453_292;
            tracking.server_velocity += DVec3::new(
                f64::from(-smth.sin()) * 0.2,
                0.0,
                f64::from(smth.cos()) * 0.2,
            );
        }
    }

    **pose = proposed;
}

/// Returns true if the position was changed, false if it was not.
///
/// Movement validity rules:
/// ```text
///   From  |   To    | Allowed
/// --------|---------|--------
/// in  🧱  | in  🧱  |   ✅
/// in  🧱  | out 🌫️  |   ✅
/// out 🌫️  | in  🧱  |   ❌
/// out 🌫️  | out 🌫️  |   ✅
/// ```
/// Only denies movement if starting outside a block and moving into a block.
/// This prevents players from glitching into blocks while allowing them to move out.
fn try_change_position(
    proposed: Vec3,
    position: &Position,
    size: EntitySize,
    blocks: &Blocks,
) -> anyhow::Result<()> {
    // Only check collision if we're starting outside a block
    if !blocks.has_block_collision(position, size) && blocks.has_block_collision(&proposed, size) {
        return Err(anyhow::anyhow!("Cannot move into solid blocks"));
    }

    Ok(())
}

// for sneaking/crouching/etc
pub(crate) fn client_command(
    mut packets: MessageReader<'_, '_, packet::play::ClientCommand>,
    mut query: Query<'_, '_, (&mut Pose, &mut EntitySize, &mut MovementTracking)>,
) {
    for packet in packets.read() {
        let (mut pose, mut size, mut tracking) = match query.get_mut(packet.sender()) {
            Ok(data) => data,
            Err(e) => {
                error!("failed to handle client command: query failed: {e}");
                continue;
            }
        };

        match packet.action {
            ClientCommand::StartSneaking => {
                *pose = Pose::Sneaking;
                size.height = 1.5;
            }
            ClientCommand::StopSneaking | ClientCommand::LeaveBed => {
                *pose = Pose::Standing;
                size.height = 1.8;
            }
            ClientCommand::StartSprinting => {
                tracking.sprinting = true;
            }
            ClientCommand::StopSprinting => {
                tracking.sprinting = false;
            }
            ClientCommand::StartJumpWithHorse
            | ClientCommand::StopJumpWithHorse
            | ClientCommand::OpenHorseInventory
            | ClientCommand::StartFlyingWithElytra => {}
        }
    }
}
