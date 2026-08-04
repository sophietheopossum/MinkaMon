import Quickshell
import Quickshell.Io
import QtQuick

// Cross-process single-instance guard.
//
// The first instance binds a Unix socket in $XDG_RUNTIME_DIR. A later launch
// fails to bind, proves the holder is actually alive, and then quits.
//
// The probe is the point of the whole thing. A crashed instance leaves its
// socket file behind, so "bind failed" on its own does NOT mean another copy
// is running — treating it that way would stop the app launching at all until
// someone deleted the file by hand, which is a worse failure than the
// duplicate it set out to prevent. So a failed bind is followed by a
// connection attempt: something answers only if a real instance owns the lock.
// Nothing answering means the file is stale, and it is removed and the bind
// retried once.
//
// Every uncertain path deliberately falls through to "keep running". This
// quits only when it has positive proof that another instance is listening.
//
// Why it exists: MinkaMon's ScreenPad mode is a layer surface, and duplicate
// layer surfaces stack invisibly — no dock entry, nothing in the window list,
// just a schematic that looks wrong. (Sophie, 3/8/2026.)
Item {
    id: root

    required property string name

    // Raised on the instance that survives when a later launch is turned
    // away: the cue to present yourself to the user, who evidently just
    // asked for this app.
    signal duplicateRejected

    readonly property string lockPath:
        (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/" + name + ".lock"

    // "binding" -> "primary" | "probing" -> "primary" (stale, retaken)
    property string phase: "binding"
    property bool staleRetried: false

    function takeLock() {
        root.phase = "binding";
        server.active = true;
    }

    Component.onCompleted: root.takeLock()

    SocketServer {
        id: server

        path: root.lockPath
        active: false

        handler: Socket {
            // A later launch connects only to prove we are alive, then drops
            // it. Treat that as a request to show ourselves.
            onConnectionStateChanged: {
                if (connected)
                    root.duplicateRejected();
            }
        }

        onActiveStatusChanged: {
            if (server.active) {
                root.phase = "primary";
                return;
            }
            if (root.phase !== "binding")
                return;
            // Could not bind. Find out whether anyone is actually there.
            root.phase = "probing";
            probeTimeout.restart();
            probe.path = root.lockPath;
            probe.connected = true;
        }
    }

    Socket {
        id: probe

        onConnectionStateChanged: {
            if (root.phase !== "probing" || !connected)
                return;
            // Positive proof: someone is listening on the lock.
            probeTimeout.stop();
            probe.connected = false;
            console.warn(root.name + ": another instance is already running, exiting.");
            Qt.quit();
        }

        // Deliberately no `onError` handler. Its parameter is a Qt enum the
        // linter cannot resolve (and a comment line must not start with that
        // tool's name, or it is parsed as a lint directive). The timeout
        // below already covers the "nothing answered" case, so dropping the
        // error signal only costs the stale-lock path one second.
    }

    // Nothing answered within the window, so no live instance owns the lock.
    // Falling through to "stale" is the branch that keeps the app running.
    Timer {
        id: probeTimeout

        interval: 1000
        onTriggered: {
            if (root.phase === "probing")
                root.reclaimStaleLock();
        }
    }

    function reclaimStaleLock() {
        if (root.staleRetried) {
            // Already tried once; run unguarded rather than refuse to start.
            console.warn(root.name + ": could not take the instance lock, continuing anyway.");
            root.phase = "primary";
            return;
        }
        root.staleRetried = true;
        probe.connected = false;
        unlink.running = true;
    }

    Process {
        id: unlink

        command: ["rm", "-f", root.lockPath]

        onRunningChanged: {
            if (!running && root.phase === "probing")
                root.takeLock();
        }
    }
}