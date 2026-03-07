#![feature(allocator_api, core_io_borrowed_buf, read_buf)]
#![cfg_attr(feature = "reflect", expect(clippy::transmute_ptr_to_ptr))]

pub mod agnostic;
mod cast;
mod compose;
pub mod decode;
pub mod decoder;
pub mod encoder;
pub mod lookup;
pub mod packet;
pub mod packet_state;
pub mod packets;
pub mod proxy;

use std::time::Duration;

use bevy_ecs::resource::Resource;
pub use cast::*;
pub use compose::*;
use libdeflater::CompressionLvl;
use valence_protocol::CompressionThreshold;
#[cfg(feature = "reflect")]
use {
    bevy_ecs::reflect::ReflectResource,
    bevy_reflect::{Reflect, reflect_remote},
};

#[cfg_attr(feature = "reflect", reflect_remote(CompressionThreshold))]
pub struct RemoteCompressionThreshold(pub i32);

/// Shared data that is shared between the ECS framework and the IO thread.
#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct Shared {
    /// The compression level to use for the server. This is how long a packet needs to be before it is compressed.
    #[cfg_attr(feature = "reflect", reflect(remote = RemoteCompressionThreshold))]
    pub compression_threshold: CompressionThreshold,

    /// The compression level to use for the server. This is the [`libdeflater`] compression level.
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    pub compression_level: CompressionLvl,
}

/// The amount of time from the last packet a player has sent before the server will kick them.
#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct KeepAliveTimeout(pub Duration); // TODO: Is this currently unused?

impl std::default::Default for KeepAliveTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(20))
    }
}

impl std::ops::Deref for KeepAliveTimeout {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The maximum amount of time a player is resistant to being hurt. This is weird as this is 20 in vanilla Minecraft.
/// However, the check to determine if a player can be hurt actually looks at this value divided by 2
#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct MaxHurtResistantTime(pub u16); // TODO: This is currently unused? Might only be relevant to external plugins?

impl std::default::Default for MaxHurtResistantTime {
    fn default() -> Self {
        Self(20)
    }
}

impl std::ops::Deref for MaxHurtResistantTime {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Resource, Default, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct PlayerCount {
    /// The amount of players currently in the playing state.
    pub count: usize, /* This was an atomic at some point, so that it didn't have to take a ResMut. This might need investigation at larger playercounts? */
}

impl std::fmt::Display for PlayerCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.count.fmt(f)
    }
}

impl std::ops::Deref for PlayerCount {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.count
    }
}

/// Data related to game ticks. This is updated every tick
#[derive(Resource, Default)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct TickData {
    /// The current tick of the game. This is incremented every 50 ms.
    pub tick: i64,
    /// The amount of time the last tick took in milliseconds.
    pub ms_last_tick: f32, // TODO: Is this actually being updated?
}
