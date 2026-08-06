import Quickshell
import Quickshell.Io
import QtQuick

// Cross-process single-instance guard.
//
// Probe first, bind second. That order is forced by Quickshell: SocketServer
// DELETES any existing socket file to claim the path ("Deleted existing file
// at ... to create socket"), so a failed bind can never be the signal — a
// second launch would simply steal the lock. Instead a launch connects to the
// lock socket first and only binds if nothing answers.
//
// The holder answers with its own PID, which is what makes this safe across
// config reloads. SocketServer is Reloadable, so its socket survives a live
// reload; without the PID a reloading instance would connect to its own
// carried-over socket, conclude a duplicate was running, and quit itself —
// far worse than the duplicate this prevents. A reply matching our own PID
// means we are already the holder, so we carry on.
//
// Every uncertain path falls through to "keep running". This quits only on
// positive proof that a DIFFERENT live process owns the lock.
//
// Why it exists: MinkaMon's ScreenPad mode is a layer surface, and duplicate
// layer surfaces stack invisibly — no dock chip, nothing in the window list,
// just a schematic that renders wrong. (Sophie, 3/8/2026.)
Item {
    id: root

    required property string name

    // Raised on the surviving instance when a later launch is turned away:
    // the cue to present yourself to the user, who just asked for this app.
    signal duplicateRejected

    readonly property string lockPath:
        (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/" + name + ".lock"

    // "probing" -> "primary" (bound, or already ours) | quit
    property string phase: "probing"

    Component.onCompleted: {
        probeTimeout.restart();
        probe.path = root.lockPath;
        probe.connected = true;
    }

    function becomePrimary() {
        probeTimeout.stop();
        probe.connected = false;
        root.phase = "primary";
        server.active = true;
    }

    function onHolderReplied(line) {
        if (root.phase !== "probing")
            return;
        const holder = parseInt(line, 10);
        if (!isFinite(holder) || holder === Quickshell.processId) {
            // Our own socket, carried across a reload — or an unreadable
            // reply. Either way, not proof of anyone else.
            root.becomePrimary();
            return;
        }
        probeTimeout.stop();
        probe.connected = false;
        console.warn(root.name + ": already running as pid " + holder + ", exiting.");
        Qt.quit();
    }

    Socket {
        id: probe

        parser: SplitParser {
            splitMarker: "\n"
            onRead: line => root.onHolderReplied(line)
        }
    }

    // Nothing answered in time, so no live holder. Falling through to taking
    // the lock is the branch that keeps the app running.
    Timer {
        id: probeTimeout

        interval: 600
        onTriggered: {
            if (root.phase === "probing")
                root.becomePrimary();
        }
    }

    SocketServer {
        id: server

        path: root.lockPath
        active: false

        handler: Socket {
            // Announce which process holds the lock, so a prober can tell a
            // real duplicate from this same process after a config reload.
            onConnectionStateChanged: {
                if (!connected)
                    return;
                write(Quickshell.processId + "\n");
                flush();
                // Logged at warning level so it survives the default Qt
                // filter rules: the whole reason this line exists is that a
                // rejected launch used to be completely silent, and an info
                // line that gets filtered would be no better.
                console.warn(root.name + ": duplicate launch rejected; we are pid "
                    + Quickshell.processId + ".");
                root.duplicateRejected();
            }
        }
    }
}