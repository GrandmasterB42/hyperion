mod cached_save;
pub mod command_channel;
pub mod iterator;
pub mod prev;
pub mod runtime;

use std::path::PathBuf;

use bevy_app::{App, Plugin};
use bevy_ecs::{
    entity::{Entity, EntityIndex},
    resource::Resource,
    system::{SystemParam, SystemState},
    world::World,
};
pub use cached_save::cached_save;
pub use prev::{Prev, track_prev};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectResource, bevy_reflect::Reflect};

pub trait EntityExt: Sized {
    fn id(&self) -> u32;
    fn from_id(id: u32, world: &World) -> anyhow::Result<Self>;

    fn minecraft_id(&self) -> i32;
    fn from_minecraft_id(id: i32, world: &World) -> anyhow::Result<Self>;
}

// TODO: How does this fit into the picture? Why are we using the internal id?
impl EntityExt for Entity {
    fn id(&self) -> u32 {
        self.index().index()
    }

    fn from_id(id: u32, world: &World) -> anyhow::Result<Self> {
        let Some(id) = EntityIndex::from_raw_u32(id) else {
            anyhow::bail!("minecraft id is should not be u32::MAX")
        };

        let entities = world.entities();
        if !entities.is_index_spawned(id) {
            anyhow::bail!("minecraft id is invalid")
        }

        Ok(world.entities().resolve_from_index(id))
    }

    fn minecraft_id(&self) -> i32 {
        bytemuck::cast(self.id())
    }

    fn from_minecraft_id(id: i32, world: &World) -> anyhow::Result<Self> {
        Self::from_id(bytemuck::cast(id), world)
    }
}

pub trait ApplyWorld {
    fn apply(&mut self, world: &mut World);
}

impl<Param> ApplyWorld for SystemState<Param>
where
    Param: SystemParam + 'static,
{
    fn apply(&mut self, world: &mut World) {
        self.apply(world);
    }
}

impl ApplyWorld for () {
    fn apply(&mut self, _: &mut World) {}
}

/// Represents application identification information used for caching and other system-level operations
#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct AppId {
    /// The qualifier/category of the application (e.g. "com", "org", "hyperion")
    pub qualifier: String,
    /// The organization that created the application
    pub organization: String,
    /// The specific application name
    pub application: String,
}

impl AppId {
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        let project_dirs = directories::ProjectDirs::from(
            self.qualifier.as_str(),
            self.organization.as_str(),
            self.application.as_str(),
        )
        .unwrap();
        project_dirs.cache_dir().to_path_buf()
    }
}

pub struct HyperionUtilsPlugin;

impl Plugin for HyperionUtilsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AppId {
            qualifier: "github".to_string(),
            organization: "hyperion-mc".to_string(),
            application: "generic".to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id() {
        let mut world = World::new();
        let entity_id = world.spawn_empty().id();
        let minecraft_id = entity_id.minecraft_id();
        assert_eq!(
            Entity::from_minecraft_id(minecraft_id, &world).unwrap(),
            entity_id
        );
    }
}
