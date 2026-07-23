import QtQuick
import "../services"

// Read/write rates plus utilisation on top, per-disk breakdown pinned to
// the bottom, and a single shared 60s chart between them: both throughput
// directions overlaid on one scale, colour-matched to the rate readouts
// (read red, write purple)
// the utilisation line rides its own fixed 0-100% scale in amber.
Panel {
    id: root

    title: "DISK"

    // Both lines must share one scale or the overlay lies about which
    // direction is busier.
    readonly property real peak: {
        let p = 1;
        for (const v of Sampler.readHistory)
            p = Math.max(p, v);
        for (const v of Sampler.writeHistory)
            p = Math.max(p, v);
        return p;
    }

    Row {
        id: rates

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 20

        Column {
            spacing: 1

            Text {
                text: "◂ READ"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            Text {
                text: Sampler.fmtBytes(Sampler.disk.readBps)
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize + 1
                color: Theme.glow
            }
        }

        Column {
            spacing: 1

            Text {
                text: "▸ WRITE"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            Text {
                text: Sampler.fmtBytes(Sampler.disk.writeBps)
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize + 1
                color: Theme.purple
            }
        }

        Column {
            spacing: 1

            Text {
                text: "◆ UTILISATION"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            Text {
                text: (Sampler.disk.utilPct || 0) + "%"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize + 1
                color: Theme.warnAmber
            }
        }
    }

    Rectangle {
        anchors.top: rates.bottom
        anchors.topMargin: 6
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: diskList.top
        anchors.bottomMargin: 6
        color: Theme.surfaceRaised

        Sparkline {
            anchors.fill: parent
            anchors.margins: 1
            values: Sampler.readHistory
            maxValue: root.peak
            lineColor: Theme.glow
            fillColor: Theme.gaugeDim
        }

        // Line-only on top of the filled read series, so the overlap stays
        // readable.
        Sparkline {
            anchors.fill: parent
            anchors.margins: 1
            values: Sampler.writeHistory
            maxValue: root.peak
            lineColor: Theme.purple
            fillColor: "transparent"
        }

        // Utilisation rides its own fixed percentage scale, not the
        // throughput peak — the amber colour is what signals the split.
        Sparkline {
            anchors.fill: parent
            anchors.margins: 1
            values: Sampler.utilHistory
            maxValue: 100
            lineColor: Theme.warnAmber
            fillColor: "transparent"
        }

        Text {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.margins: 3
            text: "THROUGHPUT"
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 4
            font.letterSpacing: 1
            color: Theme.textFaint
        }
    }

    Column {
        id: diskList

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 2

        Repeater {
            model: Object.keys(Sampler.disk.disks)

            Text {
                text: modelData + "  ◂ "
                    + Sampler.fmtBytes(Sampler.disk.disks[modelData].readBps)
                    + "  ▸ "
                    + Sampler.fmtBytes(Sampler.disk.disks[modelData].writeBps)
                    + "  · " + Sampler.disk.disks[modelData].utilPct +
                    "% util"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                color: Theme.textFaint
            }
        }
    }
}