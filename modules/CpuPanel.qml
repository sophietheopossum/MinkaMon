import QtQuick
import "../services"

// Two stacked 60s multi-line charts, every core in each: load on top,
// coretemp below (0–100°C), series colours from Theme.seriesPalette.
// Hyperthread siblings share their physical core's sensor, so their temp
// lines overlap exactly.
Panel {
    id: root

    readonly property var packageC:
        Sampler.cpu.packageC !== undefined ? Sampler.cpu.packageC : null
    readonly property bool hasTemps: Sampler.cpu.coreTemps !== undefined
        && Sampler.cpu.coreTemps.some(t => t !== null)

    title: "CPU · " + Sampler.cpu.total.toFixed(0) + "%"
        + (packageC !== null ? " · " + packageC.toFixed(0) + "°C" : "")

    component CoreLegend: Row {
        spacing: 4

        Repeater {
            model: Sampler.cores

            Text {
                text: index
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 4
                color: Theme.seriesPalette[index % Theme.seriesPalette.length]
            }
        }
    }

    Column {
        anchors.fill: parent
        spacing: 10

        Rectangle {
            width: parent.width
            height: root.hasTemps
                ? (parent.height - 10) / 2 : parent.height
            color: Theme.surfaceRaised

            MultiTrendLine {
                anchors.fill: parent
                anchors.margins: 1
                current: Sampler.cpu.cores
                maxValue: 100
            }

            Text {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.margins: 3
                text: "LOAD"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 4
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            CoreLegend {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 3
            }
        }

        Rectangle {
            width: parent.width
            height: (parent.height - 10) / 2
            visible: root.hasTemps
            color: Theme.surfaceRaised

            MultiTrendLine {
                anchors.fill: parent
                anchors.margins: 1
                current: Sampler.cpu.coreTemps !== undefined
                    ? Sampler.cpu.coreTemps : []
                maxValue: 100
            }

            Text {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.margins: 3
                text: "TEMP"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 4
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            CoreLegend {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 3
            }
        }
    }
}