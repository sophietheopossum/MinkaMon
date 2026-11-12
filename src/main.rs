//! MinkaMon.
//!
//! The AntiWebUI application is the successor to the Quickshell shell. It
//! keeps the existing `minkamon-sampler` process and its JSON-lines contract
//! (including the demand protocol: fast/eco tick plus procs/conns switches),
//! but renders with AntiWebUI's Rust/wgpu UI instead of QML.
//!
//! The sampler is found via `MINKAMON_SAMPLER_BIN`, then relative to this
//! executable / the checkout (`sampler-rs/target/release/minkamon-sampler`).
//! Assets come from `MINKAMON_ASSETS` or the checkout's `assets/` directory.

use anti_webui::prelude::svg::kurbo;
use anti_webui::prelude::*;
use anti_webui::shell::Process;
use serde_json::Value;
use std::cell::Cell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

const GROUND: u32 = 0x0a0709;
const SURFACE: u32 = 0x161013;
const LINE: u32 = 0x2e2228;
const TEXT: u32 = 0xece5e7;
const TEXT_MUTED: u32 = 0xa3959b;
const TEXT_FAINT: u32 = 0x6e6167;
const RED: u32 = 0xff0000;
const PURPLE: u32 = 0xa488c9;
const GAUGE_DIM: u32 = 0x3a1219;
const OK_GREEN: u32 = 0x7dc98c;
const WARN_AMBER: u32 = 0xe0a026;

const HISTORY: usize = 60;

fn first_existing(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.exists())
}

fn exe_ancestors() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            out.push(d.clone());
            dir = d.parent().map(|p| p.to_path_buf());
            if out.len() > 8 {
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd);
    }
    out
}

fn sampler_bin() -> PathBuf {
    if let Ok(path) = std::env::var("MINKAMON_SAMPLER_BIN")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    let mut candidates = Vec::new();
    for base in exe_ancestors() {
        candidates.push(
            base.join("sampler-rs")
                .join("target")
                .join("release")
                .join("minkamon-sampler"),
        );
        candidates.push(
            base.join("MinkaMon")
                .join("sampler-rs")
                .join("target")
                .join("release")
                .join("minkamon-sampler"),
        );
    }
    first_existing(candidates).unwrap_or_else(|| PathBuf::from("minkamon-sampler"))
}

fn assets_dir() -> PathBuf {
    if let Ok(path) = std::env::var("MINKAMON_ASSETS")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    let mut candidates = Vec::new();
    for base in exe_ancestors() {
        candidates.push(base.join("assets"));
        candidates.push(base.join("MinkaMon").join("assets"));
    }
    first_existing(candidates).unwrap_or_else(|| PathBuf::from("assets"))
}

fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

fn f64_at(value: &Value, path: &[&str]) -> f64 {
    at(value, path).and_then(Value::as_f64).unwrap_or(0.0)
}

fn i64_at(value: &Value, path: &[&str]) -> i64 {
    at(value, path).and_then(Value::as_i64).unwrap_or(0)
}

fn arr_at<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    at(value, path)
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn obj_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a serde_json::Map<String, Value>> {
    at(value, path).and_then(Value::as_object)
}

fn fmt_temp(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.1}°C"))
        .unwrap_or_else(|| "—".into())
}

fn fmt_rate(bps: i64) -> String {
    let bps = bps.max(0) as f64;
    if bps >= 1_048_576.0 {
        format!("{:.1} MB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else {
        format!("{bps:.0} B/s")
    }
}

fn fmt_kb(kb: i64) -> String {
    let kb = kb.max(0) as f64;
    if kb >= 1_048_576.0 {
        format!("{:.2} GB", kb / 1_048_576.0)
    } else if kb >= 1024.0 {
        format!("{:.0} MB", kb / 1024.0)
    } else {
        format!("{kb:.0} KB")
    }
}

fn push_history(history: Signal<VecDeque<f32>>, value: f64) {
    history.update(|h| {
        h.push_back(value as f32);
        while h.len() > HISTORY {
            h.pop_front();
        }
    });
}

fn panel(title: &'static str, child: impl IntoView) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(8.0_f32)
        .p(12.0_f32)
        .min_w(300.0)
        .flex_1()
        .rounded(6.0_f32)
        .border(1.0_f32)
        .border_color(Color::from_rgb(LINE))
        .bg(Color::from_rgb(SURFACE))
        .child(
            text(title)
                .text_size(12.0_f32)
                .text_color(Color::from_rgb(TEXT_MUTED)),
        )
        .child(child)
}

fn metric_row(label: impl IntoTextSource, value: impl IntoTextSource) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(8.0_f32)
        .child(
            text(label)
                .text_size(12.0_f32)
                .text_color(Color::from_rgb(TEXT_MUTED))
                .w(118.0),
        )
        .child(
            text(value)
                .text_size(13.0_f32)
                .text_color(Color::from_rgb(TEXT)),
        )
}

fn bar_row(label: String, percent: f64, color: u32) -> Div {
    let percent = percent.clamp(0.0, 100.0);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(8.0_f32)
        .child(
            text(label)
                .text_size(11.0_f32)
                .text_color(Color::from_rgb(TEXT_MUTED))
                .w(118.0),
        )
        .child(
            div()
                .h(8.0)
                .flex_1()
                .min_w(60.0)
                .rounded(3.0_f32)
                .bg(Color::from_rgb(GAUGE_DIM))
                .child(
                    div()
                        .h_full()
                        .rounded(3.0_f32)
                        .bg(Color::from_rgb(color))
                        .dyn_style(move |s| s.w(pct(percent as f32))),
                ),
        )
        .child(
            text(format!("{percent:.1}%"))
                .text_size(11.0_f32)
                .text_color(Color::from_rgb(TEXT))
                .w(52.0),
        )
}

fn cpu_panel(sample: Signal<Value>, history: Signal<VecDeque<f32>>) -> Div {
    panel(
        "CPU",
        div()
            .flex()
            .flex_col()
            .gap(6.0_f32)
            .child(sparkline(history, RED))
            .child(dyn_view(move || {
                let s = sample.get();
                let total = f64_at(&s, &["cpu", "total"]);
                let pkg = at(&s, &["cpu", "packageC"]).and_then(Value::as_f64);
                let mut rows = vec![bar_row("total".into(), total, RED)];
                for (i, core) in arr_at(&s, &["cpu", "cores"]).iter().enumerate() {
                    rows.push(bar_row(
                        format!("core {i}"),
                        core.as_f64().unwrap_or(0.0),
                        PURPLE,
                    ));
                }
                rows.push(metric_row("package", fmt_temp(pkg)));
                div().flex().flex_col().gap(4.0_f32).children(rows)
            })),
    )
}

fn mem_panel(sample: Signal<Value>, history: Signal<VecDeque<f32>>) -> Div {
    panel(
        "MEMORY",
        div()
            .flex()
            .flex_col()
            .gap(6.0_f32)
            .child(sparkline(history, PURPLE))
            .child(dyn_view(move || {
                let s = sample.get();
                let total = i64_at(&s, &["mem", "totalKb"]).max(1);
                let used = i64_at(&s, &["mem", "usedKb"]);
                let swap_total = i64_at(&s, &["mem", "swapTotalKb"]).max(1);
                let swap_used = i64_at(&s, &["mem", "swapUsedKb"]);
                div()
                    .flex()
                    .flex_col()
                    .gap(4.0_f32)
                    .child(bar_row(
                        "RAM".into(),
                        used as f64 * 100.0 / total as f64,
                        PURPLE,
                    ))
                    .child(bar_row(
                        "swap".into(),
                        swap_used as f64 * 100.0 / swap_total as f64,
                        WARN_AMBER,
                    ))
                    .child(metric_row("used", fmt_kb(used)))
                    .child(metric_row(
                        "available",
                        fmt_kb(i64_at(&s, &["mem", "availableKb"])),
                    ))
                    .child(metric_row("cache", fmt_kb(i64_at(&s, &["mem", "cacheKb"]))))
            })),
    )
}

fn gpu_panel(sample: Signal<Value>) -> Div {
    panel(
        "GPU",
        dyn_view(move || {
            let s = sample.get();
            let mut rows: Vec<Div> = Vec::new();
            let freq = at(&s, &["gpu", "xe", "freqMhz"]).and_then(Value::as_u64);
            rows.push(metric_row(
                "Xe freq",
                freq.map(|v| format!("{v} MHz"))
                    .unwrap_or_else(|| "—".into()),
            ));
            if let Some(engines) = obj_at(&s, &["gpu", "xe", "engines"]) {
                for (name, value) in engines {
                    rows.push(bar_row(name.clone(), value.as_f64().unwrap_or(0.0), RED));
                }
            }
            match at(&s, &["gpu", "nvidia"]) {
                Some(Value::Object(nv)) if nv.get("asleep") == Some(&Value::Bool(true)) => {
                    rows.push(metric_row("NVIDIA", "asleep"));
                }
                Some(Value::Object(nv)) => {
                    rows.push(bar_row(
                        "NVIDIA".into(),
                        nv.get("utilPct").and_then(Value::as_f64).unwrap_or(0.0),
                        OK_GREEN,
                    ));
                    rows.push(metric_row(
                        "NVIDIA temp",
                        fmt_temp(nv.get("tempC").and_then(Value::as_f64)),
                    ));
                }
                _ => rows.push(metric_row("NVIDIA", "—")),
            }
            div().flex().flex_col().gap(4.0_f32).children(rows)
        }),
    )
}

fn sparkline(history: Signal<VecDeque<f32>>, color: u32) -> Div {
    div().h(54.0).w_full().child(
        svg()
            .view_box(0.0, 0.0, 120.0, 36.0)
            .w_full()
            .h_full()
            .child(
                svg::path(move || {
                    let vals = history.get();
                    let mut p = kurbo::BezPath::new();
                    if vals.len() < 2 {
                        p.move_to((0.0, 35.0));
                        p.line_to((119.0, 35.0));
                        return p;
                    }
                    let max = vals.iter().copied().fold(1.0f32, f32::max);
                    let step = 119.0 / (HISTORY - 1) as f64;
                    let x0 = 119.0 - (vals.len() - 1) as f64 * step;
                    for (i, v) in vals.iter().enumerate() {
                        let y = 35.0 - ((*v as f64 / max as f64).clamp(0.0, 1.0) * 33.0);
                        let at = (x0 + i as f64 * step, y);
                        if i == 0 {
                            p.move_to(at);
                        } else {
                            p.line_to(at);
                        }
                    }
                    p
                })
                .fill_none()
                .stroke(Color::from_rgb(color))
                .stroke_width(1.5_f32),
            ),
    )
}

fn net_panel(sample: Signal<Value>, down: Signal<VecDeque<f32>>, up: Signal<VecDeque<f32>>) -> Div {
    panel(
        "NETWORK",
        div()
            .flex()
            .flex_col()
            .gap(6.0_f32)
            .child(sparkline(down, OK_GREEN))
            .child(sparkline(up, WARN_AMBER))
            .child(dyn_view(move || {
                let s = sample.get();
                let mut rows = vec![
                    metric_row("down", fmt_rate(i64_at(&s, &["net", "downBps"]))),
                    metric_row("up", fmt_rate(i64_at(&s, &["net", "upBps"]))),
                ];
                if let Some(ifaces) = obj_at(&s, &["net", "ifaces"]) {
                    for (name, rates) in ifaces.iter().take(6) {
                        rows.push(metric_row(
                            name.clone(),
                            format!(
                                "↓ {}  ↑ {}",
                                fmt_rate(rates.get("downBps").and_then(Value::as_i64).unwrap_or(0)),
                                fmt_rate(rates.get("upBps").and_then(Value::as_i64).unwrap_or(0))
                            ),
                        ));
                    }
                }
                div().flex().flex_col().gap(4.0_f32).children(rows)
            })),
    )
}

fn disk_panel(
    sample: Signal<Value>,
    read_hist: Signal<VecDeque<f32>>,
    write_hist: Signal<VecDeque<f32>>,
) -> Div {
    panel(
        "DISK",
        div()
            .flex()
            .flex_col()
            .gap(6.0_f32)
            .child(sparkline(read_hist, PURPLE))
            .child(sparkline(write_hist, RED))
            .child(dyn_view(move || {
                let s = sample.get();
                let mut rows = vec![
                    metric_row("read", fmt_rate(i64_at(&s, &["disk", "readBps"]))),
                    metric_row("write", fmt_rate(i64_at(&s, &["disk", "writeBps"]))),
                    bar_row("busy".into(), f64_at(&s, &["disk", "utilPct"]), RED),
                ];
                if let Some(disks) = obj_at(&s, &["disk", "disks"]) {
                    for (name, disk) in disks {
                        rows.push(metric_row(
                            name.clone(),
                            format!(
                                "r {}  w {}  {:.1}%",
                                fmt_rate(disk.get("readBps").and_then(Value::as_i64).unwrap_or(0)),
                                fmt_rate(disk.get("writeBps").and_then(Value::as_i64).unwrap_or(0)),
                                disk.get("utilPct").and_then(Value::as_f64).unwrap_or(0.0)
                            ),
                        ));
                    }
                }
                div().flex().flex_col().gap(4.0_f32).children(rows)
            })),
    )
}

fn thermal_panel(sample: Signal<Value>, all: bool) -> Div {
    panel(
        "THERMAL",
        dyn_view(move || {
            let s = sample.get();
            let temps = arr_at(&s, &["temps"]);
            let mut rows: Vec<Div> = Vec::new();
            for temp in temps.iter().take(if all { temps.len() } else { 10 }) {
                let chip = temp.get("chip").and_then(Value::as_str).unwrap_or("");
                let label = temp.get("label").and_then(Value::as_str).unwrap_or("");
                let c = temp.get("c").and_then(Value::as_f64).unwrap_or(0.0);
                rows.push(bar_row(
                    format!("{chip} {label}"),
                    c.clamp(0.0, 100.0),
                    if c >= 80.0 { RED } else { WARN_AMBER },
                ));
            }
            if rows.is_empty() {
                rows.push(metric_row("sensors", "—"));
            }
            div().flex().flex_col().gap(4.0_f32).children(rows)
        }),
    )
}

fn processes_panel(sample: Signal<Value>) -> Div {
    panel(
        "PROCESSES",
        dyn_view(move || {
            let s = sample.get();
            let procs = arr_at(&s, &["procs"]);
            if procs.is_empty() {
                return div().child(
                    text("Process collection is off. Open this satellite to demand procs; if it is already open, wait for the next 2s sample.")
                        .text_size(12.0_f32)
                        .text_color(Color::from_rgb(TEXT_MUTED)),
                );
            }
            let mut rows = vec![metric_row("count", procs.len().to_string())];
            for p in procs.iter().take(48) {
                let comm = p.get("comm").and_then(Value::as_str).unwrap_or("?");
                let pid = p.get("pid").and_then(Value::as_i64).unwrap_or(0);
                let cpu = p.get("cpuPct").and_then(Value::as_f64).unwrap_or(0.0);
                let rss = p.get("rssKb").and_then(Value::as_i64).unwrap_or(0);
                rows.push(metric_row(
                    format!("{comm} ({pid})"),
                    format!("{cpu:.1}%  {}", fmt_kb(rss)),
                ));
            }
            div().flex().flex_col().gap(3.0_f32).children(rows)
        }),
    )
}

type Hist = Signal<VecDeque<f32>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SatKind {
    Cpu,
    Gpu,
    Memory,
    Network,
    Disk,
    Thermal,
    Processes,
    Globe,
}

impl SatKind {
    fn title(self) -> &'static str {
        match self {
            SatKind::Cpu => "MinkaMon — CPU",
            SatKind::Gpu => "MinkaMon — GPU",
            SatKind::Memory => "MinkaMon — Memory",
            SatKind::Network => "MinkaMon — Network",
            SatKind::Disk => "MinkaMon — Disk",
            SatKind::Thermal => "MinkaMon — Thermal",
            SatKind::Processes => "MinkaMon — Processes",
            SatKind::Globe => "MinkaMon — Connections",
        }
    }
}

/// Runs `f` for cleanup-time signal writes. Owner disposal removes signal
/// nodes before running cleanups, so during whole-app teardown a write to a
/// signal whose owner is already gone panics — and no longer matters.
fn cleanup_write(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

fn with_hover(view: Div, hover_count: Signal<i32>) -> Div {
    // Plain cell, not a Signal: on_cleanup must not read reactive state owned
    // by the owner being disposed (it is removed before cleanups run).
    let here = Rc::new(Cell::new(false));
    let here_cleanup = Rc::clone(&here);
    on_cleanup(move || {
        if here_cleanup.get() {
            cleanup_write(move || hover_count.update(|n| *n = (*n - 1).max(0)));
        }
    });
    view.on_hover(move |on| {
        if here.get() == on {
            return;
        }
        here.set(on);
        hover_count.update(|n| *n = (*n + if on { 1 } else { -1 }).max(0));
    })
}

#[allow(clippy::too_many_arguments)]
fn satellite_root(
    kind: SatKind,
    open_flag: Signal<bool>,
    sample: Signal<Value>,
    meta: Signal<Value>,
    cpu_hist: Hist,
    mem_hist: Hist,
    down_hist: Hist,
    up_hist: Hist,
    disk_read_hist: Hist,
    disk_write_hist: Hist,
    hover_count: Signal<i32>,
    open_sats: Signal<i32>,
    procs_open: Signal<bool>,
    globe_open: Signal<bool>,
) -> Div {
    open_flag.set(true);
    open_sats.update(|n| *n += 1);
    if kind == SatKind::Processes {
        procs_open.set(true);
    }
    if kind == SatKind::Globe {
        globe_open.set(true);
    }
    on_cleanup(move || {
        cleanup_write(move || {
            open_flag.set(false);
            open_sats.update(|n| *n = (*n - 1).max(0));
            if kind == SatKind::Processes {
                procs_open.set(false);
            }
            if kind == SatKind::Globe {
                globe_open.set(false);
            }
        });
    });

    let body = match kind {
        SatKind::Cpu => cpu_panel(sample, cpu_hist),
        SatKind::Gpu => gpu_panel(sample),
        SatKind::Memory => mem_panel(sample, mem_hist),
        SatKind::Network => net_panel(sample, down_hist, up_hist),
        SatKind::Disk => disk_panel(sample, disk_read_hist, disk_write_hist),
        SatKind::Thermal => thermal_panel(sample, true),
        SatKind::Processes => processes_panel(sample),
        SatKind::Globe => globe_panel(sample, meta),
    };

    with_hover(
        div()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .bg(Color::from_rgb(GROUND))
            .text_color(Color::from_rgb(TEXT))
            .p(12.0_f32)
            .child(body),
        hover_count,
    )
}

fn globe_panel(sample: Signal<Value>, meta: Signal<Value>) -> Div {
    panel(
        "CONNECTIONS",
        div()
            .flex()
            .flex_col()
            .gap(8.0_f32)
            .child(dyn_view(move || {
                let m = meta.get();
                let coastlines = arr_at(&m, &["coastlines"]);
                let mut map = svg()
                    .view_box(-180.0, -90.0, 360.0, 180.0)
                    .w_full()
                    .h(260.0);
                for line in coastlines.iter().take(134) {
                    let Some(coords) = line.as_array() else {
                        continue;
                    };
                    let mut points: Vec<(f64, f64)> = Vec::new();
                    let mut i = 0;
                    while i + 1 < coords.len() {
                        let lon = coords[i].as_f64().unwrap_or(0.0);
                        let lat = coords[i + 1].as_f64().unwrap_or(0.0);
                        points.push((lon, -lat));
                        i += 4; // decimate: the source is already low-resolution
                    }
                    if points.len() > 1 {
                        map = map.child(
                            svg::polyline(points)
                                .fill_none()
                                .stroke(Color::from_rgb(TEXT_FAINT))
                                .stroke_width(0.7_f32),
                        );
                    }
                }
                let s = sample.get();
                for c in arr_at(&s, &["conns"]) {
                    let (Some(lat), Some(lon)) = (
                        c.get("lat").and_then(Value::as_f64),
                        c.get("lon").and_then(Value::as_f64),
                    ) else {
                        continue;
                    };
                    map = map.child(svg::circle(lon, -lat, 2.2).fill(Color::from_rgb(RED)));
                }
                div().child(map)
            }))
            .child(dyn_view(move || {
                let s = sample.get();
                let mut rows: Vec<Div> = Vec::new();
                for c in arr_at(&s, &["conns"]).iter().take(32) {
                    rows.push(metric_row(
                        c.get("country")
                            .and_then(Value::as_str)
                            .unwrap_or("??")
                            .to_owned(),
                        format!(
                            "{}:{}",
                            c.get("ip").and_then(Value::as_str).unwrap_or("?"),
                            c.get("port").and_then(Value::as_i64).unwrap_or(0)
                        ),
                    ));
                }
                if rows.is_empty() {
                    rows.push(metric_row("established", "—"));
                }
                div().flex().flex_col().gap(3.0_f32).children(rows)
            })),
    )
}

fn app_root() -> Div {
    let sample: Signal<Value> = signal(Value::Null);
    let meta: Signal<Value> = signal(Value::Null);
    let status: Signal<String> = signal(String::new());
    let cpu_hist: Hist = signal(VecDeque::new());
    let mem_hist: Hist = signal(VecDeque::new());
    let down_hist: Hist = signal(VecDeque::new());
    let up_hist: Hist = signal(VecDeque::new());
    let disk_read_hist: Hist = signal(VecDeque::new());
    let disk_write_hist: Hist = signal(VecDeque::new());

    let hover_count = signal(0i32);
    let open_sats = signal(0i32);
    let procs_open = signal(false);
    let globe_open = signal(false);

    let process = Process::new(sampler_bin())
        .arg(assets_dir())
        .stdin(true)
        .spawn();
    process.on_line(move |line| {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if let Some(m) = value.get("meta") {
            meta.set(m.clone());
            return;
        }
        push_history(cpu_hist, f64_at(&value, &["cpu", "total"]));
        let used = i64_at(&value, &["mem", "usedKb"]) as f64;
        let total = i64_at(&value, &["mem", "totalKb"]).max(1) as f64;
        push_history(mem_hist, used / total * 100.0);
        push_history(down_hist, i64_at(&value, &["net", "downBps"]) as f64);
        push_history(up_hist, i64_at(&value, &["net", "upBps"]) as f64);
        push_history(disk_read_hist, i64_at(&value, &["disk", "readBps"]) as f64);
        push_history(
            disk_write_hist,
            i64_at(&value, &["disk", "writeBps"]) as f64,
        );
        sample.set(value);
    });
    process.on_exit(move |code| {
        status.set(match code {
            Some(c) => format!("(exit {c})"),
            None => "(spawn failed or killed)".into(),
        });
    });

    effect(move || {
        let interactive = hover_count.get() > 0 || open_sats.get() > 0;
        let tick = if interactive { 1000 } else { 5000 };
        let procs = procs_open.get();
        let conns = globe_open.get();
        let _ = process.write(&format!(
            "{{\"tickMs\":{tick},\"procs\":{procs},\"conns\":{conns}}}\n"
        ));
    });

    let sat_button = move |label: &'static str, kind: SatKind, flag: Signal<bool>| {
        button(label).on_click(move |_| {
            if flag.get_untracked() {
                return;
            }
            open_window(
                WindowOptions::new().title(kind.title()).size(780, 780),
                move || {
                    satellite_root(
                        kind,
                        flag,
                        sample,
                        meta,
                        cpu_hist,
                        mem_hist,
                        down_hist,
                        up_hist,
                        disk_read_hist,
                        disk_write_hist,
                        hover_count,
                        open_sats,
                        procs_open,
                        globe_open,
                    )
                },
            );
        })
    };

    let cpu_open = signal(false);
    let gpu_open = signal(false);
    let mem_open = signal(false);
    let net_open = signal(false);
    let disk_open = signal(false);
    let thermal_open = signal(false);
    let procs_sat_open = signal(false);
    let globe_sat_open = signal(false);

    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(16.0_f32)
        .child(
            text("MinkaMon")
                .text_size(22.0_f32)
                .font_weight(FontWeight::BOLD)
                .text_color(Color::from_rgb(TEXT)),
        )
        .child(
            text(move || {
                if process.running() {
                    "● sampler live".to_owned()
                } else {
                    format!("○ sampler stopped {}", status.get())
                }
            })
            .text_size(12.0_f32)
            .text_color(Color::from_rgb(TEXT_MUTED)),
        )
        .child(
            text(move || {
                let interactive = hover_count.get() > 0 || open_sats.get() > 0;
                if interactive {
                    "sampling 1s (interactive)".to_owned()
                } else {
                    "sampling 5s (eco — hover for 1s)".to_owned()
                }
            })
            .text_size(12.0_f32)
            .text_color(Color::from_rgb(TEXT_MUTED)),
        );

    let buttons = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(8.0_f32)
        .child(sat_button("CPU details", SatKind::Cpu, cpu_open))
        .child(sat_button("GPU details", SatKind::Gpu, gpu_open))
        .child(sat_button("Memory details", SatKind::Memory, mem_open))
        .child(sat_button("Network details", SatKind::Network, net_open))
        .child(sat_button("Disk details", SatKind::Disk, disk_open))
        .child(sat_button(
            "Thermal details",
            SatKind::Thermal,
            thermal_open,
        ))
        .child(sat_button("Processes", SatKind::Processes, procs_sat_open))
        .child(sat_button("Connections", SatKind::Globe, globe_sat_open));

    // AntiWebUI has no grid layout yet; a wrapping flex row with the panels'
    // min-width gives the same multi-column flow.
    let grid = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(12.0_f32)
        .child(cpu_panel(sample, cpu_hist))
        .child(mem_panel(sample, mem_hist))
        .child(gpu_panel(sample))
        .child(net_panel(sample, down_hist, up_hist))
        .child(disk_panel(sample, disk_read_hist, disk_write_hist))
        .child(thermal_panel(sample, false));

    with_hover(
        div()
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .bg(Color::from_rgb(GROUND))
            .text_color(Color::from_rgb(TEXT))
            .p(14.0_f32)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(12.0_f32)
                    .child(header)
                    .child(buttons)
                    .child(grid),
            ),
        hover_count,
    )
}

fn main() {
    App::new()
        .title("MinkaMon")
        .size(1400, 900)
        .run(app_root)
        .unwrap();
}
