pragma Singleton
import QtQuick
import Quickshell
// Through the config-root symlink: Quickshell only honours qmldir
// singleton registration for paths inside the shell root.
import "../Proustite"

// Thin facade over the shared Proustite palette plus the eDEX-flavoured
// chart extras only MinkaMon uses. The old separate "glow" accent merged
// into red palette-wide (23/7).
Singleton {
    readonly property color ground: Proustite.ground
    readonly property color surface: Proustite.surface
    readonly property color surfaceRaised: Proustite.surfaceRaised
    readonly property color line: Proustite.line
    readonly property color text: Proustite.text
    readonly property color textMuted: Proustite.textMuted
    readonly property color textFaint: Proustite.textFaint
    readonly property color red: Proustite.red
    readonly property color redDim: Proustite.redDim
    readonly property color purple: Proustite.purple
    readonly property color gaugeDim: Proustite.gaugeDim
    readonly property color okGreen: Proustite.okGreen
    readonly property color warnAmber: Proustite.warnAmber

    // Per-series tints for multi-line charts (one per CPU core).
    readonly property var seriesPalette: [
        "#ff4a5e", "#e0a026", "#a488c9", "#7dc98c",
        "#e06c9a", "#6fb7c9", "#ece5e7", "#d97b4a",
    ]

    readonly property string fontFamily: Proustite.fontFamily
    readonly property string monoFamily: Proustite.monoFamily
    readonly property int fontSize: Proustite.fontSize
}