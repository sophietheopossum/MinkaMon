import Quickshell
import QtQuick
import "services"
import "modules"

// MinkaMon — the Minka system monitor. eDEX-UI-inspired instrument panels
// (memory blocks, rotating world view) in the Eternal Darkness palette,
// fed by scripts/sampler.py via the Sampler singleton.
ShellRoot {
    FloatingWindow {
        id: win

        title: "MinkaMon"
        implicitWidth: 1160
        implicitHeight: 720
        color: Theme.ground

        property string page: "monitor"

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

                Repeater {
                    model: [
                        { id: "monitor", label: "MONITOR" },
                        { id: "processes", label: "PROCESSES" },
                    ]

                    Rectangle {
                        width: tabLabel.implicitWidth + 26
                        height: 28
                        radius: 5
                        color: win.page === modelData.id
                            ? Theme.surfaceRaised : "transparent"
                        border.width: 1
                        border.color: win.page === modelData.id
                            ? Theme.red : Theme.line

                        Text {
                            id: tabLabel
                            anchors.centerIn: parent
                            text: modelData.label
                            font.family: Theme.monoFamily
                            font.pixelSize: Theme.fontSize - 1
                            font.letterSpacing: 2
                            color: win.page === modelData.id
                                ? Theme.text : Theme.textMuted
                        }

                        MouseArea {
                            anchors.fill: parent
                            onClicked: win.page = modelData.id
                        }
                    }
                }
            }
        }

        MonitorPage {
            anchors.top: topBar.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            visible: win.page === "monitor"
        }

        ProcessesPage {
            anchors.top: topBar.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            visible: win.page === "processes"
        }
    }
}