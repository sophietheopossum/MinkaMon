# MinkaMon

The Minka system monitor — eDEX-UI-inspired instrument panels in the
Eternal Darkness palette.

MinkaMon and its sampler are libraries in MinkaDE's one binary, `minka`
(the minkade package installs it). MinkaMon starts the sampler from that same
binary, so the two are always one build.

```sh
# from the MinkaDE checkout
cargo build --profile release-fast -p minka
target/release-fast/minka minkamon
```

The Quickshell version it replaced is gone (24/11/2026); the git history
has it.

## Panels

- **CPU** — two stacked 60 s multi-line charts with every core in each:
  load on top, coretemp below (logical cores mapped to their physical
  sensor via sysfs topology, so hyperthread siblings overlap)
- **GPU** — Iris Xe per-engine 60 s line charts (fdinfo cycle counters)
  and frequency; NVIDIA dGPU via nvidia-smi, shown as DORMANT while
  runtime-suspended (the sampler never wakes a sleeping card)
- **MEMORY** — eDEX-style block grid over the real physical address
  space: zones from /proc/zoneinfo (DMA / DMA32 / Normal), per-zone
  used/cache cell counts from actual zone occupancy, stable scatter
  within each zone (exact page positions need root); swap history
- **THERMAL** — per-component temperature satellites (board / CPU / GPU /
  SSD / Wi-Fi), each with a 60 s line chart
- **NETWORK** — up/down rates with history, per-interface breakdown
- **WORLD VIEW** — rotating orthographic globe; live established TCP
  peers geolocated offline and pulsed on the map
- **PROCESSES** tab — sortable table (pid / name / state / cpu / rss)

## Architecture

`sampler-rs` (Rust) streams JSON lines adaptively: 1s while MinkaMon is
hovered or a satellite is open, 5s while it is only a backdrop. The
application under `src/` consumes that stream and its demand protocol,
hosts the schematic either as a floating window or as the ScreenPad layer
surface in Duo layout, and uses the shared `MinkaIPC` crate for ShojiWM
leader lines, overview arrangement, and raising a duplicate launch.
Process and connection slices are
demand-gated by satellite visibility, geo data loads on first Globe use,
NVIDIA stats come from NVML, and the sampler runs at idle CPU/IO
priority. One sampler process, nothing leaves the machine.
The previous Python sampler, `scripts/sampler.py`, was removed on
20/11/2026; git history has it.

## Bundled data

- `assets/coastlines.json` — derived from Natural Earth 110m coastline
  (public domain)
- `assets/ip2country.csv.gz` — derived from the iptoasn.com table via
  sapics/ip-location-db (PDDL-1.0); country centroids in
  `sampler-rs/src/centroids.rs`
