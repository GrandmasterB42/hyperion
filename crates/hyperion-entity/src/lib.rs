mod entity_kind;
pub mod player;
mod position;
mod size;
mod uuid;

use bevy_ecs::{component::Component, entity::Entity};
pub use entity_kind::*;
use glam::Vec3;
pub use position::*;
pub use size::*;
pub use uuid::*;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

/// The reaction of an entity, in particular to collisions as calculated in `entity_detect_collisions`.
///
/// Why is this useful?
///
/// - We want to be able to detect collisions in parallel.
/// - Since we are accessing bounding boxes in parallel,
///   we need to be able to make sure the bounding boxes are immutable (unless we have something like a
///   [`std::sync::Arc`] or [`std::sync::RwLock`], but this is not efficient).
/// - Therefore, we have an [`Velocity`] component which is used to store the reaction of an entity to collisions.
/// - Later we can apply the reaction to the entity's [`Position`] to move the entity.
#[derive(Component, Default, Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Velocity(#[cfg_attr(feature = "reflect", reflect(ignore))] pub Vec3); // TODO: Reflect this once glam is updated everywhere

impl Velocity {
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    #[must_use]
    pub fn to_packet_units(self) -> valence_protocol::Velocity {
        valence_protocol::Velocity::from_ms_f32((self.0 * 20.0).into())
    }
}

#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Visible;

#[must_use]
pub fn get_rotation_from_velocity(velocity: Vec3) -> (f32, f32) {
    let yaw = (-velocity.x).atan2(velocity.z).to_degrees(); // Correct yaw calculation
    let pitch = (-velocity.y).atan2(velocity.length()).to_degrees(); // Correct pitch calculation
    (yaw, pitch)
}

#[must_use]
pub fn get_direction_from_rotation(yaw: f32, pitch: f32) -> Vec3 {
    // Convert angles from degrees to radians
    let yaw_rad = yaw.to_radians();
    let pitch_rad = pitch.to_radians();

    Vec3::new(
        -pitch_rad.cos() * yaw_rad.sin(), // x = -cos(pitch) * sin(yaw)
        -pitch_rad.sin(),                 // y = -sin(pitch)
        pitch_rad.cos() * yaw_rad.cos(),  // z = cos(pitch) * cos(yaw)
    )
}

// TODO: Do entities also get teleported or only the player??
#[derive(Component, Default, Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct PendingTeleportation {
    pub teleport_id: i32,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    // TODO: Reflect this once glam is updated everywhere
    pub destination: Vec3,
    pub ttl: u8,
}

impl PendingTeleportation {
    #[must_use]
    pub fn new(destination: Vec3) -> Self {
        Self {
            teleport_id: fastrand::i32(..),
            destination,
            ttl: 20,
        }
    }
}

/// Any living minecraft entity that is NOT a player.
///
/// Example: zombie, skeleton, etc.
#[derive(Component, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Npc;

/// The running multiplier of the entity. This defaults to 1.0.
#[derive(Component, Debug, Copy, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct RunningSpeed(pub f32);

impl Default for RunningSpeed {
    fn default() -> Self {
        Self(0.1)
    }
}

#[derive(Component, Copy, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Yaw {
    pub yaw: f32,
}

impl Yaw {
    #[must_use]
    pub const fn new(yaw: f32) -> Self {
        Self { yaw }
    }
}

impl std::fmt::Display for Yaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let yaw = self.yaw;
        write!(f, "{yaw}")
    }
}

impl std::ops::Deref for Yaw {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.yaw
    }
}

#[derive(Component, Copy, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Pitch {
    pub pitch: f32,
}

impl Pitch {
    #[must_use]
    pub const fn new(pitch: f32) -> Self {
        Self { pitch }
    }
}

impl std::fmt::Display for Pitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pitch = self.pitch;
        write!(f, "{pitch}")
    }
}

impl std::ops::Deref for Pitch {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.pitch
    }
}

#[derive(Component, Default, Debug, Copy, Clone)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Flight {
    pub allow: bool,
    pub is_flying: bool,
}

// TODO: This might be a good fit for a relationship?
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Owner {
    pub entity: Entity,
}

impl Owner {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

/// If the entity can be targeted by non-player entities.
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct AiTargetable;

#[derive(Component, Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct FlyingSpeed {
    pub speed: f32,
}

impl FlyingSpeed {
    #[must_use]
    pub const fn new(speed: f32) -> Self {
        Self { speed }
    }
}

impl Default for FlyingSpeed {
    fn default() -> Self {
        Self { speed: 0.05 }
    }
}
