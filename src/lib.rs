mod bootstrap;
mod colors;
mod config;
mod http;
#[allow(dead_code)]
mod leveldb;
#[allow(dead_code)]
mod nbt;
mod render;
mod scan;
mod util;
mod viewer;
mod ws;
#[allow(dead_code)]
mod world;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use levilamina::event::names as ev;
use levilamina::prelude::*;
use serde_json::{json, Value};

use crate::colors::ColorMap;
use crate::config::{Config, UpdateMode};
use crate::render::ToRenderer;
use crate::scan::Hint;

static RUNNING: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static CURSOR: AtomicUsize = AtomicUsize::new(0);
static PIN_CURSOR: AtomicUsize = AtomicUsize::new(0);

fn tx_cell() -> &'static Mutex<Option<Sender<ToRenderer>>> {
    static S: OnceLock<Mutex<Option<Sender<ToRenderer>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}
fn cfg_cell() -> &'static Mutex<Option<Arc<Config>>> {
    static S: OnceLock<Mutex<Option<Arc<Config>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}
fn hints_cell() -> &'static Mutex<HashMap<(i32, i32, i32), Hint>> {
    static S: OnceLock<Mutex<HashMap<(i32, i32, i32), Hint>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

struct DirtyQueue {
    q: VecDeque<(i32, i32, i32)>,
    set: HashSet<(i32, i32, i32)>,
}
fn dirty_cell() -> &'static Mutex<DirtyQueue> {
    static S: OnceLock<Mutex<DirtyQueue>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(DirtyQueue {
            q: VecDeque::new(),
            set: HashSet::new(),
        })
    })
}
fn dirty_push(dim: i32, cx: i32, cz: i32) {
    if let Ok(mut d) = dirty_cell().lock() {
        let k = (dim, cx, cz);
        if d.set.insert(k) {
            d.q.push_back(k);
            if d.q.len() > 4096 {
                if let Some(old) = d.q.pop_front() {
                    d.set.remove(&old);
                }
            }
        }
    }
}
fn dirty_pop() -> Option<(i32, i32, i32)> {
    let mut d = dirty_cell().lock().ok()?;
    let k = d.q.pop_front()?;
    d.set.remove(&k);
    Some(k)
}
fn dirty_clear() {
    if let Ok(mut d) = dirty_cell().lock() {
        d.q.clear();
        d.set.clear();
    }
}

fn scan_cycle(generation: u64) {
    if !RUNNING.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != generation {
        return;
    }
    let cfg = match cfg_cell().lock().ok().and_then(|g| g.clone()) {
        Some(c) => c,
        None => return,
    };
    let server = Server::get();

    let players = server.list_players();
    send_players(&players);

    let mode = cfg.mode();
    if mode != UpdateMode::Off {
        let budget = cfg.chunks_per_cycle;
        let mut scanned = 0usize;
        let tx = tx_cell().lock().ok().and_then(|g| g.clone());
        let mut hints_guard = hints_cell().lock().unwrap();
        let hints = &mut *hints_guard;

        if mode == UpdateMode::Full {
            let mut live: Vec<(i32, i32, i32)> = Vec::new();
            let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
            let r = cfg.render_radius_chunks;
            for p in &players {
                if !cfg.dimensions.contains(&p.dimension) {
                    continue;
                }
                let ccx = (p.pos.0.floor() as i32) >> 4;
                let ccz = (p.pos.2.floor() as i32) >> 4;
                for dz in -r..=r {
                    for dx in -r..=r {
                        let key = (p.dimension, ccx + dx, ccz + dz);
                        if seen.insert(key) {
                            live.push(key);
                        }
                    }
                }
            }
            collect_pins(&cfg, &mut live, &mut seen);
            if !live.is_empty() {
                live.sort_unstable();
                let take = budget.min(live.len());
                let start = CURSOR.load(Ordering::Relaxed) % live.len();
                for i in 0..take {
                    let (dim, cx, cz) = live[(start + i) % live.len()];
                    sweep_one(&server, &cfg, hints, &tx, dim, cx, cz);
                    scanned += 1;
                }
                CURSOR.store((start + take) % live.len(), Ordering::Relaxed);
            }
            let _ = scanned;
        } else {
            if mode.uses_nearby() {
                let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
                'nearby: for p in &players {
                    if !cfg.dimensions.contains(&p.dimension) {
                        continue;
                    }
                    for key in nearest_four(p.dimension, p.pos.0, p.pos.2) {
                        if !seen.insert(key) {
                            continue;
                        }
                        if scanned >= budget {
                            break 'nearby;
                        }
                        sweep_one(&server, &cfg, hints, &tx, key.0, key.1, key.2);
                        scanned += 1;
                    }
                }
            }
            if mode.uses_events() {
                while scanned < budget {
                    let Some((dim, cx, cz)) = dirty_pop() else { break };
                    sweep_one(&server, &cfg, hints, &tx, dim, cx, cz);
                    scanned += 1;
                }
            }
            if scanned < budget && !cfg.pinned_areas.is_empty() {
                let mut pinned: Vec<(i32, i32, i32)> = Vec::new();
                let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
                collect_pins(&cfg, &mut pinned, &mut seen);
                if !pinned.is_empty() {
                    pinned.sort_unstable();
                    let take = (budget - scanned).min(pinned.len());
                    let start = PIN_CURSOR.load(Ordering::Relaxed) % pinned.len();
                    for i in 0..take {
                        let (dim, cx, cz) = pinned[(start + i) % pinned.len()];
                        sweep_one(&server, &cfg, hints, &tx, dim, cx, cz);
                    }
                    PIN_CURSOR.store((start + take) % pinned.len(), Ordering::Relaxed);
                }
            }
        }
    }

    if RUNNING.load(Ordering::Acquire) && GENERATION.load(Ordering::Acquire) == generation {
        server.schedule_after(
            Duration::from_millis(cfg.cycle_interval_ms),
            move || scan_cycle(generation),
        );
    }
}

fn collect_pins(
    cfg: &Config,
    out: &mut Vec<(i32, i32, i32)>,
    seen: &mut HashSet<(i32, i32, i32)>,
) {
    for pa in &cfg.pinned_areas {
        if !cfg.dimensions.contains(&pa.dim) {
            continue;
        }
        let (ccx, ccz) = (pa.x >> 4, pa.z >> 4);
        let pr = pa.radius_chunks.max(0);
        for dz in -pr..=pr {
            for dx in -pr..=pr {
                let key = (pa.dim, ccx + dx, ccz + dz);
                if seen.insert(key) {
                    out.push(key);
                }
            }
        }
    }
}

fn send_players(players: &[PlayerInfo]) {
    let arr: Vec<Value> = players
        .iter()
        .map(|p| {
            json!({
                "name": p.name.clone(),
                "x": p.pos.0,
                "y": p.pos.1,
                "z": p.pos.2,
                "dim": p.dimension,
            })
        })
        .collect();
    if let Some(tx) = tx_cell().lock().ok().and_then(|g| g.clone()) {
        let _ = tx.send(ToRenderer::Players(Value::Array(arr)));
    }
}

fn sweep_one(
    server: &Server,
    cfg: &Config,
    hints: &mut HashMap<(i32, i32, i32), Hint>,
    tx: &Option<Sender<ToRenderer>>,
    dim: i32,
    cx: i32,
    cz: i32,
) {
    if let Some(surface) = scan::scan_one(server, cfg, hints, dim, cx, cz) {
        if let Some(tx) = tx {
            let _ = tx.send(ToRenderer::Chunk {
                dim,
                cx,
                cz,
                surface: Box::new(surface),
                bootstrap: false,
            });
        }
    }
}

fn nearest_four(dim: i32, x: f64, z: f64) -> [(i32, i32, i32); 4] {
    let bx = x.floor() as i32;
    let bz = z.floor() as i32;
    let ccx = bx >> 4;
    let ccz = bz >> 4;
    let dx = if bx - (ccx << 4) >= 8 { 1 } else { -1 };
    let dz = if bz - (ccz << 4) >= 8 { 1 } else { -1 };
    [
        (dim, ccx, ccz),
        (dim, ccx + dx, ccz),
        (dim, ccx, ccz + dz),
        (dim, ccx + dx, ccz + dz),
    ]
}

fn subscribe_events(server: &Server, cfg: &Arc<Config>, logger: Logger) -> Vec<Listener> {
    let mut listeners = Vec::new();
    for id in [
        ev::PLAYER_DESTROY_BLOCK,
        ev::PLAYER_PLACED_BLOCK,
        ev::PLAYER_INTERACT_BLOCK,
        ev::FIRE_SPREAD,
    ] {
        let dims = cfg.dimensions.clone();
        match server.subscribe_event(id, EventPriority::Normal, move |e| on_world_event(e, &dims)) {
            Ok(l) => listeners.push(l),
            Err(e) => logger.warn(&format!("events: couldn't subscribe {id}: {e}")),
        }
    }
    logger.info(&format!(
        "events: watching {} world-change event(s)",
        listeners.len()
    ));
    listeners
}

fn on_world_event(e: &mut EventRef, dims: &[i32]) {
    if !RUNNING.load(Ordering::Acquire) {
        return;
    }
    let Some((x, _y, z)) = event_pos(e) else { return };
    let (cx, cz) = (x >> 4, z >> 4);

    if dims.len() == 1 {
        dirty_push(dims[0], cx, cz);
        return;
    }

    let pdim = e.player().and_then(|ident| {
        Server::get()
            .list_players()
            .into_iter()
            .find(|p| (!ident.xuid.is_empty() && p.xuid == ident.xuid) || p.name == ident.name)
            .map(|p| p.dimension)
    });
    match pdim {
        Some(d) => {
            if dims.contains(&d) {
                dirty_push(d, cx, cz);
            }
        }
        None => {
            for &d in dims {
                dirty_push(d, cx, cz);
            }
        }
    }
}

fn event_pos(e: &EventRef) -> Option<(i32, i32, i32)> {
    let v = e.value().ok()?;
    for key in ["pos", "blockPos", "position"] {
        let Some(p) = v.get(key) else { continue };
        if let Some(list) = p.as_list() {
            if list.len() >= 3 {
                if let (Some(x), Some(y), Some(z)) =
                    (list[0].as_i64(), list[1].as_i64(), list[2].as_i64())
                {
                    return Some((x as i32, y as i32, z as i32));
                }
            }
        }
        if let NbtValue::IntArray(a) = p {
            if a.len() >= 3 {
                return Some((a[0], a[1], a[2]));
            }
        }
        if let (Some(x), Some(y), Some(z)) = (
            p.get("x").and_then(|t| t.as_i64()),
            p.get("y").and_then(|t| t.as_i64()),
            p.get("z").and_then(|t| t.as_i64()),
        ) {
            return Some((x as i32, y as i32, z as i32));
        }
    }
    None
}

struct App {
    http: Option<http::HttpServer>,
    renderer: Option<JoinHandle<()>>,
    bootstrap: Option<JoinHandle<()>>,
    boot_cancel: Arc<AtomicBool>,
    listeners: Vec<Listener>,
}

impl App {
    fn stop(mut self) {
        RUNNING.store(false, Ordering::Release);
        GENERATION.fetch_add(1, Ordering::AcqRel);
        self.boot_cancel.store(true, Ordering::Release);

        let listeners = std::mem::take(&mut self.listeners);
        drop(listeners);

        if let Some(h) = self.bootstrap.take() {
            let _ = h.join();
        }
        if let Some(tx) = tx_cell().lock().unwrap().take() {
            let _ = tx.send(ToRenderer::Shutdown);
        }
        if let Some(h) = self.renderer.take() {
            let _ = h.join();
        }
        if let Some(server) = self.http.take() {
            server.shutdown();
        }

        *cfg_cell().lock().unwrap() = None;
        hints_cell().lock().unwrap().clear();
        dirty_clear();
        CURSOR.store(0, Ordering::Relaxed);
        PIN_CURSOR.store(0, Ordering::Relaxed);
    }
}

struct Started {
    app: App,
    cfg: Arc<Config>,
    version: Arc<AtomicU64>,
    url: Option<String>,
}

fn start(logger: Logger) -> Started {
    let cfg_path = PathBuf::from("plugins/Atlas-plus/config.json");
    let cfg = Arc::new(config::load(&cfg_path, &logger));

    let out_dir = PathBuf::from(&cfg.out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        logger.warn(&format!(
            "could not create output dir {}: {e}",
            out_dir.display()
        ));
    }
    if let Err(e) = viewer::write_assets(&out_dir) {
        logger.warn(&format!("could not write viewer assets: {e}"));
    }

    let version = Arc::new(AtomicU64::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<ToRenderer>();
    let renderer = {
        let cfg = cfg.clone();
        let version = version.clone();
        std::thread::Builder::new()
            .name("atlas-plus-render".into())
            .spawn(move || render::run(rx, cfg, ColorMap::builtin(), version))
            .ok()
    };

    let (http, url) = match http::serve(
        &cfg.http_bind,
        cfg.http_port,
        out_dir.clone(),
        cfg.http_workers,
        logger,
    ) {
        Ok(server) => {
            let host = display_host(&cfg.http_bind);
            let url = format!("http://{host}:{}/", cfg.http_port);
            (Some(server), Some(url))
        }
        Err(e) => {
            logger.error(&format!(
                "map web server could not bind {}:{} ({e}). Tiles are still \
                 written to '{}'; serve that folder yourself or free the port.",
                cfg.http_bind,
                cfg.http_port,
                out_dir.display()
            ));
            (None, None)
        }
    };

    *tx_cell().lock().unwrap() = Some(tx.clone());
    *cfg_cell().lock().unwrap() = Some(cfg.clone());
    hints_cell().lock().unwrap().clear();
    dirty_clear();
    CURSOR.store(0, Ordering::Relaxed);
    PIN_CURSOR.store(0, Ordering::Relaxed);
    RUNNING.store(true, Ordering::Release);

    if let Some(xyz) = bootstrap::world_spawn(&cfg) {
        let _ = tx.send(ToRenderer::Spawn(xyz));
    }

    let listeners = if cfg.mode().uses_events() {
        subscribe_events(&Server::get(), &cfg, logger)
    } else {
        Vec::new()
    };

    let boot_cancel = Arc::new(AtomicBool::new(false));
    let bootstrap = bootstrap::spawn(cfg.clone(), tx, logger, boot_cancel.clone());

    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    Server::get().schedule(move || scan_cycle(generation));

    if let Some(u) = &url {
        logger.info(&format!("Atlas-plus live map: {u}"));
    }

    Started {
        app: App {
            http,
            renderer,
            bootstrap,
            boot_cancel,
            listeners,
        },
        cfg,
        version,
        url,
    }
}

fn display_host(bind: &str) -> String {
    match bind {
        "0.0.0.0" | "" => "localhost".to_string(),
        "::" => "[::1]".to_string(),
        other => other.to_string(),
    }
}

fn handle_command(
    inv: &CommandInvocation,
    cfg: &Arc<Config>,
    version: &Arc<AtomicU64>,
    url: Option<&str>,
) {
    let sub = inv.args.split_whitespace().next().unwrap_or("").to_lowercase();
    match sub.as_str() {
        "url" | "open" | "" => match url {
            Some(u) => inv.success(&format!("Atlas-plus live map: {u}")),
            None => inv.error(
                "web server isn't running (port in use?). Tiles are being written to disk \u{2014} \
                 check the server log and config.json.",
            ),
        },
        "status" => {
            let server = Server::get();
            let players = server.list_players().len();
            let dims: Vec<&str> = cfg.dimensions.iter().map(|d| render::dim_id(*d)).collect();
            let v = version.load(Ordering::Relaxed);
            inv.success(&format!(
                "Atlas-plus: {} online \u{00b7} dimensions [{}] \u{00b7} render v{} \u{00b7} mode {} \u{00b7} {} chunks/cycle",
                players,
                dims.join(", "),
                v,
                cfg.mode().label(),
                cfg.chunks_per_cycle,
            ));
            match url {
                Some(u) => inv.success(&format!("map: {u}")),
                None => inv.error("web server not running"),
            }
        }
        _ => {
            inv.success("usage: /atlas url | status");
        }
    }
}

struct AtlasPlus {
    app: Option<App>,
}

impl LeviMod for AtlasPlus {
    fn on_load(ctx: &ModContext) -> Result<Self> {
        ctx.logger().info("Atlas-plus loaded");
        Ok(AtlasPlus { app: None })
    }

    fn on_enable(&mut self, ctx: &ModContext) -> Result<()> {
        let logger = ctx.logger();
        let started = start(logger);

        let cfg = started.cfg.clone();
        let version = started.version.clone();
        let url = started.url.clone();
        if let Err(e) = ctx.server().register_command(
            "atlas",
            "Atlas-plus live map: /atlas url | status",
            CommandPermission::Any,
            move |inv| handle_command(inv, &cfg, &version, url.as_deref()),
        ) {
            started.app.stop();
            return Err(e);
        }

        self.app = Some(started.app);
        logger.info("Atlas-plus enabled \u{2014} try /atlas url");
        Ok(())
    }

    fn on_disable(&mut self, ctx: &ModContext) -> Result<()> {
        if let Some(app) = self.app.take() {
            app.stop();
        }
        ctx.logger().info("Atlas-plus disabled");
        Ok(())
    }

    fn on_unload(&mut self, _ctx: &ModContext) -> Result<()> {
        if let Some(app) = self.app.take() {
            app.stop();
        }
        Ok(())
    }
}

register_mod!(AtlasPlus);
