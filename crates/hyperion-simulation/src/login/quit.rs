use std::borrow::Cow;

use bevy_ecs::{
    lifecycle::Remove,
    observer::On,
    system::{Query, Res},
};
use hyperion_entity::Uuid;
use hyperion_net::{Compose, packet_state};
use hyperion_utils::EntityExt;
use tracing::error;
use valence_protocol::{
    VarInt,
    packets::play::{EntitiesDestroyS2c, PlayerRemoveS2c},
};

pub(crate) fn remove_player_from_visibility(
    not_playing: On<'_, '_, Remove, packet_state::Play>,
    query: Query<'_, '_, &Uuid>,
    compose: Res<'_, Compose>,
) {
    let uuid = match query.get(not_playing.entity) {
        Ok(uuid) => uuid,
        Err(e) => {
            error!("failed to send player remove packet: query failed: {e}");
            return;
        }
    };

    let uuids = &[uuid.0];
    let entity_ids = [VarInt(not_playing.entity.minecraft_id())];

    // destroy
    let pkt = EntitiesDestroyS2c {
        entity_ids: Cow::Borrowed(&entity_ids),
    };

    if let Err(e) = compose.broadcast(&pkt).send() {
        error!("failed to send player remove packet: {e}");
        return;
    }

    let pkt = PlayerRemoveS2c {
        uuids: Cow::Borrowed(uuids),
    };

    if let Err(e) = compose.broadcast(&pkt).send() {
        error!("failed to send player remove packet: {e}");
    }
}
