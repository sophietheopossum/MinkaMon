//! Proustite (the `proustite` crate, generated from Proustite.qml) and
//! MinkaMon's own tokens and text helpers, as `services/Theme.qml` is a
//! facade over the QML singleton.

use anti_webui::prelude::*;
pub use proustite::*;

pub const GLOBE_DISC: u32 = 0x120a0d;
pub const MEMORY_FREE_ODD: u32 = 0x241a1f;
pub const NVIDIA_FILL: u32 = 0x231a2e;

pub const SERIES_PALETTE: [u32; 8] = [
    0xff4a5e, 0xe0a026, 0xa488c9, 0x7dc98c, 0xe06c9a, 0x6fb7c9, 0xece5e7, 0xd97b4a,
];

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

pub fn mono_label(source: impl IntoTextSource, size: f32, hex: u32) -> Text {
    mono_text(source, size, hex).letter_spacing(1.0_f32)
}

pub fn faint_label(source: impl IntoTextSource) -> Text {
    mono_label(source, FONT_SIZE - 3.0, TEXT_FAINT)
}
