import QtQuick
import "../services"

// eDEX-UI style memory widget over the real physical address space: the grid
// spans the machine's RAM in address order, split into the kernel's zones
// (DMA / DMA32 / Normal from /proc/zoneinfo), and each zone lights up as many
// cells as it actually has non-free pages. Per-page state needs root
// (/proc/kpageflags), so placement *within* a zone is a stable deterministic
// scatter — counts per physical range are real, exact page positions are not.
Panel {
    id: root

    title: "MEMORY"

    readonly property var mem: Sampler.mem
    readonly property var zones: mem && mem.zones ? mem.zones : []
    readonly property int cells: 10 * 44

    // Cache is not attributable per-zone; approximate each zone's cache share
    // with the global cache fraction of in-use memory.
    readonly property real cacheFrac: mem && mem.totalKb > mem.freeKb
        ? mem.cacheKb / (mem.totalKb - mem.freeKb) : 0

    // [{name, count, perm}] per zone — rebuilt only when the zone structure
    // changes, so the scatter (and therefore which cell flips next) is stable
    // across ticks.
    property var zoneLayout: []
    property string zoneSig: ""

    onZonesChanged: {
        const sig = zones.map(z => z.name + ":" + z.presentPages).join();
        if (sig === zoneSig)
            return;
        zoneSig = sig;
        zoneLayout = buildLayout();
    }

    function buildLayout() {
        let totalPresent = 0;
        for (const z of zones)
            totalPresent += z.presentPages;
        if (totalPresent === 0)
            return [];
        // Proportional cells per zone, but keep tiny zones (DMA is 16M of a
        // 16G machine) visible with a 4-cell floor; the largest zone absorbs
        // the rounding.
        const counts = zones.map(z =>
            Math.max(4, Math.round(cells * z.presentPages / totalPresent)));
        let biggest = 0;
        for (let i = 1; i < zones.length; i++) {
            if (counts[i] > counts[biggest])
                biggest = i;
        }
        counts[biggest] += cells - counts.reduce((a, b) => a + b, 0);
        return zones.map((z, i) => ({
            name: z.name,
            count: counts[i],
            perm: shuffled(counts[i], z.startKb + 1),
        }));
    }

    // Deterministic Fisher–Yates (mulberry32) so the scatter never re-rolls.
    function shuffled(count, seed) {
        let s = seed >>> 0;
        const rand = () => {
            s = (s + 0x6D2B79F5) >>> 0;
            let t = Math.imul(s ^ (s >>> 15), 1 | s);
            t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
            return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
        };
        const p = Array.from({ length: count }, (_, i) => i);
        for (let i = count - 1; i > 0; i--) {
            const j = Math.floor(rand() * (i + 1));
            const tmp = p[i];
            p[i] = p[j];
            p[j] = tmp;
        }
        return p;
    }

    // Per-cell code: (state 0=free 1=used 2=cache) | 4 if odd zone, so
    // adjacent zones band visibly even where they are mostly free.
    readonly property var cellStates: {
        const states = new Array(cells).fill(0);
        const layout = zoneLayout;
        if (!mem || layout.length === 0 || layout.length !== zones.length)
            return states;
        let base = 0;
        for (let zi = 0; zi < layout.length; zi++) {
            const seg = layout[zi];
            const z = zones[zi];
            const parity = (zi % 2) * 4;
            for (let k = 0; k < seg.count; k++)
                states[base + k] = parity;
            const usedCells = Math.round(
                seg.count * (z.managedPages - z.freePages) / z.managedPages);
            const cacheCells = Math.round(usedCells * cacheFrac);
            for (let k = 0; k < usedCells; k++) {
                states[base + seg.perm[k]] =
                    (k < usedCells - cacheCells ? 1 : 2) + parity;
            }
            base += seg.count;
        }
        return states;
    }

    function fmtAddr(kb) {
        if (kb >= 1048576)
            return (kb / 1048576).toFixed(0) + "G";
        if (kb >= 1024)
            return (kb / 1024).toFixed(0) + "M";
        return kb.toFixed(0) + "K";
    }

    Column {
        anchors.fill: parent
        spacing: 8

        Grid {
            id: blockGrid

            columns: 44
            columnSpacing: 2
            rowSpacing: 2

            Repeater {
                model: root.cells

                Rectangle {
                    width: (blockGrid.parent.width - 43 * 2) / 44
                    height: 7
                    color: {
                        const code = root.cellStates[index];
                        const state = code & 3;
                        if (state === 1)
                            return Theme.red;
                        if (state === 2)
                            return Theme.gaugeDim;
                        return (code & 4) ? "#241a1f" : Theme.surfaceRaised;
                    }
                }
            }
        }

        Text {
            text: root.zones.map(z =>
                z.name.toUpperCase() + " @" + root.fmtAddr(z.startKb)
                + " " + root.fmtAddr(z.managedPages * 4)).join(" · ")
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 4
            font.letterSpacing: 1
            color: Theme.textFaint
        }

        Row {
            spacing: 13

            Repeater {
                model: [
                    {
                        label: "USED",
                        value: root.mem ? Sampler.fmtKb(root.mem.usedKb) : "—",
                        tint: Theme.red,
                    },
                    {
                        label: "CACHE",
                        value: root.mem ? Sampler.fmtKb(root.mem.cacheKb) : "—",
                        tint: Theme.textMuted,
                    },
                    {
                        label: "FREE",
                        value: root.mem ? Sampler.fmtKb(root.mem.availableKb) : "—",
                        tint: Theme.okGreen,
                    },
                    {
                        label: "TOTAL",
                        value: root.mem ? Sampler.fmtKb(root.mem.totalKb) : "—",
                        tint: Theme.textFaint,
                    },
                ]

                Column {
                    spacing: 1

                    Text {
                        text: modelData.label
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize - 3
                        font.letterSpacing: 1
                        color: Theme.textFaint
                    }

                    Text {
                        text: modelData.value
                        font.family: Theme.monoFamily
                        font.pixelSize: Theme.fontSize
                        color: modelData.tint
                    }
                }
            }
        }

        Row {
            spacing: 8
            visible: root.mem !== null && root.mem.swapTotalKb > 0

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "SWAP"
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                font.letterSpacing: 1
                color: Theme.textFaint
            }

            TrendLine {
                anchors.verticalCenter: parent.verticalCenter
                width: 140
                height: 14
                value: root.mem && root.mem.swapTotalKb > 0
                    ? 100 * root.mem.swapUsedKb / root.mem.swapTotalKb : 0
                maxValue: 100
                lineColor: Theme.warnAmber
                fillColor: Theme.gaugeDim
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.mem
                    ? Sampler.fmtKb(root.mem.swapUsedKb) + " / "
                        + Sampler.fmtKb(root.mem.swapTotalKb)
                    : ""
                font.family: Theme.monoFamily
                font.pixelSize: Theme.fontSize - 3
                color: Theme.textMuted
            }
        }
    }
}
