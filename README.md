# Atlas-plus

A **live**, top-down world map for a Minecraft **Bedrock Dedicated Server**, built as a
[LeviLamina](https://github.com/LiteLDev/LeviLamina) Rust mod.

Atlas-plus renders your world two ways and fuses them:

- **Disk bootstrap** — on enable, it reads the world's LevelDB once (strictly read-only) and
  renders the whole explored world as the base map.
- **Live layer** — it then reads blocks straight out of the running server's memory and keeps
  the map current where things change, with live player markers, **without waiting for a save**.

The map is served by a tiny built-in web server. Nothing is fetched over plain HTTP: the browser
opens a single WebSocket and the server streams the map manifest, player positions, and tile
images down it as they change — so there's no polling and nothing for the browser to cache
stale. Tiles are still written to disk in the same format as the disk-based [Atlas], so anything
that reads Atlas tiles reads these too.

> **TPS-friendly by design.** The live layer never scans more than a fixed budget of chunks per
> cycle, and by default only touches the handful of chunks nearest each player plus whatever a
> world-change event flags. A still scene produces no work at all.

---

## Features

- 🗺️ **Full base map from disk** + **live updates** where players are.
- ⚡ **Bounded cost** — one hard "chunks per cycle" budget across every update mode.
- 🎛️ **Configurable update strategy**: nearest-to-player, world-change events, both, full
  radius sweep, or off.
- 🖼️ **Minecraft-styled canvas viewer** — a self-contained world-map page, pushed pixels only,
  no blank frames.
- 🧭 Pan / zoom / grid / dimension switch / spawn marker / live player markers.
- 🔌 **Everything live over one WebSocket** — the map manifest, player positions, *and* tile
  images are all pushed down `/ws` as they change (tiles as binary PNG frames). There is no
  HTTP polling and the browser never fetches a tile over HTTP, so there's nothing for it to
  cache stale. The socket auto-reconnects and a reconnecting client is replayed the full current
  state.
- 🛠️ **Operator command** — `/atlas url | status`.
- 📦 Zero external services — hand-rolled HTTP + WebSocket server, vanilla-JS viewer, no build
  step for the front end.

---

## Requirements

- A LeviLamina Bedrock server.
- The LeviLamina **Rust loader** mod (`levilamina-rust-loader`), which loads Rust `.dll` mods.
- To build from source: a Rust toolchain (stable) and network access for crates.

---

## Install

**Option A — prebuilt.** Drop `atlas_plus.dll` and `manifest.json` into a folder under your
server's `mods/` (or wherever your loader picks up Rust mods), alongside the Rust loader. Start
the server.

**Option B — build from source.**

```bash
git https://github.com/Maskviva/Atlas-plus
cd Atlas-plus
cargo build --release
# → target/release/atlas_plus.dll   (Windows / the platform your BDS runs on)
```

Ship `target/release/atlas_plus.dll` together with `manifest.json`. `manifest.json` declares a
dependency on `levilamina-rust-loader`, so the loader must be present.

On first run Atlas-plus writes a fully-populated `plugins/Atlas-plus/config.json`. Edit that and
restart the plugin (or the server) to apply changes.

---

## Quick start

1. Start the server with the mod installed.
2. Watch the console for `Atlas-plus live map: http://<host>:8899/`.
3. Open that URL. The page connects over WebSocket immediately; the base map streams in from
   disk in the background, and areas around players update live as you watch.
4. Check status in-game with `/atlas status`, or tune behaviour by editing `config.json` and
   restarting.

---

## The `/atlas` command

`atlas` is the Atlas-plus operator command (any player can run it; it only reports state).

| Command | Effect |
| --- | --- |
| `/atlas url` | Print the live map URL. |
| `/atlas status` | Players online, dimensions, update mode, render version, and the map URL. |

Config changes are picked up on the next plugin enable (reload the plugin or restart the
server) — there's no runtime config-editing command.

---

## Configuration

`plugins/Atlas-plus/config.json` — written with defaults on first run. Unknown keys are ignored.

### Web server

| Field | Default | Notes |
| --- | --- | --- |
| `http_bind` | `"0.0.0.0"` | Interface the web server (and WebSocket) binds to. |
| `http_port` | `8899` | Map is at `http://<host>:<port>/`; the live socket is at `ws://<host>:<port>/ws`. |
| `http_workers` | `3` | Tiny HTTP worker threads; 2–4 is plenty. Each open WebSocket runs on its own thread outside this pool. |
| `out_dir` | `"plugins/Atlas-plus/map"` | Where tiles / `map.json` / the viewer are written to disk (kept for Atlas-tile compatibility and as a fallback you can serve yourself; the bundled viewer doesn't read from here — it uses the WebSocket). |
| `world_name` | `"Atlas-plus (live)"` | Shown in the viewer. |

### What to render

| Field | Default | Notes |
| --- | --- | --- |
| `dimensions` | `[0]` | `0` overworld, `1` nether, `2` end. |
| `render_radius_chunks` | `6` | Live square around each player — **`update_mode: "full"` only**. |
| `pinned_areas` | `[]` | Always-refreshed areas, e.g. spawn: `[{ "dim":0, "x":0, "z":0, "radius_chunks":6 }]`. Keep them loaded with a `tickingarea`. |

### Disk bootstrap (once, on enable, in the background)

| Field | Default | Notes |
| --- | --- | --- |
| `bootstrap` | `true` | Read the on-disk world and render the explored map. **Read-only** — never writes to the database. |
| `world_dir` | `null` | World folder (contains `db/`, `level.dat`). `null` = auto-detect from `server.properties`. |
| `bootstrap_radius_chunks` | `null` | Limit the bootstrap to this chunk radius around spawn + pinned areas. `null` = the whole world. Set on huge worlds to bound memory while decoding. |
| `bootstrap_throttle_ms` | `10` | Pause between region batches so the bootstrap trickles in gently. |

### Live update strategy

| `update_mode` | Behaviour |
| --- | --- |
| `"nearby+events"` *(default)* | The **4 chunks nearest each player** are kept fresh, plus any chunk touched by a world-change event. |
| `"nearby"` | Only the 4 chunks nearest each player. |
| `"events"` | Only event-touched chunks (near-zero idle cost). |
| `"full"` | Continuously round-robin the whole `render_radius_chunks` square around every player (heaviest). |
| `"off"` | No live scanning (bootstrap + player markers only). |

**Events covered:** player block **place / destroy / interact**, and **fire spread** — the
world-change events the loader exposes. TNT and other explosions, mob griefing, and redstone
do **not** raise loader events; in practice those happen near players, where the `nearby` half
of the default mode picks up the result. In every mode, leftover budget slowly round-robins the
pinned areas.

### Scan pacing (server-thread cost)

| Field | Default | Notes |
| --- | --- | --- |
| `chunks_per_cycle` | `6` | **Hard ceiling** on chunk scans per cycle, every mode. The main TPS knob. |
| `cycle_interval_ms` | `250` | Delay between cycles. Raise if you see tick lag. |

### Scan depth / correctness

| Field | Default | Notes |
| --- | --- | --- |
| `slab_height` | `16` | Vertical slice per `scan_region` call. |
| `scan_top` / `scan_bottom` | `null` | Override the scanned Y range (all dims). Setting `scan_top` to your real build ceiling is the biggest single speed win — but anything above it won't appear. |
| `use_height_cache` | `true` | Remember each chunk's surface height so repeat scans start near the top. Leave on. |
| `rebuild_headroom` | `16` | Blocks to start above the cached top so small new builds show without a full rescan. |
| `full_rescan_every` | `16` | Force a full-range rescan every Nth scan of a chunk (catches very tall new builds). |

### Renderer

| Field | Default | Notes |
| --- | --- | --- |
| `flush_interval_ms` | `1000` | Minimum delay between tile flushes (debounce) — also how often changed tiles are pushed over the socket. Player markers update faster, independently. |
| `max_regions` | `48` | Cap on retained 512×512 region buffers (LRU-evicted). ~2 MB each; bounds memory. Evicted tiles are flushed to disk and the socket before eviction. |

---

## How it works

On enable, a background thread reads the on-disk LevelDB with a small pure-Rust, read-only
reader (SST tables + WAL replay, Mojang's raw-deflate), computes each explored chunk's surface,
and streams them to the renderer — region-grouped and throttled — as the base map.

Every LeviLamina callback runs on the server thread, so the live scan loop uses
`schedule_after` to pace itself there and calls `scan_region` directly (the only safe way to
read live blocks). Each cycle it decides what to rescan from `update_mode`, scans at most
`chunks_per_cycle` chunks, and hands surfaces to a **background renderer** thread. A chunk the
live scanner has already drawn is never overwritten by (older) bootstrap data. A chunk that
isn't loaded reads as air through the API; Atlas-plus detects an all-air full scan and leaves
the existing pixels untouched rather than erasing them.

The renderer owns the pixel buffers, and on a debounce it hillshades the regions that changed,
writes their PNG tiles to disk (for Atlas-tile compatibility), and — this is the part that keeps
the browser both flicker-free and cache-proof — **pushes exactly those changed tiles down the
WebSocket as binary frames** (a small header with dimension/zoom/tile coordinates, followed by
the raw PNG bytes). The updated map manifest and, whenever players move, their positions go down
the same socket as JSON text frames. There is no HTTP request the browser could serve from a
stale cache: the only way tile pixels reach the page is a fresh push over an open socket. A
freshly connected (or reconnected) browser is immediately replayed the current manifest, player
list, and every tile the server currently holds, so it catches up in one burst rather than
waiting for the next change.

**Tiles:** zoom 0 is one 512×512 image per 32×32-chunk region (tile coord == region coord,
1 px = 1 block); higher zooms are 2×2 downsamples. The on-disk copies under `out_dir` use the
same `tiles/<dimension>/<zoom>/<rx>_<rz>.png` layout as disk Atlas, for compatibility with other
tools — the bundled viewer itself never reads them back over HTTP.

---

## The viewer

The viewer is a self-contained, Minecraft-styled world-map page served at your `out_dir` root
(`index.html` + `map-viewer.js`). Open `http://<host>:<port>/` in any browser — no build step, no
external assets. On load it opens `ws://<host>:<port>/ws` and receives the manifest, player
positions, and every tile image over that one connection; it decodes tile frames with
`createImageBitmap` and draws straight to a `<canvas>`. If the socket drops, it reconnects
automatically (with a short backoff) and is replayed the full current state. It can also be
embedded in another page via an `<iframe>` pointing at the served `index.html`.

---

## Performance tuning

- **Server feels laggy?** Lower `chunks_per_cycle` and/or raise `cycle_interval_ms` (the ceiling
  applies in every mode). Set `scan_top` to your real build ceiling; keep `use_height_cache`
  on. The default `update_mode` is already light — `"events"` is near-free when idle, `"off"`
  stops live scans entirely.
- **Changes appear too slowly?** Raise `chunks_per_cycle` and/or lower `cycle_interval_ms`. In
  `"full"` mode, also lower `render_radius_chunks`. You can also lower `flush_interval_ms` so
  changed tiles are pushed to the browser sooner.
- **Big world, bootstrap eats RAM?** Set `bootstrap_radius_chunks` (e.g. `128`).
- **Tall builds occasionally missing near the top?** Lower `full_rescan_every` or raise
  `rebuild_headroom` (or set an explicit `scan_top`).

---

## Notes & limitations

- Written for the loader's safe `levilamina-rust-loader` API (v1.0.0 / ABI v4). On a different loader
  version, align the versions if it won't build or load.
- `panic = "abort"` is intentionally **not** set — the loader wraps mod entry points in
  `catch_unwind`, and aborting would take the whole server down on a panic.
- The disk bootstrap reads a database the server has open. That's safe for the database (reads
  only), but a save landing mid-read can make the load fail; that's logged and the map fills in
  live instead.
- The WebSocket handshake and framing are a small hand-rolled RFC 6455 implementation (the
  loader has no WebSocket support to build on) — no external crate, no TLS. Put it behind a
  reverse proxy if you need `wss://`.
- Not affiliated with Mojang or Microsoft.

---

## Building

```bash
cargo build --release
```

Dependencies: `levilamina-rust-loader` (the Rust loader crate, via git), `image` (PNG only), `serde` /
`serde_json`, and `flate2` (for the LevelDB reader). No `panic = "abort"`.

---

## License

Choose a license before publishing (MIT or Apache-2.0 are common for this ecosystem) and add a
`LICENSE` file. Update this section to match.

[Atlas]: https://www.minebbs.com/resources/atlasmap.17092/
