//! ShojiWM IPC state for leader lines, overview arrangement, and raising.
//!
//! Transport is the shared `MinkaIPC` crate. This module owns the parsed
//! window view and the geometry maths that the QML `ShojiIpc` service and
//! `LeaderOverlay` performed.

use crate::model::{WindowKind, Zone, arr_at, at, bool_at, f64_at, obj_at, str_at};
use calloop::channel::Channel;
use minka_ipc::{IpcClient, IpcEvent};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectF {
    pub fn from_value(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            x: obj.get("x").and_then(Value::as_f64).unwrap_or(0.0),
            y: obj.get("y").and_then(Value::as_f64).unwrap_or(0.0),
            width: obj.get("width").and_then(Value::as_f64).unwrap_or(0.0),
            height: obj.get("height").and_then(Value::as_f64).unwrap_or(0.0),
        })
    }

    pub fn inset(self, by: f64) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            width: self.width - by * 2.0,
            height: self.height - by * 2.0,
        }
    }

    pub fn center(self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct IpcWindow {
    pub id: String,
    pub title: String,
    pub role: Option<String>,
    pub monitor: String,
    pub minimized: bool,
    pub fullscreen: bool,
    pub last_focused_at: f64,
    pub rect: RectF,
    pub drag_tab: Option<RectF>,
}

#[derive(Clone, Debug, Default)]
pub struct IpcData {
    pub connected: bool,
    pub windows: Vec<IpcWindow>,
    pub usable: BTreeMap<String, RectF>,
}

impl IpcData {
    pub fn by_role(&self, role: &str) -> Option<&IpcWindow> {
        self.windows
            .iter()
            .find(|w| w.role.as_deref() == Some(role))
    }

    pub fn by_title(&self, title: &str) -> Option<&IpcWindow> {
        self.windows.iter().find(|w| w.title == title)
    }

    pub fn fullscreen_on(&self, monitor: &str) -> bool {
        self.windows
            .iter()
            .any(|w| w.fullscreen && w.monitor == monitor)
    }

    fn apply_view(&mut self, view: &Value) {
        let mut windows = Vec::new();
        for monitor in arr_at(view, &["monitors"]) {
            let monitor_name = str_at(monitor, &["name"]).to_owned();
            for workspace in arr_at(monitor, &["workspaces"]) {
                for w in arr_at(workspace, &["windows"]) {
                    let rect = at(w, &["rect"])
                        .and_then(RectF::from_value)
                        .unwrap_or_default();
                    let drag_tab = w.get("dragTab").and_then(RectF::from_value);
                    windows.push(IpcWindow {
                        id: str_at(w, &["id"]).to_owned(),
                        title: str_at(w, &["title"]).to_owned(),
                        role: w.get("role").and_then(Value::as_str).map(str::to_owned),
                        monitor: monitor_name.clone(),
                        minimized: bool_at(w, &["minimized"]),
                        fullscreen: bool_at(w, &["fullscreen"]),
                        last_focused_at: f64_at(w, &["lastFocusedAt"]),
                        rect,
                        drag_tab,
                    });
                }
            }
        }
        self.windows = windows;
    }

    fn apply_rects(&mut self, payload: &Value) {
        for item in arr_at(payload, &["windows"]) {
            let id = str_at(item, &["id"]);
            let Some(window) = self.windows.iter_mut().find(|w| w.id == id) else {
                continue;
            };
            if let Some(rect) = RectF::from_value(item) {
                window.rect = rect;
            }
            if let Some(tab) = item.get("dragTab") {
                window.drag_tab = RectF::from_value(tab);
            }
        }
    }

    fn apply_geometry(&mut self, result: &Value) {
        if let Some(usable) = obj_at(result, &["usable"]) {
            self.usable = usable
                .iter()
                .filter_map(|(name, rect)| RectF::from_value(rect).map(|r| (name.clone(), r)))
                .collect();
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IpcUpdate {
    pub windows_changed: bool,
    pub geometry_changed: bool,
    pub just_connected: bool,
}

/// Lazily-started ShojiWM client. MinkaIPC's I/O thread reconnects forever;
/// this controller is polled from the UI service ticker.
pub struct IpcController {
    client: RefCell<Option<Arc<IpcClient>>>,
    events: RefCell<Option<Channel<IpcEvent>>>,
    started: Cell<bool>,
}

impl Default for IpcController {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcController {
    pub fn new() -> Self {
        Self {
            client: RefCell::new(None),
            events: RefCell::new(None),
            started: Cell::new(false),
        }
    }

    pub fn ensure_started(&self) {
        if self.started.get() {
            return;
        }
        self.started.set(true);
        if let Ok((client, events)) = IpcClient::spawn() {
            *self.client.borrow_mut() = Some(client);
            *self.events.borrow_mut() = Some(events);
        }
    }

    fn request(&self, method: &str, params: Value) -> Option<u64> {
        self.ensure_started();
        self.client
            .borrow()
            .as_ref()
            .and_then(|client| client.request(method, params))
    }

    pub fn request_windows(&self) {
        self.request("workspaces.get", json!({ "rectsLease": "minkamon" }));
    }

    pub fn request_geometry(&self) {
        self.request("debug.geometry", json!({}));
    }

    pub fn identify(&self, window_id: &str, role: &str) {
        self.request(
            "windows.identify",
            json!({ "windowId": window_id, "role": role }),
        );
    }

    pub fn activate(&self, window_id: &str) {
        self.request("windows.activate", json!({ "windowId": window_id }));
    }

    pub fn set_rect(&self, window_id: &str, rect: RectF) {
        self.request(
            "windows.setRect",
            json!({
                "windowId": window_id,
                "x": rect.x.round(),
                "y": rect.y.round(),
                "width": rect.width.round(),
                "height": rect.height.round(),
            }),
        );
    }

    /// Takes all currently pending events without touching UI state.
    pub fn take_events(&self) -> Vec<IpcEvent> {
        let mut taken = Vec::new();
        let events = self.events.borrow();
        let Some(channel) = events.as_ref() else {
            return taken;
        };
        while let Ok(event) = channel.try_recv() {
            taken.push(event);
        }
        taken
    }

    /// Applies events previously returned by [`Self::take_events`].
    pub fn apply_events(&self, data: &mut IpcData, taken: Vec<IpcEvent>) -> IpcUpdate {
        let mut update = IpcUpdate::default();
        for event in taken {
            match event {
                IpcEvent::Connected => {
                    data.connected = true;
                    update.just_connected = true;
                }
                IpcEvent::Disconnected => {
                    data.connected = false;
                }
                IpcEvent::Broadcast { event, payload } => match event.as_str() {
                    "workspaces.changed" => {
                        data.apply_view(&payload);
                        update.windows_changed = true;
                    }
                    "windows.rects" => {
                        data.apply_rects(&payload);
                        update.windows_changed = true;
                    }
                    _ => {}
                },
                IpcEvent::Response { result, .. } => {
                    if let Ok(value) = result {
                        if value.get("monitors").is_some() {
                            data.apply_view(&value);
                            update.windows_changed = true;
                        } else if value.get("usable").is_some() {
                            data.apply_geometry(&value);
                            update.geometry_changed = true;
                        }
                    }
                }
            }
        }
        update
    }
}

pub const MAIN_ROLE: &str = "minkamon.main";
const CHROME_INSET: f64 = 14.0;
const CLIENT_INSET: f64 = 16.0;

#[derive(Clone, Debug)]
pub struct LeaderLine {
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
    pub spans: Vec<(f64, f64)>,
}

/// Liang-Barsky: the `[t0, t1]` slice of segment a->b inside `r`.
fn seg_rect_interval(x0: f64, y0: f64, x1: f64, y1: f64, r: RectF) -> Option<(f64, f64)> {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let p = [-dx, dx, -dy, dy];
    let q = [x0 - r.x, r.x + r.width - x0, y0 - r.y, r.y + r.height - y0];
    for i in 0..4 {
        if p[i] == 0.0 {
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let t = q[i] / p[i];
            if p[i] < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    Some((t0, t1))
}

fn visible_spans(mut covered: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    covered.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spans = Vec::new();
    let mut pos = 0.0;
    for interval in covered {
        if interval.0 > pos {
            spans.push((pos, interval.0.min(1.0)));
        }
        pos = pos.max(interval.1);
        if pos >= 1.0 {
            break;
        }
    }
    if pos < 1.0 {
        spans.push((pos, 1.0));
    }
    spans
        .into_iter()
        .filter(|span| span.1 - span.0 > 0.004)
        .collect()
}

const TIES: [(WindowKind, Zone); 12] = [
    (WindowKind::Cpu, Zone::Cpu),
    (WindowKind::Gpu, Zone::Gpu),
    (WindowKind::Memory, Zone::Ram),
    (WindowKind::Network, Zone::Wifi),
    (WindowKind::Globe, Zone::Wifi),
    (WindowKind::Temp(crate::model::TempKind::Wifi), Zone::Wifi),
    (WindowKind::Disk, Zone::Ssd),
    (WindowKind::Temp(crate::model::TempKind::Ssd), Zone::Ssd),
    (WindowKind::Temp(crate::model::TempKind::Board), Zone::Board),
    (WindowKind::Temp(crate::model::TempKind::Cpu), Zone::Cpu),
    (WindowKind::Temp(crate::model::TempKind::Gpu), Zone::Gpu),
    (WindowKind::Processes, Zone::Cpu),
];

/// Builds leader lines in layout coordinates. `anchor` maps a schematic zone
/// plus the main window's client size to a window-local scene point.
pub fn leader_lines(
    data: &IpcData,
    anchor: impl Fn(Zone, f64, f64) -> Option<(f64, f64)>,
) -> Vec<LeaderLine> {
    let mut out = Vec::new();
    let Some(main) = data.by_role(MAIN_ROLE) else {
        return out;
    };
    if main.minimized {
        return out;
    }
    let client_w = (main.rect.width - CLIENT_INSET * 2.0).max(1.0);
    let client_h = (main.rect.height - CLIENT_INSET * 2.0).max(1.0);

    for (kind, zone) in TIES {
        let Some(sat) = data.by_role(&kind.role()) else {
            continue;
        };
        if sat.minimized || data.fullscreen_on(&main.monitor) || data.fullscreen_on(&sat.monitor) {
            continue;
        }
        let Some((scene_x, scene_y)) = anchor(zone, client_w, client_h) else {
            continue;
        };
        let ax = main.rect.x + CLIENT_INSET + scene_x;
        let ay = main.rect.y + CLIENT_INSET + scene_y;

        let sat_vis = sat.rect.inset(CHROME_INSET);
        let (cx, cy) = sat_vis.center();
        let Some(hit) = seg_rect_interval(ax, ay, cx, cy, sat_vis) else {
            continue;
        };
        if hit.0 <= 0.0 {
            continue;
        }
        let bx = ax + (cx - ax) * hit.0;
        let by = ay + (cy - ay) * hit.0;

        let zref = main.last_focused_at.max(sat.last_focused_at);
        let mut covered = Vec::new();
        for w in &data.windows {
            if w.minimized {
                continue;
            }
            if w.id != main.id
                && w.id != sat.id
                && w.last_focused_at > zref
                && let Some(iv) = seg_rect_interval(ax, ay, bx, by, w.rect.inset(CHROME_INSET))
            {
                covered.push(iv);
            }
            if let Some(tab) = w.drag_tab
                && let Some(iv) = seg_rect_interval(ax, ay, bx, by, tab)
            {
                covered.push(iv);
            }
        }
        let spans = visible_spans(covered);
        if !spans.is_empty() {
            out.push(LeaderLine {
                ax,
                ay,
                bx,
                by,
                spans,
            });
        }
    }
    out
}
