import QtQuick
import "../services"

// Instrument grid: CPU/GPU/network down the left, globe centre-stage,
// memory and thermal on the right.
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
        x: root.pad
        y: gpuPanel.y + gpuPanel.height + root.pad
        width: root.colLeft
        height: root.height - gpuPanel.y - gpuPanel.height - root.pad * 2
    }

    GlobePanel {
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

    TempsPanel {
        x: memPanel.x
        y: memPanel.y + memPanel.height + root.pad
        width: root.colRight
        height: root.height - memPanel.height - root.pad * 3
    }
}