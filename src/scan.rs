use std::collections::HashMap;

use levilamina::Server;

use crate::config::Config;

pub struct ChunkSurface {

    pub block: Vec<String>,

    pub color_state: Vec<Option<String>>,

    pub height: Vec<i32>,

    pub water_depth: Vec<u16>,
}

impl ChunkSurface {
    pub fn empty() -> Self {
        Self {
            block: vec![String::new(); 256],
            color_state: vec![None; 256],
            height: vec![i32::MIN; 256],
            water_depth: vec![0u16; 256],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Hint {

    pub top_y: i32,

    pub scans: u32,
}

#[inline]
fn is_air(name: &str) -> bool {
    name.is_empty() || name.ends_with(":air") || name == "air"
}

#[inline]
fn is_water(name: &str) -> bool {
    name.ends_with(":water") || name.ends_with(":flowing_water") || name == "water"
}

fn extract_color(snbt: &str) -> Option<String> {
    let at = snbt.find("color:")?;
    let rest = &snbt[at + "color:".len()..];
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let v = &rest[..end];
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

struct RawScan {
    surface: ChunkSurface,

    top_content_y: i32,

    resolved_at_top: bool,
}

fn scan_chunk(
    server: &Server,
    dim: i32,
    cx: i32,
    cz: i32,
    start_top: i32,
    bottom: i32,
    slab: i32,
) -> Option<RawScan> {
    let base_x = cx * 16;
    let base_z = cz * 16;

    let mut sfc = ChunkSurface::empty();
    let mut resolved = [false; 256];
    let mut count = 0usize;
    let mut top_content_y = i32::MIN;
    let mut resolved_at_top = false;

    let mut y_hi = start_top;
    while y_hi >= bottom && count < 256 {
        let y_lo = std::cmp::max(bottom, y_hi - (slab - 1));

        let scan = server
            .scan_region(dim, (base_x, y_lo, base_z), (base_x + 15, y_hi, base_z + 15))
            .ok()?;

        for layer in scan.layers.iter().rev() {
            let y = layer.y;
            for lx in 0..16usize {
                for lz in 0..16usize {
                    let idx = lx * 16 + lz;
                    if resolved[idx] {
                        continue;
                    }

                    let cell = match layer.cells.get(lx).and_then(|row| row.get(lz)) {
                        Some(c) => c,
                        None => continue,
                    };
                    let name = cell.block.name.as_str();
                    if is_air(name) {
                        continue;
                    }
                    if is_water(name) {
                        sfc.water_depth[idx] = sfc.water_depth[idx].saturating_add(1);
                        if y > top_content_y {
                            top_content_y = y;
                        }
                        if y == start_top {
                            resolved_at_top = true;
                        }
                        continue;
                    }

                    sfc.block[idx] = name.to_string();
                    sfc.color_state[idx] = extract_color(&cell.block.snbt);
                    sfc.height[idx] = y;
                    resolved[idx] = true;
                    count += 1;
                    if y > top_content_y {
                        top_content_y = y;
                    }
                    if y == start_top {
                        resolved_at_top = true;
                    }
                }
            }
            if count >= 256 {
                break;
            }
        }

        y_hi = y_lo - 1;
    }

    Some(RawScan {
        surface: sfc,
        top_content_y,
        resolved_at_top,
    })
}

pub fn scan_one(
    server: &Server,
    cfg: &Config,
    hints: &mut HashMap<(i32, i32, i32), Hint>,
    dim: i32,
    cx: i32,
    cz: i32,
) -> Option<ChunkSurface> {
    let (bottom, dim_top) = cfg.dim_y_range(dim);
    let key = (dim, cx, cz);
    let prev = hints.get(&key).copied();

    let force_full = match prev {
        None => true,
        Some(h) => !cfg.use_height_cache || (h.scans % cfg.full_rescan_every == 0),
    };
    let mut start_top = if force_full {
        dim_top
    } else {
        match prev {
            Some(h) if h.top_y != i32::MIN => {
                (h.top_y + cfg.rebuild_headroom).min(dim_top).max(bottom)
            }
            _ => dim_top,
        }
    };

    let mut raw = scan_chunk(server, dim, cx, cz, start_top, bottom, cfg.slab_height)?;

    if start_top < dim_top && (raw.resolved_at_top || raw.top_content_y == i32::MIN) {
        start_top = dim_top;
        raw = scan_chunk(server, dim, cx, cz, dim_top, bottom, cfg.slab_height)?;
    }
    let _ = start_top;

    if raw.top_content_y == i32::MIN {
        return None;
    }

    let scans = prev.map(|h| h.scans).unwrap_or(0).wrapping_add(1);
    hints.insert(
        key,
        Hint {
            top_y: raw.top_content_y,
            scans,
        },
    );

    Some(raw.surface)
}
