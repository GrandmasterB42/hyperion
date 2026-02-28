use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::ensure;
use bitfield_struct::bitfield;
use flate2::bufread::{GzDecoder, ZlibDecoder};
use glam::IVec2;
use memmap2::MmapOptions;
use tokio::{
    fs::File,
    runtime::Runtime,
    sync::{mpsc, oneshot},
};
use tracing::info;
use valence_anvil::{Compression, RawChunk, RegionError};
use valence_nbt::binary::FromModifiedUtf8;

enum RegionRequest {
    Get {
        coord: IVec2,
        response: oneshot::Sender<std::io::Result<Arc<Region>>>,
    },
}

pub struct RegionManager {
    root: PathBuf,
    sender: mpsc::Sender<RegionRequest>,
}

impl RegionManager {
    pub fn new(runtime: &Runtime, save: &Path) -> anyhow::Result<Self> {
        info!("region manager root: {}", save.display());
        let root = save.join("region");

        ensure!(root.exists(), "{} directory does not exist", root.display());

        let (sender, receiver) = mpsc::channel(100);

        runtime.spawn(RegionManagerTask::new(root.clone(), receiver).run());

        Ok(Self { root, sender })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn get_region_from_chunk(
        &self,
        pos_x: i16,
        pos_z: i16,
    ) -> std::io::Result<Arc<Region>> {
        let pos_x = i32::from(pos_x);
        let pos_z = i32::from(pos_z);

        let region_x = pos_x.div_euclid(32);
        let region_z = pos_z.div_euclid(32);
        let coord = IVec2::new(region_x, region_z);

        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(RegionRequest::Get {
                coord,
                response: response_tx,
            })
            .await
            .expect("RegionManagerTask has been dropped");

        response_rx
            .await
            .expect("RegionManagerTask has been dropped")
    }
}

struct RegionManagerTask {
    root: PathBuf,
    receiver: mpsc::Receiver<RegionRequest>,
    regions: HashMap<IVec2, std::sync::Weak<Region>>,
}

impl RegionManagerTask {
    fn new(root: PathBuf, receiver: mpsc::Receiver<RegionRequest>) -> Self {
        Self {
            root,
            receiver,
            regions: HashMap::new(),
        }
    }

    fn region_path(&self, pos_x: i32, pos_z: i32) -> PathBuf {
        self.root.join(format!("r.{pos_x}.{pos_z}.mca"))
    }

    async fn region_file(&self, pos_x: i32, pos_z: i32) -> std::io::Result<File> {
        File::open(self.region_path(pos_x, pos_z)).await
    }

    async fn run(mut self) {
        while let Some(request) = self.receiver.recv().await {
            self.handle_request(request).await;
        }
    }

    async fn handle_request(&mut self, request: RegionRequest) {
        match request {
            RegionRequest::Get { coord, response } => {
                let region = self.get_or_create_region(coord).await;
                // todo: what should we  do here
                drop(response.send(region));
            }
        }
    }

    async fn get_or_create_region(&mut self, coord: IVec2) -> std::io::Result<Arc<Region>> {
        if let Some(region) = self.regions.get(&coord)
            && let Some(region) = region.upgrade()
        {
            return Ok(region);
        }

        self.create_and_insert_region(coord).await
    }

    async fn create_and_insert_region(&mut self, coord: IVec2) -> std::io::Result<Arc<Region>> {
        let file = self.region_file(coord.x, coord.y).await?;
        let region = Region::open(&file).map_err(std::io::Error::other)?;
        let region = Arc::new(region);
        let region_weak = Arc::downgrade(&region);
        self.regions.insert(coord, region_weak);
        Ok(region)
    }
}

#[bitfield(u32)]
struct Location {
    count: u8,
    #[bits(24)]
    offset: u32,
}

impl Location {
    const fn is_none(self) -> bool {
        self.0 == 0
    }

    const fn offset_and_count(self) -> (u64, usize) {
        (self.offset() as u64, self.count() as usize)
    }
}

#[derive(Debug)]
pub struct Region {
    mmap: memmap2::Mmap,
    locations: [Location; 1024],
    timestamps: [u32; 1024],
}

const SECTOR_SIZE: usize = 4096;

impl Region {
    pub fn open(file: &File) -> Result<Self, RegionError> {
        let mmap = unsafe { MmapOptions::new().map(file)? };

        let Some(header) = &mmap.get(..SECTOR_SIZE * 2) else {
            return Err(RegionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "region header is not present",
            )));
        };

        let locations = std::array::from_fn(|i| {
            Location(u32::from_be_bytes(
                header[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        });
        let timestamps = std::array::from_fn(|i| {
            u32::from_be_bytes(
                header[i * 4 + SECTOR_SIZE..i * 4 + SECTOR_SIZE + 4]
                    .try_into()
                    .unwrap(),
            )
        });

        let mut used_sectors = bitvec::vec::BitVec::repeat(true, 2);
        for location in locations {
            if location.is_none() {
                // No chunk exists at this position.
                continue;
            }

            let (sector_offset, sector_count) = location.offset_and_count();
            if sector_offset < 2 {
                // skip locations pointing inside the header
                continue;
            }
            if sector_count == 0 {
                continue;
            }
            if sector_offset * SECTOR_SIZE as u64 > mmap.len() as u64 {
                // this would go past the end of the file, which is impossible
                continue;
            }

            Self::reserve_sectors(&mut used_sectors, sector_offset, sector_count);
        }

        Ok(Self {
            mmap,
            locations,
            timestamps,
            // used_sectors,
        })
    }

    pub fn get_chunk<S>(
        &self,
        pos_x: i32,
        pos_z: i32,
        decompress_buf: &mut Vec<u8>,
        region_root: &Path,
    ) -> Result<Option<RawChunk<S>>, RegionError>
    where
        S: for<'a> FromModifiedUtf8<'a> + core::hash::Hash + Ord,
    {
        let chunk_idx = Self::chunk_idx(pos_x, pos_z);

        let location = self.locations[chunk_idx];
        let timestamp = self.timestamps[chunk_idx];

        if location.is_none() {
            // No chunk exists at this position.
            return Ok(None);
        }

        let (sector_offset, sector_count) = location.offset_and_count();

        // If the sector offset was <2, then the chunk data would be inside the region
        // header. That doesn't make any sense.
        if sector_offset < 2 {
            return Err(RegionError::InvalidChunkSectorOffset);
        }

        let chunk_start = sector_offset * SECTOR_SIZE as u64;
        let chunk_end = chunk_start + (sector_count * SECTOR_SIZE) as u64;

        if usize::try_from(chunk_end).unwrap() > self.mmap.len() {
            return Err(RegionError::InvalidChunkSize);
        }

        let chunk_data =
            &self.mmap[usize::try_from(chunk_start).unwrap()..usize::try_from(chunk_end).unwrap()];

        let exact_chunk_size = u32::from_be_bytes(chunk_data[..4].try_into().unwrap()) as usize;
        if exact_chunk_size == 0 {
            return Err(RegionError::MissingChunkStream);
        }

        // size of this chunk in sectors must always be >= the exact size.
        if sector_count * SECTOR_SIZE < exact_chunk_size {
            return Err(RegionError::InvalidChunkSize);
        }

        let compression = chunk_data[4];

        let data_buf = if Self::is_external_stream_chunk(compression) {
            let external_file =
                std::fs::File::open(Self::external_chunk_file(pos_x, pos_z, region_root))?;
            let external_mmap = unsafe { MmapOptions::new().map(&external_file)? };
            external_mmap.to_vec().into_boxed_slice()
        } else {
            chunk_data[5..exact_chunk_size].to_vec().into_boxed_slice()
        };

        let r: &[u8] = data_buf.as_ref();

        decompress_buf.clear();

        // What compression does the chunk use?
        let mut nbt_slice = match compression_from_u8(compression) {
            Some(Compression::Gzip) => {
                let mut z = GzDecoder::new(r);
                z.read_to_end(decompress_buf)?;
                decompress_buf.as_slice()
            }
            Some(Compression::Zlib) => {
                let mut z = ZlibDecoder::new(r);
                z.read_to_end(decompress_buf)?;
                decompress_buf.as_slice()
            }
            // Uncompressed
            Some(Compression::None) => r,
            // Unknown
            None => return Err(RegionError::InvalidCompressionScheme(compression)),
            Some(_) => {
                panic!("what???????");
            }
        };

        let (data, _) = valence_nbt::from_binary(&mut nbt_slice)?;

        if !nbt_slice.is_empty() {
            return Err(RegionError::TrailingNbtData);
        }

        Ok(Some(RawChunk { data, timestamp }))
    }

    // fn chunk_positions(
    //     &self,
    //     region_x: i32,
    //     region_z: i32,
    // ) -> Vec<Result<(i32, i32), RegionError>> {
    //     self.locations
    //         .iter()
    //         .enumerate()
    //         .filter_map(move |(index, location)| {
    //             if location.is_none() {
    //                 None
    //             } else {
    //                 Some((
    //                     region_x * 32 + (index % 32) as i32,
    //                     region_z * 32 + (index / 32) as i32,
    //                 ))
    //             }
    //         })
    //         .map(Ok)
    //         .collect()
    // }

    fn external_chunk_file(pos_x: i32, pos_z: i32, region_root: &Path) -> PathBuf {
        region_root
            .to_path_buf()
            .join(format!("c.{pos_x}.{pos_z}.mcc"))
    }

    // fn delete_external_chunk_file(
    //     pos_x: i32,
    //     pos_z: i32,
    //     region_root: &Path,
    // ) -> Result<(), RegionError> {
    //     match std::fs::remove_file(Self::external_chunk_file(pos_x, pos_z, region_root)) {
    //         Ok(()) => Ok(()),
    //         Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
    //         Err(err) => Err(err.into()),
    //     }
    // }

    fn reserve_sectors(
        used_sectors: &mut bitvec::vec::BitVec,
        sector_offset: u64,
        sector_count: usize,
    ) {
        let start_index = usize::try_from(sector_offset).unwrap();
        let end_index = usize::try_from(sector_offset).unwrap() + sector_count;
        if used_sectors.len() < end_index {
            used_sectors.resize(start_index, false);
            used_sectors.resize(end_index, true);
        } else {
            used_sectors[start_index..end_index].fill(true);
        }
    }

    #[expect(clippy::cast_sign_loss, reason = "todo")]
    const fn chunk_idx(pos_x: i32, pos_z: i32) -> usize {
        (pos_x.rem_euclid(32) + pos_z.rem_euclid(32) * 32) as usize
    }

    const fn is_external_stream_chunk(stream_version: u8) -> bool {
        (stream_version & 0x80) != 0
    }

    #[expect(unused, reason = "todo")]
    const fn external_chunk_version(stream_version: u8) -> u8 {
        stream_version & !0x80
    }
}

const fn compression_from_u8(compression: u8) -> Option<Compression> {
    match compression {
        1 => Some(Compression::Gzip),
        2 => Some(Compression::Zlib),
        3 => Some(Compression::None),
        _ => None,
    }
}
