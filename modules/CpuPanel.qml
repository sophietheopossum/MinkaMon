import QtQuick
import "../services"

// Per-core 60s line charts (4-up grid) plus the total-load history below.
Panel {
    id: root

    title: "CPU · " + Sampler.cpu.total.toFixed(0) + "%"

    Column {
        anchors.fill: parent
        spacing: 10

        Grid {
            id: coreGrid

            // Model is Sampler.cores (a stable int), not the per-tick cores
            // array: recreating delegates would wipe each TrendLine history.
            readonly property int rows_: Math.max(1, Math.ceil(Sampler.cores / 4))
            readonly property real cellW: (parent.width - 3 * 6) / 4
            readonly property real cellH:
                (parent.height * 0.55 - (rows_ - 1) * 6) / rows_

            columns: 4
            columnSpacing: 6
            rowSpacing: 6

            Repeater {
                model: Sampler.cores

                Rectangle {
                    readonly property real pct:
                        Sampler.cpu.cores[index] !== undefined
                            ? Sampler.cpu.cores[index] : 0

                    width: coreGrid.cellW
                    height: coreGrid.cellH
                    color: Theme.surfaceRaised

                    TrendLine {
                        anchors.fill: parent
                        anchors.margins: 1
                        value: pct
                        maxValue: 100
                        lineColor: pct > 85 ? Theme.warnAmber : Theme.glow
                        fillColor: Theme.gaugeDim
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.margins: 2
                        text: index
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 4
                        color: Theme.textFaint
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 2
                        text: pct.toFixed(0)
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 4
                        color: pct > 85 ? Theme.warnAmber : Theme.textMuted
                    }
                }
            }
        }

        Sparkline {
            width: parent.width
            height: parent.height - coreGrid.height - 10
            values: Sampler.cpuHistory
            maxValue: 100
            lineColor: Theme.glow
            fillColor: Theme.gaugeDim
        }
    }
}
