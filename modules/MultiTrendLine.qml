import QtQuick
import "../services"

// Multi-series 60s line chart: `current` holds one value per series, a new
// point per series is recorded on every sampler tick, and each series draws
// in its Theme.seriesPalette colour. Null values leave a gap in that line.
Canvas {
    id: root

    property var current: []
    property real maxValue: 100

    property var hist: []

    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    Connections {
        target: Sampler

        function onTicked() {
            const next = [];
            for (let i = 0; i < root.current.length; i++) {
                const h = (root.hist[i] || []).slice(-59);
                h.push(root.current[i]);
                next.push(h);
            }
            root.hist = next;
            root.requestPaint();
        }
    }

    onPaint: {
        const ctx = getContext("2d");
        ctx.clearRect(0, 0, width, height);
        const stepX = width / 59;
        ctx.lineWidth = 1.2;
        for (let i = 0; i < hist.length; i++) {
            const vals = hist[i];
            if (!vals || vals.length < 2)
                continue;
            const x0 = width - (vals.length - 1) * stepX;
            ctx.beginPath();
            let pen = false;
            for (let k = 0; k < vals.length; k++) {
                if (vals[k] === null || vals[k] === undefined) {
                    pen = false;
                    continue;
                }
                const x = x0 + k * stepX;
                const y = height - 1
                    - Math.min(vals[k] / maxValue, 1) * (height - 2);
                if (pen)
                    ctx.lineTo(x, y);
                else
                    ctx.moveTo(x, y);
                pen = true;
            }
            ctx.strokeStyle =
                Theme.seriesPalette[i % Theme.seriesPalette.length];
            ctx.stroke();
        }
    }
}