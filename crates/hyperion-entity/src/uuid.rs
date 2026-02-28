use bevy_ecs::component::Component;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

/// A UUID component. Generally speaking, this tends to be tied to entities with a [`Player`] component.
#[derive(Component, Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Uuid(pub uuid::Uuid);

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
