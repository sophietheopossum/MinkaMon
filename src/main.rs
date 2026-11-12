//! MinkaMon application entry point and Shell hosting.

mod globe;
mod instance;
mod ipc;
mod model;
mod panels;
mod processes;
mod schematic;
mod state;
mod theme;
mod widgets;

use crate::ipc::leader_lines;
use crate::model::machine_is_zenbook;
use crate::state::{AppState, MainHost};
use crate::theme::*;
use anti_webui::prelude::*;
use anti_webui::shell::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

fn is_screenpad(output: &Output) -> bool {
    output.logical_size.width >= 1600 && output.logical_size.height <= 600
}

fn screenpad_signature(output: &Output) -> String {
    format!(
        "{}:{}x{}@{},{}",
        output.name,
        output.logical_size.width,
        output.logical_size.height,
        output.position.x,
        output.position.y
    )
}

fn install_screenpad_tracking(state: AppState) {
    let last_signature: Rc<RefCell<Option<Option<String>>>> = Rc::new(RefCell::new(None));
    let generation = Rc::new(Cell::new(0_u64));

    effect(move || {
        let all = outputs();
        let pad = all.iter().find(|output| is_screenpad(output));
        let signature = pad.map(screenpad_signature);

        let changed = last_signature.borrow().as_ref() != Some(&signature);
        if changed {
            *last_signature.borrow_mut() = Some(signature.clone());
            generation.set(generation.get() + 1);
            state.pad_settled.set(false);
        }

        if signature.is_some() && !state.pad_settled.get_untracked() {
            let expected = generation.get();
            let settle_state = state.clone();
            let settle_generation = Rc::clone(&generation);
            set_timeout(Duration::from_millis(2500), move || {
                if settle_generation.get() == expected {
                    settle_state.pad_settled.set(true);
                }
            });
        }

        let pad_mode = state.configured_layout.get() != "general" && signature.is_some();
        state.pad_mode.set_if_changed(pad_mode);

        let desired = if !pad_mode {
            MainHost::Floating
        } else if state.pad_settled.get() {
            match pad {
                Some(output) => MainHost::Pad(output.name.clone()),
                None => MainHost::None,
            }
        } else {
            MainHost::None
        };
        state.main_host.set_if_changed(desired);
    });
}

fn install_main_host(state: AppState) {
    effect(move || match state.main_host.get() {
        MainHost::Floating => {
            let body_state = state.clone();
            let handle = open_window(
                WindowOptions::new().title("MinkaMon").size(620, 560),
                move || schematic::main_body(body_state, true),
            );
            state.set_main_handle(Some(handle.clone()));
            let cleanup_state = state.clone();
            on_cleanup(move || {
                handle.close();
                cleanup_state.set_main_handle(None);
            });
        }
        MainHost::Pad(name) => {
            let Some(output) = outputs().into_iter().find(|o| o.name == name) else {
                return;
            };
            let lifted = state.pad_lifted.get_untracked();
            let body_state = state.clone();
            let handle = open_layer(
                LayerOptions::new()
                    .layer(if lifted { Layer::Top } else { Layer::Bottom })
                    .anchor(Anchor::ALL)
                    .exclusive(Exclusive::None)
                    .keyboard(Keyboard::None)
                    .output(&output)
                    .namespace("minkamon"),
                move || schematic::main_body(body_state, false),
            );
            state.set_pad_handle(Some(handle));
            let cleanup_state = state.clone();
            on_cleanup(move || cleanup_state.set_pad_handle(None));
        }
        MainHost::None => {}
    });
}

fn leader_overlay_body(state: AppState, output: Output) -> Div {
    let size: Signal<(f32, f32)> = signal((
        output.logical_size.width.max(1) as f32,
        output.logical_size.height.max(1) as f32,
    ));
    let ox = output.position.x as f64;
    let oy = output.position.y as f64;

    div()
        .w_full()
        .h_full()
        .overflow_hidden()
        .pointer_events_none()
        .on_resized(move |w, h| {
            if w > 0.0 && h > 0.0 {
                size.set_if_changed((w, h));
            }
        })
        .child(dyn_view(move || {
            let (width, height) = size.get();
            let zenbook = state.meta.with(machine_is_zenbook);
            let lines = state.ipc_data.with(|data| {
                leader_lines(data, |zone, client_w, client_h| {
                    schematic::anchor_scene(zone, zenbook, client_w, client_h)
                })
            });

            let mut chart = svg()
                .view_box(0.0, 0.0, width, height)
                .aspect(svg::SvgAspect {
                    align: None,
                    slice: false,
                })
                .w_full()
                .h_full();
            for line in lines {
                let dx = line.bx - line.ax;
                let dy = line.by - line.ay;
                for span in &line.spans {
                    chart = chart.child(
                        svg::line(
                            line.ax + dx * span.0 - ox,
                            line.ay + dy * span.0 - oy,
                            line.ax + dx * span.1 - ox,
                            line.ay + dy * span.1 - oy,
                        )
                        .fill_none()
                        .stroke(color_alpha(RED, 0.62))
                        .stroke_width(1.2_f32),
                    );
                }
                if let Some(first) = line.spans.first()
                    && first.0 < 0.004
                {
                    chart = chart.child(
                        svg::rect(line.ax - ox - 2.0, line.ay - oy - 2.0, 4.0, 4.0)
                            .fill(color_alpha(RED, 0.62)),
                    );
                }
                if let Some(last) = line.spans.last()
                    && last.1 > 0.996
                {
                    chart = chart.child(
                        svg::circle(line.bx - ox, line.by - oy, 2.5).fill(color_alpha(RED, 0.62)),
                    );
                }
            }
            chart
        }))
}

fn install_leader_overlays(state: AppState) {
    for_each_output(move |output| {
        let effect_state = state.clone();
        effect(move || {
            if !effect_state.leader_visible.get() {
                return;
            }
            let body_state = effect_state.clone();
            let body_output = output.clone();
            open_layer(
                LayerOptions::new()
                    .layer(Layer::Top)
                    .anchor(Anchor::ALL)
                    .exclusive(Exclusive::Ignore)
                    .input_region(InputRegion::Empty)
                    .output(&output)
                    .namespace("minkamon-leaderlines"),
                move || leader_overlay_body(body_state, body_output),
            );
        });
    });
}

fn shell_root(guard: instance::InstanceGuard) {
    let state = AppState::new(guard);
    state.spawn_sampler();
    state.install_demand_effect();
    state.start_services();
    install_screenpad_tracking(state.clone());
    install_main_host(state.clone());
    install_leader_overlays(state);
}

fn main() {
    let guard = match instance::claim() {
        instance::InstanceClaim::Primary(guard) => guard,
        instance::InstanceClaim::Duplicate => return,
    };

    if let Err(error) = Shell::new()
        .namespace("minkamon")
        .app_id("minkamon")
        .run(move || shell_root(guard))
    {
        eprintln!("minkamon: {error}");
        std::process::exit(1);
    }
}
