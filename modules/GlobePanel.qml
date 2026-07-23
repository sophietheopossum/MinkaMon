import QtQuick
import "../services"

// The eDEX-UI world view: slowly rotating orthographic wireframe globe with
// graticule, Natural Earth coastlines, and pulsing markers for the machine's
// live TCP peers (geolocated offline to country centroids).
Panel {
    id: root

    title: "WORLD VIEW · " + Sampler.conns.length + " LINKS"

    // Wireframe polylines arrive via the sampler's meta line (QML XHR can't
    // read local files without an env override).
    readonly property var coastlines: Sampler.coastlines
    property real rotation: 0
    property real pulse: 0

    Timer {
        interval: 66 // ~15 fps: plenty for a slow rotation, cheap on battery
        repeat: true
        running: root.visible
        onTriggered: {
            root.rotation = (root.rotation + 0.35) % 360;
            root.pulse = (root.pulse + 0.09) % 1;
            globe.requestPaint();
        }
    }

    Canvas {
        id: globe

        anchors.fill: parent

        function project(lat, lon) {
            // Orthographic projection, camera on the equator at `rotation`,
            // slight axial tilt for the eDEX look.
            const tilt = 18 * Math.PI / 180;
            const phi = lat * Math.PI / 180;
            const lam = (lon + root.rotation) * Math.PI / 180;
            let x = Math.cos(phi) * Math.sin(lam);
            let y = Math.sin(phi);
            let z = Math.cos(phi) * Math.cos(lam);
            const y2 = y * Math.cos(tilt) - z * Math.sin(tilt);
            const z2 = y * Math.sin(tilt) + z * Math.cos(tilt);
            return { x: x, y: y2, z: z2 };
        }

        onPaint: {
            const ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            const cx = width / 2;
            const cy = height / 2;
            const r = Math.min(width, height) / 2 - 8;
            if (r <= 0)
                return;

            // Disc + rim.
            ctx.beginPath();
            ctx.arc(cx, cy, r, 0, 2 * Math.PI);
            ctx.fillStyle = "#120a0d";
            ctx.fill();
            ctx.strokeStyle = Theme.redDim;
            ctx.lineWidth = 1;
            ctx.stroke();

            // Graticule.
            ctx.strokeStyle = "#2e222833";
            for (let lat = -60; lat <= 60; lat += 30)
                strokePath(ctx, latCircle(lat), cx, cy, r);
            for (let lon = 0; lon < 180; lon += 30)
                strokePath(ctx, lonCircle(lon), cx, cy, r);

            // Coastlines.
            ctx.strokeStyle = Theme.redDim;
            for (const line of root.coastlines)
                strokeFlat(ctx, line, cx, cy, r);

            // Connection markers.
            for (const conn of Sampler.conns) {
                if (conn.lat === undefined)
                    continue;
                const p = project(conn.lat, conn.lon);
                if (p.z < 0.02)
                    continue;
                const px = cx + p.x * r;
                const py = cy - p.y * r;
                ctx.beginPath();
                ctx.arc(px, py, 2, 0, 2 * Math.PI);
                ctx.fillStyle = Theme.red;
                ctx.fill();
                ctx.beginPath();
                ctx.arc(px, py, 2 + root.pulse * 7, 0, 2 * Math.PI);
                ctx.strokeStyle = Qt.rgba(1, 0.29, 0.37, 1 - root.pulse);
                ctx.lineWidth = 1;
                ctx.stroke();
            }
        }

        function latCircle(lat) {
            const pts = [];
            for (let lon = -180; lon <= 180; lon += 6)
                pts.push(lat, lon);
            return pts;
        }

        function lonCircle(lon) {
            const pts = [];
            for (let lat = -90; lat <= 90; lat += 6)
                pts.push(lat, lon);
            return pts;
        }

        // paths come as flat [a, b, a, b, ...]; latCircle/lonCircle emit
        // (lat, lon) pairs while coastlines.json stores (lon, lat).
        function strokePath(ctx, flat, cx, cy, r) {
            strokeProjected(ctx, flat, cx, cy, r, false);
        }

        function strokeFlat(ctx, flat, cx, cy, r) {
            strokeProjected(ctx, flat, cx, cy, r, true);
        }

        function strokeProjected(ctx, flat, cx, cy, r, lonFirst) {
            ctx.beginPath();
            let pen = false;
            for (let i = 0; i + 1 < flat.length; i += 2) {
                const lat = lonFirst ? flat[i + 1] : flat[i];
                const lon = lonFirst ? flat[i] : flat[i + 1];
                const p = project(lat, lon);
                if (p.z < 0) {
                    pen = false;
                    continue;
                }
                const px = cx + p.x * r;
                const py = cy - p.y * r;
                if (pen)
                    ctx.lineTo(px, py);
                else
                    ctx.moveTo(px, py);
                pen = true;
            }
            ctx.lineWidth = 1;
            ctx.stroke();
        }
    }

    // Peer readout under the globe frame, eDEX-terminal style.
    Column {
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: 1

        Repeater {
            model: Sampler.conns.slice(0, 8)

            Text {
                text: (modelData.country || "??") + " " + modelData.ip
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 4
                color: Theme.textFaint
            }
        }
    }
}