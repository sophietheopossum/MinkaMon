import QtQuick
import "../services"

// Per-core vertical bars plus a 60s total-load sparkline.
Panel {
    id: root

    title: "CPU · " + Sampler.cpu.total.toFixed(0) + "%"

    Column {
        anchors.fill: parent
        spacing: 10

        Row {
            id: bars

            readonly property int coreCount: Sampler.cpu.cores.length
            readonly property real barWidth: coreCount > 0
                ? (parent.width - (coreCount - 1) * 6) / coreCount : 0

            spacing: 6
            height: parent.height * 0.52

            Repeater {
                model: Sampler.cpu.cores

                Column {
                    spacing: 3

                    Rectangle {
                        width: bars.barWidth
                        height: bars.height - coreLabel.height - 3
                        color: Theme.surfaceRaised

                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: parent.height * modelData / 100
                            color: modelData > 85 ? Theme.warnAmber : Theme.red

                            Behavior on height {
                                NumberAnimation { duration: 350 }
                            }
                        }
                    }

                    Text {
                        id: coreLabel
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: index
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 4
                        color: Theme.textFaint
                    }
                }
            }
        }

        Sparkline {
            width: parent.width
            height: parent.height - bars.height - 10
            values: Sampler.cpuHistory
            maxValue: 100
            lineColor: Theme.glow
            fillColor: Theme.gaugeDim
        }
    }
}