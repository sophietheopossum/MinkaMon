//! Application state, sampler lifecycle, window management, and services.

use crate::instance::InstanceGuard;
use crate::ipc::{IpcController, IpcData, MAIN_ROLE, RectF};
use crate::model::*;
use anti_webui::prelude::*;
use anti_webui::shell::{Layer, LayerHandle, Process, RunningProcess};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MainHost {
    None,
    Floating,
    Pad(String),
}

fn exe_ancestors() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let mut cur = Some(dir.to_path_buf());
        for _ in 0..8 {
            let Some(path) = cur else { break };
            out.push(path.clone());
            cur = path.parent().map(PathBuf::from);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd);
    }
    out
}

fn first_existing(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.exists())
}

fn sampler_bin() -> PathBuf {
    if let Ok(path) = std::env::var("MINKAMON_SAMPLER_BIN")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    let mut candidates = Vec::new();
    for base in exe_ancestors() {
        candidates.push(
            base.join("sampler-rs")
                .join("target")
                .join("release")
                .join("minkamon-sampler"),
        );
        candidates.push(
            base.join("MinkaMon")
                .join("sampler-rs")
                .join("target")
                .join("release")
                .join("minkamon-sampler"),
        );
        candidates.push(base.join("minkamon-sampler"));
    }
    first_existing(candidates).unwrap_or_else(|| PathBuf::from("minkamon-sampler"))
}

fn assets_dir() -> PathBuf {
    if let Ok(path) = std::env::var("MINKAMON_ASSETS")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    let mut candidates = Vec::new();
    for base in exe_ancestors() {
        candidates.push(base.join("assets"));
        candidates.push(base.join("MinkaMon").join("assets"));
    }
    first_existing(candidates).unwrap_or_else(|| PathBuf::from("assets"))
}

/// Cleanup-time signal writes can race owner disposal (nodes are removed
/// before cleanups run). At whole-app teardown those writes no longer matter.
pub fn cleanup_write(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

fn set_if_alive<T: 'static>(sig: Signal<T>, value: T) {
    if sig.is_alive() {
        sig.set(value);
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sample: Signal<Value>,
    pub meta: Signal<Value>,
    pub alive: Signal<bool>,
    pub sampler_status: Signal<String>,
    pub histories: Signal<Histories>,
    pub procs: Signal<Vec<ProcRow>>,
    pub conns: Signal<Vec<ConnRow>>,
    pub info_mode: Signal<bool>,
    pub temp_mode: Signal<bool>,
    pub hovered_zone: Signal<Option<Zone>>,
    pub hover_count: Signal<i32>,
    pub open_windows: Signal<Vec<WindowKind>>,
    pub overview_pending: Signal<bool>,
    pub raise_pending: Signal<bool>,
    pub configured_layout: Signal<String>,
    pub pad_mode: Signal<bool>,
    pub pad_settled: Signal<bool>,
    pub pad_lifted: Signal<bool>,
    pub main_host: Signal<MainHost>,
    pub ipc_data: Signal<IpcData>,
    pub leader_visible: Signal<bool>,

    sampler_process: Signal<Option<RunningProcess>>,
    sampler_generation: Rc<Cell<u64>>,
    window_handles: Rc<RefCell<HashMap<WindowKind, WindowHandle>>>,
    main_handle: Rc<RefCell<Option<WindowHandle>>>,
    pad_handle: Rc<RefCell<Option<LayerHandle>>>,
    overview_generation: Rc<Cell<u64>>,
    pad_lift_generation: Rc<Cell<u64>>,
    service_tick: Rc<Cell<u64>>,
    settings_mtime: Rc<Cell<Option<SystemTime>>>,
    guard: Rc<InstanceGuard>,
    pub ipc: Rc<IpcController>,
}

impl AppState {
    pub fn new(guard: InstanceGuard) -> Self {
        Self {
            sample: signal(Value::Null),
            meta: signal(Value::Null),
            alive: signal(false),
            sampler_status: signal(String::new()),
            histories: signal(Histories::default()),
            procs: signal(Vec::new()),
            conns: signal(Vec::new()),
            info_mode: signal(true),
            temp_mode: signal(false),
            hovered_zone: signal(None),
            hover_count: signal(0),
            open_windows: signal(Vec::new()),
            overview_pending: signal(false),
            raise_pending: signal(false),
            configured_layout: signal("auto".to_owned()),
            pad_mode: signal(false),
            pad_settled: signal(false),
            pad_lifted: signal(false),
            main_host: signal(MainHost::None),
            ipc_data: signal(IpcData::default()),
            leader_visible: signal(false),
            sampler_process: signal(None),
            sampler_generation: Rc::new(Cell::new(0)),
            window_handles: Rc::new(RefCell::new(HashMap::new())),
            main_handle: Rc::new(RefCell::new(None)),
            pad_handle: Rc::new(RefCell::new(None)),
            overview_generation: Rc::new(Cell::new(0)),
            pad_lift_generation: Rc::new(Cell::new(0)),
            service_tick: Rc::new(Cell::new(0)),
            settings_mtime: Rc::new(Cell::new(None)),
            guard: Rc::new(guard),
            ipc: Rc::new(IpcController::new()),
        }
    }

    pub fn spawn_sampler(&self) {
        let generation = self.sampler_generation.get() + 1;
        self.sampler_generation.set(generation);

        let process = Process::new(sampler_bin())
            .arg(assets_dir())
            .stdin(true)
            .keep(4)
            .spawn();
        self.sampler_process.set(Some(process));

        let state = self.clone();
        process.on_line(move |line| state.handle_sampler_line(line));

        let state = self.clone();
        process.on_exit(move |code| {
            set_if_alive(state.alive, false);
            set_if_alive(
                state.sampler_status,
                match code {
                    Some(c) => format!("sampler exited ({c})"),
                    None => "sampler failed to start".to_owned(),
                },
            );
            let state = state.clone();
            set_timeout(Duration::from_millis(1000), move || {
                if state.sampler_generation.get() == generation {
                    state.spawn_sampler();
                }
            });
        });

        // The demand effect also runs when the process signal changes, but a
        // short delayed resend covers stdin becoming ready asynchronously.
        let state = self.clone();
        set_timeout(Duration::from_millis(50), move || state.send_demand());
    }

    fn handle_sampler_line(&self, line: &str) {
        if !self.sample.is_alive() {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return;
        };
        self.alive.set(true);
        if let Some(meta) = value.get("meta") {
            self.meta.set(meta.clone());
            return;
        }

        let readings = self.meta.with(|meta| temp_readings(&value, meta));
        self.histories
            .update(|histories| histories.record(&value, &readings));
        if let Some(procs) = value.get("procs").and_then(Value::as_array) {
            self.procs.set(parse_procs(procs));
        }
        if let Some(conns) = value.get("conns").and_then(Value::as_array) {
            self.conns.set(parse_conns(conns));
        }
        self.sample.set(value);
    }

    pub fn is_open(&self, kind: WindowKind) -> bool {
        self.open_windows.with(|open| open.contains(&kind))
    }

    fn is_open_untracked(&self, kind: WindowKind) -> bool {
        self.open_windows
            .with_untracked(|open| open.contains(&kind))
    }

    pub fn open_window_kind(&self, kind: WindowKind) {
        if self.is_open_untracked(kind) {
            return;
        }
        self.open_windows.update(|open| open.push(kind));
        let (width, height) = kind.size();
        let state = self.clone();
        let handle = open_window(
            WindowOptions::new()
                .title(kind.title())
                .size(width, height)
                .top_level(),
            move || crate::panels::satellite_root(state, kind),
        );
        self.window_handles.borrow_mut().insert(kind, handle);
    }

    pub fn close_window_kind(&self, kind: WindowKind) {
        if let Some(handle) = self.window_handles.borrow().get(&kind).cloned() {
            handle.close();
        }
    }

    pub fn toggle_window_kind(&self, kind: WindowKind) {
        if self.is_open_untracked(kind) {
            self.close_window_kind(kind);
        } else {
            self.open_window_kind(kind);
        }
    }

    pub fn mark_window_closed(&self, kind: WindowKind) {
        self.window_handles.borrow_mut().remove(&kind);
        if self.open_windows.is_alive() {
            self.open_windows.update(|open| open.retain(|k| *k != kind));
        }
    }

    pub fn open_zone(&self, zone: Zone) {
        let info = self.info_mode.get_untracked();
        let temp = self.temp_mode.get_untracked();
        match zone {
            Zone::Board => {
                if temp {
                    self.open_window_kind(WindowKind::Temp(TempKind::Board));
                }
            }
            Zone::Cpu => {
                if info {
                    self.open_window_kind(WindowKind::Cpu);
                }
                if temp {
                    self.open_window_kind(WindowKind::Temp(TempKind::Cpu));
                }
            }
            Zone::Gpu => {
                if info {
                    self.open_window_kind(WindowKind::Gpu);
                }
                if temp {
                    self.open_window_kind(WindowKind::Temp(TempKind::Gpu));
                }
            }
            Zone::Ram => {
                if info {
                    self.open_window_kind(WindowKind::Memory);
                }
            }
            Zone::Wifi => {
                if info {
                    self.open_window_kind(WindowKind::Network);
                    self.open_window_kind(WindowKind::Globe);
                }
                if temp {
                    self.open_window_kind(WindowKind::Temp(TempKind::Wifi));
                }
            }
            Zone::Ssd => {
                if info {
                    self.open_window_kind(WindowKind::Disk);
                }
                if temp {
                    self.open_window_kind(WindowKind::Temp(TempKind::Ssd));
                }
            }
        }
    }

    pub fn close_all(&self) {
        self.overview_generation
            .set(self.overview_generation.get() + 1);
        self.overview_pending.set(false);
        let handles: Vec<WindowHandle> = self.window_handles.borrow().values().cloned().collect();
        for handle in handles {
            handle.close();
        }
    }

    pub fn set_main_handle(&self, handle: Option<WindowHandle>) {
        *self.main_handle.borrow_mut() = handle;
    }

    pub fn set_pad_handle(&self, handle: Option<LayerHandle>) {
        *self.pad_handle.borrow_mut() = handle;
    }

    pub fn send_demand(&self) {
        let interactive = self.hover_count.get_untracked() > 0
            || !self.open_windows.with_untracked(|open| open.is_empty());
        self.write_demand(interactive);
    }

    fn write_demand(&self, interactive: bool) {
        let want_procs = self.is_open_untracked(WindowKind::Processes);
        let want_conns = self.is_open_untracked(WindowKind::Globe);
        if !want_procs && !self.procs.with_untracked(|procs| procs.is_empty()) {
            self.procs.set(Vec::new());
        }
        if !want_conns && !self.conns.with_untracked(|conns| conns.is_empty()) {
            self.conns.set(Vec::new());
        }
        if let Some(process) = self.sampler_process.get_untracked() {
            let tick = if interactive { 1000 } else { 5000 };
            let _ = process.write(&format!(
                "{{\"wantProcs\":{want_procs},\"wantConns\":{want_conns},\"tickMs\":{tick}}}\n"
            ));
        }
    }

    pub fn install_demand_effect(&self) {
        let state = self.clone();
        effect(move || {
            let _process = state.sampler_process.get();
            let interactive = state.hover_count.get() > 0 || !state.open_windows.get().is_empty();
            state.write_demand(interactive);
        });
    }

    pub fn start_services(&self) {
        self.poll_settings();
        schedule_service(self.clone());
    }

    fn poll_settings(&self) {
        let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) else {
            return;
        };
        let path = PathBuf::from(home).join(".config/minka-settings.json");
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok();
        if modified.is_some() && self.settings_mtime.get() == modified {
            return;
        }
        self.settings_mtime.set(modified);
        let layout = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|data| {
                data.get("shell")
                    .and_then(|shell| shell.get("layout"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .filter(|layout| matches!(layout.as_str(), "duo" | "general"))
            .unwrap_or_else(|| "auto".to_owned());
        self.configured_layout.set_if_changed(layout);
    }

    fn prune_window_handles(&self) {
        let closed: Vec<WindowKind> = self
            .window_handles
            .borrow()
            .iter()
            .filter(|(_, handle)| !handle.is_open())
            .map(|(kind, _)| *kind)
            .collect();
        for kind in closed {
            self.mark_window_closed(kind);
        }
    }

    fn ipc_active(&self) -> bool {
        if self.raise_pending.get_untracked() || self.overview_pending.get_untracked() {
            return true;
        }
        let main_visible = self.pad_mode.get_untracked()
            || matches!(
                self.main_host.get_untracked(),
                MainHost::Floating | MainHost::Pad(_)
            );
        main_visible && !self.open_windows.with_untracked(|open| open.is_empty())
    }

    fn claim_roles(&self) {
        let claims: Vec<(String, String)> = self.ipc_data.with(|data| {
            let mut out = Vec::new();
            let mut claim = |title: &str, role: &str| {
                if let Some(window) = data.by_title(title)
                    && window.role.as_deref() != Some(role)
                {
                    out.push((window.id.clone(), role.to_owned()));
                }
            };
            claim("MinkaMon", MAIN_ROLE);
            for kind in WindowKind::INFO.into_iter().chain(
                crate::model::TempKind::ALL
                    .into_iter()
                    .map(WindowKind::Temp),
            ) {
                claim(&kind.title(), &kind.role());
            }
            out
        });
        for (id, role) in claims {
            self.ipc.identify(&id, &role);
        }
    }

    pub fn full_overview(&self) {
        for kind in [
            WindowKind::Cpu,
            WindowKind::Gpu,
            WindowKind::Memory,
            WindowKind::Network,
            WindowKind::Disk,
            WindowKind::Globe,
        ] {
            self.open_window_kind(kind);
        }
        let generation = self.overview_generation.get() + 1;
        self.overview_generation.set(generation);
        self.overview_pending.set(true);
        self.ipc.request_geometry();
        self.try_arrange_overview();
        let state = self.clone();
        set_timeout(Duration::from_millis(4000), move || {
            if state.overview_generation.get() == generation {
                state.overview_pending.set(false);
            }
        });
    }

    fn try_arrange_overview(&self) {
        if !self.overview_pending.get_untracked() {
            return;
        }
        let arranged = self.ipc_data.with(|data| {
            let roles = [
                MAIN_ROLE,
                "minkamon.cpu",
                "minkamon.gpu",
                "minkamon.network",
                "minkamon.memory",
                "minkamon.disk",
                "minkamon.globe",
            ];
            let mut ids: Vec<(&str, String)> = Vec::new();
            for role in roles {
                let Some(window) = data.by_role(role) else {
                    return false;
                };
                ids.push((role, window.id.clone()));
            }
            let main = data.by_role(MAIN_ROLE).expect("checked above");
            let Some(usable) = data.usable.get(&main.monitor).copied() else {
                return false;
            };

            // The 0.4.0 monitor-page proportions from shell.qml, inflated by
            // the compositor's 14px chrome halo so visible borders land on
            // the grid.
            let pad = 10.0;
            let chrome = 14.0;
            let (w, h) = (usable.width, usable.height);
            let col_left = w * 0.32;
            let col_right = w * 0.30;
            let col_mid = w - col_left - col_right - pad * 4.0;
            let mem_h = (h - pad * 4.0) * 0.40;
            let disk_h = (h - pad * 4.0) * 0.32;
            let cpu_h = (h - pad * 4.0) * 0.36;
            let gpu_h = (h - pad * 4.0) * 0.26;
            let place = |role: &str, x: f64, y: f64, width: f64, height: f64| {
                if let Some((_, id)) = ids.iter().find(|(r, _)| *r == role) {
                    self.ipc.set_rect(
                        id,
                        RectF {
                            x: usable.x + x - chrome,
                            y: usable.y + y - chrome,
                            width: width + chrome * 2.0,
                            height: height + chrome * 2.0,
                        },
                    );
                }
            };
            place("minkamon.memory", pad, pad, col_left, mem_h);
            place("minkamon.disk", pad, pad * 2.0 + mem_h, col_left, disk_h);
            place(
                "minkamon.network",
                pad,
                pad * 3.0 + mem_h + disk_h,
                col_left,
                h - mem_h - disk_h - pad * 4.0,
            );
            place(MAIN_ROLE, col_left + pad * 2.0, pad, col_mid, h - pad * 2.0);
            place("minkamon.cpu", w - col_right - pad, pad, col_right, cpu_h);
            place(
                "minkamon.gpu",
                w - col_right - pad,
                pad * 2.0 + cpu_h,
                col_right,
                gpu_h,
            );
            place(
                "minkamon.globe",
                w - col_right - pad,
                pad * 3.0 + cpu_h + gpu_h,
                col_right,
                h - cpu_h - gpu_h - pad * 4.0,
            );
            true
        });
        if arranged {
            self.overview_pending.set(false);
        } else if self
            .ipc_data
            .with(|data| data.by_role(MAIN_ROLE).is_some() && data.usable.is_empty())
        {
            self.ipc.request_geometry();
        }
    }

    fn on_duplicate(&self) {
        if self.pad_mode.get_untracked() {
            self.pad_lifted.set(true);
            if let Some(handle) = self.pad_handle.borrow().as_ref() {
                handle.set_layer(Layer::Top);
            }
            let generation = self.pad_lift_generation.get() + 1;
            self.pad_lift_generation.set(generation);
            let state = self.clone();
            set_timeout(Duration::from_millis(4000), move || {
                if state.pad_lift_generation.get() == generation {
                    state.pad_lifted.set(false);
                    if let Some(handle) = state.pad_handle.borrow().as_ref() {
                        handle.set_layer(Layer::Bottom);
                    }
                }
            });
            return;
        }

        self.raise_pending.set(true);
        self.ipc.ensure_started();
        self.ipc.request_windows();
        let state = self.clone();
        set_timeout(Duration::from_millis(250), move || {
            let id = state.ipc_data.with(|data| {
                data.by_role(MAIN_ROLE)
                    .or_else(|| data.by_title("MinkaMon"))
                    .map(|window| window.id.clone())
            });
            if let Some(id) = id {
                state.ipc.activate(&id);
            }
            state.raise_pending.set(false);
        });
    }

    fn service_tick(&self) {
        let tick = self.service_tick.get() + 1;
        self.service_tick.set(tick);

        if self.guard.take_rejected() {
            self.on_duplicate();
        }
        if tick % 5 == 1 {
            self.poll_settings();
            self.prune_window_handles();
        }

        let active = self.ipc_active();
        let leader = !self.open_windows.with_untracked(|open| open.is_empty())
            && (self.pad_mode.get_untracked()
                || matches!(
                    self.main_host.get_untracked(),
                    MainHost::Floating | MainHost::Pad(_)
                ));
        self.leader_visible.set_if_changed(leader);
        if active {
            self.ipc.ensure_started();
            self.ipc.request_windows();
        }

        let events = self.ipc.take_events();
        if !events.is_empty() {
            let update = self
                .ipc_data
                .update(|data| self.ipc.apply_events(data, events));
            if update.just_connected && active {
                self.ipc.request_windows();
                if self.overview_pending.get_untracked() {
                    self.ipc.request_geometry();
                }
            }
            if update.windows_changed {
                self.claim_roles();
                self.try_arrange_overview();
            }
            if update.geometry_changed {
                self.try_arrange_overview();
            }
        }
    }
}

fn schedule_service(state: AppState) {
    set_timeout(Duration::from_millis(200), move || {
        state.service_tick();
        schedule_service(state);
    });
}
