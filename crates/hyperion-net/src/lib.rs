#![feature(allocator_api, core_io_borrowed_buf, read_buf)]
#![expect(clippy::transmute_ptr_to_ptr)]

pub mod agnostic;
mod compose;
pub mod decode;
pub mod decoder;
pub mod encoder;
pub mod lookup;
pub mod packet;
pub mod packet_state;
pub mod proxy;

use std::{
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
};

#[cfg(feature = "reflect")]
use bevy_reflect::{Reflect, reflect_remote};
pub use compose::*;
use libdeflater::CompressionLvl;
use valence_protocol::CompressionThreshold;

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

#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct Global {
    /// The current tick of the game. This is incremented every 50 ms.
    pub tick: i64,

    /// The maximum amount of time a player is resistant to being hurt. This is weird as this is 20 in vanilla
    /// Minecraft.
    /// However, the check to determine if a player can be hurt actually looks at this value divided by 2
    pub max_hurt_resistant_time: u16,

    /// Data shared between the IO thread and the ECS framework.
    #[cfg_attr(feature = "reflect", reflect(ignore, default = "dummy_reflect_shared"))]
    pub shared: Arc<Shared>,

    /// The amount of time from the last packet a player has sent before the server will kick them.
    pub keep_alive_timeout: Duration,

    /// The amount of time the last tick took in milliseconds.
    pub ms_last_tick: f32,

    #[cfg_attr(feature = "reflect", reflect(ignore))]
    pub player_count: AtomicUsize,
}

// Note: Caution! The Arc and AtomicUsize did not throw errors for me, it just didn't work

#[cfg(feature = "reflect")]
fn dummy_reflect_shared() -> Arc<Shared> {
    Arc::new(Shared {
        compression_threshold: CompressionThreshold::default(),
        compression_level: CompressionLvl::default(),
    })
}

impl Global {
    /// Creates a new [`Global`] with the given shared data.
    #[must_use]
    pub const fn new(shared: Arc<Shared>) -> Self {
        Self {
            tick: 0,
            max_hurt_resistant_time: 20, // actually kinda like 10 vanilla mc is weird
            shared,
            keep_alive_timeout: Duration::from_secs(20),
            ms_last_tick: 0.0,
            player_count: AtomicUsize::new(0),
        }
    }
}
