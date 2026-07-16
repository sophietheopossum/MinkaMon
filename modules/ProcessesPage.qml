import QtQuick
import "../services"

// Sortable process table. Column headers toggle sort key/direction.
Item {
    id: root

    property string sortKey: "cpuPct"
    property bool sortDesc: true

    readonly property var sorted: {
        const list = (Sampler.procs || []).slice();
        const key = sortKey;
        const sign = sortDesc ? -1 : 1;
        list.sort((a, b) => {
            const va = a[key], vb = b[key];
            if (typeof va === "string")
                return sign * va.localeCompare(vb);
            return sign * (va - vb);
        });
        return list;
    }

    function toggleSort(key) {
        if (sortKey === key)
            sortDesc = !sortDesc;
        else {
            sortKey = key;
            sortDesc = key !== "comm";
        }
    }

    readonly property var columns: [
        { key: "pid", label: "PID", width: 70, align: Text.AlignRight },
        { key: "comm", label: "NAME", width: 0, align: Text.AlignLeft },
        { key: "state", label: "S", width: 30, align: Text.AlignHCenter },
        { key: "cpuPct", label: "CPU %", width: 80, align: Text.AlignRight },
        { key: "rssKb", label: "MEMORY", width: 100, align: Text.AlignRight },
    ]

    function flexWidth() {
        let fixed = 0;
        for (const col of columns)
            fixed += col.width;
        return listView.width - fixed - 24;
    }

    Rectangle {
        id: headerRow

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 10
        anchors.bottomMargin: 0
        height: 30
        color: Theme.surface
        border.width: 1
        border.color: Theme.line

        Row {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 0

            Repeater {
                model: root.columns

                Item {
                    width: modelData.width > 0 ? modelData.width : root.flexWidth()
                    height: headerRow.height

                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: modelData.align
                        text: modelData.label
                            + (root.sortKey === modelData.key
                                ? (root.sortDesc ? " ▾" : " ▴") : "")
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 2
                        font.letterSpacing: 1
                        color: root.sortKey === modelData.key
                            ? Theme.red : Theme.textMuted
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.toggleSort(modelData.key)
                    }
                }
            }
        }
    }

    ListView {
        id: listView

        anchors.top: headerRow.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 10
        anchors.topMargin: 4
        clip: true
        model: root.sorted

        delegate: Rectangle {
            width: listView.width
            height: 24
            color: index % 2 === 0 ? "transparent" : Theme.surface

            Row {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                spacing: 0

                Text {
                    width: 70
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: modelData.pid
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.textFaint
                }

                Text {
                    width: root.flexWidth()
                    height: parent.height
                    leftPadding: 14
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: modelData.comm
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.text
                }

                Text {
                    width: 30
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData.state
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: modelData.state === "R" ? Theme.okGreen : Theme.textFaint
                }

                Text {
                    width: 80
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: modelData.cpuPct.toFixed(1)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: modelData.cpuPct >= 50 ? Theme.red
                        : modelData.cpuPct >= 10 ? Theme.warnAmber : Theme.textMuted
                }

                Text {
                    width: 100
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: Sampler.fmtKb(modelData.rssKb)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.textMuted
                }
            }
        }
    }
}