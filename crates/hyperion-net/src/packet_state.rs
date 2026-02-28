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
///
/// Players in this state are guaranteed to have the following components:
/// - [`crate::simulation::Name`]
/// - [`crate::simulation::Uuid`]
/// - [`crate::simulation::AiTargetable`]
/// - [`crate::simulation::ImmuneStatus`]
/// - [`crate::simulation::ChunkPosition`]
/// - [`crate::egress::sync_chunks::ChunkSendQueue`]
/// - [`crate::simulation::Yaw`]
/// - [`crate::simulation::Pitch`]
/// - [`crate::simulation::skin::PlayerSkin`]
/// - [`crate::simulation::Position`]
/// - [`crate::simulation::Player`]
#[derive(Component)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Play;
