import QtQuick
import "../services"

// Instrument grid:
// CPU/GPU/network down the left
// the machine schematic centre-stage
// memory and the globe on the right
// with leader lines wiring each panel to its component on the board.
Item {
    id: root

    readonly property real pad: 10
    readonly property real colLeft: width * 0.32
    readonly property real colRight: width * 0.30
    readonly property real colMid: width - colLeft - colRight - pad * 4

    CpuPanel {
        id: cpuPanel
        x: root.pad
        y: root.pad
        width: root.colLeft
        height: (root.height - root.pad * 4) * 0.4
    }

    GpuPanel {
        id: gpuPanel
        x: root.pad
        y: cpuPanel.y + cpuPanel.height + root.pad
        width: root.colLeft
        height: (root.height - root.pad * 4) * 0.32
    }

    NetworkPanel {
        id: netPanel
        x: root.pad
        y: gpuPanel.y + gpuPanel.height + root.pad
        width: root.colLeft
        height: root.height - gpuPanel.y - gpuPanel.height - root.pad * 2
    }

    SystemPanel {
        id: systemPanel
        x: root.colLeft + root.pad * 2
        y: root.pad
        width: root.colMid
        height: root.height - root.pad * 2
    }

    MemoryPanel {
        id: memPanel
        x: root.width - root.colRight - root.pad
        y: root.pad
        width: root.colRight
        height: (root.height - root.pad * 3) * 0.42
    }

    GlobePanel {
        x: memPanel.x
        y: memPanel.y + memPanel.height + root.pad
        width: root.colRight
        height: root.height - memPanel.height - root.pad * 3
    }

    LeaderLines {
        anchors.fill: parent
        system: systemPanel
        ties: [
            { item: cpuPanel, at: 0.3, target: "cpu" },
            { item: gpuPanel, at: 0.5, target: "gpu" },
            { item: netPanel, at: 0.5, target: "wifi" },
            { item: memPanel, at: 0.5, target: "ram" },
        ]
    }
}