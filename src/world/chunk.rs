use super::subchunk::SubChunk;

pub struct ChunkSurface {

    pub block: Vec<String>,

    pub color_state: Vec<Option<String>>,

    pub height: Vec<i32>,

    pub water_depth: Vec<u16>,
}

#[inline]
fn is_air(name: &str) -> bool {
    name.ends_with(":air") || name == "air"
}

#[inline]
fn is_water(name: &str) -> bool {
    name.ends_with(":water") || name.ends_with(":flowing_water")
}

pub fn compute_surface(mut subchunks: Vec<SubChunk>) -> ChunkSurface {

    subchunks.sort_by(|a, b| b.y.cmp(&a.y));

    let mut out = ChunkSurface {
        block: vec![String::new(); 256],
        color_state: vec![None; 256],
        height: vec![i32::MIN; 256],
        water_depth: vec![0u16; 256],
    };
    let mut done = [false; 256];
    let mut remaining = 256usize;

    for sc in &subchunks {
        if remaining == 0 {
            break;
        }
        let terrain = match sc.layers.first() {
            Some(l) => l,
            None => continue,
        };
        let waterlog = sc.layers.get(1);
        let base_y = sc.y as i32 * 16;

        for x in 0..16usize {
            for z in 0..16usize {
                let col = x * 16 + z;
                if done[col] {
                    continue;
                }
                for y in (0..16usize).rev() {
                    let Some(block) = terrain.block_at(x, y, z) else { continue };
                    let name = block.name.as_str();

                    let waterlogged = waterlog
                        .and_then(|l| l.block_at(x, y, z))
                        .map(|b| is_water(&b.name))
                        .unwrap_or(false);

                    if is_air(name) {
                        if waterlogged {
                            out.water_depth[col] = out.water_depth[col].saturating_add(1);
                        }
                        continue;
                    }
                    if is_water(name) {
                        out.water_depth[col] = out.water_depth[col].saturating_add(1);
                        continue;
                    }

                    out.block[col] = name.to_string();
                    out.color_state[col] = block.color.clone();
                    out.height[col] = base_y + y as i32;
                    done[col] = true;
                    remaining -= 1;
                    break;
                }
            }
        }
    }

    out
}
