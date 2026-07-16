import QtQuick
import "../services"

// Both GPUs: Iris Xe engine gauges (render/video/copy...) + frequency, and
// the MX450 via nvidia-smi — shown as DORMANT while runtime-suspended (the
// sampler deliberately never wakes it just to ask how busy it is).
Panel {
    id: root

    title: "GPU"

    readonly property var xe: Sampler.gpu ? Sampler.gpu.xe : null
    readonly property var nv: Sampler.gpu ? Sampler.gpu.nvidia : null

    readonly property var engineNames: ({
        rcs: "RENDER",
        ccs: "COMPUTE",
        bcs: "COPY",
        vcs: "VIDEO",
        vecs: "ENHANCE",
    })

    Column {
        anchors.fill: parent
        spacing: 8

        Text {
            text: "IRIS XE" + (root.xe && root.xe.freqMhz !== null
                ? " · " + root.xe.freqMhz + " MHz" : "")
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 2
            font.letterSpacing: 1
            color: Theme.textMuted
        }

        Column {
            width: parent.width
            spacing: 5

            Repeater {
                model: root.xe ? Object.keys(root.xe.engines).sort() : []

                Row {
                    spacing: 8

                    readonly property real pct: root.xe.engines[modelData]

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 62
                        text: root.engineNames[modelData] || modelData.toUpperCase()
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textFaint
                    }

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.parent.width - 62 - 46 - 16
                        height: 6
                        color: Theme.surfaceRaised

                        Rectangle {
                            width: parent.width * pct / 100
                            height: parent.height
                            color: Theme.red

                            Behavior on width {
                                NumberAnimation { duration: 350 }
                            }
                        }
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 46
                        horizontalAlignment: Text.AlignRight
                        text: pct.toFixed(0) + "%"
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textMuted
                    }
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.line
            visible: Sampler.hasNvidia
        }

        Text {
            visible: Sampler.hasNvidia
            text: {
                if (!root.nv)
                    return "MX450 · UNAVAILABLE";
                if (root.nv.asleep)
                    return "MX450 · DORMANT";
                let line = "MX450 · " + root.nv.utilPct.toFixed(0) + "% · "
                    + root.nv.tempC.toFixed(0) + "°C · "
                    + root.nv.memUsedMb.toFixed(0) + "/"
                    + root.nv.memTotalMb.toFixed(0) + " MB";
                if (root.nv.powerW !== null)
                    line += " · " + root.nv.powerW.toFixed(1) + " W";
                return line;
            }
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 2
            font.letterSpacing: 1
            color: root.nv && root.nv.asleep === false
                ? Theme.textMuted : Theme.textFaint
        }

        Rectangle {
            visible: Sampler.hasNvidia && root.nv && root.nv.asleep === false
            width: parent.width
            height: 6
            color: Theme.surfaceRaised

            Rectangle {
                width: root.nv && !root.nv.asleep
                    ? parent.width * root.nv.utilPct / 100 : 0
                height: parent.height
                color: Theme.purple

                Behavior on width {
                    NumberAnimation { duration: 350 }
                }
            }
        }
    }
}