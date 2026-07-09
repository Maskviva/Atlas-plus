use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde_json::{json, Map, Value};

use crate::colors::{ColorMap, Rgba};
use crate::config::Config;
use crate::scan::ChunkSurface;
use crate::util::iso_now;

pub const TILE: usize = 512;
const MAX_ZOOM: u32 = 8;

pub enum ToRenderer {

    Chunk {
        dim: i32,
        cx: i32,
        cz: i32,
        surface: Box<ChunkSurface>,

        bootstrap: bool,
    },

    Players(Value),

    Spawn([i32; 3]),

    Shutdown,
}

struct Region {
    color: Vec<Rgba>,
    height: Vec<i32>,
    dirty: bool,
    last_update: Instant,
}

impl Region {
    fn new() -> Self {
        Self {
            color: vec![[0u8; 4]; TILE * TILE],
            height: vec![i32::MIN; TILE * TILE],
            dirty: true,
            last_update: Instant::now(),
        }
    }
}

struct TileEntry {
    img: RgbaImage,
    version: u64,
}

struct DimState {

    regions: HashMap<(i32, i32), Region>,

    chunks_seen: HashSet<(i32, i32)>,

    tiles: HashMap<(u32, i32, i32), TileEntry>,

    pending_work: HashSet<(i32, i32)>,
}

impl DimState {
    fn new() -> Self {
        Self {
            regions: HashMap::new(),
            chunks_seen: HashSet::new(),
            tiles: HashMap::new(),
            pending_work: HashSet::new(),
        }
    }
}

struct Renderer {
    out_dir: PathBuf,
    cfg: Arc<Config>,
    colors: ColorMap,
    dims: HashMap<i32, DimState>,
    version: u64,

    version_out: Arc<AtomicU64>,
    players: Value,
    players_str: String,
    spawn: Option<[i32; 3]>,
    dirty: bool,
    last_flush: Instant,
}

pub fn run(
    rx: Receiver<ToRenderer>,
    cfg: Arc<Config>,
    colors: ColorMap,
    version_out: Arc<AtomicU64>,
) {
    let tick = Duration::from_millis(cfg.flush_interval_ms.clamp(50, 500));
    let mut r = Renderer {
        out_dir: PathBuf::from(&cfg.out_dir),
        cfg,
        colors,
        dims: HashMap::new(),
        version: 0,
        version_out,
        players: json!([]),
        players_str: "[]".to_string(),
        spawn: None,
        dirty: false,
        last_flush: Instant::now(),
    };

    r.write_initial();

    loop {
        match rx.recv_timeout(tick) {
            Ok(msg) => {
                if r.handle(msg) {
                    break;
                }

                while let Ok(msg) = rx.try_recv() {
                    if r.handle(msg) {
                        r.flush(true);
                        return;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                r.flush(true);
                break;
            }
        }
        r.maybe_flush();
    }
}

impl Renderer {

    fn handle(&mut self, msg: ToRenderer) -> bool {
        match msg {
            ToRenderer::Chunk {
                dim,
                cx,
                cz,
                surface,
                bootstrap,
            } => {
                self.apply_chunk(dim, cx, cz, &surface, bootstrap);
                false
            }
            ToRenderer::Players(v) => {
                self.set_players(v);
                false
            }
            ToRenderer::Spawn(xyz) => {
                self.spawn = Some(xyz);
                self.flush(true);
                false
            }
            ToRenderer::Shutdown => {
                self.flush(true);
                true
            }
        }
    }

    fn set_players(&mut self, v: Value) {
        let s = v.to_string();
        if s == self.players_str {
            return;
        }
        self.players = v;
        self.players_str = s;
        self.write_live();
    }

    fn apply_chunk(&mut self, dim: i32, cx: i32, cz: i32, sfc: &ChunkSurface, bootstrap: bool) {
        if !self.cfg.dimensions.contains(&dim) {
            return;
        }
        let max_regions = self.cfg.max_regions;
        let pending = self.version + 1;
        let colors = &self.colors;

        let ds = self.dims.entry(dim).or_insert_with(DimState::new);

        if bootstrap && ds.chunks_seen.contains(&(cx, cz)) {
            return;
        }
        ds.chunks_seen.insert((cx, cz));

        let rx = cx >> 5;
        let rz = cz >> 5;
        let rkey = (rx, rz);

        if !ds.regions.contains_key(&rkey) && ds.regions.len() >= max_regions {
            if let Some(oldest) = ds
                .regions
                .iter()
                .min_by_key(|(_, r)| r.last_update)
                .map(|(k, _)| *k)
            {
                if let Some(old) = ds.regions.remove(&oldest) {

                    if old.dirty {
                        let img = shade_region(&old.color, &old.height);
                        let dir = self.out_dir.join("tiles").join(dim_id(dim)).join("0");
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = write_tile(&img, &dir, oldest.0, oldest.1);
                        crate::ws::publish_tile(dim, 0, oldest.0, oldest.1, &img);
                        ds.tiles.insert(
                            (0, oldest.0, oldest.1),
                            TileEntry { img, version: pending },
                        );
                        ds.pending_work.insert(oldest);
                        self.dirty = true;
                    }
                }
            }
        }

        let region = ds.regions.entry(rkey).or_insert_with(Region::new);

        let ox = ((cx - (rx << 5)) * 16) as usize;
        let oz = ((cz - (rz << 5)) * 16) as usize;

        let mut changed = false;
        for lx in 0..16usize {
            for lz in 0..16usize {
                let sidx = lx * 16 + lz;
                let i = (oz + lz) * TILE + (ox + lx);

                let h = sfc.height[sidx];
                if h == i32::MIN {

                    if region.color[i][3] != 0 || region.height[i] != i32::MIN {
                        region.color[i] = [0, 0, 0, 0];
                        region.height[i] = i32::MIN;
                        changed = true;
                    }
                    continue;
                }

                let name = &sfc.block[sidx];
                let mut c = colors.get(name, sfc.color_state[sidx].as_deref());

                let depth = sfc.water_depth[sidx];
                if depth > 0 {
                    let water = colors.get("minecraft:water", None);
                    let t = (0.55 + depth as f32 * 0.045).min(0.92);
                    for k in 0..3 {
                        c[k] = (c[k] as f32 * (1.0 - t) + water[k] as f32 * t) as u8;
                    }
                    c[3] = 255;
                }

                let nh = h + depth as i32;
                if region.color[i] != c || region.height[i] != nh {
                    region.color[i] = c;
                    region.height[i] = nh;
                    changed = true;
                }
            }
        }

        region.last_update = Instant::now();
        if changed {
            region.dirty = true;
            self.dirty = true;
        }
    }

    fn maybe_flush(&mut self) {
        if self.dirty
            && self.last_flush.elapsed() >= Duration::from_millis(self.cfg.flush_interval_ms)
        {
            self.flush(false);
        }
    }

    fn flush(&mut self, force: bool) {
        if !self.dirty && !force {
            return;
        }

        let pending = self.version + 1;
        let mut wrote_any = false;
        let mut dim_manifests: Vec<Value> = Vec::new();

        let dims: Vec<i32> = self.dims.keys().copied().collect();
        for dim in dims {
            if let Some(m) = self.flush_dim(dim, pending, &mut wrote_any) {
                dim_manifests.push(m);
            }
        }

        if wrote_any {
            self.version = pending;
            self.version_out.store(self.version, Ordering::Relaxed);
        }
        self.write_map_json(&dim_manifests);
        self.write_live();

        self.dirty = false;
        self.last_flush = Instant::now();
    }

    fn flush_dim(&mut self, dim: i32, pending: u64, wrote_any: &mut bool) -> Option<Value> {
        let dim_name = dim_id(dim);
        let dim_dir = self.out_dir.join("tiles").join(dim_name);

        let mut work: HashSet<(i32, i32)>;
        {
            let ds = self.dims.get_mut(&dim)?;
            let dirty_keys: Vec<(i32, i32)> = ds
                .regions
                .iter()
                .filter(|(_, r)| r.dirty)
                .map(|(k, _)| *k)
                .collect();
            if !dirty_keys.is_empty() {
                let _ = std::fs::create_dir_all(dim_dir.join("0"));
            }
            for k in dirty_keys {
                let img = {
                    let region = ds.regions.get(&k).unwrap();
                    shade_region(&region.color, &region.height)
                };
                if write_tile(&img, &dim_dir.join("0"), k.0, k.1) {
                    *wrote_any = true;
                }
                crate::ws::publish_tile(dim, 0, k.0, k.1, &img);
                ds.tiles.insert((0, k.0, k.1), TileEntry { img, version: pending });
                if let Some(region) = ds.regions.get_mut(&k) {
                    region.dirty = false;
                }
                ds.pending_work.insert(k);
            }

            work = std::mem::take(&mut ds.pending_work);
        }

        let half = (TILE / 2) as u32;
        let mut z = 0u32;
        while z < MAX_ZOOM {
            let level_tiles: Vec<(i32, i32)> = {
                let ds = self.dims.get(&dim)?;
                ds.tiles
                    .keys()
                    .filter(|(zz, _, _)| *zz == z)
                    .map(|(_, tx, tz)| (*tx, *tz))
                    .collect()
            };
            if level_tiles.len() <= 1 {
                break;
            }

            let mut targets: Vec<(i32, i32)> = Vec::new();
            {
                let ds = self.dims.get(&dim)?;
                let mut seen: HashSet<(i32, i32)> = HashSet::new();
                for &(tx, tz) in &level_tiles {
                    let p = (tx >> 1, tz >> 1);
                    if !seen.insert(p) {
                        continue;
                    }
                    let missing = !ds.tiles.contains_key(&(z + 1, p.0, p.1));
                    let child_changed = (0..2i32).any(|dx| {
                        (0..2i32).any(|dz| work.contains(&((p.0 << 1) + dx, (p.1 << 1) + dz)))
                    });
                    if missing || child_changed {
                        targets.push(p);
                    }
                }
            }

            if targets.is_empty() {
                break;
            }

            let dir = dim_dir.join((z + 1).to_string());
            let _ = std::fs::create_dir_all(&dir);

            for &(px, pz) in &targets {
                let mut canvas = RgbaImage::new(TILE as u32, TILE as u32);
                for dz in 0..2i32 {
                    for dx in 0..2i32 {
                        let cxk = (px << 1) + dx;
                        let czk = (pz << 1) + dz;
                        let bx = dx as u32 * half;
                        let bz = dz as u32 * half;

                        let ds = self.dims.get(&dim)?;
                        if let Some(entry) = ds.tiles.get(&(z, cxk, czk)) {
                            let child = &entry.img;
                            for y in 0..half {
                                for x in 0..half {
                                    let mut acc = [0u32; 4];
                                    for sy in 0..2u32 {
                                        for sx in 0..2u32 {
                                            let p = child.get_pixel(x * 2 + sx, y * 2 + sy).0;
                                            for kk in 0..4usize {
                                                acc[kk] += p[kk] as u32;
                                            }
                                        }
                                    }
                                    canvas.put_pixel(
                                        bx + x,
                                        bz + y,
                                        image::Rgba([
                                            (acc[0] / 4) as u8,
                                            (acc[1] / 4) as u8,
                                            (acc[2] / 4) as u8,
                                            (acc[3] / 4) as u8,
                                        ]),
                                    );
                                }
                            }
                        }
                    }
                }
                if write_tile(&canvas, &dir, px, pz) {
                    *wrote_any = true;
                }
                crate::ws::publish_tile(dim, z + 1, px, pz, &canvas);
                let ds = self.dims.get_mut(&dim)?;
                ds.tiles
                    .insert((z + 1, px, pz), TileEntry { img: canvas, version: pending });
            }

            work = targets.into_iter().collect();
            z += 1;
        }

        let ds = self.dims.get(&dim)?;
        if ds.tiles.is_empty() {
            return None;
        }
        let (mut min_tx, mut max_tx, mut min_tz, mut max_tz) =
            (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        let mut max_zoom = 0u32;
        for (zz, tx, tz) in ds.tiles.keys() {
            if *zz > max_zoom {
                max_zoom = *zz;
            }
            if *zz == 0 {
                min_tx = min_tx.min(*tx);
                max_tx = max_tx.max(*tx);
                min_tz = min_tz.min(*tz);
                max_tz = max_tz.max(*tz);
            }
        }
        if min_tx > max_tx {
            min_tx = 0;
            max_tx = 0;
            min_tz = 0;
            max_tz = 0;
        }
        let mut tiles_map = Map::new();
        for ((zz, tx, tz), entry) in ds.tiles.iter() {
            tiles_map.insert(format!("{}/{}_{}", zz, tx, tz), json!(entry.version));
        }

        Some(json!({
            "id": dim_name,
            "minTx": min_tx,
            "maxTx": max_tx,
            "minTz": min_tz,
            "maxTz": max_tz,
            "maxZoom": max_zoom,
            "chunks": ds.chunks_seen.len(),
            "tiles": Value::Object(tiles_map),
        }))
    }

    fn write_map_json(&self, dims: &[Value]) {
        let doc = json!({
            "name": self.cfg.world_name,
            "tileSize": TILE,
            "generated": iso_now(),
            "version": self.version,
            "pollMs": self.cfg.live_poll_ms,
            "live": true,
            "spawn": self.spawn,
            "dimensions": dims,
        });
        let _ = write_atomic(
            &self.out_dir.join("map.json"),
            serde_json::to_string_pretty(&doc).unwrap_or_default().as_bytes(),
        );
        crate::ws::publish_map(&doc);
    }

    fn write_live(&self) {
        let doc = json!({
            "version": self.version,
            "generated": iso_now(),
            "players": self.players,
        });
        let _ = write_atomic(&self.out_dir.join("live.json"), doc.to_string().as_bytes());
        crate::ws::publish_players(&self.players, self.version);
    }

    fn write_initial(&self) {
        let _ = std::fs::create_dir_all(&self.out_dir);
        self.write_map_json(&[]);
        self.write_live();
    }
}

pub fn dim_id(dim: i32) -> &'static str {
    match dim {
        1 => "nether",
        2 => "end",
        _ => "overworld",
    }
}

fn shade_region(color: &[Rgba], height: &[i32]) -> RgbaImage {
    let t = TILE;
    let mut out = vec![0u8; t * t * 4];
    for pz in 0..t {
        for px in 0..t {
            let i = pz * t + px;
            let c = color[i];
            let o = i * 4;
            if c[3] == 0 {
                continue;
            }
            let h = height[i];
            let mut f = 1.0f32;
            if h != i32::MIN {
                let hw = if px > 0 { height[i - 1] } else { i32::MIN };
                let hn = if pz > 0 { height[i - t] } else { i32::MIN };
                let mut d = 0i32;
                if hw != i32::MIN {
                    d += (h - hw).clamp(-4, 4);
                }
                if hn != i32::MIN {
                    d += (h - hn).clamp(-4, 4);
                }
                if d != 0 {
                    f = (1.0 + d as f32 * 0.045).clamp(0.70, 1.30);
                }
            }
            out[o] = (c[0] as f32 * f).min(255.0) as u8;
            out[o + 1] = (c[1] as f32 * f).min(255.0) as u8;
            out[o + 2] = (c[2] as f32 * f).min(255.0) as u8;
            out[o + 3] = c[3];
        }
    }
    RgbaImage::from_raw(t as u32, t as u32, out).expect("buffer size matches TILE*TILE*4")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn write_tile(img: &RgbaImage, dir: &Path, tx: i32, tz: i32) -> bool {
    let tmp = dir.join(format!("{}_{}.tmp.png", tx, tz));
    if img.save(&tmp).is_err() {
        return false;
    }
    std::fs::rename(&tmp, dir.join(format!("{}_{}.png", tx, tz))).is_ok()
}
