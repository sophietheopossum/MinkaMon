//! Rotating orthographic globe of live TCP peers.

use crate::model::*;
use crate::state::AppState;
use crate::theme::*;
use crate::widgets::{panel_frame, panel_title};
use anti_webui::prelude::svg::kurbo::{BezPath, Point};
use anti_webui::prelude::*;
use serde_json::Value;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy)]
struct Projected {
    x: f64,
    y: f64,
    z: f64,
}

fn project(lat: f64, lon: f64, spin: f64) -> Projected {
    let tilt = 18.0_f64.to_radians();
    let phi = lat.to_radians();
    let lam = (lon + spin).to_radians();
    let x = phi.cos() * lam.sin();
    let y = phi.sin();
    let z = phi.cos() * lam.cos();
    Projected {
        x,
        y: y * tilt.cos() - z * tilt.sin(),
        z: y * tilt.sin() + z * tilt.cos(),
    }
}

#[derive(Clone, Copy)]
struct Projection {
    spin: f64,
    cx: f64,
    cy: f64,
    radius: f64,
}

fn add_projected(path: &mut BezPath, lat: f64, lon: f64, proj: Projection, pen: &mut bool) {
    let p = project(lat, lon, proj.spin);
    if p.z < 0.0 {
        *pen = false;
        return;
    }
    let point = Point::new(proj.cx + p.x * proj.radius, proj.cy - p.y * proj.radius);
    if *pen {
        path.line_to(point);
    } else {
        path.move_to(point);
        *pen = true;
    }
}

fn graticule_path(spin: f64, cx: f64, cy: f64, radius: f64) -> BezPath {
    let proj = Projection {
        spin,
        cx,
        cy,
        radius,
    };
    let mut path = BezPath::new();
    for lat_i in -2..=2 {
        let lat = lat_i as f64 * 30.0;
        let mut pen = false;
        for lon_i in -30..=30 {
            add_projected(&mut path, lat, lon_i as f64 * 6.0, proj, &mut pen);
        }
    }
    for lon_i in 0..6 {
        let lon = lon_i as f64 * 30.0;
        let mut pen = false;
        for lat_i in -15..=15 {
            add_projected(&mut path, lat_i as f64 * 6.0, lon, proj, &mut pen);
        }
    }
    path
}

fn coastline_path(meta: &Value, spin: f64, cx: f64, cy: f64, radius: f64) -> BezPath {
    let proj = Projection {
        spin,
        cx,
        cy,
        radius,
    };
    let mut path = BezPath::new();
    for line in arr_at(meta, &["coastlines"]) {
        let Some(coords) = line.as_array() else {
            continue;
        };
        let mut pen = false;
        let mut i = 0;
        while i + 1 < coords.len() {
            let lon = coords[i].as_f64().unwrap_or(0.0);
            let lat = coords[i + 1].as_f64().unwrap_or(0.0);
            add_projected(&mut path, lat, lon, proj, &mut pen);
            i += 2;
        }
    }
    path
}

fn globe_svg(
    meta: &Value,
    conns: &[ConnRow],
    spin: f64,
    pulse: f64,
    width: f64,
    height: f64,
) -> Svg {
    let mut chart = svg()
        .view_box(0.0, 0.0, width as f32, height as f32)
        .aspect(svg::SvgAspect {
            align: None,
            slice: false,
        })
        .w_full()
        .h_full();

    let cx = width / 2.0;
    let cy = height / 2.0;
    let radius = width.min(height) / 2.0 - 8.0;
    if radius <= 0.0 {
        return chart;
    }

    chart = chart
        .child(
            svg::circle(cx, cy, radius)
                .fill(color(GLOBE_DISC))
                .stroke(color(RED_DIM))
                .stroke_width(1.0_f32),
        )
        .child(
            svg::path(graticule_path(spin, cx, cy, radius))
                .fill_none()
                .stroke(color_alpha(LINE, 0.2))
                .stroke_width(1.0_f32),
        )
        .child(
            svg::path(coastline_path(meta, spin, cx, cy, radius))
                .fill_none()
                .stroke(color(RED_DIM))
                .stroke_width(1.0_f32),
        );

    for conn in conns {
        let (Some(lat), Some(lon)) = (conn.lat, conn.lon) else {
            continue;
        };
        let p = project(lat, lon, spin);
        if p.z < 0.02 {
            continue;
        }
        let px = cx + p.x * radius;
        let py = cy - p.y * radius;
        chart = chart
            .child(
                svg::circle(px, py, 2.0 + pulse * 7.0)
                    .fill_none()
                    .stroke(color_alpha(0xff4a5e, (1.0 - pulse) as f32))
                    .stroke_width(1.0_f32),
            )
            .child(svg::circle(px, py, 2.0).fill(color(RED)));
    }
    chart
}

fn schedule_globe_tick(spin: Signal<f64>, pulse: Signal<f64>, alive: Rc<Cell<bool>>) {
    set_timeout(Duration::from_millis(66), move || {
        if !alive.get() || !spin.is_alive() || !pulse.is_alive() {
            return;
        }
        spin.update(|value| *value = (*value + 0.35) % 360.0);
        pulse.update(|value| *value = (*value + 0.09) % 1.0);
        schedule_globe_tick(spin, pulse, alive);
    });
}

pub fn globe_panel(state: AppState) -> Div {
    let spin = signal(0.0_f64);
    let pulse = signal(0.0_f64);
    let alive = Rc::new(Cell::new(true));
    let alive_cleanup = Rc::clone(&alive);
    on_cleanup(move || alive_cleanup.set(false));
    schedule_globe_tick(spin, pulse, alive);

    let size: Signal<(f32, f32)> = signal((500.0, 500.0));
    let title_state = state.clone();
    let title = panel_title(move || {
        format!(
            "WORLD VIEW · {} LINKS",
            title_state.conns.with(|conns| conns.len())
        )
    });

    let globe_state = state.clone();
    let readout_state = state.clone();
    let content = div()
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
            let conns = globe_state.conns.get();
            globe_state
                .meta
                .with(|meta| globe_svg(meta, &conns, spin.get(), pulse.get(), w as f64, h as f64))
        }))
        .child(dyn_view(move || {
            let mut readout = div()
                .absolute()
                .right(0.0)
                .top(0.0)
                .flex()
                .flex_col()
                .gap(1.0_f32);
            for conn in readout_state.conns.get().into_iter().take(8) {
                readout = readout.child(mono_text(
                    format!("{} {}", conn.country, conn.ip),
                    FONT_SIZE - 4.0,
                    TEXT_FAINT,
                ));
            }
            readout
        }));

    panel_frame(title, None, content, false)
}
