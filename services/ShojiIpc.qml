pragma Singleton
import Quickshell
import QtQuick
// Through the config-root symlink: Quickshell only honours qmldir
// singleton registration for paths inside the shell root.
import "../MinkaLink"

// MinkaMon's window-geometry view of the ShojiWM IPC, layered on
// MinkaLink's ShojiClient transport. Window move/resize doesn't trigger a
// compositor broadcast, so while `active` this polls workspaces.get; the
// payload is tiny and the socket is local.
// Idle still means no socket traffic at all: `active` gates ShojiClient.wanted,
// which drops the connection entirely.
Singleton {
    id: root

    // Consumers flip this while they need geometry (main window plus at
    // least one satellite open).
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

    onActiveChanged: {
        ShojiClient.wanted = active;
        if (!active) {
            windows = {};
            windowList = [];
            fullscreenMonitors = [];
            updated();
        }
    }

    function requestWindows() {
        ShojiClient.request("workspaces.get", undefined, (result, error) => {
            if (result)
                root.applyView(result);
        });
    }

    function requestGeometry() {
        ShojiClient.request("debug.geometry", undefined, (result, error) => {
            if (result)
                root.usableAreas = result.usable || {};
        });
    }

    // Move/resize a window (layout coords, chrome-inclusive rect).
    function setRect(windowId, x, y, width, height) {
        ShojiClient.send("windows.setRect", {
            windowId: windowId,
            x: x,
            y: y,
            width: width,
            height: height,
        });
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

    Connections {
        target: ShojiClient

        function onBroadcast(name, payload) {
            if (!root.active)
                return;
            if (name === "workspaces.changed")
                root.applyView(payload);
            else if (name === "windows.rects")
                root.applyRects(payload);
        }

        function onConnectedChanged() {
            if (ShojiClient.connected && root.active)
                root.requestWindows();
        }
    }

    // Geometry poll while lines are live.
    Timer {
        interval: 200
        repeat: true
        running: root.active && ShojiClient.connected
        onTriggered: root.requestWindows()
    }

    // MinkaMon starts idle: no satellites, no socket. ShojiClient defaults
    // wanted:true, so rein it in before its first connect attempt lands.
    Component.onCompleted: ShojiClient.wanted = active
}