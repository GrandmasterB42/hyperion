mod join;
pub mod query;
mod quit;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    entity::Entity, event::EntityEvent, message::MessageReader, schedule::IntoScheduleConfigs,
    system::Commands,
};
use hyperion_net::{packet, packet_state};
use sha2::Digest;
use valence_protocol::{
    RawBytes, VarInt,
    packets::{handshaking::handshake_c2s::HandshakeNextState, play},
};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectEvent, bevy_reflect::Reflect};

use crate::login::{
    join::{ProcessPlayerJoin, add_process_player_join, process_login_hello, process_player_join},
    query::{ServerPingResponse, process_status_ping, process_status_request},
    quit::remove_player_from_visibility,
};

#[derive(EntityEvent, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Event))]
pub struct InitializePlayerPosition(pub Entity);

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
                process_login_hello.after(hyperion_net::decode::login),
                process_player_join,
            ),
        );
        app.add_observer(remove_player_from_visibility);
        app.add_observer(add_process_player_join);
        app.add_message::<ProcessPlayerJoin>();
        app.init_resource::<ServerPingResponse>();
    }
}
