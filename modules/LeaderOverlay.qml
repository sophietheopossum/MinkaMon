import Quickshell
import Quickshell.Wayland
import QtQuick
import "../services"

// One per screen: a transparent, click-through layer surface drawing leader
// lines from the schematic's components to the borders of MinkaMon's
// satellite windows, using compositor-side geometry from ShojiIpc (Wayland
// never tells a client where its windows are). Each line is clipped
// against every window stacked above its two endpoint windows, so it reads
// as sitting just above the pair it connects: fully covered lines vanish
// entirely, partially covered ones draw only their visible spans, and a
// fullscreen window suppresses lines on its whole monitor.
PanelWindow {
    id: overlay

    property var modelData
    property var systemPanel: null
    // Windows are matched by claimed role ("typed segments"), never by
    // title — titles are mutable display strings.
    property string mainRole: "minkamon.main"
    // [{ 
    // role, 
    // zone 
    // }]
    property var ties: []

    // ShojiWM window rects include invisible chrome: a 14px edge-drag halo
    // ring, then the 2px window border, then the client surface. Lines
    // attach to (and clip against) the visible border, not the halo; the
    // schematic anchor lives in client-surface coordinates.
    readonly property real chromeInset: 14
    readonly property real clientInset: 16

    // [{ ax, ay, bx, by, spans: [[t0, t1], ...] }] in layout coords
    property var lines: []

    screen: modelData
    visible: lines.length > 0
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "minkamon-leaderlines"

    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }

    // Empty region: the overlay never takes input.
    mask: Region {}

    // Liang-Barsky: the [t0, t1] slice of segment a->b inside rect r, or
    // null when they don't intersect.
    function segRectInterval(x0, y0, x1, y1, r) {
        const dx = x1 - x0, dy = y1 - y0;
        let t0 = 0, t1 = 1;
        const p = [-dx, dx, -dy, dy];
        const q = [
            x0 - r.x, r.x + r.width - x0,
            y0 - r.y, r.y + r.height - y0,
        ];
        for (let i = 0; i < 4; i++) {
            if (p[i] === 0) {
                if (q[i] < 0)
                    return null;
            } else {
                const t = q[i] / p[i];
                if (p[i] < 0) {
                    if (t > t1)
                        return null;
                    if (t > t0)
                        t0 = t;
                } else {
                    if (t < t0)
                        return null;
                    if (t < t1)
                        t1 = t;
                }
            }
        }
        return [t0, t1];
    }

    // Complement of the covered intervals over [0, 1].
    function visibleSpans(covered) {
        covered.sort((a, b) => a[0] - b[0]);
        const spans = [];
        let pos = 0;
        for (const iv of covered) {
            if (iv[0] > pos)
                spans.push([pos, Math.min(iv[0], 1)]);
            pos = Math.max(pos, iv[1]);
            if (pos >= 1)
                break;
        }
        if (pos < 1)
            spans.push([pos, 1]);
        return spans.filter(s => s[1] - s[0] > 0.004);
    }

    function rebuild() {
        const out = [];
        const wins = ShojiIpc.
            byRole;
        const main = wins[
            mainRole
            ];
        const sys = systemPanel;
        if (ShojiIpc.active && main && !main.minimized && sys) {
            const all = ShojiIpc.windowList;
            const inset = r => ({
                x: r.x + chromeInset,
                y: r.y + chromeInset,
                width: r.width - chromeInset * 2,
                height: r.height - chromeInset * 2,
            });
            for (const tie of ties) {
                const sat = wins[
                    tie.
                        role
                    ];
                if (!sat || sat.minimized)
                    continue;
                if (ShojiIpc.fullscreenMonitors.indexOf(main.monitor) >= 0
                    || ShojiIpc.fullscreenMonitors
                        .indexOf(sat.monitor) >= 0)
                    continue;
                const a = sys.anchorScene(tie.zone);
                if (!a)
                    continue;
                const ax = main.x + clientInset + a.x;
                const ay = main.y + clientInset + a.y;
                // Attach to the satellite's visible red border: first
                // intersection of the anchor->centre segment with the
                // chrome-inset rect. An anchor inside it means overlap —
                // no line.
                const satVis = inset(sat);
                const cx = satVis.x + satVis.width / 2;
                const cy = satVis.y + satVis.height / 2;
                const hit = segRectInterval(ax, ay, cx, cy, satVis);
                if (!hit || hit[0] <= 0)
                    continue;
                const ex = ax + (cx - ax) * hit[0];
                const ey = ay + (cy - ay) * hit[0];
                // Occluders: anything stacked above both endpoints
                // (stacking approximated by focus recency)
                // clipped at their visible borders too...
                const zref = Math.max(main.lastFocusedAt,
                    sat.lastFocusedAt);
                const covered = [];
                for (const w of all) {
                    if (w.minimized)
                        continue;
                    if (w !== main && w !== sat
                        && w.lastFocusedAt > zref) {
                        const iv = segRectInterval(ax, ay, ex, ey,
                            inset(w));
                        if (iv)
                            covered.push(iv);
                    }
                    // ...and every revealed drag tab sits above the
                    // lines regardless of stacking — it's interactive
                    // chrome, including on the endpoint windows.
                    if (w.dragTab) {
                        const iv = segRectInterval(ax, ay, ex, ey,
                            w.dragTab);
                        if (iv)
                            covered.push(iv);
                    }
                }
                const spans = visibleSpans(covered);
                if (spans.length)
                    out.push({
                        ax: ax, ay: ay, bx: ex, by: ey, spans: spans,
                    });
            }
        }
        lines = out;
        canvas.requestPaint();
    }

    Connections {
        target: ShojiIpc

        function onUpdated() {
            overlay.rebuild();
        }
    }

    Canvas {
        id: canvas

        anchors.fill: parent

        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        onPaint: {
            const ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            const ox = overlay.screen ? overlay.screen.x : 0;
            const oy = overlay.screen ? overlay.screen.y : 0;
            ctx.strokeStyle = Theme.red;
            ctx.fillStyle = Theme.red;
            ctx.globalAlpha = 0.62;
            ctx.lineWidth = 1.2;
            for (const line of overlay.lines) {
                const dx = line.bx - line.ax, dy = line.by - line.ay;
                for (const s of line.spans) {
                    ctx.beginPath();
                    ctx.moveTo(line.ax + dx * s[0] - ox,
                        line.ay + dy * s[0] - oy);
                    ctx.lineTo(line.ax + dx * s[1] - ox,
                        line.ay + dy * s[1] - oy);
                    ctx.stroke();
                }
                const first = line.spans[0];
                const last = line.spans[line.spans.length - 1];
                // Tick at the component while it's visible...
                if (first[0] < 0.004)
                    ctx.fillRect(line.ax - ox - 2, line.ay - oy - 2, 4, 4);
                // ...and the node on the satellite's border likewise.
                if (last[1] > 0.996) {
                    ctx.beginPath();
                    ctx.arc(line.bx - ox, line.by - oy, 2.5,
                        0, Math.PI * 2);
                    ctx.fill();
                }
            }
            ctx.globalAlpha = 1;
        }
    }
}