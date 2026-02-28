use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::{
    entity::Entity,
    lifecycle::{Add, Despawn},
    message::MessageReader,
    observer::On,
    query::With,
    system::{Query, Res},
    world::World,
};
use hyperion_entity::{EntityKind, Pitch, Position, Uuid, Velocity, Yaw};
use hyperion_net::{Compose, proxy::RequestSubscribeChannelPackets};
use hyperion_proxy_proto::{
    Channel, ChannelId, ConnectionId,
    packets::{
        intermediate::{IntermediateServerToProxyMessage, UpdateChannelPositions},
        s2p::UpdateChannelPosition,
    },
};
use hyperion_utils::EntityExt;
use tracing::error;
use valence_bytes::CowBytes;
use valence_protocol::{ByteAngle, RawBytes, VarInt, packets::play};

use crate::{
    egress::metadata::show_all,
    simulation::metadata::{MetadataChanges, get_and_clear_metadata},
};

fn add_channel(added_channel: On<'_, '_, Add, Channel>, compose: Res<'_, Compose>) {
    let packet = play::EntitiesDestroyS2c {
        entity_ids: vec![VarInt(added_channel.entity.minecraft_id())].into(),
    };

    let packet_buf = compose.io_buf().encode_packet(&packet, &compose).unwrap();

    compose
        .io_buf()
        .add_channel(ChannelId::new(added_channel.entity.id()), &packet_buf);
}

fn remove_channel(removed_channel: On<'_, '_, Despawn, Channel>, compose: Res<'_, Compose>) {
    compose
        .io_buf()
        .remove_channel(ChannelId::new(removed_channel.entity.id()));
}

fn update_channel_positions(
    compose: Res<'_, Compose>,
    query: Query<'_, '_, (Entity, &Position), With<Channel>>,
) {
    let updates = query
        .iter()
        .map(|(entity, position)| UpdateChannelPosition {
            channel_id: entity.id(),
            position: position.to_chunk().into(),
        })
        .collect::<Vec<_>>();

    compose
        .io_buf()
        .add_proxy_message(&IntermediateServerToProxyMessage::UpdateChannelPositions(
            UpdateChannelPositions { updates: &updates },
        ));
}

fn send_subscribe_channel_packets(
    mut events: MessageReader<'_, '_, RequestSubscribeChannelPackets>,
    compose: Res<'_, Compose>,
    query: Query<
        '_,
        '_,
        (
            Entity,
            &Uuid,
            &Position,
            &Pitch,
            &Yaw,
            &Velocity,
            &EntityKind,
            Option<&ConnectionId>,
        ),
    >,
    world: &World,
) {
    for event in events.read() {
        let (entity, uuid, position, pitch, yaw, velocity, &entity_kind, connection_id) =
            match query.get(event.0) {
                Ok(data) => data,
                Err(e) => {
                    error!("failed to send subscribe channel packets: query failed: {e}");
                    continue;
                }
            };

        let mut packet_buf;
        let minecraft_id = event.0.minecraft_id();

        if entity_kind == EntityKind::Player {
            let spawn_packet = play::PlayerSpawnS2c {
                entity_id: VarInt(minecraft_id),
                player_uuid: **uuid,
                position: position.as_dvec3(),
                yaw: ByteAngle::from_degrees(**yaw),
                pitch: ByteAngle::from_degrees(**pitch),
            };
            packet_buf = compose
                .io_buf()
                .encode_packet(&spawn_packet, &compose)
                .unwrap();

            let show_all = show_all(minecraft_id);
            packet_buf
                .extend_from_slice(&compose.io_buf().encode_packet(&show_all, &compose).unwrap());
        } else {
            let velocity = velocity.to_packet_units();

            let spawn_packet = play::EntitySpawnS2c {
                entity_id: VarInt(minecraft_id),
                object_uuid: uuid.0,
                kind: VarInt(entity_kind as i32),
                position: position.as_dvec3(),
                pitch: ByteAngle::from_degrees(**pitch),
                yaw: ByteAngle::from_degrees(**yaw),
                head_yaw: ByteAngle::from_degrees(0.0), // todo:
                data: VarInt::default(),                // todo:
                velocity,
            };
            packet_buf = compose
                .io_buf()
                .encode_packet(&spawn_packet, &compose)
                .unwrap();

            let velocity_packet = play::EntityVelocityUpdateS2c {
                entity_id: VarInt(minecraft_id),
                velocity,
            };
            packet_buf.extend_from_slice(
                &compose
                    .io_buf()
                    .encode_packet(&velocity_packet, &compose)
                    .unwrap(),
            );
        }

        let mut metadata = MetadataChanges::default();
        metadata.encode_non_default_components(world.entity(entity));

        if let Some(view) = get_and_clear_metadata(&mut metadata) {
            let pkt = play::EntityTrackerUpdateS2c {
                entity_id: VarInt(minecraft_id),
                tracked_values: RawBytes(CowBytes::Borrowed(&view)),
            };
            packet_buf.extend_from_slice(&compose.io_buf().encode_packet(&pkt, &compose).unwrap());
        }

        compose.io_buf().send_subscribe_channel_packets(
            event.0.into(),
            &packet_buf,
            connection_id.copied(),
        );
    }
}

pub struct ChannelPlugin;

impl Plugin for ChannelPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_channel);
        app.add_observer(remove_channel);
        app.add_systems(
            FixedUpdate,
            (update_channel_positions, send_subscribe_channel_packets),
        );
    }
}
