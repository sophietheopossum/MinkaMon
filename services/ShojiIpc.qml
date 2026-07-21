pragma Singleton
import Quickshell
import Quickshell.Io
import QtQuick

// NDJSON client for the ShojiWM IPC socket, used by the leader-line overlay
// to learn where the compositor put MinkaMon's windows — Wayland itself
// never tells a client. Window move/resize doesn't trigger a compositor
// broadcast, so while `active` this polls workspaces.get; the payload is
// tiny and the socket is local.
Singleton {
    id: root

    // Consumers flip this while they need geometry (main window plus at
    // least one satellite open). Idle means no socket traffic at all.
    property bool active: false

    // Windows on *active* workspaces only, keyed by title:
    // { x, y, width, height, focused, lastFocusedAt, fullscreen,
    //   minimized, monitor }
    property var windows: ({})
    // Every such window, including title collisions the map would swallow
    // (matters for occlusion checks). Entries are shared with `windows`.
    property var windowList: []
    // Monitors currently showing a fullscreen window (no lines there).
    property var fullscreenMonitors: []
    signal updated()

    property int nextId: 1

    readonly property string socketPath: {
        const dir = Quickshell.env("XDG_RUNTIME_DIR");
        const disp = Quickshell.env("WAYLAND_DISPLAY");
        return dir && disp ? dir + "/shojiwm-" + disp + ".sock" : "";
    }

    onActiveChanged: {
        if (active && socketPath !== "")
            socket.connected = true;
        else if (!active) {
            socket.connected = false;
            windows = {};
            windowList = [];
            fullscreenMonitors = [];
            updated();
        }
    }

    function requestWindows() {
        if (!socket.connected)
            return;
        socket.write(JSON.stringify({
            id: root.nextId++,
            method: "workspaces.get",
        }) + "\n");
        socket.flush();
    }

    function applyView(view) {
        if (!view || !view.monitors)
            return;
        const map = {};
        const list = [];
        const fs = [];
        for (const mon of view.monitors) {
            for (const ws of mon.workspaces) {
                if (!ws.active)
                    continue;
                for (const w of ws.windows) {
                    // Older compositor sessions predate the rect field.
                    if (!w.rect)
                        continue;
                    if (w.fullscreen && !w.minimized
                            && fs.indexOf(mon.name) < 0)
                        fs.push(mon.name);
                    const entry = {
                        x: w.rect.x,
                        y: w.rect.y,
                        width: w.rect.width,
                        height: w.rect.height,
                        focused: w.focused === true,
                        lastFocusedAt: w.lastFocusedAt || 0,
                        fullscreen: w.fullscreen === true,
                        minimized: w.minimized === true,
                        monitor: mon.name,
                        dragTab: w.dragTab || null,
                    };
                    map[w.title] = entry;
                    list.push(entry);
                }
            }
        }
        root.windows = map;
        root.windowList = list;
        root.fullscreenMonitors = fs;
        root.updated();
    }

    Socket {
        id: socket

        path: root.socketPath

        parser: SplitParser {
            onRead: line => {
                let msg;
                try {
                    msg = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (msg.event === "workspaces.changed")
                    root.applyView(msg.payload);
                else if (msg.result !== undefined)
                    root.applyView(msg.result);
            }
        }

        onConnectionStateChanged: {
            if (connected)
                root.requestWindows();
        }
    }

    // Reconnect while wanted (the socket is recreated when the ShojiWM
    // config hot-reloads, so retrying forever is a feature).
    Timer {
        interval: 1000
        repeat: true
        running: root.active && !socket.connected
        onTriggered: {
            if (root.socketPath !== "")
                socket.connected = true;
        }
    }

    // Geometry poll while lines are live.
    Timer {
        interval: 200
        repeat: true
        running: root.active && socket.connected
        onTriggered: root.requestWindows()
    }
}