use glam::I16Vec2;
use hyperion_proxy_proto::{
    ChannelId, ConnectionId,
    packets::{PacketBundle, shared::ChunkPosition},
};
use valence_protocol::bytes::BytesMut;

use crate::Compose;

/// A broadcast builder
pub struct Broadcast<'c, P> {
    packet: P,
    compose: &'c Compose,
    exclude: Option<ConnectionId>,
}

impl<'c, P> Broadcast<'c, P> {
    /// Creates a new [`Broadcast`] with noone excluded.
    pub const fn new(packet: P, compose: &'c Compose) -> Self {
        Self {
            packet,
            compose,
            exclude: None,
        }
    }

    /// Exclude a certain player from the broadcast. This can only be called once.
    #[must_use]
    pub fn exclude(self, exclude: impl Into<Option<ConnectionId>>) -> Self {
        let exclude = exclude.into();
        Broadcast {
            packet: self.packet,
            compose: self.compose,
            exclude,
        }
    }

    /// Send the packet to all players.
    pub fn send(self) -> anyhow::Result<()>
    where
        P: PacketBundle,
    {
        let bytes = self
            .compose
            .io_buf
            .encode_packet(self.packet, self.compose)?;

        self.compose.io_buf.broadcast_raw(&bytes, self.exclude);

        Ok(())
    }
}

/// A unicast builder
pub struct Unicast<'c, P> {
    packet: P,
    stream_id: ConnectionId,
    compose: &'c Compose,
    compress: bool,
}

impl<'c, P> Unicast<'c, P>
where
    P: PacketBundle,
{
    pub const fn new(
        packet: P,
        stream_id: ConnectionId,
        compose: &'c Compose,
        compress: bool,
    ) -> Self {
        Self {
            packet,
            stream_id,
            compose,
            // todo: Should we have this true by default, or is there a better way?
            // Or a better word for no_compress, or should we just use negative field names?
            compress,
        }
    }

    pub(crate) fn send(self) -> anyhow::Result<()> {
        self.compose.io_buf.unicast_private(
            self.packet,
            self.stream_id,
            self.compose,
            self.compress,
        )
    }
}

pub struct BroadcastLocal<'c, P> {
    packet: P,
    compose: &'c Compose,
    center: ChunkPosition,
    exclude: Option<ConnectionId>,
}

impl<'c, P> BroadcastLocal<'c, P> {
    /// Creates a new [`BroadcastLocal`] with noone excluded.
    pub const fn new(packet: P, compose: &'c Compose, center: ChunkPosition) -> Self {
        Self {
            packet,
            compose,
            center,
            exclude: None,
        }
    }

    /// Exclude a certain player from the broadcast. This can only be called once.
    #[must_use]
    pub fn exclude(self, exclude: impl Into<Option<ConnectionId>>) -> Self {
        let exclude = exclude.into();
        BroadcastLocal {
            packet: self.packet,
            compose: self.compose,
            center: self.center,
            exclude,
        }
    }

    /// Send the packet
    pub fn send(self) -> anyhow::Result<()>
    where
        P: PacketBundle,
    {
        let bytes = self
            .compose
            .io_buf
            .encode_packet(self.packet, self.compose)?;

        self.compose
            .io_buf
            .broadcast_local_raw(&bytes, self.center, self.exclude);

        Ok(())
    }
}

pub struct BroadcastChannel<'c, P> {
    packet: P,
    compose: &'c Compose,
    exclude: Option<ConnectionId>,
    channel: ChannelId,
}

impl<'c, P> BroadcastChannel<'c, P> {
    /// Constructs a new [`BroadcastChannel`] with noone excluded.
    pub const fn new(packet: P, compose: &'c Compose, channel: ChannelId) -> Self {
        Self {
            packet,
            compose,
            exclude: None,
            channel,
        }
    }

    /// Exclude a certain player from the broadcast. This can only be called once.
    #[must_use]
    pub fn exclude(self, exclude: impl Into<Option<ConnectionId>>) -> Self {
        let exclude = exclude.into();
        Self { exclude, ..self }
    }

    /// Send the packet
    pub fn send(self) -> anyhow::Result<()>
    where
        P: PacketBundle,
    {
        let bytes = self
            .compose
            .io_buf
            .encode_packet(self.packet, self.compose)?;

        self.compose
            .io_buf
            .broadcast_channel_raw(&bytes, self.channel, self.exclude);

        Ok(())
    }
}

pub struct DataBundle<'c> {
    compose: &'c Compose,
    data: BytesMut,
}

impl<'c> DataBundle<'c> {
    pub fn new(compose: &'c Compose) -> Self {
        Self {
            compose,
            data: BytesMut::new(),
        }
    }

    pub fn add_packet(&mut self, pkt: impl PacketBundle) -> anyhow::Result<()> {
        let data = self.compose.io_buf.encode_packet(pkt, self.compose)?;
        // todo: test to see if this ever actually unsplits
        self.data.unsplit(data);
        Ok(())
    }

    pub fn add_raw(&mut self, raw: &[u8]) {
        self.data.extend_from_slice(raw);
    }

    pub fn unicast(&self, stream: ConnectionId) -> anyhow::Result<()> {
        if self.data.is_empty() {
            return Ok(());
        }

        self.compose.io_buf.unicast_raw(&self.data, stream);
        Ok(())
    }

    // todo: use builder pattern for excluding
    pub fn broadcast_local(&self, center: I16Vec2) -> anyhow::Result<()> {
        if self.data.is_empty() {
            return Ok(());
        }

        self.compose
            .io_buf
            .broadcast_local_raw(&self.data, center, None);
        Ok(())
    }

    // todo: use builder pattern for excluding
    pub fn broadcast_channel(&self, channel: ChannelId) -> anyhow::Result<()> {
        if self.data.is_empty() {
            return Ok(());
        }

        self.compose
            .io_buf
            .broadcast_channel_raw(&self.data, channel, None);

        Ok(())
    }
}
