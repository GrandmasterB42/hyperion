use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use bevy_ecs::resource::Resource;
use byteorder::WriteBytesExt;
use glam::I16Vec2;
use hyperion_data::Scratch;
use hyperion_proxy_proto::{
    ChannelId, ConnectionId, ProxyId,
    encoder::PacketEncoder,
    packets::{
        PacketBundle,
        intermediate::{self, IntermediateServerToProxyMessage},
        s2p::ServerToProxyMessage,
        shared::ChunkPosition,
    },
};
use libdeflater::CompressionLvl;
use rustc_hash::FxHashMap;
use thread_local::ThreadLocal;
use tracing::error;
use valence_protocol::bytes::{Bytes, BytesMut};
#[cfg(feature = "reflect")]
use {
    bevy_ecs::reflect::ReflectResource, bevy_reflect::Reflect,
    valence_protocol::CompressionThreshold,
};

use crate::{Broadcast, BroadcastChannel, BroadcastLocal, Shared, Unicast, proxy::EgressComm};

/// A singleton that can be used to compose and encode packets.
#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(Resource))]
pub struct Compose {
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    compression_lvl: CompressionLvl,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    compressor: ThreadLocal<RefCell<libdeflater::Compressor>>,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    scratch: ThreadLocal<RefCell<Scratch>>,
    pub(crate) io_buf: IoBuf,
    /// Data shared between the IO thread and the ECS framework.
    #[cfg_attr(feature = "reflect", reflect(ignore, default = "dummy_reflect_shared"))]
    pub shared: Arc<Shared>,
}

#[cfg(feature = "reflect")]
fn dummy_reflect_shared() -> Arc<Shared> {
    Arc::new(Shared {
        compression_threshold: CompressionThreshold::default(),
        compression_level: CompressionLvl::default(),
    })
}

impl Compose {
    #[must_use]
    pub const fn new(compression_lvl: CompressionLvl, shared: Arc<Shared>, io_buf: IoBuf) -> Self {
        Self {
            compression_lvl,
            compressor: ThreadLocal::new(),
            scratch: ThreadLocal::new(),
            io_buf,
            shared,
        }
    }

    /// Broadcast globally to all players
    ///
    /// See <https://github.com/andrewgazelka/hyperion-proto/blob/main/src/server_to_proxy.proto#L17-L22>
    pub const fn broadcast<P>(&self, packet: P) -> Broadcast<'_, P>
    where
        P: PacketBundle,
    {
        Broadcast::new(packet, self)
    }

    #[must_use]
    #[expect(missing_docs)]
    pub const fn io_buf(&self) -> &IoBuf {
        &self.io_buf
    }

    #[expect(missing_docs)]
    pub const fn io_buf_mut(&mut self) -> &mut IoBuf {
        &mut self.io_buf
    }

    /// Broadcast a packet within a certain region.
    ///
    /// See <https://github.com/andrewgazelka/hyperion-proto/blob/main/src/server_to_proxy.proto#L17-L22>
    pub const fn broadcast_local<P>(&self, packet: P, center: I16Vec2) -> BroadcastLocal<'_, P>
    where
        P: PacketBundle,
    {
        BroadcastLocal::new(packet, self, ChunkPosition {
            x: center.x,
            z: center.y,
        })
    }

    /// Broadcast a packet in a channel.
    pub const fn broadcast_channel<P>(
        &self,
        packet: P,
        channel: ChannelId,
    ) -> BroadcastChannel<'_, P>
    where
        P: PacketBundle,
    {
        BroadcastChannel::new(packet, self, channel)
    }

    /// Send a packet to a single player.
    pub fn unicast<P>(&self, packet: P, stream_id: ConnectionId) -> anyhow::Result<()>
    where
        P: PacketBundle,
    {
        Unicast::new(packet, stream_id, self, true).send()
    }

    /// Send a packet to a single player without compression.
    pub fn unicast_no_compression<P>(
        &self,
        packet: &P,
        stream_id: ConnectionId,
    ) -> anyhow::Result<()>
    where
        P: valence_protocol::Packet + valence_protocol::Encode,
    {
        Unicast::new(packet, stream_id, self, false).send()
    }

    pub(crate) fn encoder(&self) -> PacketEncoder {
        PacketEncoder::new(self.shared.compression_threshold)
    }

    /// Obtain a thread-local scratch buffer.
    #[must_use]
    pub fn scratch(&self) -> &RefCell<Scratch> {
        self.scratch.get_or_default()
    }

    /// Obtain a thread-local [`libdeflater::Compressor`]
    #[must_use]
    pub fn compressor(&self) -> &RefCell<libdeflater::Compressor> {
        self.compressor
            .get_or(|| RefCell::new(libdeflater::Compressor::new(self.compression_lvl)))
    }
}

#[derive(Default)]
#[cfg_attr(feature = "reflect", derive(Reflect))]
pub struct IoBuf {
    // system_on: ThreadLocal<Cell<u32>>,
    // broadcast_buffer: ThreadLocal<RefCell<BytesMut>>,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    temp_buffer: ThreadLocal<RefCell<BytesMut>>,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    idx: ThreadLocal<Cell<u16>>,
    #[cfg_attr(feature = "reflect", reflect(ignore))]
    egress_comms: FxHashMap<ProxyId, EgressComm>,
}

impl IoBuf {
    pub fn fetch_add_idx(&self) -> u16 {
        let cell = self.idx.get_or_default();
        let result = cell.get();
        cell.set(result + 1);
        result
    }

    pub(crate) fn add_proxy(&mut self, proxy_id: ProxyId, egress_comm: EgressComm) {
        let already_exists = self.egress_comms.insert(proxy_id, egress_comm).is_some();

        if already_exists {
            error!("added multiple proxies with the same proxy id {proxy_id:?}");
        }
    }

    pub(crate) fn remove_proxy(&mut self, proxy_id: ProxyId) -> Option<EgressComm> {
        self.egress_comms.remove(&proxy_id)
    }

    pub fn encode_packet<P>(&self, packet: P, compose: &Compose) -> anyhow::Result<BytesMut>
    where
        P: PacketBundle,
    {
        let temp_buffer = self.temp_buffer.get_or_default();
        let temp_buffer = &mut *temp_buffer.borrow_mut();

        let compressor = compose.compressor();
        let mut compressor = compressor.borrow_mut();

        let scratch = compose.scratch();
        let mut scratch = scratch.borrow_mut();

        let result =
            compose
                .encoder()
                .append_packet(packet, temp_buffer, &mut *scratch, &mut compressor)?;

        Ok(result)
    }

    pub fn encode_packet_no_compression<P>(&self, packet: P) -> anyhow::Result<BytesMut>
    where
        P: PacketBundle,
    {
        let temp_buffer = self.temp_buffer.get_or_default();
        let temp_buffer = &mut *temp_buffer.borrow_mut();

        let result =
            hyperion_proxy_proto::encoder::append_packet_without_compression(packet, temp_buffer)?;

        Ok(result)
    }

    pub(crate) fn unicast_private<P>(
        &self,
        packet: P,
        id: ConnectionId,
        compose: &Compose,
        compress: bool,
    ) -> anyhow::Result<()>
    where
        P: PacketBundle,
    {
        let bytes = if compress {
            self.encode_packet(packet, compose)?
        } else {
            self.encode_packet_no_compression(packet)?
        };

        self.unicast_raw(&bytes, id);
        Ok(())
    }

    pub(crate) fn encode_proxy_message(message: &ServerToProxyMessage<'_>) -> Bytes {
        let mut buffer = Vec::<u8>::new();

        buffer.write_u64::<byteorder::BigEndian>(0x00).unwrap();

        rkyv::api::high::to_bytes_in::<_, rkyv::rancor::Error>(message, &mut buffer).unwrap();

        let packet_len = u64::try_from(buffer.len() - size_of::<u64>()).unwrap();
        buffer[0..8].copy_from_slice(&packet_len.to_be_bytes());

        Bytes::from_owner(buffer)
    }

    pub fn add_proxy_message(&self, message: &IntermediateServerToProxyMessage<'_>) {
        if message.affected_by_proxy() {
            // Encode the message for each proxy before sending it
            for (&proxy_id, egress_comm) in &self.egress_comms {
                let Some(message) = message.transform_for_proxy(proxy_id) else {
                    continue;
                };

                egress_comm
                    .tx
                    .send(Self::encode_proxy_message(&message))
                    .unwrap();
            }
        } else {
            // Encode the message once and then send it to each proxy. This uses a placeholder
            // proxy id.
            let Some(message) = message.transform_for_proxy(ProxyId::new(0)) else {
                return;
            };

            let buffer = Self::encode_proxy_message(&message);
            for egress_comm in self.egress_comms.values() {
                egress_comm.tx.send(buffer.clone()).unwrap();
            }
        }
    }

    pub(crate) fn broadcast_local_raw(
        &self,
        data: &[u8],
        center: impl Into<ChunkPosition>,
        exclude: Option<ConnectionId>,
    ) {
        let center = center.into();

        self.add_proxy_message(&IntermediateServerToProxyMessage::BroadcastLocal(
            intermediate::BroadcastLocal {
                center,
                exclude,
                data,
            },
        ));
    }

    pub(crate) fn broadcast_channel_raw(
        &self,
        data: &[u8],
        channel: ChannelId,
        exclude: Option<ConnectionId>,
    ) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::BroadcastChannel(
            intermediate::BroadcastChannel {
                channel_id: channel.inner(),
                data,
                exclude,
            },
        ));
    }

    pub fn broadcast_raw(&self, data: &[u8], exclude: Option<ConnectionId>) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::BroadcastGlobal(
            intermediate::BroadcastGlobal { exclude, data },
        ));
    }

    pub fn unicast_raw(&self, data: &[u8], stream: ConnectionId) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::Unicast(
            intermediate::Unicast { stream, data },
        ));
    }

    pub fn set_receive_broadcasts(&self, stream: ConnectionId) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::SetReceiveBroadcasts(
            intermediate::SetReceiveBroadcasts { stream },
        ));
    }

    pub fn add_channel(&self, channel: ChannelId, unsubscribe_packets: &[u8]) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::AddChannel(
            intermediate::AddChannel {
                channel_id: channel.inner(),
                unsubscribe_packets,
            },
        ));
    }

    pub fn send_subscribe_channel_packets(
        &self,
        channel: ChannelId,
        packets: &[u8],
        exclude: Option<ConnectionId>,
    ) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::SubscribeChannelPackets(
            intermediate::SubscribeChannelPackets {
                channel_id: channel.inner(),
                exclude,
                data: packets,
            },
        ));
    }

    pub fn remove_channel(&self, channel: ChannelId) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::RemoveChannel(
            intermediate::RemoveChannel {
                channel_id: channel.inner(),
            },
        ));
    }

    pub fn shutdown(&self, stream: ConnectionId) {
        self.add_proxy_message(&IntermediateServerToProxyMessage::Shutdown(
            intermediate::Shutdown { stream },
        ));
    }
}
