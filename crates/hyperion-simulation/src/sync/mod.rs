use bevy_app::{App, FixedPostUpdate, Plugin};
use hyperion_entity::{Pitch, Position, Yaw, player::Xp};
use hyperion_utils::track_prev;

use crate::sync::{
    chunk::{broadcast_chunk_deltas, send_chunk_positions},
    entity::{
        active_animation_sync, entity_metadata_sync, entity_xp_sync, sync_player_entity,
        update_projectile_positions,
    },
};

pub mod chunk;
pub mod entity;

pub struct EntityStateSyncPlugin;

impl Plugin for EntityStateSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (
                entity_xp_sync,
                entity_metadata_sync,
                active_animation_sync,
                sync_player_entity,
                update_projectile_positions,
            ),
        );

        track_prev::<Xp>(app);
        track_prev::<Position>(app);
        track_prev::<Yaw>(app);
        track_prev::<Pitch>(app);
    }
}

pub struct ChunkSyncPlugin;

impl Plugin for ChunkSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (send_chunk_positions, broadcast_chunk_deltas),
        );
    }
}

pub struct SyncPlugin;

impl Plugin for SyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EntityStateSyncPlugin, ChunkSyncPlugin));
    }
}
