use bevy_ecs::component::Component;
use glam::{I16Vec2, IVec3, Vec3};
use serde::{Deserialize, Serialize};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

/// The full pose of an entity. This is used for both [`Player`] and [`Npc`].
#[derive(Component, Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Position {
    /// The (x, y, z) position of the entity.
    /// Note we are using [`Vec3`] instead of [`glam::DVec3`] because *cache locality* is important.
    /// However, the Notchian server uses double precision floating point numbers for the position.
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    pub position: Vec3, // TODO: Reflect this once glam is updated everywhere
}

impl Position {
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: Vec3::new(x, y, z),
        }
    }

    #[must_use]
    pub fn sound_position(&self) -> IVec3 {
        let position = self.position * 8.0;
        position.as_ivec3()
    }

    /// Get the chunk position of the center of the player's bounding box.
    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn to_chunk(&self) -> I16Vec2 {
        let x = self.x as i32;
        let z = self.z as i32;
        let x = x >> 4;
        let z = z >> 4;

        let x = i16::try_from(x).unwrap();
        let z = i16::try_from(z).unwrap();

        I16Vec2::new(x, z)
    }
}

/// The initial player spawn position. todo: this should not be a constant
pub const PLAYER_SPAWN_POSITION: Vec3 = Vec3::new(-8_526_209_f32, 100f32, -6_028_464f32);

impl std::ops::Add for Position {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            position: self.position + rhs.position,
        }
    }
}

impl std::ops::Sub for Position {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            position: self.position - rhs.position,
        }
    }
}

impl From<Vec3> for Position {
    fn from(value: Vec3) -> Self {
        Self { position: value }
    }
}

impl std::ops::Deref for Position {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.position
    }
}

impl std::ops::DerefMut for Position {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.position
    }
}

// TODO: Chunkposition is duplicated for the packet and here? Maybe this happens with multiple things?
#[derive(Component, Debug, Copy, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct ChunkPosition {
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    pub position: I16Vec2, // TODO: Reflect this once glam is updated everywhere
}

impl ChunkPosition {
    const SANE_MAX_RADIUS: i16 = 128;

    #[must_use]
    #[expect(missing_docs)]
    pub const fn null() -> Self {
        // todo: huh
        Self {
            position: I16Vec2::new(Self::SANE_MAX_RADIUS, Self::SANE_MAX_RADIUS),
        }
    }
}
