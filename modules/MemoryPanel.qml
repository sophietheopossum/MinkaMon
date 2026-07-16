import QtQuick
import "../services"

// eDEX-UI style memory widget: a grid of cells that fill with usage (used
// solid, cache dim) plus numeric readouts and a swap bar.
Panel {
    id: root

    title: "MEMORY"

    readonly property var mem: Sampler.mem
    readonly property int cells: 10 * 44
    readonly property real usedFrac: mem ? mem.usedKb / mem.totalKb : 0
    readonly property real cacheFrac: mem ? mem.cacheKb / mem.totalKb : 0

    Column {
        anchors.fill: parent
        spacing: 8

        Grid {
            id: blockGrid

            columns: 44
            columnSpacing: 2
            rowSpacing: 2

            Repeater {
                model: root.cells

                Rectangle {
                    width: (blockGrid.parent.width - 43 * 2) / 44
                    height: 7
                    color: {
                        const frac = (index + 1) / root.cells;
                        if (frac <= root.usedFrac)
                            return Theme.red;
                        if (frac <= root.usedFrac + root.cacheFrac)
                            return Theme.gaugeDim;
                        return Theme.surfaceRaised;
                    }
                }
            }
        }

        Row {
            spacing: 18

            Repeater {
                model: [
                    {
                        label: "USED",
                        value: root.mem ? Sampler.fmtKb(root.mem.usedKb) : "—",
                        tint: Theme.red,
                    },
                    {
                        label: "CACHE",
                        value: root.mem ? Sampler.fmtKb(root.mem.cacheKb) : "—",
                        tint: Theme.textMuted,
                    },
                    {
                        label: "FREE",
                        value: root.mem ? Sampler.fmtKb(root.mem.availableKb) : "—",
                        tint: Theme.okGreen,
                    },
                    {
                        label: "TOTAL",
                        value: root.mem ? Sampler.fmtKb(root.mem.totalKb) : "—",
                        tint: Theme.textFaint,
                    },
                ]

                Column {
                    spacing: 1

                    Text {
                        text: modelData.label
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        font.letterSpacing: 1
                        color: Theme.textFaint
                    }

                    Text {
                        text: modelData.value
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize
                        color: modelData.tint
                    }
                }
            }
        }

        Row {
            spacing: 8
            visible: root.mem !== null && root.mem.swapTotalKb > 0

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "SWAP"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 140
                height: 5
                color: Theme.surfaceRaised

                Rectangle {
                    width: root.mem && root.mem.swapTotalKb > 0
                        ? parent.width * root.mem.swapUsedKb / root.mem.swapTotalKb
                        : 0
                    height: parent.height
                    color: Theme.warnAmber
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.mem
                    ? Sampler.fmtKb(root.mem.swapUsedKb) + " / "
                        + Sampler.fmtKb(root.mem.swapTotalKb)
                    : ""
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                color: Theme.textMuted
            }
        }
    }
}