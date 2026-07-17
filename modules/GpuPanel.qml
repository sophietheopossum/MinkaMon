import QtQuick
import "../services"

// Both GPUs: Iris Xe per-engine 60s line charts + frequency, and the MX450
// via nvidia-smi — shown as DORMANT while runtime-suspended (the sampler
// deliberately never wakes it just to ask how busy it is).
Panel {
    id: root

    title: "GPU"

    readonly property var xe: Sampler.gpu ? Sampler.gpu.xe : null
    readonly property var nv: Sampler.gpu ? Sampler.gpu.nvidia : null

    // Grow-only engine list: the reported set can gain engines mid-run (e.g.
    // vcs when video starts), and Repeater delegates must survive ticks or
    // their TrendLine histories reset.
    property var engineKeys: []
    onXeChanged: {
        if (!xe)
            return;
        const merged = engineKeys.slice();
        for (const key of Object.keys(xe.engines)) {
            if (merged.indexOf(key) < 0)
                merged.push(key);
        }
        if (merged.length !== engineKeys.length)
            engineKeys = merged.sort();
    }

    readonly property var engineNames: ({
        rcs: "RENDER",
        ccs: "COMPUTE",
        bcs: "COPY",
        vcs: "VIDEO",
        vecs: "ENHANCE",
    })

    Column {
        anchors.fill: parent
        spacing: 6

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
            spacing: 4

            Repeater {
                model: root.engineKeys

                Row {
                    spacing: 8

                    readonly property real pct:
                        root.xe && root.xe.engines[modelData] !== undefined
                            ? root.xe.engines[modelData] : 0

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 62
                        text: root.engineNames[modelData] || modelData.toUpperCase()
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textFaint
                    }

                    TrendLine {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.parent.width - 62 - 46 - 16
                        height: 16
                        value: pct
                        maxValue: 100
                        lineColor: Theme.red
                        fillColor: Theme.gaugeDim
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

        TrendLine {
            visible: Sampler.hasNvidia && root.nv && root.nv.asleep === false
            width: parent.width
            height: 18
            value: root.nv && !root.nv.asleep ? root.nv.utilPct : 0
            maxValue: 100
            lineColor: Theme.purple
            fillColor: "#231a2e"
        }
    }
}
