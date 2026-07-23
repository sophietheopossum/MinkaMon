import Quickshell
import Quickshell.Io
import QtQuick
import "services"
import "modules"

// MinkaMon — the Minka system monitor.
// The main window is the machine schematic itself; clicking hardware on the
// board opens the related instrument panels as satellite windows. Satellites
// are all pre-declared & toggled via `visible` so their chart histories keep recording from
// app start, even while the window is closed.
ShellRoot {
    id: shellRoot

    // What a zone click opens: its info panels, its temperature graph, or
    // both. Info only on launch.
    property bool infoMode: true
    property bool tempMode: false
    property bool overviewPending: false

    function openZone(zone) {
        if (zone === "cpu") {
            if (infoMode)
                cpuWin.visible = true;
            if (tempMode)
                cpuTempWin.visible = true;
        } else if (zone === "gpu") {
            if (infoMode)
                gpuWin.visible = true;
            if (tempMode)
                gpuTempWin.visible = true;
        } else if (zone === "ram") {
            if (infoMode)
                memWin.visible = true;
        } else if (zone === "wifi") {
            // The user decides which of the network windows stays open.
            if (infoMode) {
                netWin.visible = true;
                globeWin.visible = true;
            }
            if (tempMode)
                wifiTempWin.visible = true;
        } else if (zone === "ssd") {
            if (infoMode)
                diskWin.visible = true;
            if (tempMode)
                ssdTempWin.visible = true;
        } else if (zone === "board") {
            if (tempMode)
                boardTempWin.visible = true;
        } else if (zone === "processes")
            procWin.visible = true;
    }

    function closeAll() {
        overviewPending = false;
        overviewTimeout.stop();
        cpuWin.visible = false;
        gpuWin.visible = false;
        memWin.visible = false;
        netWin.visible = false;
        diskWin.visible = false;
        globeWin.visible = false;
        procWin.visible = false;
        ssdTempWin.visible = false;
        boardTempWin.visible = false;
        wifiTempWin.visible = false;
        cpuTempWin.visible = false;
        gpuTempWin.visible = false;
    }

    // Full overview: open the 0.4.0 monitor-page set and arrange the real
    // windows into its grid:
    // left:
    // CPU
    // DISK
    // NET
    // centre:
    // schematic
    // right:
    // MEMORY
    // GPU
    // GLOBE
    // via the compositor's windows.setRect.
    function fullOverview() {
        cpuWin.visible = true;
        gpuWin.visible = true;
        netWin.visible = true;
        memWin.visible = true;
        diskWin.visible = true;
        globeWin.visible = true;
        overviewPending = true;
        overviewTimeout.restart();
        ShojiIpc.requestGeometry();
        tryArrangeOverview();
    }

    function tryArrangeOverview() {
        if (!overviewPending)
            return;
        const wins = ShojiIpc.windows;
        const names = [
            "MinkaMon",
            "MinkaMon // CPU",
            "MinkaMon // GPU",
            "MinkaMon // NETWORK",
            "MinkaMon // MEMORY",
            "MinkaMon // DISK",
            "MinkaMon // GLOBE"
        ];
        for (const n of names) {
            // Freshly-opened windows take a beat to appear in the WM view;
            // retry on the next geometry update until they're all there.
            if (!wins[n] || !wins[n].id)
                return;
        }
        const usable = ShojiIpc.usableAreas[wins["MinkaMon"].monitor];
        if (!usable) {
            ShojiIpc.requestGeometry();
            return;
        }
        overviewPending = false;
        overviewTimeout.stop();

        // 0.4.0 MonitorPage proportions over the usable area. Cells are
        // inflated by the chrome inset so the *visible* window borders sit
        // on the grid, halos overlapping harmlessly in the gutters.
        const pad = 10, chrome = 14;
        const W = usable.width, H = usable.height;
        const colLeft = W * 0.32;
        const colRight = W * 0.30;
        const colMid = W - colLeft - colRight - pad * 4;
        const cpuH = (H - pad * 4) * 0.4;
        const diskH = (H - pad * 4) * 0.32;
        const memH = (H - pad * 4) * 0.36;
        const gpuH = (H - pad * 4) * 0.26;
        const place = (title, x, y, w, h) => {
            ShojiIpc.setRect(wins[title].id,
                Math.round(usable.x + x - chrome),
                Math.round(usable.y + y - chrome),
                Math.round(w + chrome * 2),
                Math.round(h + chrome * 2));
        };
        place("MinkaMon // CPU", pad, pad, colLeft, cpuH);
        place(
            "MinkaMon // DISK", 
            pad, 
            pad * 2 + cpuH, 
            colLeft,
            diskH
        );
        place(
            "MinkaMon // NETWORK",
            pad, 
            pad * 3 + cpuH + diskH,
            colLeft, 
            H - cpuH - diskH - pad * 4,
        );
        place("MinkaMon", colLeft + pad * 2, pad, colMid, H - pad * 2);
        place("MinkaMon // MEMORY", W - colRight - pad, pad,
            colRight, memH);
        place(
            "MinkaMon // GPU",
            W - colRight - pad, 
            pad * 2 + memH,
            colRight,
            gpuH,
        );
        place(
            "MinkaMon // GLOBE",
            W - colRight - pad,
            pad * 3 + memH + gpuH,
            colRight,
            H - memH - gpuH - pad * 4,
        );
    }

    // Give freshly-opened windows a bounded window to appear in the WM
    // view; past that, stop retrying quietly.
    Timer {
        id: overviewTimeout

        interval: 4000
        onTriggered: shellRoot.overviewPending = false
    }

    Connections {
        target: ShojiIpc

        function onUpdated() {
            shellRoot.tryArrangeOverview();
        }

        function onUsableAreasChanged() {
            shellRoot.tryArrangeOverview();
        }
    }

    // Debug hooks: qs -p <dir> ipc call debug shot /path.png
    //              qs -p <dir> ipc call debug open cpu
    IpcHandler {
        target: "debug"

        function shot(path: string): void {
            grabRoot.grabToImage(result => result.saveToFile(path));
        }

        function open(name: string): void {
            shellRoot.openZone(name);
        }

        function closeAll(): void {
            shellRoot.closeAll();
        }
    }

    component TopChip: Rectangle {
        id: chip

        property string label
        property bool active: false
        signal clicked()

        width: chipText.implicitWidth + 26
        height: 28
        radius: 5
        color: active ? Theme.surfaceRaised : "transparent"
        border.width: 1
        border.color: active ? Theme.red : Theme.line

        Text {
            id: chipText

            anchors.centerIn: parent
            text: chip.label
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 1
            font.letterSpacing: 2
            color: chip.active ? Theme.text : Theme.textMuted
        }

        MouseArea {
            anchors.fill: parent
            onClicked: chip.clicked()
        }
    }

    component Satellite: FloatingWindow {
        id: sat

        property string label
        default property alias content: inner.data

        visible: false
        title: "MinkaMon // " + label
        color: Theme.ground

        // A WM-side close fires `closed` but leaves `visible` true, which
        // made reopening (visible = true) a no-op. Track reality so the
        // next click remaps the window.
        onClosed: visible = false

        Rectangle {
            anchors.fill: parent
            color: Theme.ground

            Item {
                id: inner

                anchors.fill: parent
                anchors.margins: 10
            }
        }
    }

    // Per-sensor temperature satellite: one big 60s trend chart. The
    // TrendLine records history from app start even while hidden.
    component TempSatellite: Satellite {
        id: tempSat

        property var value

        readonly property color tint: value === null || value === undefined
            ? Theme.textFaint
            : value >= 85 ? Theme.red
            : value >= 70 ? Theme.warnAmber : Theme.textMuted

        implicitWidth: 420
        implicitHeight: 280
        minimumSize: Qt.size(300, 200)

        Panel {
            anchors.fill: parent
            title: tempSat.label

            headerData: Text {
                anchors.verticalCenter: parent.verticalCenter
                text: tempSat.value === null
                    || tempSat.value === undefined
                    ? "—" : tempSat.value.toFixed(1) + "°C"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 2
                color: tempSat.tint
            }

            Rectangle {
                anchors.fill: parent
                color: Theme.surfaceRaised

                TrendLine {
                    anchors.fill: parent
                    anchors.margins: 1
                    value: tempSat.value === null
                        || tempSat.value === undefined
                        ? 0 : tempSat.value
                    maxValue: 100
                    lineColor: tempSat.tint
                    fillColor: Theme.gaugeDim
                }
            }
        }
    }

    FloatingWindow {
        id: win

        title: "MinkaMon"
        implicitWidth: 620
        implicitHeight: 560
        minimumSize: Qt.size(440, 420)
        color: Theme.ground

        // Real Item root inside the proxy window; also the grab target for
        // the debug screenshot hook (proxy items have no QML engine).
        // A Rectangle rather than an Item so grabs composite over the
        // ground colour instead of leaving alpha holes where only the
        // window background shows.
        Rectangle {
            id: grabRoot

            anchors.fill: parent
            color: Theme.ground

            Rectangle {
                id: topBar

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                height: 44
                color: Theme.surface

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: Theme.line
                }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 10

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "MINKAMON"
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize + 2
                        font.letterSpacing: 3
                        font.bold: true
                        color: Theme.red
                    }

                    // Sampler health dot, same spirit as the bar's IPC dot.
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 8
                        height: 8
                        radius: 4
                        color: Sampler.alive ? Theme.okGreen : Theme.redDim
                    }
                }

                Row {
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    TopChip {
                        label: "INFO"
                        active: shellRoot.infoMode
                        onClicked: shellRoot.infoMode = !shellRoot.infoMode
                    }

                    TopChip {
                        label: "TEMP"
                        active: shellRoot.tempMode
                        onClicked: shellRoot.tempMode = !shellRoot.tempMode
                    }

                    TopChip {
                        label: "OVERVIEW"
                        active: shellRoot.overviewPending
                        onClicked: shellRoot.fullOverview()
                    }

                    TopChip {
                        label: "CLOSE ALL"
                        onClicked: shellRoot.closeAll()
                    }

                    TopChip {
                        label: "PROCESSES"
                        active: procWin.visible
                        onClicked: procWin.visible = !procWin.visible
                    }
                }
            }

            SystemPanel {
                id: sysPanel
                anchors.top: topBar.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 10
                showInfo: shellRoot.infoMode
                showTemp: shellRoot.tempMode
                onZoneClicked: zone => shellRoot.openZone(zone)
            }
        }
    }

    Satellite {
        id: cpuWin

        label: "CPU"
        implicitWidth: 540
        implicitHeight: 430
        minimumSize: Qt.size(380, 300)

        CpuPanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: gpuWin

        label: "GPU"
        implicitWidth: 540
        implicitHeight: 400
        minimumSize: Qt.size(380, 300)

        GpuPanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: memWin

        label: "MEMORY"
        implicitWidth: 500
        implicitHeight: 430
        minimumSize: Qt.size(380, 320)

        MemoryPanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: netWin

        label: "NETWORK"
        implicitWidth: 520
        implicitHeight: 430
        minimumSize: Qt.size(380, 300)

        NetworkPanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: diskWin

        label: "DISK"
        implicitWidth: 520
        implicitHeight: 400
        minimumSize: Qt.size(380, 280)

        DiskPanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: globeWin

        label: "GLOBE"
        implicitWidth: 560
        implicitHeight: 560
        minimumSize: Qt.size(360, 360)

        GlobePanel {
            anchors.fill: parent
        }
    }

    Satellite {
        id: procWin

        label: "PROCESSES"
        implicitWidth: 820
        implicitHeight: 560
        minimumSize: Qt.size(520, 400)

        ProcessesPage {
            anchors.fill: parent
        }
    }

    TempSatellite {
        id: ssdTempWin

        label: "SSD °C"
        value: sysPanel.readings.ssd
    }

    TempSatellite {
        id: boardTempWin

        label: "BOARD °C"
        value: sysPanel.readings.board
    }

    TempSatellite {
        id: wifiTempWin

        label: "WIFI °C"
        value: sysPanel.readings.wifi
    }

    TempSatellite {
        id: cpuTempWin

        label: "CPU °C"
        value: sysPanel.readings.pkg
    }

    TempSatellite {
        id: gpuTempWin

        label: "GPU °C"
        value: sysPanel.gpuC
    }

    // The ShojiWM IPC only needs to stream window geometry while a leader
    // line could actually draw: main window plus at least one satellite.
    Binding {
        target: ShojiIpc
        property: "active"
        value: win.visible && (cpuWin.visible || gpuWin.visible
            || memWin.visible || netWin.visible
            || diskWin.visible
            || globeWin.visible
            || procWin.visible || ssdTempWin.visible
            || boardTempWin.visible || wifiTempWin.visible
            || cpuTempWin.visible || gpuTempWin.visible)
    }

    // Leader lines from schematic components to satellite window borders,
    // one click-through overlay per screen.
    Variants {
        model: Quickshell.screens

        LeaderOverlay {
            systemPanel: sysPanel
            ties: [
                { title: "MinkaMon // CPU", zone: "cpu" },
                { title: "MinkaMon // GPU", zone: "gpu" },
                { title: "MinkaMon // MEMORY", zone: "ram" },
                { title: "MinkaMon // NETWORK", zone: "wifi" },
                { title: "MinkaMon // GLOBE", zone: "wifi" },
                { title: "MinkaMon // WIFI °C", zone: "wifi" },
                { title: "MinkaMon // DISK", zone: "ssd" },
                { title: "MinkaMon // SSD °C", zone: "ssd" },
                { title: "MinkaMon // BOARD °C", zone: "board" },
                { 
                    title: "MinkaMon // CPU °C",
                    zone: "cpu",
                },
                {
                    title: "MinkaMon // GPU °C", 
                    zone: "gpu",
                },
                { title: "MinkaMon // PROCESSES", zone: "cpu" },
            ]
        }
    }
}