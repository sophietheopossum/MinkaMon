import QtQuick
import "../services"

// Every hwmon temperature sensor with a 60s line chart (0–100°C scale).
Panel {
    id: root

    title: "THERMAL"

    // Stable sensor list: Sampler.temps is a fresh array every tick, and
    // rebinding it as the Repeater model would recreate delegates and wipe
    // TrendLine histories. hwmon enumeration order is stable, so delegates
    // read their current value back by index.
    property var sensors: []

    Connections {
        target: Sampler

        function onTempsChanged() {
            const names = Sampler.temps.map(t => t.chip + "/" + t.label);
            if (names.join("\n") !== root.sensors.join("\n"))
                root.sensors = names;
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: sensorColumn.height
        clip: true

        Column {
            id: sensorColumn

            width: parent.width
            spacing: 4

            Repeater {
                model: root.sensors

                Row {
                    spacing: 8

                    readonly property real c: Sampler.temps[index]
                        ? Sampler.temps[index].c : 0
                    readonly property color tint: c >= 85 ? Theme.red
                        : c >= 70 ? Theme.warnAmber : Theme.textMuted

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 150
                        elide: Text.ElideRight
                        text: modelData
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textFaint
                    }

                    TrendLine {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.parent.width - 150 - 52 - 16
                        height: 13
                        value: c
                        maxValue: 100
                        lineColor: tint
                        fillColor: Theme.gaugeDim
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 52
                        horizontalAlignment: Text.AlignRight
                        text: c.toFixed(1) + "°C"
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: tint
                    }
                }
            }
        }
    }
}
