//! Main window body and the machine schematic.

use crate::model::*;
use crate::state::AppState;
use crate::theme::*;
use crate::widgets::{panel_frame, panel_title, with_hover};
use anti_webui::prelude::svg::kurbo::{BezPath, Point};
use anti_webui::prelude::*;
use serde_json::Value;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct BoardGeom {
    bsx: f64,
    bsy: f64,
    bs: f64,
    bx: f64,
    by: f64,
}

impl BoardGeom {
    fn new(width: f64, height: f64, zenbook: bool) -> Self {
        let (board_w, board_h) = if zenbook {
            (324.0, 222.0)
        } else {
            (440.0, 400.0)
        };
        let bsx = ((width - 12.0) / board_w).max(0.05);
        let bsy = ((height - 4.0) / board_h).max(0.05);
        Self {
            bsx,
            bsy,
            bs: bsx.min(bsy),
            bx: (width - board_w * bsx) / 2.0,
            by: (height - board_h * bsy) / 2.0,
        }
    }

    fn x(&self, u: f64) -> f64 {
        self.bx + u * self.bsx
    }

    fn y(&self, v: f64) -> f64 {
        self.by + v * self.bsy
    }

    fn sx(&self, u: f64) -> f64 {
        u * self.bsx
    }

    fn sy(&self, v: f64) -> f64 {
        v * self.bsy
    }

    fn s(&self, u: f64) -> f64 {
        u * self.bs
    }
}

#[derive(Clone, Copy)]
struct ZoneDef {
    zone: Zone,
    info_label: Option<&'static str>,
    temp_label: Option<&'static str>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    chip_below: bool,
}

#[allow(clippy::too_many_arguments)]
const fn zone(
    zone: Zone,
    info_label: Option<&'static str>,
    temp_label: Option<&'static str>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    chip_below: bool,
) -> ZoneDef {
    ZoneDef {
        zone,
        info_label,
        temp_label,
        x,
        y,
        w,
        h,
        chip_below,
    }
}

const ZENBOOK_ZONES: [ZoneDef; 6] = [
    zone(
        Zone::Board,
        None,
        Some("BOARD °C"),
        18.0,
        12.0,
        288.0,
        74.0,
        true,
    ),
    zone(
        Zone::Cpu,
        Some("CPU"),
        Some("CPU °C"),
        124.0,
        29.0,
        32.0,
        32.0,
        false,
    ),
    zone(
        Zone::Gpu,
        Some("GPU"),
        Some("GPU °C"),
        179.0,
        37.0,
        26.0,
        26.0,
        false,
    ),
    zone(
        Zone::Ram,
        Some("MEMORY"),
        None,
        99.0,
        57.0,
        24.0,
        31.0,
        false,
    ),
    zone(
        Zone::Wifi,
        Some("NETWORK"),
        Some("WIFI °C"),
        237.0,
        62.0,
        26.0,
        24.0,
        false,
    ),
    zone(
        Zone::Ssd,
        Some("DISK"),
        Some("SSD °C"),
        30.0,
        90.0,
        90.0,
        22.0,
        false,
    ),
];

const TOWER_ZONES: [ZoneDef; 6] = [
    zone(
        Zone::Board,
        None,
        Some("BOARD °C"),
        170.0,
        40.0,
        240.0,
        250.0,
        true,
    ),
    zone(
        Zone::Cpu,
        Some("CPU"),
        Some("CPU °C"),
        225.0,
        85.0,
        54.0,
        54.0,
        false,
    ),
    zone(
        Zone::Ram,
        Some("MEMORY"),
        None,
        305.0,
        57.0,
        65.0,
        128.0,
        false,
    ),
    zone(
        Zone::Gpu,
        Some("GPU"),
        Some("GPU °C"),
        145.0,
        212.0,
        230.0,
        46.0,
        false,
    ),
    zone(
        Zone::Ssd,
        Some("DISK"),
        Some("SSD °C"),
        195.0,
        263.0,
        90.0,
        20.0,
        false,
    ),
    zone(
        Zone::Wifi,
        Some("NETWORK"),
        Some("WIFI °C"),
        383.0,
        294.0,
        26.0,
        22.0,
        false,
    ),
];

fn zone_defs(zenbook: bool) -> &'static [ZoneDef] {
    if zenbook {
        &ZENBOOK_ZONES
    } else {
        &TOWER_ZONES
    }
}

/// Window-local scene centre of a schematic zone, for leader lines. The
/// layout constants mirror `main_body` below plus the shared Panel frame.
pub fn anchor_scene(zone: Zone, zenbook: bool, client_w: f64, client_h: f64) -> Option<(f64, f64)> {
    let def = zone_defs(zenbook).iter().find(|d| d.zone == zone)?;
    let map_x = 22.0;
    let map_y = 71.0;
    let geom = BoardGeom::new(
        (client_w - 44.0).max(1.0),
        (client_h - 75.0).max(1.0),
        zenbook,
    );
    Some((
        map_x + geom.x(def.x + def.w / 2.0),
        map_y + geom.y(def.y + def.h / 2.0),
    ))
}

fn chip_shell(
    label: &'static str,
    active: Rc<dyn Fn() -> bool>,
    on_click: impl Fn(&PointerEvent) + 'static,
) -> Div {
    let active_style = Rc::clone(&active);
    let active_text = Rc::clone(&active);
    div()
        .h(28.0)
        .px(13.0_f32)
        .rounded(5.0_f32)
        .border(1.0_f32)
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorIcon::Pointer)
        .dyn_style(move |s| {
            if active_style() {
                s.bg(color(SURFACE_RAISED)).border_color(color(RED))
            } else {
                s.bg(Color::TRANSPARENT).border_color(color(LINE))
            }
        })
        .on_click(on_click)
        .child(
            mono_text(label, FONT_SIZE - 1.0, TEXT_MUTED)
                .letter_spacing(2.0_f32)
                .dyn_style(move |s| {
                    s.text_color(if active_text() {
                        color(TEXT)
                    } else {
                        color(TEXT_MUTED)
                    })
                }),
        )
}

fn toggle_chip(label: &'static str, value: Signal<bool>) -> Div {
    chip_shell(label, Rc::new(move || value.get()), move |_| {
        value.set(!value.get_untracked())
    })
}

fn top_bar(state: AppState) -> Div {
    let overview_state = state.clone();
    let close_state = state.clone();
    let process_state = state.clone();
    let process_active_state = state.clone();

    div()
        .relative()
        .h(44.0)
        .w_full()
        .bg(color(SURFACE))
        .flex()
        .flex_row()
        .items_center()
        .px(14.0_f32)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(10.0_f32)
                .child(
                    mono_text("MINKAMON", FONT_SIZE + 2.0, RED)
                        .font_weight(FontWeight::BOLD)
                        .letter_spacing(3.0_f32),
                )
                .child(div().w(8.0).h(8.0).rounded(4.0_f32).dyn_style(move |s| {
                    s.bg(if state.alive.get() {
                        color(OK_GREEN)
                    } else {
                        color(RED_DIM)
                    })
                })),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(6.0_f32)
                .child(toggle_chip("INFO", state.info_mode))
                .child(toggle_chip("TEMP", state.temp_mode))
                .child(chip_shell(
                    "OVERVIEW",
                    Rc::new({
                        let state = overview_state.clone();
                        move || state.overview_pending.get()
                    }),
                    move |_| overview_state.full_overview(),
                ))
                .child(chip_shell("CLOSE ALL", Rc::new(|| false), move |_| {
                    close_state.close_all()
                }))
                .child(chip_shell(
                    "PROCESSES",
                    Rc::new(move || process_active_state.is_open(WindowKind::Processes)),
                    move |_| process_state.toggle_window_kind(WindowKind::Processes),
                )),
        )
        .child(
            div()
                .absolute()
                .left(0.0)
                .right(0.0)
                .bottom(0.0)
                .h(1.0)
                .bg(color(LINE)),
        )
}

pub fn main_body(state: AppState, floating: bool) -> Div {
    if floating {
        on_close_request(|_| quit());
    }
    let root = div()
        .relative()
        .w_full()
        .h_full()
        .min_w(440.0)
        .min_h(420.0)
        .overflow_hidden()
        .bg(color(GROUND))
        .flex()
        .flex_col()
        .child(top_bar(state.clone()))
        .child(
            div()
                .flex_1()
                .w_full()
                .px(10.0_f32)
                .pt(2.0_f32)
                .pb(2.0_f32)
                .child(system_panel(state.clone())),
        );
    with_hover(root, state.hover_count)
}

fn system_panel(state: AppState) -> Div {
    let size: Signal<(f32, f32)> = signal((620.0, 470.0));

    let title_state = state.clone();
    let title = panel_title(move || {
        if title_state.meta.with(machine_is_zenbook) {
            "SYSTEM // UX482EG".to_owned()
        } else {
            "SYSTEM".to_owned()
        }
    });

    let header_state = state.clone();
    let header_style_state = state.clone();
    let header = div().child(
        mono_text(
            move || {
                header_state.sample.with(|sample| {
                    header_state.meta.with(|meta| {
                        temp_readings(sample, meta)
                            .max
                            .map_or_else(String::new, |max| format!("MAX {max:.0}°C"))
                    })
                })
            },
            FONT_SIZE - 4.0,
            TEXT_FAINT,
        )
        .dyn_style(move |s| {
            let max = header_style_state.sample.with(|sample| {
                header_style_state
                    .meta
                    .with(|meta| temp_readings(sample, meta).max)
            });
            s.text_color(color(temp_tint(max)))
        }),
    );

    let art_state = state.clone();
    let zone_state = state.clone();
    let map = div()
        .relative()
        .w_full()
        .h_full()
        .overflow_hidden()
        .on_resized(move |w, h| {
            if w > 0.0 && h > 0.0 {
                let old = size.get_untracked();
                if (old.0 - w).abs() > 0.5 || (old.1 - h).abs() > 0.5 {
                    size.set((w, h));
                }
            }
        })
        .child(dyn_view(move || {
            let (w, h) = size.get();
            art_state.sample.with(|sample| {
                art_state.meta.with(|meta| {
                    schematic_svg(sample, meta, w as f64, h as f64, art_state.alive.get())
                })
            })
        }))
        .child(dyn_view(move || zone_layer(zone_state.clone(), size.get())));

    panel_frame(title, Some(header), map, true)
}

fn zone_chip_text(def: ZoneDef, info: bool, temp: bool) -> String {
    let mut parts = Vec::new();
    if info && let Some(label) = def.info_label {
        parts.push(label);
    }
    if temp && let Some(label) = def.temp_label {
        parts.push(label);
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("▸ {}", parts.join(" + "))
    }
}

fn zone_overlay(state: AppState, def: ZoneDef, chip: String, geom: BoardGeom) -> Div {
    let zone = def.zone;
    let x = geom.x(def.x);
    let y = geom.y(def.y);
    let w = geom.sx(def.w);
    let h = geom.sy(def.h);

    let hover_state = state.clone();
    let click_state = state.clone();
    let border_state = state.clone();
    let chip_state = state.clone();

    let chip_bar = div()
        .absolute()
        .left(0.0)
        .right(0.0)
        .flex()
        .flex_row()
        .justify_center();
    let chip_bar = if def.chip_below {
        chip_bar.top((h + 4.0) as f32)
    } else {
        chip_bar.top(-21.0)
    }
    .dyn_style(move |s| {
        s.opacity(if chip_state.hovered_zone.get() == Some(zone) {
            1.0
        } else {
            0.0_f32
        })
    })
    .child(
        div()
            .h(17.0)
            .px(6.0_f32)
            .bg(color(GROUND))
            .border(1.0_f32)
            .border_color(color(RED))
            .flex()
            .items_center()
            .child(mono_text(chip, FONT_SIZE - 4.0, RED)),
    );

    div()
        .absolute()
        .left(x as f32)
        .top(y as f32)
        .w(w as f32)
        .h(h as f32)
        .cursor(CursorIcon::Pointer)
        .border(1.0_f32)
        .dyn_style(move |s| {
            s.border_color(if border_state.hovered_zone.get() == Some(zone) {
                color(RED)
            } else {
                Color::TRANSPARENT
            })
        })
        .on_hover(move |on| {
            if on {
                hover_state.hovered_zone.set(Some(zone));
            } else if hover_state.hovered_zone.get_untracked() == Some(zone) {
                hover_state.hovered_zone.set(None);
            }
        })
        .on_click(move |_| click_state.open_zone(zone))
        .child(chip_bar)
}

fn zone_layer(state: AppState, size: (f32, f32)) -> Div {
    let zenbook = state.meta.with(machine_is_zenbook);
    let geom = BoardGeom::new(size.0 as f64, size.1 as f64, zenbook);
    let info = state.info_mode.get();
    let temp = state.temp_mode.get();

    let mut layer = div().absolute().inset(0.0);
    for def in zone_defs(zenbook) {
        let chip = zone_chip_text(*def, info, temp);
        if chip.is_empty() {
            continue;
        }
        layer = layer.child(zone_overlay(state.clone(), *def, chip, geom));
    }
    layer
}

fn heat_rgb(c: f64) -> (u8, u8, u8) {
    const STOPS: [(f64, u8, u8, u8); 6] = [
        (30.0, 70, 15, 25),
        (50.0, 143, 30, 45),
        (65.0, 224, 38, 60),
        (78.0, 255, 74, 94),
        (88.0, 224, 160, 38),
        (96.0, 255, 236, 210),
    ];
    if c <= STOPS[0].0 {
        return (STOPS[0].1, STOPS[0].2, STOPS[0].3);
    }
    for i in 1..STOPS.len() {
        if c <= STOPS[i].0 {
            let a = STOPS[i - 1];
            let b = STOPS[i];
            let t = (c - a.0) / (b.0 - a.0);
            let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
            return (mix(a.1, b.1), mix(a.2, b.2), mix(a.3, b.3));
        }
    }
    let last = STOPS[STOPS.len() - 1];
    (last.1, last.2, last.3)
}

fn heat_color(c: f64, alpha: f32) -> Color {
    let (r, g, b) = heat_rgb(c);
    Color::from_rgba(
        ((r as u32) << 24)
            | ((g as u32) << 16)
            | ((b as u32) << 8)
            | (alpha.clamp(0.0, 1.0) * 255.0) as u32,
    )
}

fn blob_shape(cx: f64, cy: f64, radius: f64, temp: Option<f64>) -> Option<View> {
    let c = temp?;
    let alpha = 0.18 + ((c - 30.0) / 60.0).clamp(0.0, 1.0) as f32 * 0.55;
    Some(
        svg::circle(cx, cy, radius)
            .fill(
                Gradient::radial(0.5, 0.5, 0.5)
                    .stop(0.0, heat_color(c, alpha))
                    .stop(1.0, heat_color(c, 0.0)),
            )
            .into_view(),
    )
}

fn stroked_path(path: BezPath, hex: u32, width: f32, alpha: f32) -> View {
    svg::path(path)
        .fill_none()
        .stroke(color_alpha(hex, alpha))
        .stroke_width(width)
        .into_view()
}

fn stroked_rect(x: f64, y: f64, w: f64, h: f64, hex: u32, alpha: f32) -> View {
    svg::rect(x, y, w, h)
        .fill_none()
        .stroke(color_alpha(hex, alpha))
        .stroke_width(1.0_f32)
        .into_view()
}

fn stroked_rounded(x: f64, y: f64, w: f64, h: f64, r: f64, hex: u32, alpha: f32) -> View {
    svg::rounded_rect(x, y, w, h, r)
        .fill_none()
        .stroke(color_alpha(hex, alpha))
        .stroke_width(1.0_f32)
        .into_view()
}

fn filled_rect(x: f64, y: f64, w: f64, h: f64, fill: Color) -> View {
    svg::rect(x, y, w, h).fill(fill).into_view()
}

fn fan_shapes(cx: f64, cy: f64, radius: f64) -> Vec<View> {
    let mut out = vec![
        svg::circle(cx, cy, radius)
            .fill_none()
            .stroke(color(RED_DIM))
            .stroke_width(1.0_f32)
            .into_view(),
        svg::circle(cx, cy, radius * 0.22)
            .fill_none()
            .stroke(color(RED_DIM))
            .stroke_width(1.0_f32)
            .into_view(),
    ];
    for k in 0..5 {
        let a = k as f64 / 5.0 * std::f64::consts::TAU;
        let mut path = BezPath::new();
        path.move_to(Point::new(
            cx + a.cos() * radius * 0.3,
            cy + a.sin() * radius * 0.3,
        ));
        path.quad_to(
            Point::new(
                cx + (a + 0.5).cos() * radius * 0.72,
                cy + (a + 0.5).sin() * radius * 0.72,
            ),
            Point::new(
                cx + (a + 0.9).cos() * radius * 0.95,
                cy + (a + 0.9).sin() * radius * 0.95,
            ),
        );
        out.push(stroked_path(path, RED_DIM, 1.0, 1.0));
    }
    out
}

#[derive(Clone, Copy)]
enum TagAlign {
    Left,
    Right,
}

fn tag_shapes(
    x: f64,
    y: f64,
    name: &str,
    temp: Option<f64>,
    fallback: Option<&str>,
    align: TagAlign,
) -> Vec<View> {
    let value = temp.map_or_else(
        || fallback.unwrap_or("—").to_owned(),
        |c| format!("{c:.0}°"),
    );
    let char_w = 5.4;
    let name_with_space = format!("{name} ");
    let name_w = name_with_space.chars().count() as f64 * char_w;
    let value_w = value.chars().count() as f64 * char_w;
    let sx = match align {
        TagAlign::Left => x,
        TagAlign::Right => x - name_w - value_w,
    };
    vec![
        svg::text(name_with_space)
            .at(sx as f32, y as f32)
            .anchor(svg::TextAnchor::Start)
            .font_family(MONO_FAMILY)
            .text_size(9.0_f32)
            .fill(color(TEXT_FAINT))
            .into_view(),
        svg::text(value)
            .at((sx + name_w) as f32, y as f32)
            .anchor(svg::TextAnchor::Start)
            .font_family(MONO_FAMILY)
            .text_size(9.0_f32)
            .fill(color(temp_tint(temp)))
            .into_view(),
    ]
}

fn polyline_shape(points: &[(f64, f64)], hex: u32, width: f32, alpha: f32) -> View {
    svg::polyline(points.iter().copied())
        .fill_none()
        .stroke(color_alpha(hex, alpha))
        .stroke_width(width)
        .into_view()
}

fn zenbook_blobs(g: &BoardGeom, r: &TempReadings) -> View {
    let blobs = [
        blob_shape(g.x(165.0), g.y(62.0), g.s(62.0), r.board),
        blob_shape(g.x(140.0), g.y(45.0), g.s(52.0), r.pkg),
        blob_shape(g.x(192.0), g.y(50.0), g.s(40.0), r.gpu),
        blob_shape(g.x(75.0), g.y(101.0), g.s(36.0), r.ssd),
        blob_shape(g.x(250.0), g.y(74.0), g.s(28.0), r.wifi),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    svg::group()
        .clip_rect(g.x(2.0), g.y(2.0), g.sx(320.0), g.sy(218.0))
        .children(blobs)
        .into_view()
}

fn zenbook_structure(g: &BoardGeom) -> Vec<View> {
    let mut out = vec![
        stroked_rounded(
            g.x(2.0),
            g.y(2.0),
            g.sx(320.0),
            g.sy(218.0),
            g.s(10.0),
            RED_DIM,
            1.0,
        ),
        filled_rect(
            g.x(80.0),
            g.y(2.0),
            g.sx(44.0),
            g.sy(5.0),
            color_alpha(RED_DIM, 0.5),
        ),
        filled_rect(
            g.x(200.0),
            g.y(2.0),
            g.sx(44.0),
            g.sy(5.0),
            color_alpha(RED_DIM, 0.5),
        ),
        stroked_rounded(
            g.x(18.0),
            g.y(12.0),
            g.sx(288.0),
            g.sy(74.0),
            g.s(4.0),
            RED_DIM,
            0.6,
        ),
    ];
    out.extend(fan_shapes(g.x(45.0), g.y(45.0), g.s(30.0)));
    out.extend(fan_shapes(g.x(279.0), g.y(45.0), g.s(30.0)));

    let pipe_width = (g.s(4.0) as f32).max(1.5);
    out.push(polyline_shape(
        &[
            (g.x(70.0), g.y(40.0)),
            (g.x(126.0), g.y(40.0)),
            (g.x(150.0), g.y(45.0)),
            (g.x(190.0), g.y(50.0)),
            (g.x(230.0), g.y(45.0)),
            (g.x(254.0), g.y(42.0)),
        ],
        RED_DIM,
        pipe_width,
        0.8,
    ));
    out.push(polyline_shape(
        &[
            (g.x(72.0), g.y(50.0)),
            (g.x(140.0), g.y(54.0)),
            (g.x(192.0), g.y(58.0)),
        ],
        RED_DIM,
        pipe_width,
        0.8,
    ));
    out
}

fn zenbook_components(g: &BoardGeom, r: &TempReadings) -> Vec<View> {
    let mut out = Vec::new();
    for (i, core_temp) in r.cores.iter().take(4).enumerate() {
        out.push(filled_rect(
            g.x(129.0 + (i % 2) as f64 * 11.0),
            g.y(34.0 + (i / 2) as f64 * 11.0),
            g.sx(11.0),
            g.sy(11.0),
            heat_color(*core_temp, 0.45),
        ));
    }
    out.push(stroked_rect(
        g.x(129.0),
        g.y(34.0),
        g.sx(22.0),
        g.sy(22.0),
        RED,
        1.0,
    ));
    out.push(stroked_rect(
        g.x(184.0),
        g.y(42.0),
        g.sx(16.0),
        g.sy(16.0),
        RED,
        1.0,
    ));
    out.push(stroked_rect(
        g.x(104.0),
        g.y(62.0),
        g.sx(14.0),
        g.sy(9.0),
        RED_DIM,
        0.8,
    ));
    out.push(stroked_rect(
        g.x(104.0),
        g.y(74.0),
        g.sx(14.0),
        g.sy(9.0),
        RED_DIM,
        0.8,
    ));
    out.push(stroked_rect(
        g.x(35.0),
        g.y(95.0),
        g.sx(80.0),
        g.sy(12.0),
        RED,
        1.0,
    ));
    out.push(polyline_shape(
        &[(g.x(41.0), g.y(95.0)), (g.x(41.0), g.y(107.0))],
        RED,
        1.0,
        1.0,
    ));
    out.push(stroked_rect(
        g.x(242.0),
        g.y(67.0),
        g.sx(16.0),
        g.sy(14.0),
        RED,
        1.0,
    ));
    out.push(stroked_rounded(
        g.x(60.0),
        g.y(120.0),
        g.sx(204.0),
        g.sy(85.0),
        g.s(6.0),
        RED_DIM,
        0.7,
    ));
    out.push(stroked_rounded(
        g.x(20.0),
        g.y(175.0),
        g.sx(30.0),
        g.sy(30.0),
        g.s(4.0),
        RED_DIM,
        0.7,
    ));
    out.push(stroked_rounded(
        g.x(274.0),
        g.y(175.0),
        g.sx(30.0),
        g.sy(30.0),
        g.s(4.0),
        RED_DIM,
        0.7,
    ));
    for k in 0..3 {
        let k = k as f64;
        out.push(polyline_shape(
            &[
                (g.x(26.0 + k * 7.0), g.y(200.0)),
                (g.x(38.0 + k * 7.0), g.y(181.0)),
            ],
            RED_DIM,
            1.0,
            0.7,
        ));
        out.push(polyline_shape(
            &[
                (g.x(280.0 + k * 7.0), g.y(200.0)),
                (g.x(292.0 + k * 7.0), g.y(181.0)),
            ],
            RED_DIM,
            1.0,
            0.7,
        ));
    }
    out.push(
        svg::text("BATTERY")
            .at(g.x(162.0) as f32, g.y(166.0) as f32)
            .anchor(svg::TextAnchor::Middle)
            .font_family(MONO_FAMILY)
            .text_size(9.0_f32)
            .fill(color(TEXT_FAINT))
            .into_view(),
    );

    out.extend(tag_shapes(
        g.x(129.0),
        g.y(30.0),
        "CPU",
        r.pkg,
        None,
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(184.0),
        g.y(74.0),
        "MX450",
        r.gpu,
        Some(if r.gpu_dormant { "DORMANT" } else { "—" }),
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(35.0),
        g.y(91.0),
        "SSD",
        r.ssd,
        None,
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(242.0),
        g.y(63.0),
        "WIFI",
        r.wifi,
        None,
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(20.0),
        g.y(82.0),
        "BOARD",
        r.board,
        None,
        TagAlign::Left,
    ));
    out
}

fn zenbook_shapes(g: &BoardGeom, r: &TempReadings) -> Vec<View> {
    let mut out = vec![zenbook_blobs(g, r)];
    out.extend(zenbook_structure(g));
    out.extend(zenbook_components(g, r));
    out
}

fn tower_blobs(g: &BoardGeom, r: &TempReadings) -> View {
    let blobs = [
        blob_shape(g.x(252.0), g.y(112.0), g.s(50.0), r.pkg),
        blob_shape(g.x(255.0), g.y(237.0), g.s(46.0), r.gpu),
        blob_shape(g.x(240.0), g.y(275.0), g.s(30.0), r.ssd),
        blob_shape(g.x(341.0), g.y(276.0), g.s(34.0), r.board),
        blob_shape(g.x(397.0), g.y(306.0), g.s(24.0), r.wifi),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    svg::group()
        .clip_rect(g.x(5.0), g.y(5.0), g.sx(430.0), g.sy(390.0))
        .children(blobs)
        .into_view()
}

fn tower_shapes(g: &BoardGeom, r: &TempReadings) -> Vec<View> {
    let mut out = vec![
        tower_blobs(g, r),
        stroked_rounded(
            g.x(5.0),
            g.y(5.0),
            g.sx(430.0),
            g.sy(390.0),
            g.s(8.0),
            RED_DIM,
            1.0,
        ),
        stroked_rounded(
            g.x(170.0),
            g.y(40.0),
            g.sx(240.0),
            g.sy(250.0),
            g.s(4.0),
            RED_DIM,
            0.6,
        ),
    ];
    out.extend(fan_shapes(g.x(35.0), g.y(120.0), g.s(26.0)));
    out.extend(fan_shapes(g.x(35.0), g.y(210.0), g.s(26.0)));
    out.extend(fan_shapes(g.x(398.0), g.y(80.0), g.s(26.0)));

    for (i, core_temp) in r.cores.iter().take(4).enumerate() {
        out.push(filled_rect(
            g.x(230.0 + (i % 2) as f64 * 22.0),
            g.y(90.0 + (i / 2) as f64 * 22.0),
            g.sx(22.0),
            g.sy(22.0),
            heat_color(*core_temp, 0.45),
        ));
    }
    out.push(
        svg::circle(g.x(252.0), g.y(112.0), g.s(32.0))
            .fill_none()
            .stroke(color(RED_DIM))
            .stroke_width(1.0_f32)
            .into_view(),
    );
    out.push(stroked_rect(
        g.x(230.0),
        g.y(90.0),
        g.sx(44.0),
        g.sy(44.0),
        RED,
        1.0,
    ));
    for k in 0..4 {
        out.push(stroked_rect(
            g.x(310.0 + k as f64 * 14.0),
            g.y(62.0),
            g.sx(6.0),
            g.sy(118.0),
            RED_DIM,
            1.0,
        ));
    }
    out.push(stroked_rect(
        g.x(150.0),
        g.y(220.0),
        g.sx(220.0),
        g.sy(34.0),
        RED,
        1.0,
    ));
    for cx in [210.0, 290.0] {
        out.push(
            svg::circle(g.x(cx), g.y(237.0), g.s(13.0))
                .fill_none()
                .stroke(color(RED_DIM))
                .stroke_width(1.0_f32)
                .into_view(),
        );
    }
    out.push(stroked_rect(
        g.x(200.0),
        g.y(270.0),
        g.sx(80.0),
        g.sy(10.0),
        RED,
        1.0,
    ));
    out.push(stroked_rect(
        g.x(330.0),
        g.y(265.0),
        g.sx(22.0),
        g.sy(22.0),
        RED,
        1.0,
    ));
    out.push(stroked_rect(
        g.x(390.0),
        g.y(300.0),
        g.sx(14.0),
        g.sy(12.0),
        RED,
        1.0,
    ));
    out.push(stroked_rounded(
        g.x(20.0),
        g.y(320.0),
        g.sx(140.0),
        g.sy(60.0),
        g.s(4.0),
        RED_DIM,
        0.7,
    ));
    out.extend(fan_shapes(g.x(55.0), g.y(350.0), g.s(22.0)));
    out.push(
        svg::text("PSU")
            .at(g.x(110.0) as f32, g.y(354.0) as f32)
            .anchor(svg::TextAnchor::Start)
            .font_family(MONO_FAMILY)
            .text_size(9.0_f32)
            .fill(color(TEXT_FAINT))
            .into_view(),
    );

    out.extend(tag_shapes(
        g.x(230.0),
        g.y(84.0),
        "CPU",
        r.pkg,
        None,
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(150.0),
        g.y(214.0),
        "GPU",
        r.gpu,
        Some(if r.gpu_dormant { "DORMANT" } else { "—" }),
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(200.0),
        g.y(266.0),
        "SSD",
        r.ssd,
        None,
        TagAlign::Left,
    ));
    out.extend(tag_shapes(
        g.x(330.0),
        g.y(261.0),
        "BOARD",
        r.board,
        None,
        TagAlign::Right,
    ));
    out.extend(tag_shapes(
        g.x(388.0),
        g.y(296.0),
        "WIFI",
        r.wifi,
        None,
        TagAlign::Right,
    ));
    out
}

fn schematic_svg(sample: &Value, meta: &Value, width: f64, height: f64, alive: bool) -> Svg {
    let mut chart = svg()
        .view_box(0.0, 0.0, width as f32, height as f32)
        .aspect(svg::SvgAspect {
            align: None,
            slice: false,
        })
        .w_full()
        .h_full();

    if !alive || arr_at(sample, &["temps"]).is_empty() {
        return chart.child(
            svg::text("NO TELEMETRY")
                .at((width / 2.0) as f32, (height / 2.0) as f32)
                .anchor(svg::TextAnchor::Middle)
                .font_family(MONO_FAMILY)
                .text_size(9.0_f32)
                .fill(color(TEXT_FAINT)),
        );
    }

    let zenbook = machine_is_zenbook(meta);
    let geom = BoardGeom::new(width, height, zenbook);
    let readings = temp_readings(sample, meta);
    let shapes = if zenbook {
        zenbook_shapes(&geom, &readings)
    } else {
        tower_shapes(&geom, &readings)
    };
    chart = chart.children(shapes);
    chart
}
