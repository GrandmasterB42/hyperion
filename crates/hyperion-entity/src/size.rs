use bevy_ecs::component::Component;
use geometry::aabb::Aabb;
use glam::{IVec3, Vec3};
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

use crate::player::{PLAYER_HEIGHT, PLAYER_WIDTH};

#[derive(Component, Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct EntitySize {
    pub half_width: f32,
    pub height: f32,
}

impl EntitySize {
    #[must_use]
    pub fn aabb(&self, position: Vec3) -> Aabb {
        let half_width = self.half_width;
        let height = self.height;
        Aabb::new(
            position - Vec3::new(half_width, 0.0, half_width),
            position + Vec3::new(half_width, height, half_width),
        )
    }

    #[must_use]
    pub fn block_bounds(&self, position: Vec3) -> (IVec3, IVec3) {
        let bounding = self.aabb(position);
        let min = bounding.min.floor().as_ivec3();
        let max = bounding.max.ceil().as_ivec3();

        (min, max)
    }
}

impl core::fmt::Display for EntitySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let half_width = self.half_width;
        let height = self.height;
        write!(f, "{half_width}x{height}")
    }
}

impl Default for EntitySize {
    fn default() -> Self {
        Self {
            half_width: PLAYER_WIDTH / 2.0,
            height: PLAYER_HEIGHT,
        }
    }
}
