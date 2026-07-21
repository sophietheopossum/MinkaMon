import QtQuick
import "../services"

// Page-level wiring: leader lines from each panel's tie point to its
// component on the SystemPanel schematic, routed through the column gutters
// and the lane region above the chassis, plus risers from the thermal
// readout strip up into the board. Hovering a line lights it up, runs
// pulses toward the component, and names the target — so there's never any
// doubt where a line points.
Item {
    id: root

    // [{ item, at (0..1 down the panel edge), target }]
    property var ties: []
    property var system: null

    // Rebuilt on every paint: [{ pts, len, label, riser }]
    property var routes: []
    property int hovered: -1
    property real phase: 0

    NumberAnimation on phase {
        from: 0
        to: 1
        duration: 1400
        loops: Animation.Infinite
        running: root.hovered >= 0
    }

    onPhaseChanged: canvas.requestPaint()
    onHoveredChanged: canvas.requestPaint()

    function mkRoute(pts, label, riser) {
        let len = 0;
        for (let k = 0; k < pts.length - 1; k++)
            len += Math.hypot(pts[k + 1].x - pts[k].x,
                pts[k + 1].y - pts[k].y);
        return { pts: pts, len: len, label: label, riser: riser };
    }

    function buildRoutes() {
        const out = [];
        if (!system || !system.schematicItem)
            return out;
        const sm = system.schematicItem;
        const org = sm.mapToItem(root, 0, 0);
        let lane = 0;
        for (const t of ties) {
            const a = system.anchorPoint(t.target, "top");
            if (!a || !t.item || !t.item.visible)
                continue;
            const end = { x: org.x + a.x, y: org.y + a.y - 3 };
            const fromLeft = t.item.x + t.item.width <= org.x;
            const sy = t.item.y + t.item.height
                * (t.at === undefined ? 0.5 : t.at);
            const sx = fromLeft ? t.item.x + t.item.width : t.item.x;
            const gx = fromLeft
                ? sx + 3 + lane * 2.5 : sx - 3 - lane * 2.5;
            const laneY = org.y + system.laneBottom - 6 - lane * 5;
            out.push(mkRoute([
                { x: sx, y: sy },
                { x: gx, y: sy },
                { x: gx, y: laneY },
                { x: end.x, y: laneY },
                end,
            ], system.labelFor(t.target), false));
            lane++;
        }
        for (const rt of (system.readoutTies || [])) {
            const a = system.anchorPoint(rt.target, "bottom");
            if (!a || !rt.item)
                continue;
            const end = { x: org.x + a.x, y: org.y + a.y + 3 };
            const s = rt.item.mapToItem(root, rt.item.width / 2, 0);
            out.push(mkRoute([
                { x: s.x, y: s.y },
                { x: s.x, y: end.y + 22 },
                end,
            ], system.labelFor(rt.target), true));
        }
        return out;
    }

    function segDist(px, py, ax, ay, bx, by) {
        const dx = bx - ax, dy = by - ay;
        const len2 = dx * dx + dy * dy;
        let t = len2 > 0
            ? ((px - ax) * dx + (py - ay) * dy) / len2 : 0;
        t = Math.max(0, Math.min(1, t));
        return Math.hypot(px - (ax + dx * t), py - (ay + dy * t));
    }

    function routeAt(px, py) {
        for (let i = 0; i < routes.length; i++) {
            const p = routes[i].pts;
            for (let k = 0; k < p.length - 1; k++)
                if (segDist(px, py, p[k].x, p[k].y,
                        p[k + 1].x, p[k + 1].y) < 8)
                    return i;
        }
        return -1;
    }

    function pointAlong(route, t) {
        const p = route.pts;
        const want = route.len * t;
        let acc = 0;
        for (let k = 0; k < p.length - 1; k++) {
            const seg = Math.hypot(p[k + 1].x - p[k].x,
                p[k + 1].y - p[k].y);
            if (seg > 0 && acc + seg >= want) {
                const f = (want - acc) / seg;
                return {
                    x: p[k].x + (p[k + 1].x - p[k].x) * f,
                    y: p[k].y + (p[k + 1].y - p[k].y) * f,
                };
            }
            acc += seg;
        }
        return p[p.length - 1];
    }

    Canvas {
        id: canvas

        anchors.fill: parent

        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        Connections {
            target: Sampler

            function onTicked() {
                canvas.requestPaint();
            }
        }

        onPaint: {
            const ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            root.routes = root.buildRoutes();
            const hov = root.hovered;
            for (let i = 0; i < root.routes.length; i++) {
                const r = root.routes[i];
                const hot = i === hov;
                const base = r.riser ? 0.4 : 0.6;
                ctx.globalAlpha = hov < 0 ? base : (hot ? 1 : 0.15);
                ctx.strokeStyle = Theme.glow;
                ctx.lineWidth = hot ? 1.8 : 1.2;
                ctx.beginPath();
                ctx.moveTo(r.pts[0].x, r.pts[0].y);
                for (let k = 1; k < r.pts.length; k++)
                    ctx.lineTo(r.pts[k].x, r.pts[k].y);
                ctx.stroke();

                const end = r.pts[r.pts.length - 1];
                ctx.fillStyle = Theme.glow;
                ctx.beginPath();
                ctx.arc(end.x, end.y, hot ? 3.2 : 2.2, 0, Math.PI * 2);
                ctx.fill();
                if (!r.riser) {
                    const s0 = r.pts[0];
                    ctx.fillRect(s0.x - 2, s0.y - 2, 4, 4);
                }

                if (hot) {
                    // Expanding ring at the target...
                    ctx.globalAlpha = (1 - root.phase) * 0.8;
                    ctx.lineWidth = 1.2;
                    ctx.beginPath();
                    ctx.arc(end.x, end.y, 4 + root.phase * 9, 0,
                        Math.PI * 2);
                    ctx.stroke();
                    // ...pulses travelling toward it...
                    ctx.globalAlpha = 0.95;
                    for (let k = 0; k < 3; k++) {
                        const p = root.pointAlong(r,
                            (root.phase + k / 3) % 1);
                        ctx.beginPath();
                        ctx.arc(p.x, p.y, 2, 0, Math.PI * 2);
                        ctx.fill();
                    }
                    // ...and the target's name.
                    ctx.font = "10px " + Theme.monoFamily;
                    const tw = ctx.measureText(r.label).width;
                    let lx = end.x + 10;
                    if (lx + tw + 8 > width)
                        lx = end.x - tw - 10;
                    const ly = r.riser ? end.y + 16 : end.y - 12;
                    ctx.globalAlpha = 0.92;
                    ctx.fillStyle = Theme.ground;
                    ctx.fillRect(lx - 4, ly - 10, tw + 8, 14);
                    ctx.strokeStyle = Theme.glow;
                    ctx.lineWidth = 1;
                    ctx.strokeRect(lx - 4, ly - 10, tw + 8, 14);
                    ctx.fillStyle = Theme.glow;
                    ctx.fillText(r.label, lx, ly + 1);
                }
            }
            ctx.globalAlpha = 1;
        }
    }

    HoverHandler {
        onPointChanged: {
            const h = root.routeAt(point.position.x, point.position.y);
            if (h !== root.hovered)
                root.hovered = h;
        }
        onHoveredChanged: {
            if (!hovered)
                root.hovered = -1;
        }
    }
}