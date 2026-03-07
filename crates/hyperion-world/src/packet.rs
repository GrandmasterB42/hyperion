use std::io::Write;

use hyperion_proxy_proto::packets::PacketBundle;
use valence_protocol::{
    ChunkSectionPos, Encode, Packet, VarInt,
    packets::play::{ChunkDeltaUpdateS2c, chunk_delta_update_s2c::ChunkDeltaUpdateEntry},
};

use crate::loader::section::Section;

pub struct DeltaDrainPacket<'a> {
    pub position: ChunkSectionPos,
    pub section: &'a mut Section,
}

impl PacketBundle for DeltaDrainPacket<'_> {
    fn encode_including_ids(self, mut write: impl Write) -> anyhow::Result<()> {
        VarInt(ChunkDeltaUpdateS2c::ID).encode(&mut write)?;

        self.position.encode(&mut write)?;

        let deltas = &mut self.section.changed_since_last_tick;
        let len = deltas.len();
        VarInt(i32::try_from(len)?).encode(&mut write)?;

        for delta_idx in deltas.iter() {
            let block_state =
                unsafe { self.section.block_states.get_unchecked(delta_idx as usize) };

            // Convert delta (u16) to y, z, x
            let y = (delta_idx >> 8) & 0xF;
            let z = (delta_idx >> 4) & 0xF;
            let x = delta_idx & 0xF;

            let entry = ChunkDeltaUpdateEntry::new()
                .with_off_x(x as u8)
                .with_off_y(y as u8)
                .with_off_z(z as u8)
                .with_block_state(u32::from(block_state));

            entry.encode(&mut write)?;
        }

        deltas.clear();

        self.section.reset_tick_deltas();

        Ok(())
    }
}

pub struct DeltaPacket<'a> {
    pub position: ChunkSectionPos,
    pub section: &'a Section,
}

impl PacketBundle for DeltaPacket<'_> {
    fn encode_including_ids(self, mut write: impl Write) -> anyhow::Result<()> {
        VarInt(ChunkDeltaUpdateS2c::ID).encode(&mut write)?;

        self.position.encode(&mut write)?;

        let deltas = &self.section.changed;
        let len = deltas.len();
        VarInt(i32::try_from(len)?).encode(&mut write)?;

        for delta_idx in deltas {
            let block_state =
                unsafe { self.section.block_states.get_unchecked(delta_idx as usize) };

            // Convert delta (u16) to y, z, x
            let y = (delta_idx >> 8) & 0xF;
            let z = (delta_idx >> 4) & 0xF;
            let x = delta_idx & 0xF;

            let entry = ChunkDeltaUpdateEntry::new()
                .with_off_x(x as u8)
                .with_off_y(y as u8)
                .with_off_z(z as u8)
                .with_block_state(u32::from(block_state));

            entry.encode(&mut write)?;
        }

        Ok(())
    }
}
