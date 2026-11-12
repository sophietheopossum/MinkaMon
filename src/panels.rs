//! Satellite instrument windows and their panels.

use crate::model::*;
use crate::state::{AppState, cleanup_write};
use crate::theme::*;
use crate::widgets::*;
use anti_webui::prelude::svg::kurbo::BezPath;
use anti_webui::prelude::*;
use serde_json::Value;

pub fn satellite_root(state: AppState, kind: WindowKind) -> Div {
    let cleanup_state = state.clone();
    on_cleanup(move || {
        cleanup_write(move || cleanup_state.mark_window_closed(kind));
    });

    let (min_w, min_h) = kind.min_size();
    let content = match kind {
        WindowKind::Cpu => cpu_panel(state.clone()),
        WindowKind::Gpu => gpu_panel(state.clone()),
        WindowKind::Memory => memory_panel(state.clone()),
        WindowKind::Network => network_panel(state.clone()),
        WindowKind::Disk => disk_panel(state.clone()),
        WindowKind::Globe => crate::globe::globe_panel(state.clone()),
        WindowKind::Processes => crate::processes::processes_panel(state.clone()),
        WindowKind::Temp(temp) => temp_panel(state.clone(), temp),
    };

    let root = div()
        .w_full()
        .h_full()
        .min_w(min_w as f32)
        .min_h(min_h as f32)
        .overflow_hidden()
        .bg(color(GROUND))
        .p(10.0_f32)
        .child(content);
    with_hover(root, state.hover_count)
}

fn cpu_panel(state: AppState) -> Div {
    let title_state = state.clone();
    let title = panel_title(move || {
        title_state.sample.with(|sample| {
            let total = f64_at(sample, &["cpu", "total"]);
            match opt_f64_at(sample, &["cpu", "packageC"]) {
                Some(pkg) => format!("CPU · {total:.0}% · {pkg:.0}°C"),
                None => format!("CPU · {total:.0}%"),
            }
        })
    });

    let legend_state = state.clone();
    let header = div().child(dyn_view(move || {
        let count = legend_state
            .sample
            .with(|sample| arr_at(sample, &["cpu", "cores"]).len());
        let mut row = div().flex().flex_row().gap(4.0_f32);
        for i in 0..count {
            row = row.child(mono_text(i.to_string(), FONT_SIZE - 4.0, series_color(i)));
        }
        row
    }));

    let content_state = state.clone();
    let content = dyn_view(move || {
        let has_temps = content_state.sample.with(|sample| {
            arr_at(sample, &["cpu", "coreTemps"])
                .iter()
                .any(|value| !value.is_null())
        });
        let load = chart_box(
            "LOAD",
            content_state
                .histories
                .with(|histories| multi_trend(&histories.cpu_cores, 100.0)),
        );
        let mut column = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .gap(10.0_f32)
            .child(load);
        if has_temps {
            column = column.child(chart_box(
                "TEMP",
                content_state
                    .histories
                    .with(|histories| multi_trend(&histories.cpu_core_temps, 100.0)),
            ));
        }
        column
    });

    panel_frame(title, Some(header), content, false)
}

fn engine_display_name(key: &str) -> String {
    match key {
        "rcs" => "RENDER".to_owned(),
        "ccs" => "COMPUTE".to_owned(),
        "bcs" => "COPY".to_owned(),
        "vcs" => "VIDEO".to_owned(),
        "vecs" => "ENHANCE".to_owned(),
        other => other.to_uppercase(),
    }
}

fn xe_title(sample: &Value) -> String {
    let Some(xe) = at(sample, &["gpu", "xe"]).filter(|v| v.is_object()) else {
        return "IRIS XE".to_owned();
    };
    let Some(freq) = opt_f64_at(xe, &["freqMhz"]) else {
        return "IRIS XE".to_owned();
    };
    if freq > 0.0 {
        format!("IRIS XE · {freq:.0} MHz")
    } else {
        format!(
            "IRIS XE · {:.0} MHz · GATED",
            opt_f64_at(xe, &["curFreqMhz"]).unwrap_or(0.0)
        )
    }
}

fn mx450_line(sample: &Value) -> (String, bool) {
    let Some(nv) = at(sample, &["gpu", "nvidia"]).filter(|v| v.is_object()) else {
        return ("MX450 · UNAVAILABLE".to_owned(), false);
    };
    if nv.get("asleep").and_then(Value::as_bool).unwrap_or(true) {
        return ("MX450 · DORMANT".to_owned(), false);
    }
    let mut line = format!(
        "MX450 · {:.0}% · {:.0}°C · {:.0}/{:.0} MB",
        f64_at(nv, &["utilPct"]),
        f64_at(nv, &["tempC"]),
        f64_at(nv, &["memUsedMb"]),
        f64_at(nv, &["memTotalMb"])
    );
    if let Some(power) = opt_f64_at(nv, &["powerW"]) {
        line.push_str(&format!(" · {power:.1} W"));
    }
    (line, true)
}

fn gpu_panel(state: AppState) -> Div {
    let content_state = state.clone();
    let content = dyn_view(move || {
        content_state.sample.with(|sample| {
            content_state.meta.with(|meta| {
                let has_nvidia = bool_at(meta, &["hasNvidia"]);
                let (mx_line, mx_awake) = mx450_line(sample);

                let mut legend = div().flex().flex_row().gap(6.0_f32);
                for (index, key) in content_state
                    .histories
                    .with(|histories| histories.engine_keys.clone())
                    .iter()
                    .enumerate()
                {
                    legend = legend.child(mono_text(
                        engine_display_name(key),
                        FONT_SIZE - 4.0,
                        series_color(index),
                    ));
                }

                let mut column = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h_full()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .child(mono_label(xe_title(sample), FONT_SIZE - 2.0, TEXT_MUTED))
                            .child(div().flex_1())
                            .child(legend),
                    )
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .pt(6.0_f32)
                            .pb(if has_nvidia { 10.0_f32 } else { 0.0_f32 })
                            .child(chart_box(
                                "ENGINES",
                                content_state
                                    .histories
                                    .with(|histories| multi_trend(&histories.engine_series, 100.0)),
                            )),
                    );

                if has_nvidia {
                    column = column
                        .child(hrule())
                        .child(div().pt(6.0_f32).child(mono_label(
                            mx_line,
                            FONT_SIZE - 2.0,
                            if mx_awake { TEXT_MUTED } else { TEXT_FAINT },
                        )));
                    if mx_awake {
                        column = column.child(
                            div().h(44.0).w_full().pt(6.0_f32).child(
                                chart_box(
                                    "MX450",
                                    content_state.histories.with(|histories| {
                                        sparkline(
                                            &histories.nvidia_util,
                                            100.0,
                                            PURPLE,
                                            Some(NVIDIA_FILL),
                                        )
                                    }),
                                )
                                .h_full(),
                            ),
                        );
                    }
                }
                column
            })
        })
    });

    panel_frame(panel_title("GPU"), None, content, false)
}

struct ZoneInfo {
    name: String,
    start_kb: u64,
    managed_kb: u64,
    present_pages: i64,
    managed_pages: i64,
    free_pages: i64,
}

fn shuffled(count: usize, seed: u32) -> Vec<usize> {
    let mut s = seed;
    let mut rand = move || {
        s = s.wrapping_add(0x6D2B_79F5);
        let mut t = (s ^ (s >> 15)).wrapping_mul(1 | s);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4_294_967_296.0
    };
    let mut perm: Vec<usize> = (0..count).collect();
    for i in (1..count).rev() {
        let j = (rand() * (i + 1) as f64).floor() as usize;
        perm.swap(i, j.min(i));
    }
    perm
}

fn push_cell(path: &mut BezPath, x: f64, y: f64, w: f64, h: f64) {
    path.move_to((x, y));
    path.line_to((x + w, y));
    path.line_to((x + w, y + h));
    path.line_to((x, y + h));
    path.close_path();
}

fn memory_grid(mem: &Value, zones: &[ZoneInfo]) -> Svg {
    const CELLS: usize = 440;
    let mut used = BezPath::new();
    let mut cache = BezPath::new();
    let mut free_even = BezPath::new();
    let mut free_odd = BezPath::new();

    let total_present: i64 = zones.iter().map(|z| z.present_pages).sum();
    if total_present > 0 {
        let mut counts: Vec<usize> = zones
            .iter()
            .map(|z| {
                ((CELLS as f64 * z.present_pages as f64 / total_present as f64).round() as usize)
                    .max(4)
            })
            .collect();
        let biggest = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| **count)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let sum: usize = counts.iter().sum();
        counts[biggest] = counts[biggest].saturating_add(CELLS).saturating_sub(sum);

        let total_kb = i64_at(mem, &["totalKb"]);
        let free_kb = i64_at(mem, &["freeKb"]);
        let cache_kb = i64_at(mem, &["cacheKb"]);
        let cache_frac = if total_kb > free_kb {
            cache_kb as f64 / (total_kb - free_kb) as f64
        } else {
            0.0
        };

        let mut states = vec![0u8; CELLS];
        let mut base = 0usize;
        for (index, zone) in zones.iter().enumerate() {
            let count = counts[index];
            let parity = if index % 2 == 1 { 4 } else { 0 };
            for state in states.iter_mut().skip(base).take(count) {
                *state = parity;
            }
            if zone.managed_pages > 0 {
                let used_cells = (count as f64 * (zone.managed_pages - zone.free_pages) as f64
                    / zone.managed_pages as f64)
                    .round()
                    .clamp(0.0, count as f64) as usize;
                let cache_cells = (used_cells as f64 * cache_frac)
                    .round()
                    .clamp(0.0, used_cells as f64) as usize;
                let perm = shuffled(count, zone.start_kb.wrapping_add(1) as u32);
                for k in 0..used_cells {
                    states[base + perm[k]] =
                        (if k < used_cells - cache_cells { 1 } else { 2 }) + parity;
                }
            }
            base += count;
        }

        for (index, code) in states.into_iter().enumerate() {
            let x = (index % 44) as f64 * 12.0;
            let y = (index / 44) as f64 * 9.0;
            match code & 3 {
                1 => push_cell(&mut used, x, y, 10.0, 7.0),
                2 => push_cell(&mut cache, x, y, 10.0, 7.0),
                _ if code & 4 != 0 => push_cell(&mut free_odd, x, y, 10.0, 7.0),
                _ => push_cell(&mut free_even, x, y, 10.0, 7.0),
            }
        }
    }

    svg()
        .view_box(0.0, 0.0, 526.0, 88.0)
        .aspect(svg::SvgAspect {
            align: None,
            slice: false,
        })
        .w_full()
        .h_full()
        .child(svg::path(free_even).fill(color(SURFACE_RAISED)))
        .child(svg::path(free_odd).fill(color(MEMORY_FREE_ODD)))
        .child(svg::path(cache).fill(color(GAUGE_DIM)))
        .child(svg::path(used).fill(color(RED)))
}

fn memory_readout(label: &'static str, value: String, tint: u32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(1.0_f32)
        .child(faint_label(label))
        .child(mono_text(value, FONT_SIZE, tint))
}

fn memory_panel(state: AppState) -> Div {
    let content_state = state.clone();
    let content = dyn_view(move || {
        content_state.sample.with(|sample| {
            let mem = at(sample, &["mem"]).cloned().unwrap_or(Value::Null);
            let zones: Vec<ZoneInfo> = arr_at(&mem, &["zones"])
                .iter()
                .map(|z| ZoneInfo {
                    name: str_at(z, &["name"]).to_owned(),
                    start_kb: u64_at(z, &["startKb"]),
                    managed_kb: u64_at(z, &["managedKb"]),
                    present_pages: i64_at(z, &["presentPages"]),
                    managed_pages: i64_at(z, &["managedPages"]),
                    free_pages: i64_at(z, &["freePages"]),
                })
                .collect();
            let zone_line = zones
                .iter()
                .map(|z| {
                    format!(
                        "{} @{} {}",
                        z.name.to_uppercase(),
                        fmt_addr(z.start_kb),
                        fmt_addr(z.managed_kb)
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");

            let mut column = div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .gap(8.0_f32)
                .child(div().h(88.0).w_full().child(memory_grid(&mem, &zones)))
                .child(mono_label(zone_line, FONT_SIZE - 4.0, TEXT_FAINT))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(13.0_f32)
                        .child(memory_readout(
                            "USED",
                            fmt_kb(i64_at(&mem, &["usedKb"])),
                            RED,
                        ))
                        .child(memory_readout(
                            "CACHE",
                            fmt_kb(i64_at(&mem, &["cacheKb"])),
                            TEXT_MUTED,
                        ))
                        .child(memory_readout(
                            "FREE",
                            fmt_kb(i64_at(&mem, &["availableKb"])),
                            OK_GREEN,
                        ))
                        .child(memory_readout(
                            "TOTAL",
                            fmt_kb(i64_at(&mem, &["totalKb"])),
                            TEXT_FAINT,
                        )),
                );

            if i64_at(&mem, &["swapTotalKb"]) > 0 {
                column = column.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(8.0_f32)
                        .child(faint_label("SWAP"))
                        .child(div().w(140.0).h(14.0).child(content_state.histories.with(
                            |histories| {
                                sparkline(&histories.swap_pct, 100.0, WARN_AMBER, Some(GAUGE_DIM))
                            },
                        )))
                        .child(mono_text(
                            format!(
                                "{} / {}",
                                fmt_kb(i64_at(&mem, &["swapUsedKb"])),
                                fmt_kb(i64_at(&mem, &["swapTotalKb"]))
                            ),
                            FONT_SIZE - 3.0,
                            TEXT_MUTED,
                        )),
                );
            }
            column
        })
    });

    panel_frame(panel_title("MEMORY"), None, content, false)
}

fn network_panel(state: AppState) -> Div {
    let content_state = state.clone();
    let content = dyn_view(move || {
        content_state.sample.with(|sample| {
            let peak = content_state.histories.with(|histories| {
                histories
                    .down
                    .iter()
                    .chain(histories.up.iter())
                    .copied()
                    .fold(1.0_f32, f32::max)
            });
            let chart = content_state.histories.with(|histories| {
                overlay_sparklines(
                    &histories.down,
                    RED,
                    Some(GAUGE_DIM),
                    &histories.up,
                    PURPLE,
                    peak,
                )
            });

            let mut list = div().flex().flex_col().gap(2.0_f32);
            if let Some(ifaces) = obj_at(sample, &["net", "ifaces"]) {
                for (name, rates) in ifaces {
                    list = list.child(mono_text(
                        format!(
                            "{}  ▼ {}  ▲ {}",
                            name,
                            fmt_rate(rates.get("downBps").and_then(Value::as_i64).unwrap_or(0)),
                            fmt_rate(rates.get("upBps").and_then(Value::as_i64).unwrap_or(0))
                        ),
                        FONT_SIZE - 3.0,
                        TEXT_FAINT,
                    ));
                }
            }

            div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(20.0_f32)
                        .child(rate_readout(
                            "▼ DOWN",
                            fmt_rate(i64_at(sample, &["net", "downBps"])),
                            RED,
                        ))
                        .child(rate_readout(
                            "▲ UP",
                            fmt_rate(i64_at(sample, &["net", "upBps"])),
                            PURPLE,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .py(6.0_f32)
                        .child(chart_box("TRAFFIC", chart).h_full()),
                )
                .child(list)
        })
    });

    panel_frame(panel_title("NETWORK"), None, content, false)
}

fn disk_panel(state: AppState) -> Div {
    let content_state = state.clone();
    let content = dyn_view(move || {
        content_state.sample.with(|sample| {
            let peak = content_state.histories.with(|histories| {
                histories
                    .disk_read
                    .iter()
                    .chain(histories.disk_write.iter())
                    .copied()
                    .fold(1.0_f32, f32::max)
            });
            let chart = content_state.histories.with(|histories| {
                let base = overlay_sparklines(
                    &histories.disk_read,
                    RED,
                    Some(GAUGE_DIM),
                    &histories.disk_write,
                    PURPLE,
                    peak,
                );
                add_series(base, &histories.disk_util, 100.0, WARN_AMBER)
            });

            let mut list = div().flex().flex_col().gap(2.0_f32);
            if let Some(disks) = obj_at(sample, &["disk", "disks"]) {
                for (name, disk) in disks {
                    list = list.child(mono_text(
                        format!(
                            "{}  ◂ {}  ▸ {}  · {:.1}% util",
                            name,
                            fmt_rate(disk.get("readBps").and_then(Value::as_i64).unwrap_or(0)),
                            fmt_rate(disk.get("writeBps").and_then(Value::as_i64).unwrap_or(0)),
                            disk.get("utilPct").and_then(Value::as_f64).unwrap_or(0.0)
                        ),
                        FONT_SIZE - 3.0,
                        TEXT_FAINT,
                    ));
                }
            }

            div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(20.0_f32)
                        .child(rate_readout(
                            "◂ READ",
                            fmt_rate(i64_at(sample, &["disk", "readBps"])),
                            RED,
                        ))
                        .child(rate_readout(
                            "▸ WRITE",
                            fmt_rate(i64_at(sample, &["disk", "writeBps"])),
                            PURPLE,
                        ))
                        .child(rate_readout(
                            "◆ UTILISATION",
                            format!("{:.1}%", f64_at(sample, &["disk", "utilPct"])),
                            WARN_AMBER,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .py(6.0_f32)
                        .child(chart_box("THROUGHPUT", chart).h_full()),
                )
                .child(list)
        })
    });

    panel_frame(panel_title("DISK"), None, content, false)
}

fn temp_panel(state: AppState, kind: TempKind) -> Div {
    let header_state = state.clone();
    let header_style_state = state.clone();
    let header = div().child(
        mono_text(
            move || {
                fmt_temp1(header_state.sample.with(|sample| {
                    header_state
                        .meta
                        .with(|meta| temp_readings(sample, meta).get(kind))
                }))
            },
            FONT_SIZE - 1.0,
            TEXT_MUTED,
        )
        .dyn_style(move |s| {
            let current = header_style_state.sample.with(|sample| {
                header_style_state
                    .meta
                    .with(|meta| temp_readings(sample, meta).get(kind))
            });
            s.text_color(color(temp_tint(current)))
        }),
    );

    let content_state = state.clone();
    let content = dyn_view(move || {
        div()
            .relative()
            .w_full()
            .h_full()
            .overflow_hidden()
            .bg(color(SURFACE_RAISED))
            .child(
                content_state
                    .histories
                    .with(|histories| sparkline(histories.temp(kind), 100.0, RED, Some(GAUGE_DIM)))
                    .absolute()
                    .inset(1.0),
            )
            .child(div().absolute().left(3.0).top(3.0).child(mono_label(
                kind.label(),
                FONT_SIZE - 4.0,
                TEXT_FAINT,
            )))
    });

    panel_frame(panel_title(kind.label()), Some(header), content, false)
}
