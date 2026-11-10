pragma Singleton
import Quickshell
import QtQuick
// Through the config-root symlink: Quickshell only honours qmldir
// registration for paths inside the shell root.
import "../MinkaLink"

// Runs scripts/sampler.py for the life of the app and mirrors its JSON-lines
// stream into reactive properties. One sample per second; procs/connections
// are demand-gated by the shell (see sampler.py header for the full contract).
Singleton {
    id: root

    readonly property string scriptPath: {
        return Qt.resolvedUrl("../scripts/sampler.py")
            .toString().replace("file://", "");
    }

    property bool alive: false
    property int cores: 0
    property bool hasNvidia: false
    property string machineModel: ""
    property string chassisType: ""
    property var coastlines: []

    // Demand switches for the expensive sampler slices. The shell binds
    // these to the processes/globe satellite visibility; sendDemand()
    // forwards every change to the sampler over stdin.
    property bool wantProcs: false
    property bool wantConns: false

    onWantProcsChanged: sendDemand()
    onWantConnsChanged: sendDemand()

    function sendDemand() {
        if (!wantProcs)
            procs = [];
        if (!wantConns)
            conns = [];
        samplerProc.write(JSON.stringify({
            wantProcs: wantProcs,
            wantConns: wantConns,
        }) + "\n");
    }

    // Last sample slices (see sampler.py for shapes).
    property var cpu: ({ total: 0, cores: [] })
    property var mem: null
    property var gpu: null
    property var temps: []
    property var net: ({ downBps: 0, upBps: 0, ifaces: ({}) })
    property var disk: ({ readBps: 0, writeBps: 0, disks: ({}) })
    property var conns: []
    property var procs: []

    // Rolling 60-sample histories for sparklines (plain JS arrays of numbers).
    property var cpuHistory: []
    property var downHistory: []
    property var upHistory: []
    property var readHistory: []
    property var writeHistory: []
    property var utilHistory: []

    signal ticked()

    function pushHistory(list, value) {
        const next = list.slice(-59);
        next.push(value);
        return next;
    }

    CoreProcess {
        id: samplerProc

        command: ["python3", "-B", root.scriptPath]

        // Restart the sampler if it dies (e.g. transient python error).
        autoRestart: true

        onStarted: root.sendDemand()

        onLine: line => {
                let sample;
                try {
                    sample = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (sample.meta) {
                    root.cores = sample.meta.cores;
                    root.hasNvidia = sample.meta.hasNvidia;
                    root.machineModel = sample.meta.model || "";
                    root.chassisType = sample.meta.chassis || "";
                    root.coastlines = sample.meta.coastlines || [];
                    root.alive = true;
                    return;
                }
                root.alive = true;
                root.cpu = sample.cpu;
                root.mem = sample.mem;
                root.gpu = sample.gpu;
                root.temps = sample.temps;
                root.net = sample.net;
                if (sample.disk !== undefined)
                    root.disk = sample.disk;
                if (sample.conns !== undefined)
                    root.conns = sample.conns;
                if (sample.procs !== undefined)
                    root.procs = sample.procs;
                root.cpuHistory = root.pushHistory(root.cpuHistory, sample.cpu.total);
                root.downHistory = root.pushHistory(root.downHistory, sample.net.downBps);
                root.upHistory = root.pushHistory(root.upHistory, sample.net.upBps);
                root.readHistory = root.pushHistory(root.readHistory, root.disk.readBps);
                root.writeHistory = root.pushHistory(root.writeHistory, root.disk.writeBps);
                root.utilHistory = root.pushHistory(root.utilHistory, root.disk.utilPct || 0);
                root.ticked();
        }
        onStderrLine: line => console.warn("MinkaMon: sampler:", line)
        onExited: root.alive = false
    }

    function fmtBytes(bps) {
        if (bps >= 1048576)
            return (bps / 1048576).toFixed(1) + " MB/s";
        if (bps >= 1024)
            return (bps / 1024).toFixed(1) + " KB/s";
        return bps + " B/s";
    }

    function fmtKb(kb) {
        if (kb >= 1048576)
            return (kb / 1048576).toFixed(2) + " GB";
        if (kb >= 1024)
            return (kb / 1024).toFixed(0) + " MB";
        return kb + " KB";
    }
}