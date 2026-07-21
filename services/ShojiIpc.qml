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
    // Usable (layer-exclusive-zone-free) area per monitor name, from
    // debug.geometry — filled on requestGeometry().
    property var usableAreas: ({})
    signal updated()

    property int nextId: 1
    property int geomRequestId: -1

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

    function requestGeometry() {
        if (!socket.connected)
            return;
        root.geomRequestId = root.nextId;
        socket.write(JSON.stringify({
            id: root.nextId++,
            method: "debug.geometry",
        }) + "\n");
        socket.flush();
    }

    // Move/resize a window (layout coords, chrome-inclusive rect).
    function setRect(windowId, x, y, width, height) {
        if (!socket.connected)
            return;
        socket.write(JSON.stringify({
            id: root.nextId++,
            method: "windows.setRect",
            params: {
                windowId: windowId,
                x: x,
                y: y,
                width: width,
                height: height,
            },
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
                        id: w.id,
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

    // Event-rate rect updates during drags/resizes: patch the existing
    // entries in place (they're shared with `windows`) and re-emit.
    function applyRects(payload) {
        if (!payload || !payload.windows)
            return;
        const byId = {};
        for (const w of root.windowList) {
            if (w.id)
                byId[w.id] = w;
        }
        let touched = false;
        for (const u of payload.windows) {
            const entry = byId[u.id];
            if (!entry)
                continue;
            entry.x = u.x;
            entry.y = u.y;
            entry.width = u.width;
            entry.height = u.height;
            entry.dragTab = u.dragTab || null;
            touched = true;
        }
        if (touched)
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
                else if (msg.event === "windows.rects")
                    root.applyRects(msg.payload);
                else if (msg.id === root.geomRequestId
                        && msg.result !== undefined)
                    root.usableAreas = msg.result.usable || {};
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