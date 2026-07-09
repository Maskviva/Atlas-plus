use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{atomic::AtomicBool, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use levilamina::Logger;

use crate::config::Config;
use crate::leveldb;
use crate::render::ToRenderer;
use crate::scan::ChunkSurface;
use crate::world::chunk::compute_surface;
use crate::world::keys::{parse_key, ChunkKey};
use crate::world::subchunk::{decode, SubChunk};

const ST_OFF: u8 = 0;
const ST_RUNNING: u8 = 1;
const ST_DONE: u8 = 2;
const ST_FAILED: u8 = 3;
const ST_CANCELLED: u8 = 4;

static STATE: AtomicU8 = AtomicU8::new(ST_OFF);
static DONE: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

pub fn status() -> String {
    match STATE.load(Ordering::Relaxed) {
        ST_RUNNING => format!(
            "bootstrapping from disk {}/{}",
            DONE.load(Ordering::Relaxed),
            TOTAL.load(Ordering::Relaxed)
        ),
        ST_DONE => format!("base map: {} chunks from disk", DONE.load(Ordering::Relaxed)),
        ST_FAILED => "disk bootstrap failed (see log); live-only".into(),
        ST_CANCELLED => "disk bootstrap cancelled".into(),
        _ => "disk bootstrap off".into(),
    }
}

pub fn spawn(
    cfg: Arc<Config>,
    tx: Sender<ToRenderer>,
    logger: Logger,
    cancel: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    DONE.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    if !cfg.bootstrap {
        STATE.store(ST_OFF, Ordering::Relaxed);
        return None;
    }
    STATE.store(ST_RUNNING, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("atlas-plus-bootstrap".into())
        .spawn(move || {
            let state = run(&cfg, &tx, logger, &cancel);
            STATE.store(state, Ordering::Relaxed);
        })
        .ok()
}

pub fn world_spawn(cfg: &Config) -> Option<[i32; 3]> {
    let world = locate_world(cfg)?;
    read_spawn(&world)
}

fn run(cfg: &Config, tx: &Sender<ToRenderer>, logger: Logger, cancel: &AtomicBool) -> u8 {
    let world = match locate_world(cfg) {
        Some(w) => w,
        None => {
            logger.warn(
                "bootstrap: couldn't locate the world folder (set \"world_dir\" in config.json \
                 to the folder containing db/ and level.dat); starting live-only",
            );
            return ST_FAILED;
        }
    };
    let db_dir = world.join("db");
    logger.info(&format!("bootstrap: reading {}", db_dir.display()));

    let spawn_xz = read_spawn(&world).map(|[x, _, z]| (x, z));
    let radius = cfg.bootstrap_radius_chunks;

    let mut grand_total = 0usize;
    for &dim in &cfg.dimensions {
        if cancel.load(Ordering::Acquire) {
            return ST_CANCELLED;
        }

        let mut centers: Vec<(i32, i32)> = Vec::new();
        if radius.is_some() {
            for pa in &cfg.pinned_areas {
                if pa.dim == dim {
                    centers.push((pa.x >> 4, pa.z >> 4));
                }
            }
            if dim == 0 {
                if let Some((sx, sz)) = spawn_xz {
                    centers.push((sx >> 4, sz >> 4));
                }
            }
            if centers.is_empty() {
                centers.push((0, 0));
            }
        }
        let within = |cx: i32, cz: i32| -> bool {
            match radius {
                None => true,
                Some(r) => centers
                    .iter()
                    .any(|&(ax, az)| (cx - ax).abs() <= r && (cz - az).abs() <= r),
            }
        };

        let snapshot = match leveldb::load(&db_dir, |key| match parse_key(key) {
            Some(ChunkKey::SubChunk { pos, .. }) => pos.dim == dim && within(pos.x, pos.z),
            _ => false,
        }) {
            Ok(s) => s,
            Err(e) => {
                logger.warn(&format!(
                    "bootstrap: database read failed ({e}); starting live-only"
                ));
                return ST_FAILED;
            }
        };
        if snapshot.is_empty() {
            continue;
        }

        let mut chunks: HashMap<(i32, i32), Vec<SubChunk>> = HashMap::new();
        for (key, value) in snapshot {
            if let Some(ChunkKey::SubChunk { pos, y }) = parse_key(&key) {
                if let Some(sc) = decode(&value, y) {
                    chunks.entry((pos.x, pos.z)).or_default().push(sc);
                }
            }
        }

        let mut regions: BTreeMap<(i32, i32), Vec<(i32, i32)>> = BTreeMap::new();
        for &(cx, cz) in chunks.keys() {
            regions.entry((cx >> 5, cz >> 5)).or_default().push((cx, cz));
        }

        let dim_total = chunks.len();
        TOTAL.fetch_add(dim_total, Ordering::Relaxed);
        logger.info(&format!(
            "bootstrap: dimension {dim}: {dim_total} chunks in {} regions",
            regions.len()
        ));

        for (_rk, mut list) in regions {
            if cancel.load(Ordering::Acquire) {
                return ST_CANCELLED;
            }
            list.sort_unstable();
            for (cx, cz) in list {
                if let Some(subs) = chunks.remove(&(cx, cz)) {
                    let s = compute_surface(subs);
                    let surface = ChunkSurface {
                        block: s.block,
                        color_state: s.color_state,
                        height: s.height,
                        water_depth: s.water_depth,
                    };
                    if tx
                        .send(ToRenderer::Chunk {
                            dim,
                            cx,
                            cz,
                            surface: Box::new(surface),
                            bootstrap: true,
                        })
                        .is_err()
                    {

                        return ST_CANCELLED;
                    }
                    DONE.fetch_add(1, Ordering::Relaxed);
                }
            }
            if cfg.bootstrap_throttle_ms > 0 {
                std::thread::sleep(Duration::from_millis(cfg.bootstrap_throttle_ms));
            }
        }
        grand_total += dim_total;
    }

    logger.info(&format!(
        "bootstrap: base map complete — {grand_total} chunks rendered from disk"
    ));
    ST_DONE
}

fn locate_world(cfg: &Config) -> Option<PathBuf> {
    if let Some(dir) = &cfg.world_dir {
        let p = PathBuf::from(dir);
        if p.join("db").is_dir() {
            return Some(p);
        }

        if p.ends_with("db") && p.is_dir() {
            return p.parent().map(|q| q.to_path_buf());
        }
        return None;
    }
    if let Ok(props) = std::fs::read_to_string("server.properties") {
        for line in props.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("level-name=") {
                let p = PathBuf::from("worlds").join(name.trim());
                if p.join("db").is_dir() {
                    return Some(p);
                }
            }
        }
    }

    let mut found: Option<PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir("worlds") {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.join("db").is_dir() {
                if found.is_some() {
                    return None;
                }
                found = Some(p);
            }
        }
    }
    found
}

fn read_spawn(world_dir: &Path) -> Option<[i32; 3]> {
    let bytes = std::fs::read(world_dir.join("level.dat")).ok()?;
    if bytes.len() <= 8 {
        return None;
    }
    let mut reader = crate::nbt::NbtReader::new(&bytes[8..]);
    let (_, root) = reader.read_root()?;
    let get = |k: &str| root.get(k).and_then(|t| t.as_int()).map(|v| v as i32);
    Some([get("SpawnX")?, get("SpawnY").unwrap_or(64), get("SpawnZ")?])
}
