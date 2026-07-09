pub const TAG_DATA_3D: u8 = 0x2b;
pub const TAG_CHUNK_VERSION: u8 = 0x2c;
pub const TAG_DATA_2D: u8 = 0x2d;
pub const TAG_SUB_CHUNK: u8 = 0x2f;
pub const TAG_LEGACY_VERSION: u8 = 0x76;

pub const DIM_OVERWORLD: i32 = 0;
pub const DIM_NETHER: i32 = 1;
pub const DIM_END: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub dim: i32,
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum ChunkKey {
    SubChunk { pos: ChunkPos, y: i8 },
    Other { pos: ChunkPos, tag: u8 },
}

const COORD_LIMIT: i32 = 2_000_000;

fn plausible(x: i32, z: i32) -> bool {
    x.abs() <= COORD_LIMIT && z.abs() <= COORD_LIMIT
}

fn known_tag(tag: u8) -> bool {
    matches!(
        tag,
        TAG_DATA_3D | TAG_CHUNK_VERSION | TAG_DATA_2D | TAG_SUB_CHUNK | TAG_LEGACY_VERSION
    ) || (0x2b..=0x3d).contains(&tag)
}

pub fn parse_key(key: &[u8]) -> Option<ChunkKey> {
    let (x, z, dim, tag, sub_y) = match key.len() {
        9 => {
            let x = i32::from_le_bytes(key[0..4].try_into().unwrap());
            let z = i32::from_le_bytes(key[4..8].try_into().unwrap());
            (x, z, DIM_OVERWORLD, key[8], None)
        }
        10 => {
            let x = i32::from_le_bytes(key[0..4].try_into().unwrap());
            let z = i32::from_le_bytes(key[4..8].try_into().unwrap());
            (x, z, DIM_OVERWORLD, key[8], Some(key[9] as i8))
        }
        13 => {
            let x = i32::from_le_bytes(key[0..4].try_into().unwrap());
            let z = i32::from_le_bytes(key[4..8].try_into().unwrap());
            let dim = i32::from_le_bytes(key[8..12].try_into().unwrap());
            (x, z, dim, key[12], None)
        }
        14 => {
            let x = i32::from_le_bytes(key[0..4].try_into().unwrap());
            let z = i32::from_le_bytes(key[4..8].try_into().unwrap());
            let dim = i32::from_le_bytes(key[8..12].try_into().unwrap());
            (x, z, dim, key[12], Some(key[13] as i8))
        }
        _ => return None,
    };

    if !plausible(x, z) || !known_tag(tag) {
        return None;
    }
    if key.len() >= 13 && !(1..=2).contains(&dim) {
        return None;
    }

    let pos = ChunkPos { dim, x, z };
    if tag == TAG_SUB_CHUNK {
        let y = sub_y?;

        if !(-9..=24).contains(&(y as i32)) {
            return None;
        }
        Some(ChunkKey::SubChunk { pos, y })
    } else {
        if sub_y.is_some() {
            return None;
        }
        Some(ChunkKey::Other { pos, tag })
    }
}
