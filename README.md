# MinkaMon

The Minka system monitor — eDEX-UI-inspired instrument panels in the
Eternal Darkness palette.

```sh
qs -p /path/to/MinkaMon
```

## Panels

- **CPU** — per-core 60 s line charts + total-load history
- **GPU** — Iris Xe per-engine 60 s line charts (fdinfo cycle counters)
  and frequency; NVIDIA dGPU via nvidia-smi, shown as DORMANT while
  runtime-suspended (the sampler never wakes a sleeping card)
- **MEMORY** — eDEX-style block grid over the real physical address
  space: zones from /proc/zoneinfo (DMA / DMA32 / Normal), per-zone
  used/cache cell counts from actual zone occupancy, stable scatter
  within each zone (exact page positions need root); swap history
- **THERMAL** — every hwmon temperature sensor, each with a 60 s line
  chart
- **NETWORK** — up/down rates with history, per-interface breakdown
- **WORLD VIEW** — rotating orthographic globe; live established TCP
  peers geolocated offline and pulsed on the map
- **PROCESSES** tab — sortable table (pid / name / state / cpu / rss)

## Architecture

`scripts/sampler.py` (stdlib-only Python) streams JSON lines once per
second; `services/Sampler.qml` mirrors the stream into reactive
properties the panels bind to. No polling from QML, one process, nothing
leaves the machine.

## Bundled data

- `assets/coastlines.json` — derived from Natural Earth 110m coastline
  (public domain)
- `assets/ip2country.csv.gz` — derived from the iptoasn.com table via
  sapics/ip-location-db (PDDL-1.0); country centroids in sampler.py

## Debug IPC

```sh
qs -p MinkaMon ipc call debug setPage processes
qs -p MinkaMon ipc call debug shot /tmp/minkamon.png
```
