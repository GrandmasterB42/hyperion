mod crypto;
pub mod encoder;
pub mod packets;

use bevy_ecs::{component::Component, entity::Entity};
use hyperion_utils::EntityExt;
#[cfg(feature = "reflect")]
use {bevy_ecs::reflect::ReflectComponent, bevy_reflect::Reflect};

pub use crate::crypto::*;

/// The Minecraft protocol version this library currently targets.
pub const PROTOCOL_VERSION: i32 = 763;

/// The stringified name of the Minecraft version this library currently
/// targets.
pub const MINECRAFT_VERSION: &str = "1.20.1";

/// A unique identifier for a proxy to game server connection
#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct ProxyId {
    /// The underlying unique identifier for the proxy connection.
    /// This value is guaranteed to be unique among all active connections.
    proxy_id: u64,
}

impl ProxyId {
    /// Creates a new proxy ID with the specified proxy identifier.
    ///
    /// This is an internal API used by the proxy management system.
    #[must_use]
    pub const fn new(proxy_id: u64) -> Self {
        Self { proxy_id }
    }

    /// Returns the underlying proxy identifier.
    ///
    /// This method is primarily used by internal networking code to interact
    /// with the proxy layer. Most application code should not need this.
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.proxy_id
    }
}

/// A unique identifier for a client connection
///
/// Each `ConnectionId` represents an active network connection between the server and a client,
/// corresponding to a single player session. The ID is used throughout the networking
/// system to:
///
/// - Route packets to a specific client
/// - Target or exclude specific clients in broadcast operations
/// - Track connection state through the proxy layer
///
/// Note: Connection IDs are managed internally by the networking system and should be obtained
/// through the appropriate connection establishment handlers rather than created directly.
#[derive(Component, Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct ConnectionId {
    /// The underlying unique identifier for this connection.
    /// This value is guaranteed to be unique among all active connections.
    stream_id: u64,

    /// The proxy which this player connection is connected to
    proxy_id: ProxyId,
}

impl ConnectionId {
    /// Creates a new connection ID with the specified stream identifier.
    ///
    /// This is an internal API used by the connection management system.
    /// External code should obtain connection IDs through the appropriate
    /// connection handlers.
    #[must_use]
    pub const fn new(stream_id: u64, proxy_id: ProxyId) -> Self {
        Self {
            stream_id,
            proxy_id,
        }
    }

    /// Returns the proxy which this player connection is connected to.
    ///
    /// This method is primarily used by internal networking code.
    /// Most application code should not need this.
    #[must_use]
    pub const fn proxy_id(self) -> ProxyId {
        self.proxy_id
    }

    /// Returns the underlying stream identifier.
    ///
    /// This method is primarily used by internal networking code to interact
    /// with the proxy layer. Most application code should work with the
    /// `ConnectionId` type directly rather than the raw ID.
    #[must_use]
    pub const fn inner(self) -> u64 {
        self.stream_id
    }
}

/// A component marking an entity as a packet channel.
#[derive(Component, Copy, Clone, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct Channel;

/// A unique identifier for a channel. The server is responsible for managing channel IDs.
#[derive(Component, Copy, Clone, Debug)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Component))]
pub struct ChannelId {
    /// The underlying unique identifier for this channel.
    channel_id: u32,
}

impl ChannelId {
    /// Creates a new channel ID with the specified stream identifier.
    #[must_use]
    pub const fn new(channel_id: u32) -> Self {
        Self { channel_id }
    }

    /// Returns the underlying channel identifier.
    ///
    /// This method is primarily used by internal networking code to interact
    /// with the proxy layer. Most application code should work with the
    /// `ChannelId` type directly rather than the raw ID.
    #[must_use]
    pub const fn inner(self) -> u32 {
        self.channel_id
    }
}

impl From<Entity> for ChannelId {
    fn from(entity: Entity) -> Self {
        Self::new(entity.id())
    }
}
