//! Sortable process table with desktop-entry icons and a kill menu.

use crate::model::*;
use crate::state::AppState;
use crate::theme::*;
use anti_webui::prelude::*;
use anti_webui::shell::{apps, icon_path};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortKey {
    Pid,
    Comm,
    State,
    CpuPct,
    RssKb,
}

fn sorted_rows(mut rows: Vec<ProcRow>, key: SortKey, desc: bool) -> Vec<ProcRow> {
    let sign = if desc { -1.0 } else { 1.0 };
    rows.sort_by(|a, b| {
        let ordering = match key {
            SortKey::Comm => a.comm.to_lowercase().cmp(&b.comm.to_lowercase()),
            SortKey::State => a.state.cmp(&b.state),
            SortKey::Pid => a.pid.cmp(&b.pid),
            SortKey::CpuPct => a.cpu_pct.total_cmp(&b.cpu_pct),
            SortKey::RssKb => a.rss_kb.cmp(&b.rss_kb),
        };
        if sign < 0.0 {
            ordering.reverse()
        } else {
            ordering
        }
    });
    rows
}

fn toggle_sort(key: SortKey, sort_key: Signal<SortKey>, sort_desc: Signal<bool>) {
    if sort_key.get_untracked() == key {
        sort_desc.set(!sort_desc.get_untracked());
    } else {
        sort_key.set(key);
        sort_desc.set(key != SortKey::Comm);
    }
}

type IconCache = Rc<RefCell<BTreeMap<String, Option<PathBuf>>>>;

fn resolve_icon(comm: &str) -> (Option<PathBuf>, bool) {
    let key = comm.to_lowercase();
    if let Some(path) = icon_path(comm, 32).or_else(|| icon_path(&key, 32)) {
        return (Some(path), true);
    }

    let all = apps();
    if !all.loaded() {
        return (icon_path("application-x-executable", 32), false);
    }
    for app in all.search(comm) {
        let id = app.id.trim_end_matches(".desktop");
        let base = id.rsplit('.').next().unwrap_or(id).to_lowercase();
        let icon_name = app.icon.as_deref().unwrap_or("").to_lowercase();
        let strict = base == key
            || icon_name == key
            || app.name.eq_ignore_ascii_case(comm)
            || (comm.chars().count() == 15 && base.starts_with(&key));
        if strict {
            return (
                app.icon_path(32)
                    .or_else(|| icon_path("application-x-executable", 32)),
                true,
            );
        }
    }
    (icon_path("application-x-executable", 32), true)
}

fn icon_for(comm: &str, cache: &IconCache) -> Option<PathBuf> {
    if comm.is_empty() {
        return None;
    }
    let key = comm.to_lowercase();
    if let Some(hit) = cache.borrow().get(&key) {
        return hit.clone();
    }
    let (path, cacheable) = resolve_icon(comm);
    if cacheable {
        cache.borrow_mut().insert(key, path.clone());
    }
    path
}

#[derive(Clone)]
struct MenuState {
    pid: i64,
    comm: String,
    x: f32,
    y: f32,
}

fn header_cell(
    label: &'static str,
    key: SortKey,
    width: Option<f32>,
    align_right: bool,
    align_center: bool,
    sort_key: Signal<SortKey>,
    sort_desc: Signal<bool>,
) -> Div {
    let mut cell = div()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .cursor(CursorIcon::Pointer)
        .on_click(move |_| toggle_sort(key, sort_key, sort_desc));
    cell = match width {
        Some(w) => cell.w(w),
        None => cell.flex_1(),
    };
    if align_right {
        cell = cell.justify_end();
    } else if align_center {
        cell = cell.justify_center();
    }
    if key == SortKey::Comm {
        cell = cell.pl(14.0_f32);
    }

    cell.child(
        mono_text(
            move || {
                if sort_key.get() == key {
                    format!("{}{}", label, if sort_desc.get() { " ▾" } else { " ▴" })
                } else {
                    label.to_owned()
                }
            },
            FONT_SIZE - 2.0,
            TEXT_MUTED,
        )
        .letter_spacing(1.0_f32)
        .dyn_style(move |s| {
            s.text_color(if sort_key.get() == key {
                color(RED)
            } else {
                color(TEXT_MUTED)
            })
        }),
    )
}

fn send_signal(pid: i64, signal: &'static str) {
    let _ = std::process::Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .spawn();
}

fn fixed_cell(width: f32, align_right: bool, align_center: bool, child: Text) -> Div {
    let mut cell = div().w(width).h_full().flex().flex_row().items_center();
    if align_right {
        cell = cell.justify_end();
    } else if align_center {
        cell = cell.justify_center();
    }
    cell.child(child)
}

fn process_row(
    index: usize,
    rows: Signal<Vec<ProcRow>>,
    menu: Signal<Option<MenuState>>,
    panel_size: Signal<(f32, f32)>,
    icon_cache: IconCache,
) -> Div {
    let pid = fixed_cell(
        70.0,
        true,
        false,
        mono_text(
            move || {
                rows.with(|rows| {
                    rows.get(index)
                        .map_or_else(String::new, |row| row.pid.to_string())
                })
            },
            FONT_SIZE - 2.0,
            TEXT_FAINT,
        ),
    );

    let icon_cache_for_view = Rc::clone(&icon_cache);
    let name = div()
        .relative()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(14.0)
                .top(4.0)
                .w(16.0)
                .h(16.0)
                .child(dyn_view(move || {
                    let comm = rows.with(|rows| {
                        rows.get(index)
                            .map(|row| row.comm.clone())
                            .unwrap_or_default()
                    });
                    match icon_for(&comm, &icon_cache_for_view) {
                        Some(path) => div().child(img(path).w(16.0).h(16.0)),
                        None => div(),
                    }
                })),
        )
        .child(
            div()
                .absolute()
                .left(38.0)
                .right(0.0)
                .top(0.0)
                .bottom(0.0)
                .flex()
                .items_center()
                .overflow_hidden()
                .child(
                    mono_text(
                        move || {
                            rows.with(|rows| {
                                rows.get(index)
                                    .map(|row| row.comm.clone())
                                    .unwrap_or_default()
                            })
                        },
                        FONT_SIZE - 2.0,
                        TEXT,
                    )
                    .whitespace_nowrap(),
                ),
        );

    let state = fixed_cell(
        30.0,
        false,
        true,
        mono_text(
            move || {
                rows.with(|rows| {
                    rows.get(index)
                        .map(|row| row.state.clone())
                        .unwrap_or_default()
                })
            },
            FONT_SIZE - 2.0,
            TEXT_FAINT,
        )
        .dyn_style(move |s| {
            let running = rows.with(|rows| rows.get(index).is_some_and(|row| row.state == "R"));
            s.text_color(if running {
                color(OK_GREEN)
            } else {
                color(TEXT_FAINT)
            })
        }),
    );

    let cpu = fixed_cell(
        80.0,
        true,
        false,
        mono_text(
            move || {
                rows.with(|rows| {
                    rows.get(index)
                        .map_or_else(String::new, |row| format!("{:.1}", row.cpu_pct))
                })
            },
            FONT_SIZE - 2.0,
            TEXT_MUTED,
        )
        .dyn_style(move |s| {
            let value = rows.with(|rows| rows.get(index).map_or(0.0, |row| row.cpu_pct));
            s.text_color(color(if value >= 50.0 {
                RED
            } else if value >= 10.0 {
                WARN_AMBER
            } else {
                TEXT_MUTED
            }))
        }),
    );

    let memory = fixed_cell(
        100.0,
        true,
        false,
        mono_text(
            move || {
                rows.with(|rows| {
                    rows.get(index)
                        .map_or_else(String::new, |row| fmt_kb(row.rss_kb))
                })
            },
            FONT_SIZE - 2.0,
            TEXT_MUTED,
        ),
    );

    div()
        .relative()
        .w_full()
        .h_full()
        .px(12.0_f32)
        .bg(if index % 2 == 1 {
            color(SURFACE)
        } else {
            Color::TRANSPARENT
        })
        .flex()
        .flex_row()
        .items_center()
        .on_pointer_down(move |event| {
            if event.button != Some(PointerButton::Secondary) {
                return;
            }
            let Some(row) = rows.with_untracked(|rows| rows.get(index).cloned()) else {
                return;
            };
            event.stop_propagation();
            let (panel_w, panel_h) = panel_size.get_untracked();
            menu.set(Some(MenuState {
                pid: row.pid,
                comm: row.comm,
                x: (event.position.x - 10.0).clamp(0.0, (panel_w - 194.0).max(0.0)),
                y: (event.position.y - 10.0).clamp(0.0, (panel_h - 88.0).max(0.0)),
            }));
        })
        .child(pid)
        .child(name)
        .child(state)
        .child(cpu)
        .child(memory)
}

fn menu_option(
    label: &'static str,
    hint: &'static str,
    signal_name: &'static str,
    pid: i64,
    menu: Signal<Option<MenuState>>,
) -> Div {
    let hovered = signal(false);
    div()
        .relative()
        .w_full()
        .h(28.0)
        .px(10.0_f32)
        .cursor(CursorIcon::Pointer)
        .dyn_style(move |s| {
            s.bg(if hovered.get() {
                color(RED_DIM)
            } else {
                Color::TRANSPARENT
            })
        })
        .on_hover(move |on| hovered.set(on))
        .on_pointer_down(|event| event.stop_propagation())
        .on_click(move |_| {
            send_signal(pid, signal_name);
            menu.set(None);
        })
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h_full()
                .child(
                    mono_text(
                        label,
                        FONT_SIZE - 2.0,
                        if signal_name == "KILL" { RED } else { TEXT },
                    )
                    .letter_spacing(1.0_f32)
                    .dyn_style(move |s| {
                        s.text_color(if signal_name == "KILL" && !hovered.get() {
                            color(RED)
                        } else {
                            color(TEXT)
                        })
                    }),
                )
                .child(div().flex_1())
                .child(mono_text(hint, FONT_SIZE - 3.0, TEXT_FAINT)),
        )
}

pub fn processes_panel(state: AppState) -> Div {
    let sort_key = signal(SortKey::CpuPct);
    let sort_desc = signal(true);
    let rows: Signal<Vec<ProcRow>> = signal(Vec::new());
    let menu: Signal<Option<MenuState>> = signal(None);
    let panel_size: Signal<(f32, f32)> = signal((800.0, 540.0));
    let icon_cache: IconCache = Rc::new(RefCell::new(BTreeMap::new()));

    let sort_state = state.clone();
    effect(move || {
        let raw = sort_state.procs.get();
        rows.set(sorted_rows(raw, sort_key.get(), sort_desc.get()));
    });

    let header = div()
        .h(30.0)
        .w_full()
        .bg(color(SURFACE))
        .border(1.0_f32)
        .border_color(color(LINE))
        .px(12.0_f32)
        .flex()
        .flex_row()
        .items_center()
        .child(header_cell(
            "PID",
            SortKey::Pid,
            Some(70.0),
            true,
            false,
            sort_key,
            sort_desc,
        ))
        .child(header_cell(
            "NAME",
            SortKey::Comm,
            None,
            false,
            false,
            sort_key,
            sort_desc,
        ))
        .child(header_cell(
            "S",
            SortKey::State,
            Some(30.0),
            false,
            true,
            sort_key,
            sort_desc,
        ))
        .child(header_cell(
            "CPU %",
            SortKey::CpuPct,
            Some(80.0),
            true,
            false,
            sort_key,
            sort_desc,
        ))
        .child(header_cell(
            "MEMORY",
            SortKey::RssKb,
            Some(100.0),
            true,
            false,
            sort_key,
            sort_desc,
        ));

    let row_cache = Rc::clone(&icon_cache);
    let list = div()
        .relative()
        .flex_1()
        .w_full()
        .overflow_hidden()
        .child(
            virtual_list(
                move || rows.with(|rows| rows.len()),
                24.0,
                move |index| process_row(index, rows, menu, panel_size, Rc::clone(&row_cache)),
            )
            .w_full()
            .h_full(),
        )
        .child(dyn_view(move || {
            if rows.with(|rows| rows.is_empty()) {
                div()
                    .absolute()
                    .inset(0.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .pointer_events_none()
                    .child(mono_text("— WAITING", FONT_SIZE - 2.0, TEXT_FAINT))
            } else {
                div()
            }
        }));

    let catcher_menu = menu;
    let overlay_menu = menu;
    div()
        .relative()
        .w_full()
        .h_full()
        .overflow_hidden()
        .p(10.0_f32)
        .on_resized(move |w, h| {
            if w > 0.0 && h > 0.0 {
                panel_size.set_if_changed((w, h));
            }
        })
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(header)
                .child(div().h(4.0))
                .child(list),
        )
        .child(dyn_view(move || {
            if catcher_menu.get().is_some() {
                div()
                    .absolute()
                    .inset(0.0)
                    .z_index(10)
                    .on_pointer_down(move |_| catcher_menu.set(None))
            } else {
                div()
            }
        }))
        .child(dyn_view(move || {
            let Some(open) = overlay_menu.get() else {
                return div();
            };
            div()
                .absolute()
                .left(open.x)
                .top(open.y)
                .w(190.0)
                .z_index(11)
                .bg(color(SURFACE_RAISED))
                .border(1.0_f32)
                .border_color(color(LINE))
                .p(1.0_f32)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .child(
                            div()
                                .h(26.0)
                                .w_full()
                                .bg(color(SURFACE))
                                .px(10.0_f32)
                                .flex()
                                .items_center()
                                .child(
                                    mono_text(
                                        format!("{}  [{}]", open.comm, open.pid),
                                        FONT_SIZE - 2.0,
                                        TEXT_MUTED,
                                    )
                                    .whitespace_nowrap(),
                                ),
                        )
                        .child(menu_option(
                            "TERMINATE",
                            "SIGTERM",
                            "TERM",
                            open.pid,
                            overlay_menu,
                        ))
                        .child(menu_option(
                            "KILL",
                            "SIGKILL",
                            "KILL",
                            open.pid,
                            overlay_menu,
                        )),
                )
        }))
}
