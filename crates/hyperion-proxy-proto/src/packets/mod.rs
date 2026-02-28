pub mod intermediate;
pub mod shared;

mod proxy_to_server;
pub mod p2s {
    pub use super::proxy_to_server::*;
}

mod server_to_proxy;
pub mod s2p {
    pub use super::server_to_proxy::*;
}

pub trait PacketBundle {
    fn encode_including_ids(self, w: impl std::io::Write) -> anyhow::Result<()>;
}

impl<T: valence_protocol::Packet + valence_protocol::Encode> PacketBundle for &T {
    fn encode_including_ids(self, w: impl std::io::Write) -> anyhow::Result<()> {
        self.encode_with_id(w)
    }
}
