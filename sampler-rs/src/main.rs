//! MinkaMon system sampler (Rust port of scripts/sampler.py).
//!
//! Long-running process: emits one JSON object per line on stdout.
//!
//!   every tick:       cpu, mem, gpu, temps, net, disk
//!   every 2 ticks:    procs (full process table), while demanded
//!   every 5 ticks:    conns (established TCP, geolocated offline), while demanded
//!   every 10 ticks:   nvidia (while the card is runtime-active)
//!
//! The tick is adaptive: 1s while the shell is interactive (hovered or a
//! satellite is open), 5s while it is only a backdrop. Demand arrives as
//! JSON lines on stdin, e.g. {"wantProcs": true, "wantConns": false,
//! "tickMs": 1000}. The QML shell sends it when satellites open/close or the
//! pointer enters/leaves MinkaMon, so expensive collectors stay idle while
//! nobody is looking at them. Geo data for conns loads lazily on first use.
//!
//! GPU notes for this machine (and portability):
//!   - Intel xe: busyness from /proc/*/fdinfo drm-cycles-*/drm-total-cycles-*
//!     deltas summed across DRM clients (the nvtop method); frequency from
//!     /sys/class/drm/cardN/device/tile0/gt0/freq0/act_freq.
//!   - NVIDIA proprietary: NVML — but ONLY while the card's PCI
//!     runtime_status is "active". Polling the card wakes a runtime-suspended
//!     Optimus dGPU and keeps it awake, wrecking battery; a suspended card is
//!     reported as {"asleep": true} instead. `nvidia-smi` remains only as a
//!     fallback if NVML cannot be loaded.
//!
//! IP geolocation is fully offline: bundled iptoasn ipv4 table (PDDL-1.0)
//! mapped to country centroids. Nothing leaves the machine.

mod centroids;

use flate2::read::GzDecoder;
use libloading::Library;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufRead, BufWriter, Read, Write};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const TICK_SECONDS: f64 = 1.0;
const PROCS_EVERY: u64 = 2;
const CONNS_EVERY: u64 = 5;
const NVIDIA_EVERY: u64 = 10;
const XE_RESCAN_SECONDS: f64 = 60.0;
const TEMP_RESCAN_SECONDS: f64 = 60.0;
const BLOCK_RESCAN_SECONDS: f64 = 30.0;

extern "C" {
    fn sysconf(name: c_int) -> c_long;
}

const SC_CLK_TCK: c_int = 2;
const SC_PAGESIZE: c_int = 30;

fn sysconf_or(name: c_int, fallback: u64) -> u64 {
    let value = unsafe { sysconf(name) };
    if value > 0 {
        value as u64
    } else {
        fallback
    }
}

fn page_kb() -> u64 {
    sysconf_or(SC_PAGESIZE, 4096) / 1024
}

fn clock_ticks() -> u64 {
    sysconf_or(SC_CLK_TCK, 100)
}

fn read_bytes(path: impl AsRef<Path>) -> Option<Vec<u8>> {
    // Most procfs/sysfs inputs report a zero length but fit in one page-ish
    // read. Preallocating avoids fs::read's tiny probe/growth reads turning
    // dozens of fdinfo files into hundreds of read syscalls per tick.
    let mut file = File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(8192);
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn read_string(path: impl AsRef<Path>) -> Option<String> {
    read_bytes(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    read_string(path).map(|s| s.trim().to_owned())
}

fn read_dir_sorted(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = match fs::read_dir(path) {
        Ok(read_dir) => read_dir
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort();
    entries
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn opt_f64(value: Option<f64>) -> Value {
    match value {
        Some(v) => json!(v),
        None => Value::Null,
    }
}

fn opt_u64(value: Option<u64>) -> Value {
    match value {
        Some(v) => json!(v),
        None => Value::Null,
    }
}

#[derive(Clone, Copy)]
struct Demand {
    procs: bool,
    conns: bool,
    tick_ms: u64,
}

impl Default for Demand {
    fn default() -> Self {
        Self {
            procs: false,
            conns: false,
            tick_ms: FAST_TICK_MS,
        }
    }
}

impl Demand {
    fn interval(&self) -> Duration {
        Duration::from_millis(self.tick_ms.clamp(MIN_TICK_MS, MAX_TICK_MS))
    }
}

#[derive(Clone, Copy)]
struct DemandUpdate {
    procs: Option<bool>,
    conns: Option<bool>,
    tick_ms: Option<u64>,
}

fn spawn_demand_reader() -> Receiver<DemandUpdate> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let Some(obj) = msg.as_object() else { continue };
            let get = |keys: &[&str]| {
                keys.iter()
                    .find_map(|key| obj.get(*key).and_then(Value::as_bool))
            };
            let tick_ms = obj
                .get("tickMs")
                .or_else(|| obj.get("tick_ms"))
                .or_else(|| obj.get("intervalMs"))
                .and_then(Value::as_u64)
                .map(|v| v.clamp(MIN_TICK_MS, MAX_TICK_MS))
                .or_else(|| {
                    obj.get("eco").and_then(Value::as_bool).map(|eco| {
                        if eco {
                            ECO_TICK_MS
                        } else {
                            FAST_TICK_MS
                        }
                    })
                });
            let update = DemandUpdate {
                procs: get(&["wantProcs", "procs"]),
                conns: get(&["wantConns", "conns"]),
                tick_ms,
            };
            if (update.procs.is_some() || update.conns.is_some() || update.tick_ms.is_some())
                && tx.send(update).is_err()
            {
                break;
            }
        }
    });
    rx
}

fn drain_demand(rx: &Receiver<DemandUpdate>, demand: &mut Demand) {
    while let Ok(update) = rx.try_recv() {
        if let Some(v) = update.procs {
            demand.procs = v;
        }
        if let Some(v) = update.conns {
            demand.conns = v;
        }
        if let Some(v) = update.tick_ms {
            demand.tick_ms = v.clamp(MIN_TICK_MS, MAX_TICK_MS);
        }
    }
}

fn wait_for_tick(rx: &Receiver<DemandUpdate>, demand: &mut Demand, prev_time: Instant) {
    // Sleep in short slices so a mouse-over can pull an eco tick back to
    // the fast cadence within ~100ms instead of after the full 5s.
    loop {
        drain_demand(rx, demand);
        let deadline = prev_time + demand.interval();
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        thread::sleep((deadline - now).min(Duration::from_millis(100)));
    }
}

fn assets_dir() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    if let Ok(dir) = std::env::var("MINKAMON_ASSETS") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..6 {
            let Some(current) = dir else { break };
            let candidate = current.join("assets");
            if candidate.is_dir() {
                return candidate;
            }
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    PathBuf::from("assets")
}

#[derive(Default)]
struct IpTable {
    starts: Vec<u32>,
    // Two-byte ISO codes, [0, 0] for unassigned/private table sentinels.
    // Interning avoids ~300k tiny String allocations for mostly "-" rows.
    countries: Vec<[u8; 2]>,
}

impl IpTable {
    fn load(assets: &Path) -> Self {
        let path = assets.join("ip2country.csv.gz");
        let Ok(file) = File::open(&path) else {
            return Self::default();
        };
        let mut decoder = GzDecoder::new(file);
        let mut text = String::new();
        if decoder.read_to_string(&mut text).is_err() {
            return Self::default();
        }
        let mut table = Self::default();
        for line in text.lines() {
            let Some((start, country)) = line.split_once(',') else {
                continue;
            };
            let Ok(start) = start.trim().parse::<u32>() else {
                continue;
            };
            table.starts.push(start);
            let country = country.trim().as_bytes();
            let code = if country.len() == 2 && country.iter().all(|b| b.is_ascii_alphabetic()) {
                [country[0], country[1]]
            } else {
                [0, 0]
            };
            table.countries.push(code);
        }
        table
    }

    fn country_code_for_ip(&self, ip: Ipv4Addr) -> Option<[u8; 2]> {
        if self.starts.is_empty() {
            return None;
        }
        let n = u32::from(ip);
        let idx = self.starts.partition_point(|start| *start <= n);
        if idx == 0 {
            return None;
        }
        let code = *self.countries.get(idx - 1)?;
        if code == [0, 0] {
            None
        } else {
            Some(code)
        }
    }
}

// --- CPU ---

type CpuTimes = BTreeMap<String, (u64, u64)>;

fn read_cpu_times() -> CpuTimes {
    let mut out = BTreeMap::new();
    let Some(text) = read_string("/proc/stat") else {
        return out;
    };
    for line in text.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let vals: Vec<u64> = parts.filter_map(|v| v.parse::<u64>().ok()).collect();
        if vals.len() < 4 {
            continue;
        }
        let idle = vals[3] + vals.get(4).copied().unwrap_or(0);
        out.insert(name.to_owned(), (vals.iter().sum(), idle));
    }
    out
}

fn read_core_ids() -> BTreeMap<usize, i32> {
    let mut ids = BTreeMap::new();
    for cpu_dir in read_dir_sorted("/sys/devices/system/cpu") {
        let name = file_name(&cpu_dir);
        let Some(rest) = name.strip_prefix("cpu") else {
            continue;
        };
        let Ok(cpu) = rest.parse::<usize>() else {
            continue;
        };
        if let Some(core) =
            read_trimmed(cpu_dir.join("topology/core_id")).and_then(|v| v.parse::<i32>().ok())
        {
            ids.insert(cpu, core);
        }
    }
    ids
}

#[derive(Clone)]
struct TempReading {
    chip: String,
    label: String,
    c: f64,
}

fn cpu_temp_fields(
    temps: &[TempReading],
    ncores: usize,
    core_ids: &BTreeMap<usize, i32>,
) -> (Option<f64>, Vec<Option<f64>>) {
    let mut package = None;
    let mut phys: HashMap<i32, f64> = HashMap::new();
    for temp in temps {
        if temp.chip != "coretemp" {
            continue;
        }
        if temp.label.starts_with("Package") {
            package = Some(temp.c);
        } else if let Some(rest) = temp.label.strip_prefix("Core ") {
            if let Ok(core) = rest.trim().parse::<i32>() {
                phys.insert(core, temp.c);
            }
        }
    }
    let core_temps = (0..ncores)
        .map(|i| core_ids.get(&i).and_then(|core| phys.get(core).copied()))
        .collect();
    (package, core_temps)
}

fn cpu_percentages(prev: &CpuTimes, cur: &CpuTimes) -> (f64, Vec<f64>) {
    let mut total = 0.0;
    let mut cores: Vec<(usize, f64)> = Vec::new();
    for (key, &(cur_total, cur_idle)) in cur {
        let &(prev_total, prev_idle) = prev.get(key).unwrap_or(&(cur_total, cur_idle));
        let dt = cur_total as i128 - prev_total as i128;
        let di = cur_idle as i128 - prev_idle as i128;
        let busy = if dt <= 0 {
            0.0
        } else {
            (100.0 * (dt - di) as f64 / dt as f64).clamp(0.0, 100.0)
        };
        if key == "cpu" {
            total = round1(busy);
        } else if let Ok(idx) = key[3..].parse::<usize>() {
            cores.push((idx, round1(busy)));
        }
    }
    cores.sort_by_key(|(idx, _)| *idx);
    (total, cores.into_iter().map(|(_, v)| v).collect())
}

fn cpu_json(
    prev: &CpuTimes,
    cur: &CpuTimes,
    temps: &[TempReading],
    core_ids: &BTreeMap<usize, i32>,
) -> Value {
    let (total, cores) = cpu_percentages(prev, cur);
    let (package, core_temps) = cpu_temp_fields(temps, cores.len(), core_ids);
    json!({
        "total": total,
        "cores": cores,
        "packageC": opt_f64(package),
        "coreTemps": core_temps.iter().map(|v| opt_f64(*v)).collect::<Vec<_>>(),
    })
}

// --- memory ---

#[derive(Default)]
struct Zone {
    name: String,
    start_kb: u64,
    present_pages: u64,
    managed_pages: u64,
    free_pages: u64,
}

fn read_zones(page_kb: u64) -> Vec<Zone> {
    let Some(text) = read_string("/proc/zoneinfo") else {
        return Vec::new();
    };
    let mut zones: Vec<Zone> = Vec::new();
    for line in text.lines() {
        if line.starts_with("Node") {
            let cleaned = line.replace(',', " ");
            let parts: Vec<&str> = cleaned.split_whitespace().collect();
            zones.push(Zone {
                name: parts.get(3).unwrap_or(&"").to_string(),
                ..Zone::default()
            });
            continue;
        }
        let Some(zone) = zones.last_mut() else {
            continue;
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if line.starts_with("  pages free") {
            zone.free_pages = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if parts.first() == Some(&"present") {
            zone.present_pages = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if parts.first() == Some(&"managed") {
            zone.managed_pages = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if parts.first() == Some(&"start_pfn:") {
            zone.start_kb = parts
                .get(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                * page_kb;
        }
    }
    zones.retain(|z| z.managed_pages > 0);
    zones.sort_by_key(|z| z.start_kb);
    zones
}

fn read_mem(page_kb: u64) -> Value {
    let mut fields: HashMap<String, u64> = HashMap::new();
    if let Some(text) = read_string("/proc/meminfo") {
        for line in text.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            if let Some(value) = rest.split_whitespace().next().and_then(|v| v.parse().ok()) {
                fields.insert(key.to_owned(), value);
            }
        }
    }
    let get = |key: &str| fields.get(key).copied().unwrap_or(0);
    let total = get("MemTotal");
    let available = get("MemAvailable");
    let swap_total = get("SwapTotal");
    let swap_free = get("SwapFree");
    let zones: Vec<Value> = read_zones(page_kb)
        .into_iter()
        .map(|z| {
            json!({
                "name": z.name,
                "startKb": z.start_kb,
                "presentPages": z.present_pages,
                "managedPages": z.managed_pages,
                "freePages": z.free_pages,
                "managedKb": z.managed_pages * page_kb,
            })
        })
        .collect();
    json!({
        "totalKb": total,
        "usedKb": total.saturating_sub(available),
        "availableKb": available,
        "freeKb": get("MemFree"),
        "cacheKb": get("Cached") + get("Buffers"),
        "swapTotalKb": swap_total,
        "swapUsedKb": swap_total.saturating_sub(swap_free),
        "zones": zones,
    })
}

// --- Intel xe via fdinfo ---

type XeClients = BTreeMap<String, BTreeMap<String, (u64, u64)>>;

fn find_xe_nodes() -> HashSet<PathBuf> {
    let mut nodes = HashSet::new();
    for entry in read_dir_sorted("/sys/class/drm") {
        let Ok(driver) = fs::read_link(entry.join("device/driver")) else {
            continue;
        };
        if driver.file_name().map(|s| s == "xe").unwrap_or(false) {
            nodes.insert(PathBuf::from("/dev/dri").join(file_name(&entry)));
        }
    }
    nodes
}

#[derive(Default)]
struct XeScan {
    fds: HashSet<PathBuf>,
    pids: HashSet<u32>,
    rescan_at: Option<Instant>,
}

fn scan_xe_fds(pid: u32, xe_nodes: &HashSet<PathBuf>, out: &mut HashSet<PathBuf>) {
    let fd_dir = PathBuf::from(format!("/proc/{}/fd", pid));
    for fd_path in read_dir_paths(&fd_dir) {
        let Ok(target) = fs::read_link(&fd_path) else {
            continue;
        };
        if xe_nodes.contains(&target) {
            out.insert(PathBuf::from(format!(
                "/proc/{}/fdinfo/{}",
                pid,
                file_name(&fd_path)
            )));
        }
    }
}

fn read_xe_clients(scan: &mut XeScan, xe_nodes: &HashSet<PathBuf>) -> XeClients {
    let mut clients = XeClients::new();
    if xe_nodes.is_empty() {
        return clients;
    }
    let pids: HashSet<u32> = read_dir_paths("/proc")
        .iter()
        .filter_map(|p| file_name(p).parse::<u32>().ok())
        .collect();
    let now = Instant::now();
    let rescan = scan.rescan_at.map(|at| now >= at).unwrap_or(true);
    let new_pids: Vec<u32> = if rescan {
        scan.fds.clear();
        scan.rescan_at = Some(now + Duration::from_secs_f64(XE_RESCAN_SECONDS));
        pids.iter().copied().collect()
    } else {
        pids.difference(&scan.pids).copied().collect()
    };
    for pid in new_pids {
        scan_xe_fds(pid, xe_nodes, &mut scan.fds);
    }
    scan.pids = pids;

    let mut scratch: Vec<u8> = Vec::with_capacity(8192);
    for fdinfo in std::mem::take(&mut scan.fds) {
        if !read_bytes_into(&fdinfo, &mut scratch) {
            continue;
        }
        let text = String::from_utf8_lossy(&scratch[..scratch.len().min(4096)]);
        if !text.contains("drm-driver:\txe") && !text.contains("drm-driver: xe") {
            continue;
        }
        scan.fds.insert(fdinfo);
        let mut client: Option<String> = None;
        let mut cycles: BTreeMap<String, u64> = BTreeMap::new();
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("drm-client-id:") {
                client = Some(value.trim().to_owned());
            } else if line.starts_with("drm-total-cycles-") {
                if let Some((key, value)) = line.split_once(':') {
                    if let Ok(parsed) = value.trim().parse::<u64>() {
                        totals.insert(key["drm-total-cycles-".len()..].to_owned(), parsed);
                    }
                }
            } else if line.starts_with("drm-cycles-") {
                if let Some((key, value)) = line.split_once(':') {
                    if let Ok(parsed) = value.trim().parse::<u64>() {
                        cycles.insert(key["drm-cycles-".len()..].to_owned(), parsed);
                    }
                }
            }
        }
        let Some(client) = client else { continue };
        let entry = clients.entry(client).or_default();
        for (engine, cyc) in cycles {
            let total = totals.get(&engine).copied().unwrap_or(0);
            entry.insert(engine, (cyc, total));
        }
    }
    clients
}

fn xe_busy(prev: &XeClients, cur: &XeClients) -> Map<String, Value> {
    let mut engines: BTreeMap<String, (i128, i128)> = BTreeMap::new();
    for (client, cur_engines) in cur {
        let prev_engines = prev.get(client);
        for (engine, &(cyc, total)) in cur_engines {
            let (pcyc, ptotal) = prev_engines
                .and_then(|m| m.get(engine))
                .copied()
                .unwrap_or((cyc, total));
            let dcyc = cyc as i128 - pcyc as i128;
            let dtotal = total as i128 - ptotal as i128;
            if dtotal <= 0 {
                continue;
            }
            let entry = engines.entry(engine.clone()).or_insert((0, 0));
            entry.0 += dcyc;
            entry.1 = entry.1.max(dtotal);
        }
    }
    let mut out = Map::new();
    for (engine, (dcyc, dtotal)) in engines {
        if dtotal > 0 {
            let busy = (100.0 * dcyc as f64 / dtotal as f64).min(100.0);
            out.insert(engine, json!(round1(busy)));
        }
    }
    out
}

fn xe_freq() -> (Option<u64>, Option<u64>) {
    for card in read_dir_sorted("/sys/class/drm") {
        if !file_name(&card).starts_with("card") {
            continue;
        }
        let tile = card.join("device/tile0");
        for gt in read_dir_sorted(&tile) {
            if !file_name(&gt).starts_with("gt") {
                continue;
            }
            let act_path = gt.join("freq0/act_freq");
            let act = read_trimmed(&act_path).and_then(|v| v.parse::<u64>().ok());
            let cur = read_trimmed(act_path.with_file_name("cur_freq"))
                .and_then(|v| v.parse::<u64>().ok());
            if act.is_some() && cur.is_some() {
                return (act, cur);
            }
        }
    }
    (None, None)
}

// --- NVIDIA (Optimus-safe) ---

fn find_nvidia_pci() -> Option<PathBuf> {
    for dev in read_dir_sorted("/sys/bus/pci/devices") {
        if read_trimmed(dev.join("vendor")).as_deref() != Some("0x10de") {
            continue;
        }
        let class = read_trimmed(dev.join("class")).unwrap_or_default();
        if class.starts_with("0x03") {
            return Some(dev);
        }
    }
    None
}

type NvmlResult = libc::c_int;
const NVML_SUCCESS: NvmlResult = 0;
type NvmlInit = unsafe extern "C" fn() -> NvmlResult;
type NvmlGetHandle = unsafe extern "C" fn(libc::c_uint, *mut *mut libc::c_void) -> NvmlResult;
type NvmlGetUtilization =
    unsafe extern "C" fn(*mut libc::c_void, *mut NvmlUtilization) -> NvmlResult;
type NvmlGetTemperature =
    unsafe extern "C" fn(*mut libc::c_void, libc::c_int, *mut libc::c_uint) -> NvmlResult;
type NvmlGetMemoryInfo = unsafe extern "C" fn(*mut libc::c_void, *mut NvmlMemory) -> NvmlResult;
type NvmlGetPowerUsage = unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_uint) -> NvmlResult;

#[repr(C)]
struct NvmlUtilization {
    gpu: libc::c_uint,
    memory: libc::c_uint,
}

#[repr(C)]
struct NvmlMemory {
    total: libc::c_ulonglong,
    free: libc::c_ulonglong,
    used: libc::c_ulonglong,
}

struct Nvml {
    _lib: Library,
    device: usize,
}

impl Nvml {
    fn load() -> Option<Self> {
        unsafe {
            let lib = Library::new("libnvidia-ml.so.1").ok()?;
            let init: libloading::Symbol<NvmlInit> = lib.get(b"nvmlInit_v2\0").ok()?;
            if init() != NVML_SUCCESS {
                return None;
            }
            let get_handle: libloading::Symbol<NvmlGetHandle> =
                lib.get(b"nvmlDeviceGetHandleByIndex_v2\0").ok()?;
            let mut device: *mut libc::c_void = std::ptr::null_mut();
            if get_handle(0, &mut device) != NVML_SUCCESS || device.is_null() {
                return None;
            }
            Some(Self {
                _lib: lib,
                device: device as usize,
            })
        }
    }

    fn read(&self) -> Option<Value> {
        unsafe {
            let device = self.device as *mut libc::c_void;
            let get_util: libloading::Symbol<NvmlGetUtilization> =
                self._lib.get(b"nvmlDeviceGetUtilizationRates\0").ok()?;
            let mut util = NvmlUtilization { gpu: 0, memory: 0 };
            if get_util(device, &mut util) != NVML_SUCCESS {
                return None;
            }
            let get_temp: libloading::Symbol<NvmlGetTemperature> =
                self._lib.get(b"nvmlDeviceGetTemperature\0").ok()?;
            let mut temp: libc::c_uint = 0;
            // NVML_TEMPERATURE_GPU = 0
            if get_temp(device, 0, &mut temp) != NVML_SUCCESS {
                return None;
            }
            let get_mem: libloading::Symbol<NvmlGetMemoryInfo> =
                self._lib.get(b"nvmlDeviceGetMemoryInfo\0").ok()?;
            let mut mem = NvmlMemory {
                total: 0,
                free: 0,
                used: 0,
            };
            if get_mem(device, &mut mem) != NVML_SUCCESS {
                return None;
            }
            let power = self
                ._lib
                .get::<NvmlGetPowerUsage>(b"nvmlDeviceGetPowerUsage\0")
                .ok()
                .and_then(|get_power| {
                    let mut milliwatts: libc::c_uint = 0;
                    if get_power(device, &mut milliwatts) == NVML_SUCCESS {
                        Some(milliwatts as f64 / 1000.0)
                    } else {
                        None
                    }
                });
            Some(json!({
                "asleep": false,
                "utilPct": util.gpu as f64,
                "tempC": temp as f64,
                "memUsedMb": mem.used as f64 / 1_048_576.0,
                "memTotalMb": mem.total as f64 / 1_048_576.0,
                "powerW": opt_f64(power),
            }))
        }
    }
}

#[derive(Default)]
struct NvidiaState {
    nvml: Option<Nvml>,
    nvml_unavailable: bool,
}

fn read_nvidia(pci: Option<&Path>, state: &mut NvidiaState) -> Value {
    let Some(pci) = pci else {
        return Value::Null;
    };
    let status =
        read_trimmed(pci.join("power/runtime_status")).unwrap_or_else(|| "unknown".to_owned());
    if status == "suspended" {
        return json!({"asleep": true});
    }
    if state.nvml.is_none() && !state.nvml_unavailable {
        match Nvml::load() {
            Some(nvml) => state.nvml = Some(nvml),
            None => state.nvml_unavailable = true,
        }
    }
    if let Some(nvml) = &state.nvml {
        if let Some(value) = nvml.read() {
            return value;
        }
        // Drop a bad handle and fall through to the old path this tick;
        // a later active poll can try NVML again.
        state.nvml = None;
    }
    read_nvidia_smi()
}

fn read_nvidia_smi() -> Value {
    let Ok(output) = Command::new("timeout")
        .args([
            "3",
            "nvidia-smi",
            "--query-gpu=utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return Value::Null;
    };
    if !output.status.success() {
        return Value::Null;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(line) = text.lines().next() else {
        return Value::Null;
    };
    let fields: Vec<&str> = line.split(',').map(|v| v.trim()).collect();
    if fields.len() != 5 {
        return Value::Null;
    }
    let parse = |idx: usize| fields[idx].parse::<f64>().ok();
    let (Some(util), Some(temp), Some(mem_used), Some(mem_total)) =
        (parse(0), parse(1), parse(2), parse(3))
    else {
        return Value::Null;
    };
    let power = if fields[4].contains("N/A") {
        None
    } else {
        parse(4)
    };
    json!({
        "asleep": false,
        "utilPct": util,
        "tempC": temp,
        "memUsedMb": mem_used,
        "memTotalMb": mem_total,
        "powerW": opt_f64(power),
    })
}

// --- temperatures ---

#[derive(Default)]
struct TempCache {
    sensors: Vec<(String, String, PathBuf)>,
    rescan_at: Option<Instant>,
    readings: Vec<TempReading>,
    read_at: Option<Instant>,
}

fn discover_temps() -> Vec<(String, String, PathBuf)> {
    let mut sensors = Vec::new();
    for hwmon in read_dir_sorted("/sys/class/hwmon") {
        if !file_name(&hwmon).starts_with("hwmon") {
            continue;
        }
        let Some(chip) = read_trimmed(hwmon.join("name")) else {
            continue;
        };
        for input_path in read_dir_sorted(&hwmon) {
            let name = file_name(&input_path);
            if !(name.starts_with("temp") && name.ends_with("_input")) {
                continue;
            }
            let label_name = name.replace("_input", "_label");
            let label = read_trimmed(hwmon.join(&label_name))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| name.trim_end_matches("_input").to_owned());
            sensors.push((chip.clone(), label, input_path));
        }
    }
    sensors
}

fn read_temps(cache: &mut TempCache) -> &[TempReading] {
    let now = Instant::now();
    if cache.rescan_at.map(|at| now >= at).unwrap_or(true) {
        cache.sensors = discover_temps();
        cache.rescan_at = Some(now + Duration::from_secs_f64(TEMP_RESCAN_SECONDS));
        cache.read_at = None;
    }
    let stale = cache
        .read_at
        .map(|at| now.duration_since(at).as_secs_f64() >= TEMP_READ_SECONDS)
        .unwrap_or(true);
    if !stale {
        return &cache.readings;
    }
    let mut temps = Vec::new();
    for (chip, label, path) in &cache.sensors {
        let Some(milli) = read_trimmed(path).and_then(|v| v.parse::<i64>().ok()) else {
            continue;
        };
        temps.push(TempReading {
            chip: chip.clone(),
            label: label.clone(),
            c: round1(milli as f64 / 1000.0),
        });
    }
    cache.readings = temps;
    cache.read_at = Some(now);
    &cache.readings
}

fn temps_json(temps: &[TempReading]) -> Value {
    Value::Array(
        temps
            .iter()
            .map(|t| json!({"chip": t.chip, "label": t.label, "c": t.c}))
            .collect(),
    )
}

// --- machine identity ---

fn read_dmi() -> (String, String) {
    (
        read_trimmed("/sys/class/dmi/id/product_name").unwrap_or_default(),
        read_trimmed("/sys/class/dmi/id/chassis_type").unwrap_or_default(),
    )
}

// --- network ---

type NetCounters = BTreeMap<String, (u64, u64)>;

fn read_net() -> NetCounters {
    let mut ifaces = BTreeMap::new();
    let Some(text) = read_string("/proc/net/dev") else {
        return ifaces;
    };
    for line in text.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name == "lo" {
            continue;
        }
        let vals: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|v| v.parse::<u64>().ok())
            .collect();
        if vals.len() > 8 {
            ifaces.insert(name.to_owned(), (vals[0], vals[8]));
        }
    }
    ifaces
}

fn net_rates(prev: &NetCounters, cur: &NetCounters, dt: f64) -> Value {
    let mut per_iface = Map::new();
    let mut rx_total = 0.0;
    let mut tx_total = 0.0;
    for (name, &(rx, tx)) in cur {
        let (prx, ptx) = prev.get(name).copied().unwrap_or((rx, tx));
        let drx = (rx as i128 - prx as i128).max(0) as f64 / dt;
        let dtx = (tx as i128 - ptx as i128).max(0) as f64 / dt;
        if drx > 0.0
            || dtx > 0.0
            || name.starts_with("wl")
            || name.starts_with("en")
            || name.starts_with("eth")
        {
            per_iface.insert(
                name.clone(),
                json!({"downBps": drx.round() as i64, "upBps": dtx.round() as i64}),
            );
        }
        rx_total += drx;
        tx_total += dtx;
    }
    json!({
        "downBps": rx_total.round() as i64,
        "upBps": tx_total.round() as i64,
        "ifaces": Value::Object(per_iface),
    })
}

// --- disk ---

#[derive(Default)]
struct BlockCache {
    disks: HashSet<String>,
    rescan_at: Option<Instant>,
}

impl BlockCache {
    fn whole_disks(&mut self) -> &HashSet<String> {
        let now = Instant::now();
        if self.rescan_at.map(|at| now >= at).unwrap_or(true) {
            self.disks = read_dir_sorted("/sys/block")
                .iter()
                .map(|p| file_name(p))
                .collect();
            self.rescan_at = Some(now + Duration::from_secs_f64(BLOCK_RESCAN_SECONDS));
        }
        &self.disks
    }
}

type DiskCounters = BTreeMap<String, (u64, u64, u64)>;

fn read_disks(cache: &mut BlockCache) -> DiskCounters {
    let mut disks = BTreeMap::new();
    let whole = cache.whole_disks().clone();
    let Some(text) = read_string("/proc/diskstats") else {
        return disks;
    };
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() <= 12 {
            continue;
        }
        let name = parts[2];
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("dm-")
            || name.starts_with("sr")
            || name.starts_with("md")
        {
            continue;
        }
        if !whole.contains(name) {
            continue;
        }
        let (Ok(read_sectors), Ok(write_sectors), Ok(io_ms)) = (
            parts[5].parse::<u64>(),
            parts[9].parse::<u64>(),
            parts[12].parse::<u64>(),
        ) else {
            continue;
        };
        disks.insert(
            name.to_owned(),
            (read_sectors * 512, write_sectors * 512, io_ms),
        );
    }
    disks
}

fn disk_rates(prev: &DiskCounters, cur: &DiskCounters, dt: f64) -> Value {
    let mut per_disk = Map::new();
    let mut read_total = 0.0;
    let mut write_total = 0.0;
    let mut util_max: f64 = 0.0;
    for (name, &(rd, wr, io_ms)) in cur {
        let (prd, pwr, pio) = prev.get(name).copied().unwrap_or((rd, wr, io_ms));
        let drd = (rd as i128 - prd as i128).max(0) as f64 / dt;
        let dwr = (wr as i128 - pwr as i128).max(0) as f64 / dt;
        let util = ((io_ms as i128 - pio as i128) as f64 / dt / 10.0).clamp(0.0, 100.0);
        per_disk.insert(
            name.clone(),
            json!({
                "readBps": drd.round() as i64,
                "writeBps": dwr.round() as i64,
                "utilPct": round1(util),
            }),
        );
        read_total += drd;
        write_total += dwr;
        util_max = util_max.max(util);
    }
    json!({
        "readBps": read_total.round() as i64,
        "writeBps": write_total.round() as i64,
        "utilPct": round1(util_max),
        "disks": Value::Object(per_disk),
    })
}

// --- connections ---

fn is_private_or_reserved(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    matches!(octets[0], 127 | 10 | 100)
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
}

fn read_connections(ip_table: &IpTable) -> Value {
    let mut seen: HashSet<Ipv4Addr> = HashSet::new();
    let mut ordered: Vec<(Ipv4Addr, u16)> = Vec::new();
    if let Some(text) = read_string("/proc/net/tcp") {
        for line in text.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() <= 3 || parts[3] != "01" {
                continue;
            }
            let Some((ip_hex, port_hex)) = parts[2].split_once(':') else {
                continue;
            };
            let (Ok(ip_raw), Ok(port)) = (
                u32::from_str_radix(ip_hex, 16),
                u16::from_str_radix(port_hex, 16),
            ) else {
                continue;
            };
            let ip = Ipv4Addr::from(ip_raw.to_le_bytes());
            if is_private_or_reserved(ip) || !seen.insert(ip) {
                continue;
            }
            ordered.push((ip, port));
        }
    }
    let mut conns = Vec::new();
    for (ip, port) in ordered.into_iter().take(64) {
        let country_code = ip_table.country_code_for_ip(ip);
        let country = country_code
            .as_ref()
            .and_then(|code| std::str::from_utf8(code).ok());
        let mut entry = Map::new();
        entry.insert("ip".to_owned(), json!(ip.to_string()));
        entry.insert("port".to_owned(), json!(port));
        entry.insert(
            "country".to_owned(),
            country.map(|c| json!(c)).unwrap_or(Value::Null),
        );
        if let Some(country) = country {
            if let Some((lat, lon)) = centroids::centroid(country) {
                entry.insert("lat".to_owned(), json!(lat));
                entry.insert("lon".to_owned(), json!(lon));
            }
        }
        conns.push(Value::Object(entry));
    }
    Value::Array(conns)
}

// --- processes ---

#[derive(Clone)]
struct ProcInfo {
    comm: String,
    state: String,
    ticks: u64,
    rss_kb: i64,
}

type ProcMap = BTreeMap<u32, ProcInfo>;

fn read_procs(page_kb: u64) -> ProcMap {
    let mut procs = BTreeMap::new();
    let mut data: Vec<u8> = Vec::with_capacity(8192);
    for entry in read_dir_paths("/proc") {
        let Ok(pid) = file_name(&entry).parse::<u32>() else {
            continue;
        };
        if !read_bytes_into(entry.join("stat"), &mut data) {
            continue;
        }
        let Some(first_space) = data.iter().position(|b| *b == b' ') else {
            continue;
        };
        let rest = &data[first_space + 1..];
        let Some(lparen) = rest.iter().position(|b| *b == b'(') else {
            continue;
        };
        let Some(rparen) = rest.iter().rposition(|b| *b == b')') else {
            continue;
        };
        if rparen + 2 > rest.len() {
            continue;
        }
        let comm = String::from_utf8_lossy(&rest[lparen + 1..rparen]).into_owned();
        let fields_text = String::from_utf8_lossy(&rest[rparen + 2..]);
        let fields: Vec<&str> = fields_text.split_whitespace().collect();
        if fields.len() <= 21 {
            continue;
        }
        let (Ok(utime), Ok(stime), Ok(rss_pages)) = (
            fields[11].parse::<u64>(),
            fields[12].parse::<u64>(),
            fields[21].parse::<i64>(),
        ) else {
            continue;
        };
        procs.insert(
            pid,
            ProcInfo {
                comm,
                state: fields[0].to_owned(),
                ticks: utime + stime,
                rss_kb: rss_pages * page_kb as i64,
            },
        );
    }
    procs
}

fn proc_deltas(prev: &ProcMap, cur: &ProcMap, dt: f64, clock: u64, num_cpus: usize) -> Value {
    let mut out: Vec<(f64, Value)> = Vec::new();
    let cap = 100.0 * num_cpus.max(1) as f64;
    for (pid, info) in cur {
        let prev_ticks = prev.get(pid).map(|p| p.ticks).unwrap_or(info.ticks);
        let cpu_pct =
            100.0 * (info.ticks as i128 - prev_ticks as i128) as f64 / clock.max(1) as f64 / dt;
        let cpu_pct = round1(cpu_pct.min(cap));
        out.push((
            cpu_pct,
            json!({
                "pid": pid,
                "comm": info.comm,
                "state": info.state,
                "cpuPct": cpu_pct,
                "rssKb": info.rss_kb,
            }),
        ));
    }
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Value::Array(out.into_iter().map(|(_, v)| v).collect())
}

fn meta_value(
    cores: usize,
    has_nvidia: bool,
    geo_ranges: usize,
    model: &str,
    chassis: &str,
    coastlines: Value,
) -> Value {
    json!({
        "meta": {
            "cores": cores,
            "hasNvidia": has_nvidia,
            "geoRanges": geo_ranges,
            "model": model,
            "chassis": chassis,
            "coastlines": coastlines,
        }
    })
}

fn load_coastlines(assets: &Path) -> Value {
    let Some(text) = read_string(assets.join("coastlines.json")) else {
        return json!([]);
    };
    serde_json::from_str(&text).unwrap_or_else(|_| json!([]))
}

fn write_line(out: &mut impl Write, value: &Value) -> io::Result<()> {
    let line = serde_json::to_string(value).expect("sampler JSON is serializable");
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

fn run() -> io::Result<()> {
    set_idle_priority();
    let demand_rx = spawn_demand_reader();
    let mut demand = Demand::default();
    let assets = assets_dir();
    let mut ip_table = IpTable::default();
    let mut coastlines = json!([]);
    let mut geo_loaded = false;
    let core_ids = read_core_ids();
    let xe_nodes = find_xe_nodes();
    let mut xe_scan = XeScan::default();
    let mut temp_cache = TempCache::default();
    let mut mem_cache = MemCache::default();
    let mut block_cache = BlockCache::default();
    let page_kb = page_kb();
    let clock = clock_ticks();

    let mut prev_cpu = read_cpu_times();
    let num_cpus = prev_cpu.len().saturating_sub(1).max(1);
    let mut prev_xe = read_xe_clients(&mut xe_scan, &xe_nodes);
    let mut prev_net = read_net();
    let mut prev_disk = read_disks(&mut block_cache);
    let mut prev_procs = ProcMap::new();
    let mut prev_time = Instant::now();
    let mut prev_procs_time = prev_time;
    let nvidia_pci = find_nvidia_pci();
    let mut nvidia_state = NvidiaState::default();
    let mut nvidia = read_nvidia(nvidia_pci.as_deref(), &mut nvidia_state);

    let (model, chassis) = read_dmi();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    write_line(
        &mut out,
        &meta_value(
            prev_cpu.len().saturating_sub(1),
            nvidia_pci.is_some(),
            ip_table.starts.len(),
            &model,
            &chassis,
            coastlines.clone(),
        ),
    )?;

    drain_demand(&demand_rx, &mut demand);
    let mut tick: u64 = 0;
    loop {
        wait_for_tick(&demand_rx, &mut demand, prev_time);
        tick += 1;
        drain_demand(&demand_rx, &mut demand);
        if demand.conns && !geo_loaded {
            ip_table = IpTable::load(&assets);
            coastlines = load_coastlines(&assets);
            geo_loaded = true;
            write_line(
                &mut out,
                &meta_value(
                    prev_cpu.len().saturating_sub(1),
                    nvidia_pci.is_some(),
                    ip_table.starts.len(),
                    &model,
                    &chassis,
                    coastlines.clone(),
                ),
            )?;
        }
        let now = Instant::now();
        let dt = (now - prev_time).as_secs_f64().max(1e-3);

        let cur_cpu = read_cpu_times();
        let cur_xe = read_xe_clients(&mut xe_scan, &xe_nodes);
        let cur_net = read_net();
        let cur_disk = read_disks(&mut block_cache);

        if tick.is_multiple_of(NVIDIA_EVERY) {
            nvidia = read_nvidia(nvidia_pci.as_deref(), &mut nvidia_state);
        }

        let temps = read_temps(&mut temp_cache);
        let cpu = cpu_json(&prev_cpu, &cur_cpu, temps, &core_ids);
        let (freq_mhz, cur_freq_mhz) = xe_freq();

        let mut sample = Map::new();
        sample.insert("cpu".to_owned(), cpu);
        sample.insert("mem".to_owned(), read_mem(page_kb, &mut mem_cache));
        sample.insert(
            "gpu".to_owned(),
            json!({
                "xe": {
                    "freqMhz": opt_u64(freq_mhz),
                    "curFreqMhz": opt_u64(cur_freq_mhz),
                    "engines": Value::Object(xe_busy(&prev_xe, &cur_xe)),
                },
                "nvidia": nvidia.clone(),
            }),
        );
        sample.insert("temps".to_owned(), temps_json(temps));
        sample.insert("net".to_owned(), net_rates(&prev_net, &cur_net, dt));
        sample.insert("disk".to_owned(), disk_rates(&prev_disk, &cur_disk, dt));

        if demand.procs && tick.is_multiple_of(PROCS_EVERY) {
            let cur_procs = read_procs(page_kb);
            let procs_dt = (now - prev_procs_time).as_secs_f64().max(1e-3);
            sample.insert(
                "procs".to_owned(),
                proc_deltas(&prev_procs, &cur_procs, procs_dt, clock, num_cpus),
            );
            prev_procs = cur_procs;
            prev_procs_time = now;
        } else if !demand.procs {
            prev_procs.clear();
            prev_procs_time = now;
        }
        if demand.conns && tick % CONNS_EVERY == 1 {
            sample.insert("conns".to_owned(), read_connections(&ip_table));
        }

        write_line(&mut out, &Value::Object(sample))?;
        prev_cpu = cur_cpu;
        prev_xe = cur_xe;
        prev_net = cur_net;
        prev_disk = cur_disk;
        prev_time = now;
    }
}

fn main() {
    if let Err(err) = run() {
        if err.kind() != io::ErrorKind::BrokenPipe {
            eprintln!("minkamon-sampler: {}", err);
        }
    }
}
