pragma Singleton
import QtQuick
import Quickshell

// Eternal Darkness tokens (mirrored from MinkaConf/MinkaShell) plus the
// eDEX-flavoured accents MinkaMon's instrument panels use.
Singleton {
    readonly property color ground: "#0a0709"
    readonly property color surface: "#161013"
    readonly property color surfaceRaised: "#1e161a"
    readonly property color line: "#2e2228"
    readonly property color text: "#ece5e7"
    readonly property color textMuted: "#a3959b"
    readonly property color textFaint: "#6e6167"
    readonly property color red: "#e0263c"
    readonly property color redDim: "#8f1e2d"
    readonly property color purple: "#a488c9"

    // Panel accents: red-on-dark reinterpretation of eDEX-UI's cyan glow.
    readonly property color glow: "#ff4a5e"
    readonly property color gaugeDim: "#3a1219"
    readonly property color okGreen: "#7dc98c"
    readonly property color warnAmber: "#e0a026"

    readonly property string fontFamily: "Noto Sans"
    readonly property string monoFamily: "monospace"
    readonly property int fontSize: 13
}