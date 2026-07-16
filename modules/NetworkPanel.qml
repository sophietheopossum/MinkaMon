import QtQuick
import "../services"

// Up/down rates with 60s sparklines, plus per-interface breakdown.
Panel {
    id: root

    title: "NETWORK"

    Column {
        anchors.fill: parent
        spacing: 6

        Row {
            spacing: 20

            Column {
                spacing: 1

                Text {
                    text: "▼ DOWN"
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 3
                    font.letterSpacing: 1
                    color: Theme.textFaint
                }

                Text {
                    text: Sampler.fmtBytes(Sampler.net.downBps)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize + 1
                    color: Theme.glow
                }
            }

            Column {
                spacing: 1

                Text {
                    text: "▲ UP"
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 3
                    font.letterSpacing: 1
                    color: Theme.textFaint
                }

                Text {
                    text: Sampler.fmtBytes(Sampler.net.upBps)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize + 1
                    color: Theme.purple
                }
            }
        }

        Sparkline {
            width: parent.width
            height: 34
            values: Sampler.downHistory
            lineColor: Theme.glow
            fillColor: Theme.gaugeDim
        }

        Sparkline {
            width: parent.width
            height: 34
            values: Sampler.upHistory
            lineColor: Theme.purple
            fillColor: "#231a2e"
        }

        Column {
            spacing: 2

            Repeater {
                model: Object.keys(Sampler.net.ifaces)

                Text {
                    text: modelData + "  ▼ "
                        + Sampler.fmtBytes(Sampler.net.ifaces[modelData].downBps)
                        + "  ▲ "
                        + Sampler.fmtBytes(Sampler.net.ifaces[modelData].upBps)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 3
                    color: Theme.textFaint
                }
            }
        }
    }
}