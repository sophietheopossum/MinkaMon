pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import "../services"

// Sortable process table. Column headers toggle sort key/direction.
// Right-clicking a row opens a kill menu (SIGTERM / SIGKILL).
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

    // comm → resolved icon source. /proc comms rarely match desktop entry ids
    // directly, so first try the basename of each entry's Exec line, then the
    // heuristic lookup, then prefix-match (comm is truncated to 15 chars).
    // Held in one readonly object mutated in place: Image source bindings call
    // iconFor(), and a notifying property assignment mid-evaluation would be a
    // binding loop.
    readonly property var iconState: (
        { 
            cache: ({}), 
            execMap: null,
        }
    )

    function buildExecMap() {
        const map = {};
        const apps = DesktopEntries.applications.values;
        for (let i = 0; i < apps.length; i++) {
            const entry = apps[i];
            if (!entry.icon)
                continue;
            const toks = (entry.execString || "").split(/\s+/);
            for (const t of toks) {
                if (!t || t === "env" || t.includes("=")
                        || t.startsWith("-") || t.startsWith("%"))
                    continue;
                const base = t.split("/").pop().toLowerCase();
                if (base && map[base] === undefined)
                    map[base] = entry.icon;
                break;
            }
        }
        return map;
    }

    function iconFor(comm) {
        const state = iconState;
        const key = comm.toLowerCase();
        const hit = state.cache[key];
        if (hit !== undefined)
            return hit;
        if (state.execMap === null)
            state.execMap = buildExecMap();
        let icon = state.execMap[key] || "";
        if (!icon) {
            const entry = DesktopEntries.heuristicLookup(comm);
            if (entry && entry.icon)
                icon = entry.icon;
        }
        if (!icon && comm.length === 15) {
            for (const k in state.execMap) {
                if (k.startsWith(key)) {
                    icon = state.execMap[k];
                    break;
                }
            }
        }
        const src = icon
            ? Quickshell.iconPath(icon, "application-x-executable")
            : Quickshell.iconPath("application-x-executable");
        state.cache[key] = src;
        return src;
    }

    // Kill menu state. Snapshot pid/comm so the row churning out from under
    // the menu on the next procs tick doesn't retarget it.
    property var menuProc: null
    property real menuX: 0
    property real menuY: 0

    function openMenu(proc, x, y) {
        menuProc = { pid: proc.pid, comm: proc.comm };
        menuX = Math.max(0, Math.min(x, width - killMenu.width - 4));
        menuY = Math.max(0, Math.min(y, height - killMenu.height - 4));
    }

    function sendSignal(sig) {
        if (menuProc)
            Quickshell.execDetached(["kill", "-" + sig, String(menuProc.pid)]);
        menuProc = null;
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
                    id: headerCell

                    required property var modelData

                    width: modelData.width > 0 ? modelData.width : root.flexWidth()
                    height: headerRow.height

                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: headerCell.modelData.align
                        leftPadding: headerCell.modelData.key === "comm" ? 14 : 0
                        text: headerCell.modelData.label
                            + (root.sortKey === headerCell.modelData.key
                                ? (root.sortDesc ? " ▾" : " ▴") : "")
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 2
                        font.letterSpacing: 1
                        color: root.sortKey === headerCell.modelData.key
                            ? Theme.red : Theme.textMuted
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.toggleSort(headerCell.modelData.key)
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
            id: procRow

            required property var modelData
            required property int index

            width: listView.width
            height: 24
            color: index % 2 === 0 ? "transparent" : Theme.surface

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onPressed: mouse => {
                    const p = mapToItem(root, mouse.x, mouse.y);
                    root.openMenu(procRow.modelData, p.x, p.y);
                }
            }

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
                    text: procRow.modelData.pid
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.textFaint
                }

                Item {
                    width: root.flexWidth()
                    height: parent.height

                    Image {
                        id: procIcon
                        x: 14
                        anchors.verticalCenter: parent.verticalCenter
                        width: 16
                        height: 16
                        sourceSize: Qt.size(32, 32)
                        source: root.iconFor(procRow.modelData.comm)
                    }

                    Text {
                        anchors.left: procIcon.right
                        anchors.leftMargin: 8
                        anchors.right: parent.right
                        height: parent.height
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                        text: procRow.modelData.comm
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 2
                        color: Theme.text
                    }
                }

                Text {
                    width: 30
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: procRow.modelData.state
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: procRow.modelData.state === "R" ? Theme.okGreen : Theme.textFaint
                }

                Text {
                    width: 80
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: procRow.modelData.cpuPct.toFixed(1)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: procRow.modelData.cpuPct >= 50 ? Theme.red
                        : procRow.modelData.cpuPct >= 10 ? Theme.warnAmber : Theme.textMuted
                }

                Text {
                    width: 100
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignRight
                    text: Sampler.fmtKb(procRow.modelData.rssKb)
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.textMuted
                }
            }
        }
    }

    // Click-away catcher under the kill menu.
    MouseArea {
        anchors.fill: parent
        visible: root.menuProc !== null
        acceptedButtons: Qt.AllButtons
        z: 10
        onPressed: root.menuProc = null
    }

    Rectangle {
        id: killMenu

        visible: root.menuProc !== null
        x: root.menuX
        y: root.menuY
        width: 190
        height: menuColumn.height + 2
        color: Theme.surfaceRaised
        border.width: 1
        border.color: Theme.line
        z: 11

        Column {
            id: menuColumn

            x: 1
            y: 1
            width: parent.width - 2

            Rectangle {
                width: menuColumn.width
                height: 26
                color: Theme.surface

                Text {
                    anchors.fill: parent
                    leftPadding: 10
                    rightPadding: 10
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: root.menuProc
                        ? root.menuProc.comm + "  [" + root.menuProc.pid + "]" : ""
                    font.family: Theme.monoFamily
                    font.pixelSize: Theme.fontSize - 2
                    color: Theme.textMuted
                }
            }

            Repeater {
                model: [
                    { label: "TERMINATE", sig: "TERM", hint: "SIGTERM" },
                    { label: "KILL", sig: "KILL", hint: "SIGKILL" },
                ]

                Rectangle {
                    id: optionRow

                    required property var modelData

                    width: menuColumn.width
                    height: 28
                    color: optionArea.containsMouse ? Theme.redDim : "transparent"

                    Text {
                        anchors.fill: parent
                        leftPadding: 10
                        verticalAlignment: Text.AlignVCenter
                        text: optionRow.modelData.label
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 2
                        font.letterSpacing: 1
                        color: optionRow.modelData.sig === "KILL" && !optionArea.containsMouse
                            ? Theme.red : Theme.text
                    }

                    Text {
                        anchors.fill: parent
                        rightPadding: 10
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: Text.AlignRight
                        text: optionRow.modelData.hint
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        color: Theme.textFaint
                    }

                    MouseArea {
                        id: optionArea
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: root.sendSignal(optionRow.modelData.sig)
                    }
                }
            }
        }
    }
}