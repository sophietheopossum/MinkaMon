//! Proustite theme tokens and small text helpers.
//!
//! The values mirror the shared `Proustite` QML singleton and MinkaMon's
//! `services/Theme.qml` facade. They are duplicated here as constants because
//! the Rust application cannot consume the QML singleton; keep them in sync
//! with `../../Proustite/Proustite.qml`.

use anti_webui::prelude::*;

pub const GROUND: u32 = 0x0a0709;
pub const SURFACE: u32 = 0x161013;
pub const SURFACE_RAISED: u32 = 0x1e161a;
pub const LINE: u32 = 0x2e2228;
pub const TEXT: u32 = 0xece5e7;
pub const TEXT_MUTED: u32 = 0xa3959b;
pub const TEXT_FAINT: u32 = 0x6e6167;
pub const RED: u32 = 0xff0000;
pub const RED_DIM: u32 = 0x8f1e2d;
pub const PURPLE: u32 = 0xa488c9;
pub const GAUGE_DIM: u32 = 0x3a1219;
pub const OK_GREEN: u32 = 0x7dc98c;
pub const WARN_AMBER: u32 = 0xe0a026;

pub const GLOBE_DISC: u32 = 0x120a0d;
pub const MEMORY_FREE_ODD: u32 = 0x241a1f;
pub const NVIDIA_FILL: u32 = 0x231a2e;

pub const MONO_FAMILY: &str = "monospace";
pub const FONT_SIZE: f32 = 13.0;

pub const SERIES_PALETTE: [u32; 8] = [
    0xff4a5e, 0xe0a026, 0xa488c9, 0x7dc98c, 0xe06c9a, 0x6fb7c9, 0xece5e7, 0xd97b4a,
];

pub fn color(hex: u32) -> Color {
    Color::from_rgb(hex)
}

pub fn color_alpha(hex: u32, alpha: f32) -> Color {
    Color::from_rgb(hex).with_alpha(alpha.clamp(0.0, 1.0))
}

pub fn series_color(index: usize) -> u32 {
    SERIES_PALETTE[index % SERIES_PALETTE.len()]
}

/// Temperature tint shared by panel headers and schematic tags.
pub fn temp_tint(c: Option<f64>) -> u32 {
    match c {
        Some(v) if v >= 85.0 => RED,
        Some(v) if v >= 70.0 => WARN_AMBER,
        Some(_) => TEXT_MUTED,
        None => TEXT_FAINT,
    }
}

pub fn mono_text(source: impl IntoTextSource, size: f32, hex: u32) -> Text {
    text(source)
        .font_family(MONO_FAMILY)
        .text_size(size)
        .text_color(color(hex))
}

pub fn mono_label(source: impl IntoTextSource, size: f32, hex: u32) -> Text {
    mono_text(source, size, hex).letter_spacing(1.0_f32)
}

pub fn faint_label(source: impl IntoTextSource) -> Text {
    mono_label(source, FONT_SIZE - 3.0, TEXT_FAINT)
}
