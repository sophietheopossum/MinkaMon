//! Single-instance guard, wire-compatible with `services/SingleInstance.qml`.
//!
//! The holder listens on `$XDG_RUNTIME_DIR/minkamon.lock` and answers each
//! probe with its PID. A second launch that hears a different live PID exits;
//! the holder is notified so it can raise itself.

use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

const LOCK_NAME: &str = "minkamon.lock";

pub struct InstanceGuard {
    rejected: Receiver<()>,
}

impl InstanceGuard {
    fn inert() -> Self {
        let (_tx, rx) = channel();
        Self { rejected: rx }
    }

    /// Drains pending duplicate notifications; true if at least one arrived.
    pub fn take_rejected(&self) -> bool {
        let mut any = false;
        loop {
            match self.rejected.try_recv() {
                Ok(()) => any = true,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
            }
        }
    }
}

pub enum InstanceClaim {
    Primary(InstanceGuard),
    Duplicate,
}

fn lock_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(|dir| PathBuf::from(dir).join(LOCK_NAME))
}

fn probe(path: &Path) -> bool {
    let own = std::process::id();
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(600)));
    let mut buf = [0u8; 64];
    let mut text = String::new();
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                text.push_str(&String::from_utf8_lossy(&buf[..n]));
                if text.contains('\n') || text.len() >= 64 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let Some(pid) = text.trim().parse::<u32>().ok() else {
        return false;
    };
    pid != own && Path::new(&format!("/proc/{pid}")).exists()
}

fn remove_stale_socket(path: &Path) {
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.file_type().is_socket()
    {
        let _ = std::fs::remove_file(path);
    }
}

fn serve(listener: UnixListener) -> InstanceGuard {
    let (tx, rx) = channel();
    let pid = std::process::id();
    let spawned = std::thread::Builder::new()
        .name("minkamon-instance".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let _ = writeln!(stream, "{pid}");
                let _ = tx.send(());
            }
        });
    if spawned.is_err() {
        return InstanceGuard::inert();
    }
    InstanceGuard { rejected: rx }
}

pub fn claim() -> InstanceClaim {
    let Some(path) = lock_path() else {
        return InstanceClaim::Primary(InstanceGuard::inert());
    };

    if path.exists() && probe(&path) {
        return InstanceClaim::Duplicate;
    }

    match UnixListener::bind(&path) {
        Ok(listener) => InstanceClaim::Primary(serve(listener)),
        Err(_) if probe(&path) => InstanceClaim::Duplicate,
        Err(_) => {
            remove_stale_socket(&path);
            match UnixListener::bind(&path) {
                Ok(listener) => InstanceClaim::Primary(serve(listener)),
                Err(_) if probe(&path) => InstanceClaim::Duplicate,
                // If the lock cannot be established, running unguarded is
                // less bad than refusing to start (matching the QML guard's
                // bind-failure fallback).
                Err(_) => InstanceClaim::Primary(InstanceGuard::inert()),
            }
        }
    }
}
