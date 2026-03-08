mod join;
pub mod query;
mod quit;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    entity::Entity,
    message::{Message, MessageReader},
    resource::Resource,
    schedule::IntoScheduleConfigs,
    system::Commands,
    world::World,
};
use hyperion_entity::Position;
use hyperion_net::{packet, packet_state};
use sha2::Digest;
use valence_protocol::{
    RawBytes, VarInt,
    packets::{handshaking::handshake_c2s::HandshakeNextState, play},
};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectResource, bevy_reflect::Reflect};

use crate::login::{
    join::{
        ResolvePosition, UpdateState, join_finish, process_login_hello, process_resolved_skin,
        resolve_joined_position,
    },
    query::{ServerPingResponse, process_status_ping, process_status_request},
    quit::remove_player_from_visibility,
};

type SpawnFunction = fn(Entity, &mut World) -> SpawnPosition;

pub struct SpawnPosition {
    pub pos: Position,
    pub pitch: f32,
    pub yaw: f32,
}

impl SpawnPosition {
    #[must_use]
    pub fn new(pos: Position, pitch: f32, yaw: f32) -> Self {
        Self { pos, pitch, yaw }
    }

    #[must_use]
    pub fn from_pos(x: f32, y: f32, z: f32) -> Self {
        Self {
            pos: Position::new(x, y, z),
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct PlayerSpawnPosition {
    #[cfg_attr(
        feature = "reflect",
        reflect(ignore, default = "ret_default_spawn_position")
    )]
    f: SpawnFunction,
    // TODO: Make this optionally parallel, in case the calculation is expensive. Maybe a type parameter?
    // parallel: bool,
}

impl PlayerSpawnPosition {
    pub fn new(f: SpawnFunction) -> Self {
        Self {
            f,
            // parallel: false,
        }
    }
}

fn ret_default_spawn_position() -> SpawnFunction {
    default_spawn_position
}

fn default_spawn_position(_: Entity, _: &mut World) -> SpawnPosition {
    SpawnPosition::from_pos(0.0, 64.0, 0.0)
}

impl std::default::Default for PlayerSpawnPosition {
    fn default() -> Self {
        Self {
            f: default_spawn_position,
            // parallel: false,
        }
    }
}

pub fn process_handshake(
    mut packets: MessageReader<'_, '_, packet::handshake::Handshake>,
    mut commands: Commands<'_, '_>,
) {
    for packet in packets.read() {
        let mut entity = commands.entity(packet.sender());

        entity.remove::<packet_state::Handshake>();
        match packet.next_state {
            HandshakeNextState::Status => {
                entity.insert(packet_state::Status);
            }
            HandshakeNextState::Login => {
                entity.insert(packet_state::Login);
            }
        }
    }
}

/// Get a [`uuid::Uuid`] based on the given user's name.
fn offline_uuid(username: &str) -> uuid::Uuid {
    let digest = sha2::Sha256::digest(username);
    let digest: [u8; 32] = digest.into();

    // UUid expects 16 bytes
    // todo: I have no idea which way we should go (be or le)
    uuid::Uuid::from_slice(&digest[0..16]).unwrap()
}

/// Packet to show all parts of the skin.
#[must_use]
pub fn show_all(id: i32) -> play::EntityTrackerUpdateS2c<'static> {
    // https://wiki.vg/Entity_metadata#Entity_Metadata_Format
    // https://wiki.vg/Entity_metadata#Player
    // 17 = Metadata, type = byte
    static BYTES: &[u8] = &[17, 0, 0xff, 0xff];

    let entity_id = VarInt(id);

    play::EntityTrackerUpdateS2c {
        entity_id,
        tracked_values: RawBytes(BYTES.into()),
    }
}

pub struct PlayerJoinPlugin;

impl Plugin for PlayerJoinPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(hyperion_net::decode::DecodePlugin);
        app.add_systems(
            FixedUpdate,
            (
                process_handshake.after(hyperion_net::decode::handshake),
                (process_status_request, process_status_ping).after(hyperion_net::decode::status),
                (process_login_hello, resolve_joined_position, join_finish)
                    .chain()
                    .run_if(reader_not_empty::<packet::login::LoginHello>)
                    .after(hyperion_net::decode::login),
                process_resolved_skin,
            ),
        );

        // Messages to coordiante the login process
        app.add_message::<ResolvePosition>()
            .add_message::<UpdateState>();

        app.add_observer(remove_player_from_visibility);

        app.init_resource::<ServerPingResponse>();
        app.init_resource::<PlayerSpawnPosition>();
    }
}

fn reader_not_empty<M: Message>(mut reader: MessageReader<'_, '_, M>) -> bool {
    let ret = !reader.is_empty();
    reader.clear();
    ret
}
