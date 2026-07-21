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

    function openZone(zone) {
        if (zone === "cpu")
            cpuWin.visible = true;
        else if (zone === "gpu")
            gpuWin.visible = true;
        else if (zone === "ram")
            memWin.visible = true;
        else if (zone === "wifi") {
            // The user decides which of the network trio stays open.
            netWin.visible = true;
            globeWin.visible = true;
            wifiTempWin.visible = true;
        } else if (zone === "ssd")
            ssdTempWin.visible = true;
        else if (zone === "board")
            boardTempWin.visible = true;
        else if (zone === "processes")
            procWin.visible = true;
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
    }

    component Satellite: FloatingWindow {
        id: sat

        property string label
        default property alias content: inner.data

        visible: false
        title: "MinkaMon // " + label
        color: Theme.ground

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

                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: procLabel.implicitWidth + 26
                    height: 28
                    radius: 5
                    color: procWin.visible
                        ? Theme.surfaceRaised : "transparent"
                    border.width: 1
                    border.color: procWin.visible ? Theme.red : Theme.line

                    Text {
                        id: procLabel

                        anchors.centerIn: parent
                        text: "PROCESSES"
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 1
                        font.letterSpacing: 2
                        color: procWin.visible
                            ? Theme.text : Theme.textMuted
                    }

                    MouseArea {
                        anchors.fill: parent
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
}