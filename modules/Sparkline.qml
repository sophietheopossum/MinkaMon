import QtQuick
import "../services"

// Minimal filled line chart over a fixed-length history array.
Canvas {
    id: root

    property var values: []
    property real maxValue: 0 // 0 = autoscale
    property color lineColor: Theme.red
    property color fillColor: Theme.gaugeDim

    onValuesChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        const ctx = getContext("2d");
        ctx.clearRect(0, 0, width, height);
        const vals = values;
        if (!vals || vals.length < 2)
            return;
        let peak = maxValue;
        if (peak <= 0) {
            for (const v of vals)
                peak = Math.max(peak, v);
            peak = Math.max(peak, 1);
        }
        const stepX = width / 59;
        const x0 = width - (vals.length - 1) * stepX;

        ctx.beginPath();
        ctx.moveTo(x0, height);
        for (let i = 0; i < vals.length; i++) {
            const y = height - Math.min(vals[i] / peak, 1) * (height - 2);
            ctx.lineTo(x0 + i * stepX, y);
        }
        ctx.lineTo(width, height);
        ctx.closePath();
        ctx.fillStyle = fillColor;
        ctx.fill();

        ctx.beginPath();
        for (let i = 0; i < vals.length; i++) {
            const y = height - Math.min(vals[i] / peak, 1) * (height - 2);
            if (i === 0)
                ctx.moveTo(x0, y);
            else
                ctx.lineTo(x0 + i * stepX, y);
        }
        ctx.strokeStyle = lineColor;
        ctx.lineWidth = 1.5;
        ctx.stroke();
    }
}