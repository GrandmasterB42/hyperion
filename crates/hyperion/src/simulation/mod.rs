use bevy_app::{App, Plugin};
use bevy_ecs::{
    component::Component,
    lifecycle::Insert,
    observer::On,
    system::{Query, Res},
};
use glam::{DVec3, Vec3};
use hyperion_command::CommandPlugin;
use hyperion_entity::{Flight, FlyingSpeed, PendingTeleportation, Pitch, Yaw};
use hyperion_net::{Compose, packet::PacketPlugin, proxy::RequestSubscribeChannelPackets};
use hyperion_proxy_proto::ConnectionId;
use tracing::error;
use valence_protocol::{
    VarInt,
    packets::play::{
        self,
        player_abilities_s2c::{PlayerAbilitiesFlags, PlayerAbilitiesS2c},
        player_position_look_s2c::PlayerPositionLookFlags,
    },
};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

use crate::simulation::{
    handlers::HandlersPlugin,
    inventory::InventoryPlugin,
    metadata::{Metadata, MetadataPlugin},
};

pub mod event;
pub mod handlers;
pub mod inventory;
pub mod metadata;
pub mod skin;

#[derive(Component, Debug, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct RaycastTravel;

#[derive(Component, Default, Debug, Copy, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct MovementTracking {
    pub fall_start_y: f32,
    pub last_tick_flying: bool,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    // TODO: Reflect this once glam is updated everywhere
    pub last_tick_position: Vec3,
    pub received_movement_packets: u8,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    // TODO: Reflect this once glam is updated everywhere
    pub server_velocity: DVec3,
    pub sprinting: bool,
    pub was_on_ground: bool,
}

fn send_pending_teleportation(
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

fn update_flight(
    now_flying: On<'_, '_, Insert, (FlyingSpeed, Flight)>,
    compose: Res<'_, Compose>,
    query: Query<'_, '_, (&ConnectionId, &Flight, &FlyingSpeed)>,
) {
    let Ok((&connection_id, flight, flying_speed)) = query.get(now_flying.entity) else {
        return;
    };

    let pkt = PlayerAbilitiesS2c {
        flags: PlayerAbilitiesFlags::default()
            .with_allow_flying(flight.allow)
            .with_flying(flight.is_flying),
        flying_speed: flying_speed.speed,
        fov_modifier: 0.0,
    };

    compose.unicast(&pkt, connection_id).unwrap();
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(send_pending_teleportation);
        app.add_observer(update_flight);

        app.add_plugins((
            CommandPlugin,
            HandlersPlugin,
            PacketPlugin,
            InventoryPlugin,
            MetadataPlugin,
        ));

        app.add_message::<RequestSubscribeChannelPackets>();
        app.add_message::<event::ItemDropEvent>();
        app.add_message::<event::SetSkin>();
        app.add_message::<event::AttackEntity>();
        app.add_message::<event::StartDestroyBlock>();
        app.add_message::<event::DestroyBlock>();
        app.add_message::<event::PlaceBlock>();
        app.add_message::<event::ToggleDoor>();
        app.add_message::<event::SwingArm>();
        app.add_message::<event::ReleaseUseItem>();
        app.add_message::<event::PostureUpdate>();
        app.add_message::<event::BlockInteract>();
        app.add_message::<event::ProjectileEntityEvent>();
        app.add_message::<event::ProjectileBlockEvent>();
        app.add_message::<event::ClickSlotEvent>();
        app.add_message::<event::DropItemStackEvent>();
        app.add_message::<event::UpdateSelectedSlotEvent>();
        app.add_message::<event::HitGroundEvent>();
    }
}
