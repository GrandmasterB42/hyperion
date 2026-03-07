mod index;
mod world;
use bevy_app::{App, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
pub use index::*;
pub use world::*;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

use crate::spatial::index::recalculate_spatial_index;

// TODO: Is this ever used?
#[derive(Component, Debug, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct RaycastTravel;

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpatialIndex>();
        app.add_systems(FixedPreUpdate, recalculate_spatial_index);
    }
}

pub struct SyncChunksPlugin;

impl Plugin for SyncChunksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (generate_chunk_changes, send_full_loaded_chunks),
        );
    }
}
