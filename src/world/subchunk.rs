use crate::nbt::NbtReader;

#[derive(Debug, Clone)]
pub struct PaletteBlock {
    pub name: String,

    pub color: Option<String>,
}

pub struct Layer {
    pub indices: Vec<u16>,
    pub palette: Vec<PaletteBlock>,
}

impl Layer {
    #[inline]
    pub fn block_at(&self, x: usize, y: usize, z: usize) -> Option<&PaletteBlock> {
        let idx = *self.indices.get((x << 8) | (z << 4) | y)? as usize;
        self.palette.get(idx)
    }
}

pub struct SubChunk {
    pub y: i8,
    pub layers: Vec<Layer>,
}

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    fn i8(&mut self) -> Option<i8> {
        self.u8().map(|v| v as i8)
    }
    fn u32(&mut self) -> Option<u32> {
        if self.pos + 4 > self.data.len() {
            return None;
        }
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Some(v)
    }
    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
}

pub fn decode(value: &[u8], key_y: i8) -> Option<SubChunk> {
    let mut b = Bytes {
        data: value,
        pos: 0,
    };
    let version = b.u8()?;

    match version {
        1 => {
            let layer = decode_storage(&mut b)?;
            Some(SubChunk {
                y: key_y,
                layers: vec![layer],
            })
        }
        8 | 9 => {
            let count = b.u8()? as usize;
            let y = if version == 9 { b.i8()? } else { key_y };
            let mut layers = Vec::with_capacity(count.min(2));
            for i in 0..count {
                let layer = decode_storage(&mut b)?;
                if i < 2 {
                    layers.push(layer);
                }
            }
            Some(SubChunk { y, layers })
        }
        0 | 2..=7 => decode_legacy(&mut b, key_y),
        _ => None,
    }
}

fn decode_storage(b: &mut Bytes) -> Option<Layer> {
    let format = b.u8()?;
    let bits = (format >> 1) as usize;

    let mut indices = vec![0u16; 4096];
    if bits > 0 {

        if ![1, 2, 3, 4, 5, 6, 8, 16].contains(&bits) {
            return None;
        }
        let blocks_per_word = 32 / bits;
        let word_count = (4096 + blocks_per_word - 1) / blocks_per_word;
        let mask = (1u32 << bits) - 1;
        for w in 0..word_count {
            let word = b.u32()?;
            for j in 0..blocks_per_word {
                let i = w * blocks_per_word + j;
                if i >= 4096 {
                    break;
                }
                indices[i] = ((word >> (j * bits)) & mask) as u16;
            }
        }
    }

    let count = b.i32()?;
    if !(0..=4096).contains(&count) {
        return None;
    }
    let mut palette = Vec::with_capacity(count as usize);
    let mut reader = NbtReader::new(&b.data[b.pos..]);
    for _ in 0..count {
        let (_name, tag) = reader.read_root()?;
        let name = tag
            .get("name")
            .and_then(|t| t.as_str())
            .unwrap_or("minecraft:unknown")
            .to_string();
        let color = tag
            .get("states")
            .and_then(|s| s.get("color"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        palette.push(PaletteBlock { name, color });
    }
    b.pos += reader.pos;

    if palette.is_empty() {
        palette.push(PaletteBlock {
            name: "minecraft:air".into(),
            color: None,
        });
    }
    Some(Layer { indices, palette })
}

fn decode_legacy(b: &mut Bytes, key_y: i8) -> Option<SubChunk> {
    if b.data.len() < b.pos + 4096 {
        return None;
    }
    let ids = &b.data[b.pos..b.pos + 4096];

    let mut palette: Vec<PaletteBlock> = Vec::new();
    let mut id_to_idx = [u16::MAX; 256];
    let mut indices = vec![0u16; 4096];
    for (i, &id) in ids.iter().enumerate() {
        let idx = if id_to_idx[id as usize] != u16::MAX {
            id_to_idx[id as usize]
        } else {
            let idx = palette.len() as u16;
            palette.push(PaletteBlock {
                name: legacy_id_name(id).to_string(),
                color: None,
            });
            id_to_idx[id as usize] = idx;
            idx
        };
        indices[i] = idx;
    }

    Some(SubChunk {
        y: key_y,
        layers: vec![Layer { indices, palette }],
    })
}

fn legacy_id_name(id: u8) -> &'static str {
    match id {
        0 => "minecraft:air",
        1 => "minecraft:stone",
        2 => "minecraft:grass_block",
        3 => "minecraft:dirt",
        4 => "minecraft:cobblestone",
        5 => "minecraft:planks",
        6 => "minecraft:sapling",
        7 => "minecraft:bedrock",
        8 | 9 => "minecraft:water",
        10 | 11 => "minecraft:lava",
        12 => "minecraft:sand",
        13 => "minecraft:gravel",
        14 => "minecraft:gold_ore",
        15 => "minecraft:iron_ore",
        16 => "minecraft:coal_ore",
        17 => "minecraft:oak_log",
        18 => "minecraft:oak_leaves",
        20 => "minecraft:glass",
        24 => "minecraft:sandstone",
        31 => "minecraft:short_grass",
        35 => "minecraft:white_wool",
        37 | 38 => "minecraft:poppy",
        43 | 44 => "minecraft:stone_slab",
        45 => "minecraft:bricks",
        48 => "minecraft:mossy_cobblestone",
        49 => "minecraft:obsidian",
        50 => "minecraft:torch",
        53 => "minecraft:oak_stairs",
        56 => "minecraft:diamond_ore",
        58 => "minecraft:crafting_table",
        59 => "minecraft:wheat",
        60 => "minecraft:farmland",
        61 | 62 => "minecraft:furnace",
        64 => "minecraft:oak_door",
        65 => "minecraft:ladder",
        66 => "minecraft:rail",
        67 => "minecraft:stone_stairs",
        78 => "minecraft:snow_layer",
        79 => "minecraft:ice",
        80 => "minecraft:snow_block",
        81 => "minecraft:cactus",
        82 => "minecraft:clay",
        83 => "minecraft:sugar_cane",
        86 => "minecraft:pumpkin",
        87 => "minecraft:netherrack",
        88 => "minecraft:soul_sand",
        89 => "minecraft:glowstone",
        98 => "minecraft:stone_bricks",
        103 => "minecraft:melon_block",
        110 => "minecraft:mycelium",
        112 => "minecraft:nether_bricks",
        121 => "minecraft:end_stone",
        129 => "minecraft:emerald_ore",
        133 => "minecraft:emerald_block",
        152 => "minecraft:redstone_block",
        155 => "minecraft:quartz_block",
        159 => "minecraft:stained_hardened_clay",
        161 => "minecraft:acacia_leaves",
        162 => "minecraft:acacia_log",
        172 => "minecraft:hardened_clay",
        174 => "minecraft:packed_ice",
        179 => "minecraft:red_sandstone",
        _ => "minecraft:stone",
    }
}
