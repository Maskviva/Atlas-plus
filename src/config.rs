use std::path::Path;

use serde::{Deserialize, Serialize};

use levilamina::Logger;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedArea {
    pub dim: i32,
    pub x: i32,
    pub z: i32,
    #[serde(default = "default_pin_radius")]
    pub radius_chunks: i32,
}

fn default_pin_radius() -> i32 {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {

    pub http_bind: String,

    pub http_port: u16,

    pub http_workers: usize,

    pub live_poll_ms: u64,

    pub out_dir: String,

    pub world_name: String,

    pub dimensions: Vec<i32>,

    pub render_radius_chunks: i32,

    pub pinned_areas: Vec<PinnedArea>,

    pub bootstrap: bool,

    pub world_dir: Option<String>,

    pub bootstrap_radius_chunks: Option<i32>,

    pub bootstrap_throttle_ms: u64,

    pub update_mode: String,

    pub chunks_per_cycle: usize,

    pub cycle_interval_ms: u64,

    pub slab_height: i32,

    pub scan_top: Option<i32>,

    pub scan_bottom: Option<i32>,

    pub use_height_cache: bool,

    pub rebuild_headroom: i32,

    pub full_rescan_every: u32,

    pub flush_interval_ms: u64,

    pub max_regions: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_bind: "0.0.0.0".into(),
            http_port: 8899,
            http_workers: 3,
            live_poll_ms: 1000,
            out_dir: "plugins/Atlas-plus/map".into(),
            world_name: "Atlas-plus (live)".into(),
            dimensions: vec![0],
            render_radius_chunks: 6,
            pinned_areas: Vec::new(),
            bootstrap: true,
            world_dir: None,
            bootstrap_radius_chunks: None,
            bootstrap_throttle_ms: 10,
            update_mode: "nearby+events".into(),
            chunks_per_cycle: 6,
            cycle_interval_ms: 250,
            slab_height: 16,
            scan_top: None,
            scan_bottom: None,
            use_height_cache: true,
            rebuild_headroom: 16,
            full_rescan_every: 16,
            flush_interval_ms: 1000,
            max_regions: 48,
        }
    }
}

impl Config {

    pub fn dim_y_range(&self, dim: i32) -> (i32, i32) {
        let (mut bottom, mut top) = match dim {
            1 => (0, 127),
            2 => (0, 255),
            _ => (-64, 319),
        };
        if let Some(v) = self.scan_bottom {
            bottom = v;
        }
        if let Some(v) = self.scan_top {
            top = v;
        }
        if top < bottom {
            std::mem::swap(&mut bottom, &mut top);
        }
        (bottom, top)
    }

    pub fn sanitized(mut self) -> Self {
        self.http_workers = self.http_workers.clamp(1, 32);
        self.live_poll_ms = self.live_poll_ms.clamp(200, 60_000);
        self.chunks_per_cycle = self.chunks_per_cycle.max(1);
        self.cycle_interval_ms = self.cycle_interval_ms.clamp(20, 60_000);
        self.slab_height = self.slab_height.clamp(1, 384);
        self.render_radius_chunks = self.render_radius_chunks.clamp(0, 64);
        self.rebuild_headroom = self.rebuild_headroom.clamp(0, 384);
        self.full_rescan_every = self.full_rescan_every.max(1);
        self.flush_interval_ms = self.flush_interval_ms.clamp(100, 60_000);
        self.max_regions = self.max_regions.clamp(1, 4096);
        self.bootstrap_throttle_ms = self.bootstrap_throttle_ms.min(2_000);
        if let Some(r) = self.bootstrap_radius_chunks {
            self.bootstrap_radius_chunks = Some(r.clamp(0, 100_000));
        }
        self.update_mode = self.update_mode.trim().to_ascii_lowercase();
        if self.dimensions.is_empty() {
            self.dimensions = vec![0];
        }
        self
    }

    pub fn mode(&self) -> UpdateMode {
        let s = self.update_mode.as_str();
        let nearby = s.contains("nearby") || s.contains("near");
        let events = s.contains("event");
        match (nearby, events) {
            (true, true) => UpdateMode::NearbyEvents,
            (true, false) => UpdateMode::Nearby,
            (false, true) => UpdateMode::Events,
            (false, false) => match s {
                "full" | "all" | "radius" => UpdateMode::Full,
                "off" | "none" | "disabled" => UpdateMode::Off,
                _ => UpdateMode::NearbyEvents,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    Full,
    Nearby,
    Events,
    NearbyEvents,
    Off,
}

impl UpdateMode {
    pub fn uses_events(self) -> bool {
        matches!(self, UpdateMode::Events | UpdateMode::NearbyEvents)
    }
    pub fn uses_nearby(self) -> bool {
        matches!(self, UpdateMode::Nearby | UpdateMode::NearbyEvents)
    }
    pub fn label(self) -> &'static str {
        match self {
            UpdateMode::Full => "full",
            UpdateMode::Nearby => "nearby",
            UpdateMode::Events => "events",
            UpdateMode::NearbyEvents => "nearby+events",
            UpdateMode::Off => "off",
        }
    }
}

pub fn load(path: &Path, logger: &Logger) -> Config {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Config>(&text) {
            Ok(cfg) => {
                logger.info(&format!("config loaded from {}", path.display()));
                cfg.sanitized()
            }
            Err(e) => {
                logger.warn(&format!(
                    "config at {} could not be parsed ({e}); using defaults",
                    path.display()
                ));
                Config::default().sanitized()
            }
        },
        Err(_) => {
            let cfg = Config::default();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match serde_json::to_string_pretty(&cfg) {
                Ok(json) => {
                    if std::fs::write(path, json).is_ok() {
                        logger.info(&format!(
                            "no config found — wrote defaults to {}",
                            path.display()
                        ));
                    }
                }
                Err(_) => {}
            }
            cfg.sanitized()
        }
    }
}
