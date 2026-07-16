import QtQuick
import "../services"

// Every hwmon temperature sensor, grouped under its chip name.
Panel {
    id: root

    title: "THERMAL"

    Flickable {
        anchors.fill: parent
        contentHeight: sensorColumn.height
        clip: true

        Column {
            id: sensorColumn

            width: parent.width
            spacing: 4

            Repeater {
                model: Sampler.temps

                Row {
                    spacing: 8

                    readonly property real c: modelData.c
                    readonly property color tint: c >= 85 ? Theme.red
                        : c >= 70 ? Theme.warnAmber : Theme.textMuted

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 150
                        elide: Text.ElideRight
                        text: modelData.chip + " / " + modelData.label
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textFaint
                    }

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.parent.width - 150 - 52 - 16
                        height: 4
                        color: Theme.surfaceRaised

                        Rectangle {
                            width: parent.width * Math.min(c / 100, 1)
                            height: parent.height
                            color: tint

                            Behavior on width {
                                NumberAnimation { duration: 500 }
                            }
                        }
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