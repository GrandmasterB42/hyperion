use bevy_ecs::component::Component;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

/// Uniquely identifies an Entity
#[derive(Component, Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Uuid(pub uuid::Uuid);

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Uuid {
    #[must_use]
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::ops::Deref for Uuid {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
