import QtQuick
import "../services"

// Live thermal map: a red schematic of the machine's internals with heat
// blobs rendered at the physical sensor locations. Knows the ZenBook UX482's
// board layout (per the UX482EG teardown: dual fans at the hinge, one heat
// pipe run across CPU + MX450, M.2 below the left fan, Wi-Fi right of
// centre, battery and speakers along the front). Anything else gets a
// generic tower.
Panel {
    id: root

    title: "THERMAL"

    readonly property bool zenbook:
        Sampler.machineModel.indexOf("UX482") >= 0

    readonly property var nv: Sampler.gpu ? Sampler.gpu.nvidia : null
    readonly property var gpuC:
        nv && nv.asleep === false ? nv.tempC : null
    readonly property bool gpuDormant:
        Sampler.hasNvidia && (!nv || nv.asleep !== false)

    // Sensor readings mapped to physical components. Recomputed whenever
    // the sampler publishes a fresh temps array.
    readonly property var readings: {
        const find = pred => {
            for (const s of Sampler.temps)
                if (pred(s))
                    return s.c;
            return null;
        };
        let pkg = find(s => s.chip === "coretemp"
            && s.label.indexOf("Package") === 0);
        if (pkg === null)
            pkg = find(s => s.chip === "k10temp"
                && (s.label === "Tctl" || s.label === "Tdie"));
        let ssd = find(s => s.chip === "nvme" && s.label === "Composite");
        if (ssd === null)
            ssd = find(s => s.chip === "nvme");
        return {
            pkg: pkg,
            cores: Sampler.temps
                .filter(s => s.chip === "coretemp"
                    && s.label.indexOf("Core") === 0)
                .map(s => s.c),
            ssd: ssd,
            wifi: find(s => s.chip.indexOf("iwlwifi") === 0
                || s.chip.indexOf("mt76") === 0),
            board: find(s => s.chip === "acpitz"),
        };
    }

    readonly property var maxC: {
        let m = null;
        for (const s of Sampler.temps)
            m = m === null ? s.c : Math.max(m, s.c);
        if (gpuC !== null)
            m = m === null ? gpuC : Math.max(m, gpuC);
        return m;
    }

    headerData: Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.maxC !== null
            ? "MAX " + root.maxC.toFixed(0) + "°C" : ""
        font.family: Theme.monoFamily
        font.pixelSize: Theme.fontSize - 4
        color: root.maxC >= 85 ? Theme.red
            : root.maxC >= 70 ? Theme.warnAmber : Theme.textFaint
    }

    Canvas {
        id: map

        anchors.fill: parent

        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        Connections {
            target: Sampler

            function onTicked() {
                map.requestPaint();
            }
        }

        // Temperature -> [r, g, b]: ember -> red -> glow -> amber ->
        // white-hot.
        function heatRgb(c) {
            const stops = [
                [30, 70, 15, 25],
                [50, 143, 30, 45],
                [65, 224, 38, 60],
                [78, 255, 74, 94],
                [88, 224, 160, 38],
                [96, 255, 236, 210],
            ];
            if (c <= stops[0][0])
                return stops[0].slice(1);
            for (let i = 1; i < stops.length; i++) {
                if (c <= stops[i][0]) {
                    const a = stops[i - 1], b = stops[i];
                    const t = (c - a[0]) / (b[0] - a[0]);
                    return [
                        Math.round(a[1] + (b[1] - a[1]) * t),
                        Math.round(a[2] + (b[2] - a[2]) * t),
                        Math.round(a[3] + (b[3] - a[3]) * t),
                    ];
                }
            }
            return stops[stops.length - 1].slice(1);
        }

        function roundedPath(ctx, x, y, w, h, r) {
            ctx.beginPath();
            ctx.moveTo(x + r, y);
            ctx.arcTo(x + w, y, x + w, y + h, r);
            ctx.arcTo(x + w, y + h, x, y + h, r);
            ctx.arcTo(x, y + h, x, y, r);
            ctx.arcTo(x, y, x + w, y, r);
            ctx.closePath();
        }

        function blob(ctx, x, y, r, c) {
            if (c === null || c === undefined)
                return;
            const rgb = heatRgb(c);
            const a = 0.18
                + Math.max(0, Math.min((c - 30) / 60, 1)) * 0.55;
            const g = ctx.createRadialGradient(x, y, 0, x, y, r);
            g.addColorStop(0, "rgba(" + rgb[0] + "," + rgb[1] + ","
                + rgb[2] + "," + a.toFixed(3) + ")");
            g.addColorStop(1, "rgba(" + rgb[0] + "," + rgb[1] + ","
                + rgb[2] + ",0)");
            ctx.fillStyle = g;
            ctx.beginPath();
            ctx.arc(x, y, r, 0, Math.PI * 2);
            ctx.fill();
        }

        // "NAME 62°" label; the value tints with the shared warn scheme.
        function tag(ctx, x, y, name, c, opts) {
            opts = opts || {};
            ctx.font = "9px " + Theme.monoFamily;
            ctx.textAlign = "left";
            const value = c === null || c === undefined
                ? (opts.fallback || "—") : c.toFixed(0) + "°";
            const nameW = ctx.measureText(name + " ").width;
            let sx = x;
            if (opts.align === "center")
                sx = x - (nameW + ctx.measureText(value).width) / 2;
            else if (opts.align === "right")
                sx = x - nameW - ctx.measureText(value).width;
            ctx.fillStyle = Theme.textFaint;
            ctx.fillText(name + " ", sx, y);
            ctx.fillStyle = c === null || c === undefined ? Theme.textFaint
                : c >= 85 ? Theme.red
                : c >= 70 ? Theme.warnAmber : Theme.textMuted;
            ctx.fillText(value, sx + nameW, y);
        }

        function fan(ctx, x, y, r) {
            ctx.strokeStyle = Theme.redDim;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.arc(x, y, r, 0, Math.PI * 2);
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(x, y, r * 0.22, 0, Math.PI * 2);
            ctx.stroke();
            for (let k = 0; k < 5; k++) {
                const a = k / 5 * Math.PI * 2;
                ctx.beginPath();
                ctx.moveTo(x + Math.cos(a) * r * 0.3,
                    y + Math.sin(a) * r * 0.3);
                ctx.quadraticCurveTo(
                    x + Math.cos(a + 0.5) * r * 0.72,
                    y + Math.sin(a + 0.5) * r * 0.72,
                    x + Math.cos(a + 0.9) * r * 0.95,
                    y + Math.sin(a + 0.9) * r * 0.95);
                ctx.stroke();
            }
        }

        // Bottom-cover-off top view, hinge at the top. Board units are mm
        // on the real 324x222 chassis, scaled to fit.
        function paintZenbook(ctx) {
            const bw = 324, bh = 222, m = 6;
            const s = Math.min((width - m * 2) / bw,
                (height - m * 2) / bh);
            const ox = (width - bw * s) / 2;
            const oy = (height - bh * s) / 2;
            const X = u => ox + u * s;
            const Y = v => oy + v * s;
            const S = u => u * s;
            const r = root.readings;

            // Heat blobs first, additive, clipped to the chassis.
            ctx.save();
            roundedPath(ctx, X(2), Y(2), S(320), S(218), S(10));
            ctx.clip();
            ctx.globalCompositeOperation = "lighter";
            blob(ctx, X(165), Y(62), S(62), r.board);
            blob(ctx, X(140), Y(45), S(52), r.pkg);
            blob(ctx, X(192), Y(50), S(40), root.gpuC);
            blob(ctx, X(75), Y(101), S(36), r.ssd);
            blob(ctx, X(250), Y(74), S(28), r.wifi);
            ctx.restore();

            // Chassis + hinge stubs.
            ctx.strokeStyle = Theme.redDim;
            ctx.lineWidth = 1;
            roundedPath(ctx, X(2), Y(2), S(320), S(218), S(10));
            ctx.stroke();
            ctx.globalAlpha = 0.5;
            ctx.fillStyle = Theme.redDim;
            ctx.fillRect(X(80), Y(2), S(44), S(5));
            ctx.fillRect(X(200), Y(2), S(44), S(5));

            // Motherboard region under the ScreenPad.
            ctx.globalAlpha = 0.6;
            roundedPath(ctx, X(18), Y(12), S(288), S(74), S(4));
            ctx.stroke();
            ctx.globalAlpha = 1;

            fan(ctx, X(45), Y(45), S(30));
            fan(ctx, X(279), Y(45), S(30));

            // Heat pipe run: left fan -> CPU -> MX450 -> right fan.
            ctx.strokeStyle = Theme.redDim;
            ctx.globalAlpha = 0.8;
            ctx.lineWidth = Math.max(1.5, S(4));
            ctx.beginPath();
            ctx.moveTo(X(70), Y(40));
            ctx.lineTo(X(126), Y(40));
            ctx.lineTo(X(150), Y(45));
            ctx.lineTo(X(190), Y(50));
            ctx.lineTo(X(230), Y(45));
            ctx.lineTo(X(254), Y(42));
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(X(72), Y(50));
            ctx.lineTo(X(140), Y(54));
            ctx.lineTo(X(192), Y(58));
            ctx.stroke();
            ctx.globalAlpha = 1;
            ctx.lineWidth = 1;

            // CPU die, one quadrant per core.
            const q = root.readings.cores;
            for (let i = 0; i < Math.min(q.length, 4); i++) {
                const rgb = heatRgb(q[i]);
                ctx.fillStyle = "rgba(" + rgb[0] + "," + rgb[1] + ","
                    + rgb[2] + ",0.45)";
                ctx.fillRect(X(129 + (i % 2) * 11),
                    Y(34 + Math.floor(i / 2) * 11), S(11), S(11));
            }
            ctx.strokeStyle = Theme.red;
            ctx.strokeRect(X(129), Y(34), S(22), S(22));

            // MX450 die.
            ctx.strokeRect(X(184), Y(42), S(16), S(16));

            // M.2 SSD with its connector notch.
            ctx.strokeRect(X(35), Y(95), S(80), S(12));
            ctx.beginPath();
            ctx.moveTo(X(41), Y(95));
            ctx.lineTo(X(41), Y(107));
            ctx.stroke();

            // Wi-Fi module.
            ctx.strokeRect(X(242), Y(67), S(16), S(14));

            // Battery + speakers along the front edge.
            ctx.strokeStyle = Theme.redDim;
            ctx.globalAlpha = 0.7;
            roundedPath(ctx, X(60), Y(120), S(204), S(85), S(6));
            ctx.stroke();
            roundedPath(ctx, X(20), Y(175), S(30), S(30), S(4));
            ctx.stroke();
            roundedPath(ctx, X(274), Y(175), S(30), S(30), S(4));
            ctx.stroke();
            for (let k = 0; k < 3; k++) {
                ctx.beginPath();
                ctx.moveTo(X(26 + k * 7), Y(200));
                ctx.lineTo(X(38 + k * 7), Y(181));
                ctx.moveTo(X(280 + k * 7), Y(200));
                ctx.lineTo(X(292 + k * 7), Y(181));
                ctx.stroke();
            }
            ctx.globalAlpha = 1;
            ctx.font = "9px " + Theme.monoFamily;
            ctx.fillStyle = Theme.textFaint;
            ctx.textAlign = "center";
            ctx.fillText("BATTERY", X(162), Y(166));
            ctx.textAlign = "left";

            tag(ctx, X(129), Y(30), "CPU", r.pkg);
            tag(ctx, X(184), Y(74), "MX450", root.gpuC,
                { fallback: root.gpuDormant ? "DORMANT" : "—" });
            tag(ctx, X(35), Y(91), "SSD", r.ssd);
            tag(ctx, X(242), Y(63), "WIFI", r.wifi);
            tag(ctx, X(20), Y(82), "BOARD", r.board);
        }

        // Generic side-view tower for machines this panel doesn't know.
        function paintTower(ctx) {
            const bw = 440, bh = 400, m = 6;
            const s = Math.min((width - m * 2) / bw,
                (height - m * 2) / bh);
            const ox = (width - bw * s) / 2;
            const oy = (height - bh * s) / 2;
            const X = u => ox + u * s;
            const Y = v => oy + v * s;
            const S = u => u * s;
            const r = root.readings;

            ctx.save();
            roundedPath(ctx, X(5), Y(5), S(430), S(390), S(8));
            ctx.clip();
            ctx.globalCompositeOperation = "lighter";
            blob(ctx, X(252), Y(112), S(50), r.pkg);
            blob(ctx, X(255), Y(237), S(46), root.gpuC);
            blob(ctx, X(240), Y(275), S(30), r.ssd);
            blob(ctx, X(341), Y(276), S(34), r.board);
            blob(ctx, X(397), Y(306), S(24), r.wifi);
            ctx.restore();

            ctx.strokeStyle = Theme.redDim;
            ctx.lineWidth = 1;
            roundedPath(ctx, X(5), Y(5), S(430), S(390), S(8));
            ctx.stroke();

            // Motherboard region.
            ctx.globalAlpha = 0.6;
            roundedPath(ctx, X(170), Y(40), S(240), S(250), S(4));
            ctx.stroke();
            ctx.globalAlpha = 1;

            fan(ctx, X(35), Y(120), S(26));
            fan(ctx, X(35), Y(210), S(26));
            fan(ctx, X(398), Y(80), S(26));

            // CPU socket + cooler, one quadrant per core.
            const q = r.cores;
            for (let i = 0; i < Math.min(q.length, 4); i++) {
                const rgb = heatRgb(q[i]);
                ctx.fillStyle = "rgba(" + rgb[0] + "," + rgb[1] + ","
                    + rgb[2] + ",0.45)";
                ctx.fillRect(X(230 + (i % 2) * 22),
                    Y(90 + Math.floor(i / 2) * 22), S(22), S(22));
            }
            ctx.strokeStyle = Theme.redDim;
            ctx.beginPath();
            ctx.arc(X(252), Y(112), S(32), 0, Math.PI * 2);
            ctx.stroke();
            ctx.strokeStyle = Theme.red;
            ctx.strokeRect(X(230), Y(90), S(44), S(44));

            // RAM slots.
            ctx.strokeStyle = Theme.redDim;
            for (let k = 0; k < 4; k++)
                ctx.strokeRect(X(310 + k * 14), Y(62), S(6), S(118));

            // GPU card.
            ctx.strokeStyle = Theme.red;
            ctx.strokeRect(X(150), Y(220), S(220), S(34));
            ctx.strokeStyle = Theme.redDim;
            ctx.beginPath();
            ctx.arc(X(210), Y(237), S(13), 0, Math.PI * 2);
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(X(290), Y(237), S(13), 0, Math.PI * 2);
            ctx.stroke();

            // M.2, chipset, Wi-Fi.
            ctx.strokeStyle = Theme.red;
            ctx.strokeRect(X(200), Y(270), S(80), S(10));
            ctx.strokeRect(X(330), Y(265), S(22), S(22));
            ctx.strokeRect(X(390), Y(300), S(14), S(12));

            // PSU.
            ctx.strokeStyle = Theme.redDim;
            ctx.globalAlpha = 0.7;
            roundedPath(ctx, X(20), Y(320), S(140), S(60), S(4));
            ctx.stroke();
            ctx.globalAlpha = 1;
            fan(ctx, X(55), Y(350), S(22));
            ctx.font = "9px " + Theme.monoFamily;
            ctx.fillStyle = Theme.textFaint;
            ctx.fillText("PSU", X(110), Y(354));

            tag(ctx, X(230), Y(84), "CPU", r.pkg);
            tag(ctx, X(150), Y(214), "GPU", root.gpuC,
                { fallback: root.gpuDormant ? "DORMANT" : "—" });
            tag(ctx, X(200), Y(266), "SSD", r.ssd);
            tag(ctx, X(330), Y(261), "BOARD", r.board, { align: "right" });
            tag(ctx, X(388), Y(296), "WIFI", r.wifi, { align: "right" });
        }

        onPaint: {
            const ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            if (!Sampler.alive || Sampler.temps.length === 0) {
                ctx.font = "9px " + Theme.monoFamily;
                ctx.fillStyle = Theme.textFaint;
                ctx.textAlign = "center";
                ctx.fillText("NO TELEMETRY", width / 2, height / 2);
                ctx.textAlign = "left";
                return;
            }
            if (root.zenbook)
                paintZenbook(ctx);
            else
                paintTower(ctx);
        }
    }
}