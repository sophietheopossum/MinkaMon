//! Shared instrument widgets: the eDEX-style panel frame and SVG charts.

use crate::theme::*;
use anti_webui::prelude::svg::kurbo::{BezPath, Point};
use anti_webui::prelude::*;
use std::collections::VecDeque;

fn corner_bracket(right: bool, bottom: bool) -> Div {
    let mut bracket = div().absolute().w(10.0).h(10.0).z_index(5);
    bracket = if right {
        bracket.right(0.0)
    } else {
        bracket.left(0.0)
    };
    bracket = if bottom {
        bracket.bottom(0.0)
    } else {
        bracket.top(0.0)
    };
    let horizontal = div().absolute().left(0.0).right(0.0).h(2.0);
    let horizontal = if bottom {
        horizontal.bottom(0.0)
    } else {
        horizontal.top(0.0)
    }
    .bg(color(RED_DIM));
    let vertical = div().absolute().top(0.0).bottom(0.0).w(2.0);
    let vertical = if right {
        vertical.right(0.0)
    } else {
        vertical.left(0.0)
    }
    .bg(color(RED_DIM));
    bracket.child(horizontal).child(vertical)
}

/// The shared instrument frame from `modules/Panel.qml`.
///
/// `tight_vertical` matches SystemPanel's reduced top/bottom content inset;
/// the header height is fixed at QML's 8px offset plus one 15px text line so
/// leader-line anchor maths can reproduce the same client coordinates.
pub fn panel_frame(
    title: Text,
    header_extra: Option<Div>,
    content: impl IntoView,
    tight_vertical: bool,
) -> Div {
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .h(23.0)
        .w_full()
        .px(12.0_f32)
        .pt(8.0_f32)
        .child(div().w(4.0).h(12.0).bg(color(RED)).mr(8.0))
        .child(title)
        .child(div().flex_1())
        .children(header_extra);

    let body = div()
        .flex_1()
        .min_h(0.0)
        .w_full()
        .overflow_hidden()
        .px(12.0_f32)
        .pt(if tight_vertical { 2.0 } else { 8.0_f32 })
        .pb(if tight_vertical { 2.0 } else { 12.0_f32 })
        .child(content);

    div()
        .relative()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(color(SURFACE))
        .border(1.0_f32)
        .border_color(color(LINE))
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(header)
                .child(body),
        )
        .child(corner_bracket(false, false))
        .child(corner_bracket(true, false))
        .child(corner_bracket(false, true))
        .child(corner_bracket(true, true))
}

pub fn panel_title(source: impl IntoTextSource) -> Text {
    mono_text(source, FONT_SIZE - 1.0, TEXT_MUTED).letter_spacing(2.0_f32)
}

/// A raised chart surface with the small corner label QML draws over it.
pub fn chart_box(label: &'static str, chart: Svg) -> Div {
    div()
        .relative()
        .flex_1()
        .w_full()
        .h_full()
        .min_h(0.0)
        .overflow_hidden()
        .bg(color(SURFACE_RAISED))
        .child(chart.absolute().inset(1.0))
        .child(div().absolute().left(3.0).top(3.0).child(mono_label(
            label,
            FONT_SIZE - 4.0,
            TEXT_FAINT,
        )))
}

fn points_for(values: &[Option<f32>], width: f64, height: f64, max: f32) -> Vec<Option<Point>> {
    if values.len() < 2 {
        return Vec::new();
    }
    let peak = if max > 0.0 { max } else { 1.0 };
    let step_x = width / 59.0;
    let x0 = width - (values.len() - 1) as f64 * step_x;
    values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            value.map(|v| {
                let y = height - 1.0 - (v / peak).clamp(0.0, 1.0) as f64 * (height - 2.0);
                Point::new(x0 + i as f64 * step_x, y)
            })
        })
        .collect()
}

fn line_path(points: &[Option<Point>]) -> BezPath {
    let mut path = BezPath::new();
    let mut pen = false;
    for point in points {
        match point {
            Some(p) if pen => path.line_to(*p),
            Some(p) => {
                path.move_to(*p);
                pen = true;
            }
            None => pen = false,
        }
    }
    path
}

fn chart_svg(width: f32, height: f32) -> Svg {
    svg()
        .view_box(0.0, 0.0, width, height)
        .aspect(svg::SvgAspect {
            align: None,
            slice: false,
        })
        .w_full()
        .h_full()
}

/// Filled single-series chart, matching `Sparkline.qml`. `max <= 0` autoscales.
pub fn sparkline(values: &VecDeque<f32>, max: f32, line: u32, fill: Option<u32>) -> Svg {
    let optional: Vec<Option<f32>> = values.iter().copied().map(Some).collect();
    sparkline_optional(&optional, max, line, fill)
}

pub fn sparkline_optional(values: &[Option<f32>], max: f32, line: u32, fill: Option<u32>) -> Svg {
    let peak = if max > 0.0 {
        max
    } else {
        values.iter().filter_map(|v| *v).fold(1.0_f32, f32::max)
    };
    let points = points_for(values, 120.0, 36.0, peak);
    let mut chart = chart_svg(120.0, 36.0);
    if points.len() < 2 {
        return chart;
    }

    if let Some(fill_hex) = fill {
        let mut area = BezPath::new();
        let mut pen = false;
        let mut first_x = 120.0;
        for point in &points {
            match point {
                Some(p) if !pen => {
                    first_x = p.x;
                    area.move_to(Point::new(p.x, 36.0));
                    area.line_to(*p);
                    pen = true;
                }
                Some(p) => area.line_to(*p),
                None => {}
            }
        }
        if pen {
            area.line_to(Point::new(120.0, 36.0));
            area.line_to(Point::new(first_x, 36.0));
            area.close_path();
            chart = chart.child(svg::path(area).fill(color(fill_hex)));
        }
    }

    chart.child(
        svg::path(line_path(&points))
            .fill_none()
            .stroke(color(line))
            .stroke_width(0.55_f32),
    )
}

/// Multi-series chart matching `MultiTrendLine.qml`, including null gaps.
pub fn multi_trend(series: &[VecDeque<Option<f32>>], max: f32) -> Svg {
    let mut chart = chart_svg(120.0, 36.0);
    for (index, values) in series.iter().enumerate() {
        let values: Vec<Option<f32>> = values.iter().copied().collect();
        if values.len() < 2 {
            continue;
        }
        let points = points_for(&values, 120.0, 36.0, max);
        chart = chart.child(
            svg::path(line_path(&points))
                .fill_none()
                .stroke(color(series_color(index)))
                .stroke_width(0.45_f32),
        );
    }
    chart
}

/// Two overlaid series sharing a scale: the first is filled, the second is
/// line-only, as in NetworkPanel/DiskPanel. Additional fixed-scale series can
/// be composited by calling this with a prebuilt underlay in a chart_box.
pub fn overlay_sparklines(
    first: &VecDeque<f32>,
    first_line: u32,
    first_fill: Option<u32>,
    second: &VecDeque<f32>,
    second_line: u32,
    peak: f32,
) -> Svg {
    let mut chart = sparkline(first, peak, first_line, first_fill);
    let second_values: Vec<Option<f32>> = second.iter().copied().map(Some).collect();
    let points = points_for(&second_values, 120.0, 36.0, peak);
    if points.len() >= 2 {
        chart = chart.child(
            svg::path(line_path(&points))
                .fill_none()
                .stroke(color(second_line))
                .stroke_width(0.55_f32),
        );
    }
    chart
}

pub fn add_series(chart: Svg, values: &VecDeque<f32>, max: f32, line: u32) -> Svg {
    let values: Vec<Option<f32>> = values.iter().copied().map(Some).collect();
    let points = points_for(&values, 120.0, 36.0, max);
    if points.len() < 2 {
        return chart;
    }
    chart.child(
        svg::path(line_path(&points))
            .fill_none()
            .stroke(color(line))
            .stroke_width(0.55_f32),
    )
}

/// Label + right-aligned value readout used by the rate panels.
pub fn rate_readout(label: &'static str, value: String, tint: u32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(1.0_f32)
        .child(faint_label(label))
        .child(mono_text(value, FONT_SIZE + 1.0, tint))
}

pub fn hrule() -> Div {
    div().w_full().h(1.0).bg(color(LINE))
}

/// Tracks hover on a window root in the shared interactive-sampling counter.
/// A plain cell is used for the local state because cleanup runs after the
/// owner's reactive nodes have been removed.
pub fn with_hover(view: Div, hover_count: Signal<i32>) -> Div {
    let here = std::rc::Rc::new(std::cell::Cell::new(false));
    let here_cleanup = std::rc::Rc::clone(&here);
    on_cleanup(move || {
        if here_cleanup.get() {
            crate::state::cleanup_write(move || {
                if hover_count.is_alive() {
                    hover_count.update(|n| *n = (*n - 1).max(0));
                }
            });
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
