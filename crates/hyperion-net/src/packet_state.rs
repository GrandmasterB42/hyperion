// TODO: A lot of these compoment requirements are quite fragile. This needs to be looked into, maybe required components?
//! Components marking a player packet state. Players will have at most 1 state component at a time (they may have no components during state transitions)
//!
//! All players with a state component assigned are guaranteed to have the following components:
/// - [`crate::ConnectionId`]
/// - [`crate::PacketDecoder`]
use bevy_ecs::component::Component;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

/// Marks players who are in the handshake state.
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Handshake;
/// Marks players who are in the status state.
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Status;

/// Marks players who are in the login state.
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Login;

/// Marks players who are in the play state.
#[derive(Component, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Play;
