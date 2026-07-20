import QtQuick
import "../services"

// Both GPUs: Iris Xe engines as one stacked 60s multi-line chart (CPU-panel
// style, one series per engine), and the MX450 via nvidia-smi — shown as
// DORMANT while runtime-suspended (the sampler deliberately never wakes it
// just to ask how busy it is).
Panel {
    id: root

    title: "GPU"

    readonly property var xe: Sampler.gpu ? Sampler.gpu.xe : null
    readonly property var nv: Sampler.gpu ? Sampler.gpu.nvidia : null

    // Grow-only engine list: the reported set can gain engines mid-run (e.g.
    // vcs when video starts), and the chart tracks series history by index,
    // so existing entries must keep their positions across ticks.
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
            engineKeys = merged;
    }

    readonly property var engineNames: ({
        rcs: "RENDER",
        ccs: "COMPUTE",
        bcs: "COPY",
        vcs: "VIDEO",
        vecs: "ENHANCE",
    })

    Item {
        id: xeHeader

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: xeTitle.height

        Text {
            id: xeTitle

            // act_freq drops to 0 while the GT is power-gated; show the
            // requested clock with a GATED tag instead of a scary 0 MHz.
            text: {
                let line = "IRIS XE";
                if (!root.xe || root.xe.freqMhz === null)
                    return line;
                if (root.xe.freqMhz > 0)
                    return line + " · " + root.xe.freqMhz + " MHz";
                return line + " · " + (root.xe.curFreqMhz || 0)
                    + " MHz · GATED";
            }
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 2
            font.letterSpacing: 1
            color: Theme.textMuted
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: 
                parent
                    .verticalCenter
            spacing: 6

            Repeater {
                model: root.engineKeys

                Text {
                    text: root.engineNames[modelData]
                        || modelData.toUpperCase()
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 4
                    color: Theme.seriesPalette[
                        index % Theme.seriesPalette.length]
                }
            }
        }
    }

    // The Xe chart takes whatever height the MX450 block leaves, so a short
    // window squeezes the chart instead of clipping the dGPU off the panel.
    Rectangle {
        anchors.top: xeHeader.bottom
        anchors.topMargin: 6
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: nvBlock.top
        anchors.bottomMargin: Sampler.hasNvidia ? 10 : 0
        color: Theme.surfaceRaised

        MultiTrendLine {
            anchors.fill: parent
            anchors.margins: 1
            current: root.engineKeys.map(key =>
                root.xe && root.xe.engines[key] !== undefined
                    ? root.xe.engines[key] : null)
            maxValue: 100
        }

        Text {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.margins: 3
            text: "ENGINES"
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 4
            font.letterSpacing: 1
            color: Theme.textFaint
        }
    }

    Column {
        id: nvBlock

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 6

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
            height: 44
            color: Theme.surfaceRaised

            TrendLine {
                anchors.fill: parent
                anchors.margins: 1
                value: root.nv && !root.nv.asleep ? root.nv.utilPct : 0
                maxValue: 100
                lineColor: Theme.purple
                fillColor: "#231a2e"
            }

            Text {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.margins: 3
                text: "MX450"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 4
                font.letterSpacing: 1
                color: Theme.textFaint
            }
        }
    }
}