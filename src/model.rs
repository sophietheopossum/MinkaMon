//! Sampler data shapes, derived readings, histories, and formatting.
//!
//! The sampler remains the source of truth for the wire contract; this module
//! only interprets its JSON for the UI. Formatting intentionally matches the
//! Quickshell frontend's `Sampler.fmtBytes` / `Sampler.fmtKb` labels.

use serde_json::Value;
use std::collections::VecDeque;

pub const HISTORY_LEN: usize = 60;

pub fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    Some(current)
}

pub fn f64_at(value: &Value, path: &[&str]) -> f64 {
    at(value, path).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn opt_f64_at(value: &Value, path: &[&str]) -> Option<f64> {
    at(value, path).and_then(Value::as_f64)
}

pub fn i64_at(value: &Value, path: &[&str]) -> i64 {
    at(value, path).and_then(Value::as_i64).unwrap_or(0)
}

pub fn u64_at(value: &Value, path: &[&str]) -> u64 {
    at(value, path).and_then(Value::as_u64).unwrap_or(0)
}

pub fn str_at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    at(value, path).and_then(Value::as_str).unwrap_or("")
}

pub fn bool_at(value: &Value, path: &[&str]) -> bool {
    at(value, path).and_then(Value::as_bool).unwrap_or(false)
}

pub fn arr_at<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    at(value, path)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub fn obj_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a serde_json::Map<String, Value>> {
    at(value, path).and_then(Value::as_object)
}

/// `Sampler.fmtBytes` in the QML frontend: binary thresholds, decimal labels.
pub fn fmt_rate(bps: i64) -> String {
    let bps = bps.max(0);
    if bps >= 1_048_576 {
        format!("{:.1} MB/s", bps as f64 / 1_048_576.0)
    } else if bps >= 1024 {
        format!("{:.1} KB/s", bps as f64 / 1024.0)
    } else {
        format!("{bps} B/s")
    }
}

/// `Sampler.fmtKb` in the QML frontend.
pub fn fmt_kb(kb: i64) -> String {
    let kb = kb.max(0);
    if kb >= 1_048_576 {
        format!("{:.2} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{:.0} MB", kb as f64 / 1024.0)
    } else {
        format!("{kb} KB")
    }
}

pub fn fmt_temp1(c: Option<f64>) -> String {
    c.map_or_else(|| "—".to_owned(), |v| format!("{v:.1}°C"))
}

pub fn fmt_addr(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.0}G", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{:.0}M", kb as f64 / 1024.0)
    } else {
        format!("{kb}K")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TempKind {
    Board,
    Cpu,
    Gpu,
    Ssd,
    Wifi,
}

impl TempKind {
    pub const ALL: [Self; 5] = [Self::Board, Self::Cpu, Self::Gpu, Self::Ssd, Self::Wifi];

    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "BOARD °C",
            Self::Cpu => "CPU °C",
            Self::Gpu => "GPU °C",
            Self::Ssd => "SSD °C",
            Self::Wifi => "WIFI °C",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Board => 0,
            Self::Cpu => 1,
            Self::Gpu => 2,
            Self::Ssd => 3,
            Self::Wifi => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    Board,
    Cpu,
    Gpu,
    Ram,
    Wifi,
    Ssd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Cpu,
    Gpu,
    Memory,
    Network,
    Disk,
    Globe,
    Processes,
    Temp(TempKind),
}

impl WindowKind {
    pub const INFO: [Self; 7] = [
        Self::Cpu,
        Self::Gpu,
        Self::Memory,
        Self::Network,
        Self::Disk,
        Self::Globe,
        Self::Processes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Memory => "MEMORY",
            Self::Network => "NETWORK",
            Self::Disk => "DISK",
            Self::Globe => "GLOBE",
            Self::Processes => "PROCESSES",
            Self::Temp(kind) => kind.label(),
        }
    }

    pub fn title(self) -> String {
        format!("MinkaMon // {}", self.label())
    }

    /// The compositor role, derived exactly as the QML Satellite derives it.
    pub fn role(self) -> String {
        let mut slug = String::new();
        for c in self.label().to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.ends_with('-') {
                slug.push('-');
            }
        }
        format!("minkamon.{}", slug.trim_matches('-'))
    }

    pub fn size(self) -> (i32, i32) {
        match self {
            Self::Cpu => (540, 430),
            Self::Gpu => (540, 400),
            Self::Memory => (500, 430),
            Self::Network => (520, 430),
            Self::Disk => (520, 400),
            Self::Globe => (560, 560),
            Self::Processes => (820, 560),
            Self::Temp(_) => (420, 280),
        }
    }

    pub fn min_size(self) -> (i32, i32) {
        match self {
            Self::Cpu | Self::Gpu | Self::Network => (380, 300),
            Self::Memory => (380, 320),
            Self::Disk => (380, 280),
            Self::Globe => (360, 360),
            Self::Processes => (520, 400),
            Self::Temp(_) => (300, 200),
        }
    }
}

/// Sensor readings mapped to physical components, as `SystemPanel.readings`.
#[derive(Clone, Debug, Default)]
pub struct TempReadings {
    pub pkg: Option<f64>,
    pub cores: Vec<f64>,
    pub ssd: Option<f64>,
    pub wifi: Option<f64>,
    pub board: Option<f64>,
    pub gpu: Option<f64>,
    pub gpu_dormant: bool,
    pub max: Option<f64>,
}

impl TempReadings {
    pub fn get(&self, kind: TempKind) -> Option<f64> {
        match kind {
            TempKind::Board => self.board,
            TempKind::Cpu => self.pkg,
            TempKind::Gpu => self.gpu,
            TempKind::Ssd => self.ssd,
            TempKind::Wifi => self.wifi,
        }
    }
}

pub fn temp_readings(sample: &Value, meta: &Value) -> TempReadings {
    let temps = arr_at(sample, &["temps"]);
    let find = |pred: &dyn Fn(&str, &str) -> bool| {
        temps.iter().find_map(|s| {
            let chip = str_at(s, &["chip"]);
            let label = str_at(s, &["label"]);
            pred(chip, label).then(|| f64_at(s, &["c"]))
        })
    };
    let pkg = find(&|chip, label| chip == "coretemp" && label.starts_with("Package"))
        .or_else(|| find(&|chip, label| chip == "k10temp" && matches!(label, "Tctl" | "Tdie")));
    let ssd = find(&|chip, label| chip == "nvme" && label == "Composite")
        .or_else(|| find(&|chip, _| chip == "nvme"));
    let cores = temps
        .iter()
        .filter(|s| str_at(s, &["chip"]) == "coretemp" && str_at(s, &["label"]).starts_with("Core"))
        .map(|s| f64_at(s, &["c"]))
        .collect::<Vec<_>>();
    let wifi = find(&|chip, _| chip.starts_with("iwlwifi") || chip.starts_with("mt76"));
    let board = find(&|chip, _| chip == "acpitz");

    let nv = at(sample, &["gpu", "nvidia"]);
    let nv_awake = nv
        .and_then(|v| v.get("asleep"))
        .and_then(Value::as_bool)
        .is_some_and(|asleep| !asleep);
    let gpu = nv_awake.then(|| f64_at(sample, &["gpu", "nvidia", "tempC"]));
    let has_nvidia = bool_at(meta, &["hasNvidia"]);
    let gpu_dormant = has_nvidia && !nv_awake;

    let mut max = gpu;
    for s in temps {
        let c = f64_at(s, &["c"]);
        max = Some(max.map_or(c, |m: f64| m.max(c)));
    }

    TempReadings {
        pkg,
        cores,
        ssd,
        wifi,
        board,
        gpu,
        gpu_dormant,
        max,
    }
}

pub fn machine_is_zenbook(meta: &Value) -> bool {
    str_at(meta, &["model"]).contains("UX482")
}

fn push_capped<T>(history: &mut VecDeque<T>, value: T) {
    if history.len() >= HISTORY_LEN {
        history.pop_front();
    }
    history.push_back(value);
}

/// Rolling 60-sample histories. Centralised so satellites show a full chart
/// the moment they open, just as the QML predeclared satellites do.
#[derive(Clone, Debug, Default)]
pub struct Histories {
    pub cpu_total: VecDeque<f32>,
    pub cpu_cores: Vec<VecDeque<Option<f32>>>,
    pub cpu_core_temps: Vec<VecDeque<Option<f32>>>,
    pub engine_keys: Vec<String>,
    pub engine_series: Vec<VecDeque<Option<f32>>>,
    pub nvidia_util: VecDeque<f32>,
    pub down: VecDeque<f32>,
    pub up: VecDeque<f32>,
    pub disk_read: VecDeque<f32>,
    pub disk_write: VecDeque<f32>,
    pub disk_util: VecDeque<f32>,
    pub swap_pct: VecDeque<f32>,
    temps: [VecDeque<f32>; 5],
}

impl Histories {
    pub fn temp(&self, kind: TempKind) -> &VecDeque<f32> {
        &self.temps[kind.index()]
    }

    pub fn record(&mut self, sample: &Value, readings: &TempReadings) {
        push_capped(
            &mut self.cpu_total,
            f64_at(sample, &["cpu", "total"]) as f32,
        );

        let cores = arr_at(sample, &["cpu", "cores"]);
        self.cpu_cores.resize_with(cores.len(), VecDeque::new);
        for (i, value) in cores.iter().enumerate() {
            push_capped(&mut self.cpu_cores[i], value.as_f64().map(|v| v as f32));
        }

        let core_temps = arr_at(sample, &["cpu", "coreTemps"]);
        self.cpu_core_temps
            .resize_with(core_temps.len(), VecDeque::new);
        for (i, value) in core_temps.iter().enumerate() {
            push_capped(
                &mut self.cpu_core_temps[i],
                value.as_f64().map(|v| v as f32),
            );
        }

        // Xe engines are grow-only, matching GpuPanel: a series must never
        // change index when an engine appears mid-run.
        if let Some(engines) = obj_at(sample, &["gpu", "xe", "engines"]) {
            let mut new_keys: Vec<&String> = engines.keys().collect();
            new_keys.sort();
            for key in new_keys {
                if !self.engine_keys.iter().any(|k| k == key) {
                    self.engine_keys.push(key.clone());
                    self.engine_series.push(VecDeque::new());
                }
            }
            for (i, key) in self.engine_keys.iter().enumerate() {
                let value = engines.get(key).and_then(Value::as_f64).map(|v| v as f32);
                push_capped(&mut self.engine_series[i], value);
            }
        }

        let nv_awake = at(sample, &["gpu", "nvidia", "asleep"])
            .and_then(Value::as_bool)
            .is_some_and(|asleep| !asleep);
        push_capped(
            &mut self.nvidia_util,
            if nv_awake {
                f64_at(sample, &["gpu", "nvidia", "utilPct"]) as f32
            } else {
                0.0
            },
        );

        push_capped(&mut self.down, i64_at(sample, &["net", "downBps"]) as f32);
        push_capped(&mut self.up, i64_at(sample, &["net", "upBps"]) as f32);
        push_capped(
            &mut self.disk_read,
            i64_at(sample, &["disk", "readBps"]) as f32,
        );
        push_capped(
            &mut self.disk_write,
            i64_at(sample, &["disk", "writeBps"]) as f32,
        );
        push_capped(
            &mut self.disk_util,
            f64_at(sample, &["disk", "utilPct"]) as f32,
        );

        let swap_total = i64_at(sample, &["mem", "swapTotalKb"]);
        let swap_used = i64_at(sample, &["mem", "swapUsedKb"]);
        push_capped(
            &mut self.swap_pct,
            if swap_total > 0 {
                (100.0 * swap_used as f64 / swap_total as f64) as f32
            } else {
                0.0
            },
        );

        for kind in TempKind::ALL {
            let value = readings.get(kind).unwrap_or(0.0) as f32;
            push_capped(&mut self.temps[kind.index()], value);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcRow {
    pub pid: i64,
    pub comm: String,
    pub state: String,
    pub cpu_pct: f64,
    pub rss_kb: i64,
}

pub fn parse_procs(values: &[Value]) -> Vec<ProcRow> {
    values
        .iter()
        .map(|p| ProcRow {
            pid: i64_at(p, &["pid"]),
            comm: str_at(p, &["comm"]).to_owned(),
            state: str_at(p, &["state"]).to_owned(),
            cpu_pct: f64_at(p, &["cpuPct"]),
            rss_kb: i64_at(p, &["rssKb"]),
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct ConnRow {
    pub country: String,
    pub ip: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub fn parse_conns(values: &[Value]) -> Vec<ConnRow> {
    values
        .iter()
        .map(|c| ConnRow {
            country: {
                let country = str_at(c, &["country"]);
                if country.is_empty() {
                    "??".to_owned()
                } else {
                    country.to_owned()
                }
            },
            ip: str_at(c, &["ip"]).to_owned(),
            lat: opt_f64_at(c, &["lat"]),
            lon: opt_f64_at(c, &["lon"]),
        })
        .collect()
}
