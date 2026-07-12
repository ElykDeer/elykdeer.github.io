use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::collections::VecDeque;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use leptos::html::Canvas;
#[cfg(target_arch = "wasm32")]
use leptos::reactive::owner::on_cleanup;
#[cfg(target_arch = "wasm32")]
use send_wrapper::SendWrapper;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent, MouseEvent, PointerEvent,
    WheelEvent,
};

#[cfg(target_arch = "wasm32")]
use super::state::{
    station_cell, BootstrapActivityPhase, Cell, Direction, Lcg, SavedSnekGame, Snek,
    SnekGame as SnekState, SplitterActivity, SplitterActivityPhase, SplitterStation, StationDesign,
    APPLE_COST, AUTO_SNEK_COST, DEFAULT_COLS, DEFAULT_ROWS, JUICER_PLACEMENT_UNLOCK_COST,
    MIN_SNEK_LEN, SPLITTER_STATION_UNLOCK_COST,
};

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "elyk.snek.save.v14";
#[cfg(target_arch = "wasm32")]
const CLEAR_EVENT: &str = "elyk:snek-clear";
#[cfg(target_arch = "wasm32")]
const CELL_SIZE: f64 = 15.0;
#[cfg(target_arch = "wasm32")]
const STEP_MS: f64 = 50.0;
#[cfg(target_arch = "wasm32")]
const SAVE_MS: f64 = 5_000.0;
#[cfg(target_arch = "wasm32")]
const RATE_SAMPLE_MS: f64 = 3_000.0;
#[cfg(target_arch = "wasm32")]
const SWIPE_THRESHOLD: f64 = 18.0;
#[cfg(target_arch = "wasm32")]
const HUD_MARGIN: f64 = 14.0;
#[cfg(target_arch = "wasm32")]
const HUD_BUTTON_HEIGHT: f64 = 34.0;
#[cfg(target_arch = "wasm32")]
const STATION_OPTION_HEIGHT: f64 = 54.0;
#[cfg(target_arch = "wasm32")]
const ICON_BUTTON_SIZE: f64 = 34.0;
#[cfg(target_arch = "wasm32")]
const TOOLBELT_SLOTS: usize = 9;
#[cfg(target_arch = "wasm32")]
const TOOLBELT_SLOT_SIZE: f64 = 44.0;
#[cfg(target_arch = "wasm32")]
const TOOLBELT_GAP: f64 = 4.0;
#[cfg(target_arch = "wasm32")]
const SITE_BACKGROUND: &str = "#1e1e1e";
#[cfg(target_arch = "wasm32")]
const CAMERA_ABSOLUTE_MIN_ZOOM: f64 = 0.05;
#[cfg(target_arch = "wasm32")]
const CAMERA_MAX_ZOOM: f64 = 1.75;
#[cfg(target_arch = "wasm32")]
const CAMERA_NATIVE_ZOOM: f64 = 1.0;
#[cfg(target_arch = "wasm32")]
const CAMERA_NATIVE_STICKY_RANGE: f64 = 0.06;

#[cfg(target_arch = "wasm32")]
#[component]
pub fn SnekGame(#[prop(default = false)] cheat: bool) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let runner = Rc::new(RefCell::new(SnekRunner::new(cheat)));
    let status_text = RwSignal::new(runner.borrow().status_text());
    let animation = Rc::new(RefCell::new(AnimationLoop::default()));
    let clear_listener = Rc::new(RefCell::new(SnekClearListener::attach(
        Rc::clone(&runner),
        canvas_ref,
        status_text,
    )));

    let start_runner = Rc::clone(&runner);
    let start_animation = Rc::clone(&animation);
    let frame_status_text = status_text;
    Effect::new(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        start_runner.borrow_mut().render(&canvas);

        if start_animation.borrow().is_running() {
            return;
        }

        let animation_loop = Rc::clone(&start_animation);
        let frame_runner = Rc::clone(&start_runner);
        let frame_canvas = canvas.clone();
        let callback = Closure::wrap(Box::new(move |timestamp: f64| {
            {
                let mut runner = frame_runner.borrow_mut();
                runner.update(timestamp);
                runner.render(&frame_canvas);
                let status = runner.status_text();
                if frame_status_text.get_untracked() != status {
                    frame_status_text.set(status);
                }
            }

            if let Some(request_id) = request_animation_frame(&animation_loop) {
                animation_loop.borrow_mut().request_id = Some(request_id);
            }
        }) as Box<dyn FnMut(f64)>);

        start_animation.borrow_mut().callback = Some(callback);
        if let Some(request_id) = request_animation_frame(&start_animation) {
            start_animation.borrow_mut().request_id = Some(request_id);
        }
    });

    let cleanup_animation = SendWrapper::new(Rc::clone(&animation));
    let cleanup_runner = SendWrapper::new(Rc::clone(&runner));
    let cleanup_clear_listener = SendWrapper::new(Rc::clone(&clear_listener));
    on_cleanup(move || {
        cleanup_runner.borrow().save_now();
        cleanup_animation.borrow_mut().cancel();
        let _ = cleanup_clear_listener.borrow_mut().take();
    });

    let handle_click = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.focus();
        }
    };

    let key_runner = Rc::clone(&runner);
    let handle_keydown = move |ev: KeyboardEvent| {
        let direction = match ev.key().as_str() {
            "ArrowUp" | "w" | "W" => Some(Direction::Up),
            "ArrowDown" | "s" | "S" => Some(Direction::Down),
            "ArrowLeft" | "a" | "A" => Some(Direction::Left),
            "ArrowRight" | "d" | "D" => Some(Direction::Right),
            " " | "p" | "P" => {
                ev.prevent_default();
                key_runner.borrow_mut().toggle_pause();
                return;
            }
            _ => None,
        };

        if let Some(direction) = direction {
            ev.prevent_default();
            key_runner.borrow_mut().set_direction(direction);
        }
    };

    let pointer_down_runner = Rc::clone(&runner);
    let handle_pointer_down = move |ev: PointerEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.set_pointer_capture(ev.pointer_id());
            pointer_down_runner.borrow_mut().pointer_down(
                &canvas,
                ev.client_x() as f64,
                ev.client_y() as f64,
                ev.pointer_id(),
            );
            let _ = canvas.focus();
        }
    };

    let pointer_move_runner = Rc::clone(&runner);
    let handle_pointer_move = move |ev: PointerEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            pointer_move_runner.borrow_mut().pointer_move(
                &canvas,
                ev.client_x() as f64,
                ev.client_y() as f64,
                ev.pointer_id(),
            );
        }
    };

    let pointer_up_runner = Rc::clone(&runner);
    let handle_pointer_up = move |ev: PointerEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            pointer_up_runner.borrow_mut().pointer_up(
                &canvas,
                ev.client_x() as f64,
                ev.client_y() as f64,
                ev.pointer_id(),
            );
            let _ = canvas.release_pointer_capture(ev.pointer_id());
            let _ = canvas.focus();
        }
    };

    let pointer_cancel_runner = Rc::clone(&runner);
    let handle_pointer_cancel = move |ev: PointerEvent| {
        if let Some(canvas) = canvas_ref.get() {
            pointer_cancel_runner
                .borrow_mut()
                .pointer_cancel(ev.pointer_id());
            let _ = canvas.release_pointer_capture(ev.pointer_id());
        }
    };

    let wheel_runner = Rc::clone(&runner);
    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            wheel_runner.borrow_mut().wheel(
                &canvas,
                ev.client_x() as f64,
                ev.client_y() as f64,
                ev.delta_y(),
            );
            let _ = canvas.focus();
        }
    };

    view! {
        <section
            class="terminal-game snek-game"
            aria-label="Snek"
            style="width:100%;height:calc(var(--app-height,100vh) - 5rem);max-height:none;margin:0;padding:0;overflow:hidden;background:#1e1e1e;"
        >
            <canvas
                class="terminal-game-canvas snek-game-canvas"
                style="width:100%;height:100%;border:0;background:#1e1e1e;cursor:pointer;image-rendering:pixelated;touch-action:none;"
                node_ref=canvas_ref
                tabindex="0"
                role="application"
                aria-label="Snek game canvas. Move with arrow keys, WASD, or swipe. Pause with Space."
                on:click=handle_click
                on:keydown=handle_keydown
                on:pointerdown=handle_pointer_down
                on:pointermove=handle_pointer_move
                on:pointerup=handle_pointer_up
                on:pointercancel=handle_pointer_cancel
                on:wheel=handle_wheel
            />
            <div class="visually-hidden" role="status" aria-live="polite">
                {move || status_text.get()}
            </div>
        </section>
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn SnekGame(#[prop(default = false)] cheat: bool) -> impl IntoView {
    let _ = cheat;
    view! {
        <section class="terminal-game">
            <div class="terminal-game-header">
                <span>"snek"</span>
            </div>
            <div class="terminal-game-fallback">
                "The Snek canvas is available in the browser build."
            </div>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
pub fn clear_save() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item(STORAGE_KEY);
        }
        if let Ok(event) = web_sys::Event::new(CLEAR_EVENT) {
            let _ = window.dispatch_event(&event);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_save() {}

#[cfg(target_arch = "wasm32")]
struct SnekClearListener {
    window: web_sys::Window,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(target_arch = "wasm32")]
impl SnekClearListener {
    fn attach(
        runner: Rc<RefCell<SnekRunner>>,
        canvas_ref: NodeRef<Canvas>,
        status_text: RwSignal<String>,
    ) -> Option<Self> {
        let window = web_sys::window()?;
        let callback = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let mut runner = runner.borrow_mut();
            runner.reset_after_clear();
            if let Some(canvas) = canvas_ref.get() {
                runner.render(&canvas);
            }
            let status = runner.status_text();
            if status_text.get_untracked() != status {
                status_text.set(status);
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        window
            .add_event_listener_with_callback(CLEAR_EVENT, callback.as_ref().unchecked_ref())
            .ok()?;

        Some(Self { window, callback })
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for SnekClearListener {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            CLEAR_EVENT,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct SnekLayout {
    width: f64,
    height: f64,
    cols: usize,
    rows: usize,
    dpr: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
}

#[cfg(target_arch = "wasm32")]
struct SnekRunner {
    game: SnekState,
    rng: Lcg,
    paused: bool,
    last_step: f64,
    last_save: f64,
    dirty: bool,
    save_enabled: bool,
    pointer_start: Option<(f64, f64)>,
    buttons: Vec<HudButton>,
    station_draft: Option<StationDraft>,
    selected_station: Option<usize>,
    drag_pointer: Option<i32>,
    draft_anchor: Option<(f64, f64)>,
    camera: Camera,
    last_layout: Option<SnekLayout>,
    panning_pointer: Option<i32>,
    pan_start: Option<(f64, f64, f64, f64)>,
    population_open: bool,
    population_scroll: f64,
    population_drag: Option<(i32, f64, f64)>,
    unlock_queue_open: bool,
    toolbar_collapsed: bool,
    last_rate_sample: f64,
    rate_totals: (usize, usize, usize),
    rates: (f64, f64, f64),
    cheat: bool,
    active_pointers: Vec<ActivePointer>,
    pinch: Option<PinchState>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ActivePointer {
    id: i32,
    x: f64,
    y: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct PinchState {
    first_id: i32,
    second_id: i32,
    start_distance: f64,
    start_zoom: f64,
    focus_world_x: f64,
    focus_world_y: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct HudSnapshot<'a> {
    game: &'a SnekState,
    draft: Option<&'a StationDraft>,
    selected_station: Option<usize>,
    camera: Camera,
    population_open: bool,
    population_scroll: f64,
    unlock_queue_open: bool,
    toolbar_collapsed: bool,
    rates: (f64, f64, f64),
    cheat: bool,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct HudButton {
    kind: HudButtonKind,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum HudButtonKind {
    UnlockJuicer,
    UnlockJuicerPlacement,
    AutoSnek,
    StationOption(StationDesign),
    PlaceStation,
    RotateStation,
    CancelStation,
    MoveStation,
    DeleteStation,
    BuyApple,
    BuySplitterUnlock,
    UnlockQueueToggle,
    ToolbarToggle,
    PopulationToggle,
    PopulationClose,
    MaxLengthDown,
    MaxLengthUp,
    TargetDown(usize),
    TargetUp(usize),
    DebugAdd10Apls,
    DebugAdd10Juice,
    PanelBlocker,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct StationDraft {
    origin: Cell,
    rotation: u8,
    design: StationDesign,
    dragging: bool,
    source: StationDraftSource,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum StationDraftSource {
    New,
    Move { index: usize },
}

#[cfg(target_arch = "wasm32")]
impl SnekRunner {
    fn new(cheat: bool) -> Self {
        let seed = random_seed();
        let game = load_save()
            .and_then(|save| SnekState::from_save(save, DEFAULT_COLS, DEFAULT_ROWS))
            .unwrap_or_else(|| SnekState::new(DEFAULT_COLS, DEFAULT_ROWS, seed));
        let rate_totals = game.production_totals();

        Self {
            game,
            rng: Lcg::new(seed ^ 0xa2f9_83d4_6e7b_5119),
            paused: false,
            last_step: 0.0,
            last_save: 0.0,
            dirty: false,
            save_enabled: true,
            pointer_start: None,
            buttons: Vec::new(),
            station_draft: None,
            selected_station: None,
            drag_pointer: None,
            draft_anchor: None,
            camera: Camera {
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
            },
            last_layout: None,
            panning_pointer: None,
            pan_start: None,
            population_open: false,
            population_scroll: 0.0,
            population_drag: None,
            unlock_queue_open: true,
            toolbar_collapsed: true,
            last_rate_sample: 0.0,
            rate_totals,
            rates: (0.0, 0.0, 0.0),
            cheat,
            active_pointers: Vec::new(),
            pinch: None,
        }
    }

    fn update(&mut self, timestamp: f64) {
        let toolbelt_was_available = self.game.toolbelt_available();
        if self.last_step == 0.0 {
            self.last_step = timestamp;
        }

        if !self.paused {
            let mut steps = 0;
            while timestamp - self.last_step >= STEP_MS && steps < 6 {
                let step_timestamp = self.last_step + STEP_MS;
                self.game.step(&mut self.rng);
                self.last_step = step_timestamp;
                self.dirty = true;
                steps += 1;
            }
            if steps == 6 {
                self.last_step = timestamp;
            }
        } else {
            self.last_step = timestamp;
        }

        if !toolbelt_was_available && self.game.toolbelt_available() {
            self.selected_station = None;
            self.station_draft = None;
            self.population_open = false;
            self.population_drag = None;
            self.unlock_queue_open = false;
            self.toolbar_collapsed = false;
            self.pointer_start = None;
            self.drag_pointer = None;
            self.draft_anchor = None;
            self.panning_pointer = None;
            self.pan_start = None;
            self.active_pointers.clear();
            self.pinch = None;
        }

        if self.dirty && timestamp - self.last_save >= SAVE_MS {
            self.save_now();
            self.last_save = timestamp;
        }
        if self.last_rate_sample == 0.0 {
            self.last_rate_sample = timestamp;
            self.rate_totals = self.game.production_totals();
        } else if timestamp - self.last_rate_sample >= RATE_SAMPLE_MS {
            let seconds = (timestamp - self.last_rate_sample) / 1_000.0;
            let totals = self.game.production_totals();
            self.rates = (
                totals.0.saturating_sub(self.rate_totals.0) as f64 / seconds,
                totals.1.saturating_sub(self.rate_totals.1) as f64 / seconds,
                totals.2.saturating_sub(self.rate_totals.2) as f64 / seconds,
            );
            self.rate_totals = totals;
            self.last_rate_sample = timestamp;
        }
    }

    fn render(&mut self, canvas: &HtmlCanvasElement) {
        let layout = resize_canvas(canvas);
        self.last_layout = Some(layout);
        self.game.resize(layout.cols, layout.rows);
        self.clamp_camera(layout);
        if self
            .selected_station
            .is_some_and(|index| index >= self.game.splitter_stations().len())
        {
            self.selected_station = None;
        }
        let Some(context) = canvas_context(canvas) else {
            return;
        };

        reset_screen_transform(&context, layout);
        context.set_fill_style_str(SITE_BACKGROUND);
        context.fill_rect(0.0, 0.0, layout.width, layout.height);

        set_world_transform(
            &context,
            layout,
            self.camera,
            self.game.cols(),
            self.game.rows(),
        );
        draw_board_background(&context, self.game.cols(), self.game.rows());
        for apple in self.game.apples() {
            draw_apple(&context, *apple);
        }
        let moving_station = self
            .station_draft
            .as_ref()
            .and_then(|draft| match draft.source {
                StationDraftSource::Move { index } => Some(index),
                StationDraftSource::New => None,
            });
        for (index, station) in self.game.splitter_stations().iter().copied().enumerate() {
            if moving_station == Some(index) {
                continue;
            }
            let construction = self.game.construction_progress(station);
            draw_station(
                &context,
                station,
                CELL_SIZE,
                construction.map_or(1.0, |progress| 0.3 + progress * 0.55),
                self.game.wiggle_t(),
                self.game.juicer_activity(station),
            );
            if let Some(progress) = construction {
                draw_construction_packets(&context, station, progress);
            }
            if self.selected_station == Some(index) {
                draw_station_selection(&context, station);
            }
        }
        if let Some(draft) = self.station_draft.as_ref() {
            draw_station_pixels(
                &context,
                draft.origin.col as f64 * CELL_SIZE,
                draft.origin.row as f64 * CELL_SIZE,
                draft.design,
                draft.rotation,
                CELL_SIZE,
                if draft.dragging { 0.76 } else { 0.9 },
            );
            let moving_index = match draft.source {
                StationDraftSource::Move { index } => Some(index),
                StationDraftSource::New => None,
            };
            if !self.game.station_draft_valid(
                draft.origin,
                draft.rotation,
                draft.design,
                moving_index,
            ) {
                draw_station_invalid_overlay(
                    &context,
                    draft.origin.col as f64 * CELL_SIZE,
                    draft.origin.row as f64 * CELL_SIZE,
                    draft.design,
                    draft.rotation,
                    CELL_SIZE,
                );
            }
        }
        for snek in self.game.sneks() {
            draw_snek(
                &context,
                snek,
                self.game.wiggle_t(),
                self.game.auto_snek_unlocked() && self.game.manual_snek() == Some(snek.id()),
            );
        }
        let bootstrap_activity = self.game.bootstrap_activity();
        if let Some(activity) = bootstrap_activity {
            match activity.phase {
                BootstrapActivityPhase::Cutting => {
                    draw_bootstrap_blade(&context, activity.center, activity.progress);
                }
                BootstrapActivityPhase::Walking => draw_bootstrap_cost_walk(
                    &context,
                    self.game.bootstrap_cost_segments(),
                    activity.direction,
                    activity.progress,
                    activity.center,
                    self.game.cols(),
                ),
                BootstrapActivityPhase::Routing | BootstrapActivityPhase::Flash => {}
            }
        }
        draw_juicer_feeds(&context, self.game.juicer_feeds(), self.game.wiggle_t());
        let splitter_activities = self.game.splitter_activities();
        for activity in splitter_activities.iter().copied() {
            if activity.phase() == SplitterActivityPhase::Cutting {
                draw_split_effect(&context, activity);
            }
        }
        for (index, station) in self.game.splitter_stations().iter().copied().enumerate() {
            if moving_station == Some(index) || self.game.construction_progress(station).is_some() {
                continue;
            }
            let activity = splitter_activities
                .iter()
                .rev()
                .copied()
                .find(|activity| activity.station() == station);
            draw_station_access(&context, station, activity, self.game.juicer_busy(station));
        }
        reset_screen_transform(&context, layout);
        self.buttons = draw_hud(
            &context,
            layout,
            HudSnapshot {
                game: &self.game,
                draft: self.station_draft.as_ref(),
                selected_station: self.selected_station,
                camera: self.camera,
                population_open: self.population_open,
                population_scroll: self.population_scroll,
                unlock_queue_open: self.unlock_queue_open,
                toolbar_collapsed: self.toolbar_collapsed,
                rates: self.rates,
                cheat: self.cheat,
            },
        );
        if self.paused {
            draw_paused(&context, layout);
        }
        if let Some(activity) =
            bootstrap_activity.filter(|activity| activity.phase == BootstrapActivityPhase::Flash)
        {
            draw_bootstrap_flash(
                &context,
                layout,
                self.camera,
                self.game.cols(),
                self.game.rows(),
                activity.flash_origin,
                activity.progress,
            );
        }
    }

    fn set_direction(&mut self, direction: Direction) {
        self.game.set_direction(direction);
        self.dirty = true;
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.last_step = 0.0;
        self.dirty = true;
    }

    fn pointer_down(&mut self, canvas: &HtmlCanvasElement, x: f64, y: f64, pointer_id: i32) {
        if self
            .station_draft
            .as_ref()
            .is_some_and(|draft| draft.dragging)
            && self.drag_pointer != Some(pointer_id)
        {
            self.rotate_station_draft();
            return;
        }

        let Some((canvas_x, canvas_y)) = canvas_point(canvas, x, y) else {
            return;
        };

        if let Some(kind) = self.button_at(canvas_x, canvas_y) {
            match kind {
                HudButtonKind::StationOption(design) => {
                    self.start_station_draft(canvas_x, canvas_y, design, pointer_id);
                }
                HudButtonKind::PanelBlocker if self.population_open => {
                    self.population_drag = Some((pointer_id, canvas_y, self.population_scroll));
                }
                _ => self.activate_button(kind),
            }
            return;
        }

        if self.station_draft.is_some() {
            self.drag_pointer = Some(pointer_id);
            let (world_x, world_y) = self.screen_to_world(canvas_x, canvas_y);
            self.draft_anchor = Some((world_x, world_y));
            if let Some(draft) = &mut self.station_draft {
                draft.dragging = true;
            }
            self.update_station_draft(world_x, world_y);
            self.pointer_start = None;
            return;
        }

        self.track_pointer(pointer_id, canvas_x, canvas_y);
        if self.active_pointers.len() >= 2 {
            self.start_pinch();
            return;
        }

        if self.game.auto_snek_unlocked() && self.game.manual_snek().is_none() {
            self.panning_pointer = Some(pointer_id);
            self.pan_start = Some((canvas_x, canvas_y, self.camera.pan_x, self.camera.pan_y));
        }
        self.pointer_start = Some((x, y));
    }

    fn pointer_move(&mut self, canvas: &HtmlCanvasElement, x: f64, y: f64, pointer_id: i32) {
        if let Some((drag_id, start_y, start_scroll)) = self.population_drag {
            if drag_id == pointer_id {
                if let (Some((_, canvas_y)), Some(layout)) =
                    (canvas_point(canvas, x, y), self.last_layout)
                {
                    self.population_scroll = (start_scroll + start_y - canvas_y)
                        .clamp(0.0, population_max_scroll(layout, &self.game));
                }
                return;
            }
        }
        if let Some((canvas_x, canvas_y)) = canvas_point(canvas, x, y) {
            self.update_active_pointer(pointer_id, canvas_x, canvas_y);
            if self.pinch.is_some() {
                self.update_pinch();
                return;
            }
        }

        if self.panning_pointer == Some(pointer_id) {
            if let (Some((canvas_x, canvas_y)), Some((start_x, start_y, pan_x, pan_y))) =
                (canvas_point(canvas, x, y), self.pan_start)
            {
                self.camera.pan_x = pan_x + start_x - canvas_x;
                self.camera.pan_y = pan_y + start_y - canvas_y;
            }
            return;
        }

        if self.drag_pointer != Some(pointer_id) {
            return;
        }
        let Some((canvas_x, canvas_y)) = canvas_point(canvas, x, y) else {
            return;
        };
        let (world_x, world_y) = self.screen_to_world(canvas_x, canvas_y);
        self.draft_anchor = Some((world_x, world_y));
        self.update_station_draft(world_x, world_y);
    }

    fn pointer_up(&mut self, canvas: &HtmlCanvasElement, x: f64, y: f64, pointer_id: i32) {
        if self
            .population_drag
            .is_some_and(|(drag_id, _, _)| drag_id == pointer_id)
        {
            self.population_drag = None;
            return;
        }
        if self.drag_pointer == Some(pointer_id) {
            if let Some((canvas_x, canvas_y)) = canvas_point(canvas, x, y) {
                let (world_x, world_y) = self.screen_to_world(canvas_x, canvas_y);
                self.update_station_draft(world_x, world_y);
            }
            if let Some(draft) = &mut self.station_draft {
                draft.dragging = false;
            }
            self.drag_pointer = None;
            self.draft_anchor = None;
            return;
        }

        let was_pinching = self
            .pinch
            .is_some_and(|pinch| pinch.first_id == pointer_id || pinch.second_id == pointer_id);
        self.remove_active_pointer(pointer_id);
        if was_pinching {
            self.pinch = None;
            self.panning_pointer = None;
            self.pan_start = None;
            self.pointer_start = None;
            return;
        }

        let Some((start_x, start_y)) = self.pointer_start.take() else {
            return;
        };
        let dx = x - start_x;
        let dy = y - start_y;
        if self.panning_pointer == Some(pointer_id) {
            self.panning_pointer = None;
            self.pan_start = None;
            if dx.abs().max(dy.abs()) < SWIPE_THRESHOLD {
                self.tap_canvas(canvas, x, y);
            }
            return;
        }

        if dx.abs().max(dy.abs()) < SWIPE_THRESHOLD {
            self.tap_canvas(canvas, x, y);
            return;
        }

        let direction = if dx.abs() > dy.abs() {
            if dx > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else if dy > 0.0 {
            Direction::Down
        } else {
            Direction::Up
        };
        self.set_direction(direction);
    }

    fn pointer_cancel(&mut self, pointer_id: i32) {
        if self
            .population_drag
            .is_some_and(|(drag_id, _, _)| drag_id == pointer_id)
        {
            self.population_drag = None;
        }
        if self.drag_pointer == Some(pointer_id) {
            if let Some(draft) = &mut self.station_draft {
                draft.dragging = false;
            }
            self.drag_pointer = None;
            self.draft_anchor = None;
        } else if self.panning_pointer == Some(pointer_id) {
            self.panning_pointer = None;
            self.pan_start = None;
        } else {
            self.pointer_start = None;
        }
        self.remove_active_pointer(pointer_id);
        if self
            .pinch
            .is_some_and(|pinch| pinch.first_id == pointer_id || pinch.second_id == pointer_id)
        {
            self.pinch = None;
        }
    }

    fn reset_after_clear(&mut self) {
        let seed = random_seed();
        let (cols, rows) = self
            .last_layout
            .map(|layout| (layout.cols, layout.rows))
            .unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
        self.game = SnekState::new(cols, rows, seed);
        self.rng = Lcg::new(seed ^ 0xa2f9_83d4_6e7b_5119);
        self.paused = false;
        self.last_step = 0.0;
        self.last_save = 0.0;
        self.dirty = false;
        self.save_enabled = true;
        self.pointer_start = None;
        self.buttons.clear();
        self.station_draft = None;
        self.selected_station = None;
        self.drag_pointer = None;
        self.draft_anchor = None;
        self.camera = Camera {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
        self.panning_pointer = None;
        self.pan_start = None;
        self.population_open = false;
        self.population_scroll = 0.0;
        self.population_drag = None;
        self.unlock_queue_open = true;
        self.toolbar_collapsed = true;
        self.last_rate_sample = 0.0;
        self.rate_totals = self.game.production_totals();
        self.rates = (0.0, 0.0, 0.0);
        self.active_pointers.clear();
        self.pinch = None;
    }

    fn status_text(&self) -> String {
        let mode = if self.game.auto_snek_unlocked() {
            if let Some(id) = self.game.manual_snek() {
                format!("manual control on Snek {id}")
            } else {
                "auto snek active".to_string()
            }
        } else {
            "manual control".to_string()
        };
        if self.paused {
            format!("Snek paused. {} apples. {mode}.", self.game.apls())
        } else {
            format!("Snek. {} apples. {mode}.", self.game.apls())
        }
    }

    fn save_now(&self) {
        if !self.save_enabled {
            return;
        }
        save_game(&self.game);
    }

    fn tap_canvas(&mut self, canvas: &HtmlCanvasElement, client_x: f64, client_y: f64) {
        let Some((x, y)) = canvas_point(canvas, client_x, client_y) else {
            return;
        };

        if let Some(kind) = self.button_at(x, y) {
            self.activate_button(kind);
            return;
        }

        let (world_x, world_y) = self.screen_to_world(x, y);
        let cell = Cell::new(
            (world_x / CELL_SIZE).floor().max(0.0) as usize,
            (world_y / CELL_SIZE).floor().max(0.0) as usize,
        );
        if self.game.tap_cell(cell) {
            self.selected_station = None;
            self.population_open = false;
            self.dirty = true;
            return;
        }
        if let Some(station_index) = self.game.station_index_at(cell) {
            self.selected_station = Some(station_index);
            self.population_open = false;
            self.dirty = true;
            return;
        }

        self.selected_station = None;
        self.dirty = true;
    }

    fn button_at(&self, x: f64, y: f64) -> Option<HudButtonKind> {
        self.buttons
            .iter()
            .rev()
            .find(|button| button.contains(x, y))
            .map(|button| button.kind)
    }

    fn activate_button(&mut self, kind: HudButtonKind) {
        match kind {
            HudButtonKind::UnlockJuicer => {
                if self.game.unlock_juicer() {
                    self.dirty = true;
                }
            }
            HudButtonKind::UnlockJuicerPlacement => {
                if self.game.buy_juicer_placement_unlock() {
                    self.dirty = true;
                }
            }
            HudButtonKind::AutoSnek => {
                if self.game.buy_auto_snek() {
                    self.dirty = true;
                }
            }
            HudButtonKind::StationOption(design) => {
                self.station_draft = Some(StationDraft {
                    origin: Cell::new(1, 1),
                    rotation: 0,
                    design,
                    dragging: false,
                    source: StationDraftSource::New,
                });
                self.selected_station = None;
            }
            HudButtonKind::PlaceStation => {
                if let Some(draft) = self.station_draft {
                    let placed = match draft.source {
                        StationDraftSource::New => self.game.buy_splitter_station(
                            draft.origin,
                            draft.rotation,
                            draft.design,
                            &mut self.rng,
                        ),
                        StationDraftSource::Move { index } => self.game.move_splitter_station(
                            index,
                            draft.origin,
                            draft.rotation,
                            &mut self.rng,
                        ),
                    };
                    if placed {
                        self.station_draft = None;
                        self.selected_station = None;
                        self.drag_pointer = None;
                        self.draft_anchor = None;
                        self.dirty = true;
                    }
                }
            }
            HudButtonKind::RotateStation => self.rotate_station_draft(),
            HudButtonKind::CancelStation => {
                self.selected_station = self.station_draft.and_then(|draft| match draft.source {
                    StationDraftSource::Move { index } => Some(index),
                    StationDraftSource::New => None,
                });
                self.station_draft = None;
                self.drag_pointer = None;
                self.draft_anchor = None;
            }
            HudButtonKind::MoveStation => {
                if let Some(index) = self.selected_station {
                    let Some(station) = self.game.splitter_stations().get(index).copied() else {
                        return;
                    };
                    self.station_draft = Some(StationDraft {
                        origin: station.origin(),
                        rotation: station.rotation(),
                        design: station.design(),
                        dragging: false,
                        source: StationDraftSource::Move { index },
                    });
                    self.selected_station = None;
                }
            }
            HudButtonKind::DeleteStation => {
                if let Some(index) = self.selected_station {
                    if self.game.delete_splitter_station(index) {
                        self.selected_station = None;
                        self.station_draft = None;
                        self.drag_pointer = None;
                        self.draft_anchor = None;
                        self.dirty = true;
                    }
                }
            }
            HudButtonKind::BuySplitterUnlock => {
                if self.game.buy_splitter_unlock() {
                    self.dirty = true;
                }
            }
            HudButtonKind::UnlockQueueToggle => {
                self.unlock_queue_open = !self.unlock_queue_open;
                if self.unlock_queue_open {
                    self.population_open = false;
                    self.toolbar_collapsed = true;
                }
            }
            HudButtonKind::ToolbarToggle => {
                self.toolbar_collapsed = !self.toolbar_collapsed;
            }
            HudButtonKind::PopulationToggle => {
                if !self.game.population_unlocked() {
                    return;
                }
                self.population_open = !self.population_open;
                if self.population_open {
                    self.unlock_queue_open = false;
                }
            }
            HudButtonKind::PopulationClose => {
                self.population_open = false;
                self.population_drag = None;
            }
            HudButtonKind::MaxLengthDown => {
                self.game.adjust_max_snek_len(-1);
                self.dirty = true;
            }
            HudButtonKind::MaxLengthUp => {
                self.game.adjust_max_snek_len(1);
                self.dirty = true;
            }
            HudButtonKind::TargetDown(length) => {
                self.game.adjust_length_target(length, -1);
                self.dirty = true;
            }
            HudButtonKind::TargetUp(length) => {
                self.game.adjust_length_target(length, 1);
                self.dirty = true;
            }
            HudButtonKind::DebugAdd10Apls => {
                #[cfg(debug_assertions)]
                if self.cheat {
                    self.game.debug_add_apls(10);
                    self.dirty = true;
                }
                #[cfg(not(debug_assertions))]
                {
                    let _ = self.cheat;
                }
            }
            HudButtonKind::DebugAdd10Juice => {
                #[cfg(debug_assertions)]
                if self.cheat {
                    self.game.debug_add_juice(10);
                    self.dirty = true;
                }
                #[cfg(not(debug_assertions))]
                {
                    let _ = self.cheat;
                }
            }
            HudButtonKind::BuyApple => {
                if self.game.buy_apple(&mut self.rng) {
                    self.dirty = true;
                }
            }
            HudButtonKind::PanelBlocker => {}
        }
    }

    fn start_station_draft(
        &mut self,
        canvas_x: f64,
        canvas_y: f64,
        design: StationDesign,
        pointer_id: i32,
    ) {
        let (world_x, world_y) = self.screen_to_world(canvas_x, canvas_y);
        self.station_draft = Some(StationDraft {
            origin: station_origin_from_point(world_x, world_y, design, 0),
            rotation: 0,
            design,
            dragging: true,
            source: StationDraftSource::New,
        });
        self.selected_station = None;
        self.drag_pointer = Some(pointer_id);
        self.draft_anchor = Some((world_x, world_y));
        self.pointer_start = None;
    }

    fn update_station_draft(&mut self, canvas_x: f64, canvas_y: f64) {
        if let Some(draft) = &mut self.station_draft {
            draft.origin =
                station_origin_from_point(canvas_x, canvas_y, draft.design, draft.rotation);
        }
    }

    fn rotate_station_draft(&mut self) {
        if let Some(draft) = &mut self.station_draft {
            draft.rotation = (draft.rotation + 1) % 4;
        }
        if let Some((canvas_x, canvas_y)) = self.draft_anchor {
            self.update_station_draft(canvas_x, canvas_y);
        }
    }

    fn screen_to_world(&self, x: f64, y: f64) -> (f64, f64) {
        let (offset_x, offset_y) = self
            .last_layout
            .map(|layout| {
                board_screen_offset(layout, self.camera, self.game.cols(), self.game.rows())
            })
            .unwrap_or((0.0, 0.0));
        (
            (x - offset_x + self.camera.pan_x) / self.camera.zoom,
            (y - offset_y + self.camera.pan_y) / self.camera.zoom,
        )
    }

    fn clamp_camera(&mut self, layout: SnekLayout) {
        self.camera.zoom = self.normalize_zoom(self.camera.zoom, layout);
        let world_width = self.game.cols() as f64 * CELL_SIZE * self.camera.zoom;
        let world_height = self.game.rows() as f64 * CELL_SIZE * self.camera.zoom;
        self.camera.pan_x = self
            .camera
            .pan_x
            .clamp(0.0, (world_width - layout.width).max(0.0));
        self.camera.pan_y = self
            .camera
            .pan_y
            .clamp(0.0, (world_height - layout.height).max(0.0));
    }

    fn normalize_zoom(&self, zoom: f64, layout: SnekLayout) -> f64 {
        let min_zoom = self.camera_min_zoom(layout);
        let mut zoom = zoom.clamp(min_zoom, CAMERA_MAX_ZOOM);
        if (zoom - CAMERA_NATIVE_ZOOM).abs() <= CAMERA_NATIVE_STICKY_RANGE
            && CAMERA_NATIVE_ZOOM >= min_zoom
        {
            zoom = CAMERA_NATIVE_ZOOM;
        }
        zoom
    }

    fn camera_min_zoom(&self, layout: SnekLayout) -> f64 {
        let world_width = (self.game.cols() as f64 * CELL_SIZE).max(1.0);
        let world_height = (self.game.rows() as f64 * CELL_SIZE).max(1.0);
        (layout.width / world_width)
            .min(layout.height / world_height)
            .clamp(CAMERA_ABSOLUTE_MIN_ZOOM, 1.0)
    }

    fn track_pointer(&mut self, id: i32, x: f64, y: f64) {
        self.remove_active_pointer(id);
        self.active_pointers.push(ActivePointer { id, x, y });
    }

    fn update_active_pointer(&mut self, id: i32, x: f64, y: f64) {
        if let Some(pointer) = self
            .active_pointers
            .iter_mut()
            .find(|pointer| pointer.id == id)
        {
            pointer.x = x;
            pointer.y = y;
        }
    }

    fn remove_active_pointer(&mut self, id: i32) {
        self.active_pointers.retain(|pointer| pointer.id != id);
    }

    fn start_pinch(&mut self) {
        let [first, second, ..] = self.active_pointers.as_slice() else {
            return;
        };
        let start_distance = pointer_distance(*first, *second);
        if start_distance <= 0.0 {
            return;
        }
        let focus_x = (first.x + second.x) / 2.0;
        let focus_y = (first.y + second.y) / 2.0;
        let (focus_world_x, focus_world_y) = self.screen_to_world(focus_x, focus_y);
        self.pinch = Some(PinchState {
            first_id: first.id,
            second_id: second.id,
            start_distance,
            start_zoom: self.camera.zoom,
            focus_world_x,
            focus_world_y,
        });
        self.panning_pointer = None;
        self.pan_start = None;
        self.pointer_start = None;
    }

    fn update_pinch(&mut self) {
        let Some(pinch) = self.pinch else {
            return;
        };
        let Some(first) = self
            .active_pointers
            .iter()
            .find(|pointer| pointer.id == pinch.first_id)
            .copied()
        else {
            return;
        };
        let Some(second) = self
            .active_pointers
            .iter()
            .find(|pointer| pointer.id == pinch.second_id)
            .copied()
        else {
            return;
        };
        let Some(layout) = self.last_layout else {
            return;
        };
        let distance = pointer_distance(first, second);
        if distance <= 0.0 {
            return;
        }
        let focus_x = (first.x + second.x) / 2.0;
        let focus_y = (first.y + second.y) / 2.0;
        self.camera.zoom =
            self.normalize_zoom(pinch.start_zoom * distance / pinch.start_distance, layout);
        let (offset_x, offset_y) =
            board_screen_offset(layout, self.camera, self.game.cols(), self.game.rows());
        self.camera.pan_x = pinch.focus_world_x * self.camera.zoom + offset_x - focus_x;
        self.camera.pan_y = pinch.focus_world_y * self.camera.zoom + offset_y - focus_y;
        self.clamp_camera(layout);
    }

    fn wheel(&mut self, canvas: &HtmlCanvasElement, client_x: f64, client_y: f64, delta_y: f64) {
        let Some(layout) = self.last_layout else {
            return;
        };
        let Some((x, y)) = canvas_point(canvas, client_x, client_y) else {
            return;
        };
        if self.population_open {
            let (panel_x, panel_y, panel_width, panel_height) =
                population_panel_rect(layout, &self.game);
            if x >= panel_x
                && x <= panel_x + panel_width
                && y >= panel_y
                && y <= panel_y + panel_height
            {
                self.population_scroll = (self.population_scroll + delta_y)
                    .clamp(0.0, population_max_scroll(layout, &self.game));
                return;
            }
        }
        let (world_x, world_y) = self.screen_to_world(x, y);
        let factor = (-delta_y / 420.0).exp().clamp(0.76, 1.32);
        self.camera.zoom = self.normalize_zoom(self.camera.zoom * factor, layout);
        let (offset_x, offset_y) =
            board_screen_offset(layout, self.camera, self.game.cols(), self.game.rows());
        self.camera.pan_x = world_x * self.camera.zoom + offset_x - x;
        self.camera.pan_y = world_y * self.camera.zoom + offset_y - y;
        self.clamp_camera(layout);
    }
}

#[cfg(target_arch = "wasm32")]
impl HudButton {
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[cfg(target_arch = "wasm32")]
fn pointer_distance(first: ActivePointer, second: ActivePointer) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct AnimationLoop {
    callback: Option<Closure<dyn FnMut(f64)>>,
    request_id: Option<i32>,
}

#[cfg(target_arch = "wasm32")]
impl AnimationLoop {
    fn is_running(&self) -> bool {
        self.request_id.is_some()
    }

    fn cancel(&mut self) {
        if let (Some(window), Some(request_id)) = (web_sys::window(), self.request_id.take()) {
            let _ = window.cancel_animation_frame(request_id);
        }
        self.callback = None;
    }
}

#[cfg(target_arch = "wasm32")]
fn request_animation_frame(animation: &Rc<RefCell<AnimationLoop>>) -> Option<i32> {
    let animation = animation.borrow();
    let window = web_sys::window()?;
    let callback = animation.callback.as_ref()?;
    window
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn resize_canvas(canvas: &HtmlCanvasElement) -> SnekLayout {
    let width = canvas.client_width().max(1) as f64;
    let height = canvas.client_height().max(1) as f64;
    let dpr = web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0)
        .clamp(1.0, 2.0);
    let pixel_width = (width * dpr).round() as u32;
    let pixel_height = (height * dpr).round() as u32;

    if canvas.width() != pixel_width {
        canvas.set_width(pixel_width);
    }
    if canvas.height() != pixel_height {
        canvas.set_height(pixel_height);
    }

    if let Some(context) = canvas_context(canvas) {
        let _ = context.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
    }

    SnekLayout {
        width,
        height,
        cols: (width / CELL_SIZE).floor().max(8.0) as usize,
        rows: (height / CELL_SIZE).floor().max(8.0) as usize,
        dpr,
    }
}

#[cfg(target_arch = "wasm32")]
fn canvas_context(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas
        .get_context("2d")
        .ok()
        .flatten()?
        .dyn_into::<CanvasRenderingContext2d>()
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn canvas_point(canvas: &HtmlCanvasElement, client_x: f64, client_y: f64) -> Option<(f64, f64)> {
    let rect = canvas.get_bounding_client_rect();
    let x = client_x - rect.left();
    let y = client_y - rect.top();
    if x.is_finite() && y.is_finite() {
        Some((x.max(0.0), y.max(0.0)))
    } else {
        None
    }
}

#[cfg(target_arch = "wasm32")]
fn reset_screen_transform(context: &CanvasRenderingContext2d, layout: SnekLayout) {
    let _ = context.set_transform(layout.dpr, 0.0, 0.0, layout.dpr, 0.0, 0.0);
}

#[cfg(target_arch = "wasm32")]
fn set_world_transform(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    camera: Camera,
    cols: usize,
    rows: usize,
) {
    let (offset_x, offset_y) = board_screen_offset(layout, camera, cols, rows);
    let _ = context.set_transform(
        layout.dpr * camera.zoom,
        0.0,
        0.0,
        layout.dpr * camera.zoom,
        (offset_x - camera.pan_x) * layout.dpr,
        (offset_y - camera.pan_y) * layout.dpr,
    );
}

#[cfg(target_arch = "wasm32")]
fn board_screen_offset(layout: SnekLayout, camera: Camera, cols: usize, rows: usize) -> (f64, f64) {
    let board_width = cols as f64 * CELL_SIZE * camera.zoom;
    let board_height = rows as f64 * CELL_SIZE * camera.zoom;
    (
        ((layout.width - board_width) / 2.0).max(0.0),
        ((layout.height - board_height) / 2.0).max(0.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn draw_board_background(context: &CanvasRenderingContext2d, cols: usize, rows: usize) {
    let width = cols as f64 * CELL_SIZE;
    let height = rows as f64 * CELL_SIZE;
    context.set_fill_style_str("#f4f4f4");
    context.fill_rect(0.0, 0.0, width, height);

    context.set_stroke_style_str("rgba(17, 17, 17, 0.05)");
    context.set_line_width(1.0);
    let mut x = 0.5;
    while x <= width {
        context.begin_path();
        context.move_to(x, 0.0);
        context.line_to(x, height);
        context.stroke();
        x += CELL_SIZE;
    }
    let mut y = 0.5;
    while y <= height {
        context.begin_path();
        context.move_to(0.0, y);
        context.line_to(width, y);
        context.stroke();
        y += CELL_SIZE;
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_apple(context: &CanvasRenderingContext2d, apple: Cell) {
    context.set_fill_style_str("#d81f26");
    context.fill_rect(
        apple.col as f64 * CELL_SIZE,
        apple.row as f64 * CELL_SIZE,
        CELL_SIZE,
        CELL_SIZE,
    );
    context.set_stroke_style_str("#9c1015");
    context.stroke_rect(
        apple.col as f64 * CELL_SIZE + 0.5,
        apple.row as f64 * CELL_SIZE + 0.5,
        CELL_SIZE - 1.0,
        CELL_SIZE - 1.0,
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_snek(context: &CanvasRenderingContext2d, snek: &Snek, wiggle_t: f64, selected: bool) {
    let positions = snek.positions();
    draw_snek_segments(context, positions, wiggle_t, positions.len());
    let offsets = wiggle_offsets(positions, wiggle_t);
    for (index, cell) in positions
        .iter()
        .enumerate()
        .rev()
        .take(snek.carried_juice_segments())
    {
        let (dx, dy) = offsets.get(index).copied().unwrap_or((0.0, 0.0));
        let x = cell.col as f64 * CELL_SIZE + dx + CELL_SIZE * 0.2;
        let y = cell.row as f64 * CELL_SIZE + dy + CELL_SIZE * 0.2;
        let size = CELL_SIZE * 0.6;
        context.set_fill_style_str("#f1c84b");
        context.fill_rect(x, y, size, size);
        context.set_stroke_style_str("#ffffff");
        context.set_line_width(1.0);
        context.begin_path();
        context.move_to(x + size / 2.0, y);
        context.line_to(x + size / 2.0, y + size);
        context.move_to(x, y + size / 2.0);
        context.line_to(x + size, y + size / 2.0);
        context.stroke();
    }

    if selected {
        let head = snek.head();
        context.set_stroke_style_str("#111111");
        context.set_line_width(2.0);
        context.stroke_rect(
            head.col as f64 * CELL_SIZE + 2.0,
            head.row as f64 * CELL_SIZE + 2.0,
            CELL_SIZE - 4.0,
            CELL_SIZE - 4.0,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_snek_segments(
    context: &CanvasRenderingContext2d,
    positions: &VecDeque<Cell>,
    wiggle_t: f64,
    gradient_len: usize,
) {
    let offsets = wiggle_offsets(positions, wiggle_t);
    let green_step = 255.0 / gradient_len.max(1) as f64;
    let mut green: f64 = 32.0;

    for (index, cell) in positions.iter().copied().enumerate() {
        context.set_fill_style_str(&format!("rgb(0, {}, 20)", green.floor() as u8));
        if green + green_step <= 255.0 {
            green += green_step;
        }
        let (dx, dy) = offsets.get(index).copied().unwrap_or((0.0, 0.0));
        context.fill_rect(
            cell.col as f64 * CELL_SIZE + dx,
            cell.row as f64 * CELL_SIZE + dy,
            CELL_SIZE,
            CELL_SIZE,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_juicer_feeds<'a>(
    context: &CanvasRenderingContext2d,
    feeds: impl Iterator<Item = (&'a VecDeque<Cell>, usize)>,
    wiggle_t: f64,
) {
    for (segments, gradient_len) in feeds {
        draw_snek_segments(context, segments, wiggle_t, gradient_len);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_split_effect(context: &CanvasRenderingContext2d, activity: SplitterActivity) {
    draw_machine_cut_effect(context, activity.station(), activity.progress());
}

#[cfg(target_arch = "wasm32")]
fn draw_machine_cut_effect(
    context: &CanvasRenderingContext2d,
    station: SplitterStation,
    progress: f64,
) {
    let cell = station.cut_target();
    let flow = station.flow_direction();
    let service_side = flow.counter_clockwise();
    let (flow_x, flow_y) = direction_vector(flow);
    let (side_x, side_y) = direction_vector(service_side);
    let x = cell.col as f64 * CELL_SIZE;
    let y = cell.row as f64 * CELL_SIZE;
    let center_x = x + CELL_SIZE / 2.0;
    let center_y = y + CELL_SIZE / 2.0;
    let previous_alpha = context.global_alpha();
    context.set_global_alpha(0.98);

    draw_effect_anvil(
        context,
        center_x + flow_x * CELL_SIZE * 0.08 - side_x * CELL_SIZE * 0.22,
        center_y + flow_y * CELL_SIZE * 0.08 - side_y * CELL_SIZE * 0.22,
        flow,
    );

    let approach = smooth_step((progress / 0.28).clamp(0.0, 1.0));
    let retract = smooth_step(((progress - 0.76) / 0.24).clamp(0.0, 1.0));
    let travel = approach - retract;
    let carriage_offset = CELL_SIZE * (1.2 - travel * 1.15);
    let carriage_x = center_x - flow_x * carriage_offset + side_x * CELL_SIZE * 0.72;
    let carriage_y = center_y - flow_y * carriage_offset + side_y * CELL_SIZE * 0.72;
    let tooth_drop = smooth_step(((progress - 0.2) / 0.22).clamp(0.0, 1.0))
        - smooth_step(((progress - 0.7) / 0.2).clamp(0.0, 1.0));
    draw_effect_blade(
        context,
        carriage_x,
        carriage_y,
        flow,
        service_side,
        tooth_drop,
    );

    if (0.4..=0.72).contains(&progress) {
        draw_cut_flash(context, center_x, center_y, flow, progress);
    }

    context.set_global_alpha(previous_alpha);
}

#[cfg(target_arch = "wasm32")]
fn draw_bootstrap_blade(context: &CanvasRenderingContext2d, center: Cell, progress: f64) {
    let center_x = (center.col as f64 + 0.5) * CELL_SIZE;
    let center_y = (center.row as f64 + 0.5) * CELL_SIZE;
    let drop = smooth_step((progress / 0.58).clamp(0.0, 1.0));
    let retract = smooth_step(((progress - 0.78) / 0.22).clamp(0.0, 1.0));
    let y = center_y - CELL_SIZE * (5.0 * (1.0 - drop) + retract * 2.0);
    let width = CELL_SIZE * 3.4;
    let height = CELL_SIZE * 0.72;
    context.set_fill_style_str("#bcc5ca");
    context.fill_rect(center_x - width / 2.0, y - height, width, height);
    context.set_stroke_style_str("#222222");
    context.set_line_width(2.0);
    context.stroke_rect(center_x - width / 2.0, y - height, width, height);
    context.set_fill_style_str("#737d84");
    for tooth in 0..7 {
        context.fill_rect(
            center_x - width / 2.0 + tooth as f64 * width / 7.0,
            y,
            width / 14.0,
            CELL_SIZE * 0.32,
        );
    }
    if (0.48..=0.72).contains(&progress) {
        draw_cut_flash(context, center_x, center_y, Direction::Right, progress);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_bootstrap_cost_walk(
    context: &CanvasRenderingContext2d,
    segments: &VecDeque<Cell>,
    direction: Direction,
    progress: f64,
    center: Cell,
    cols: usize,
) {
    let sign = if direction == Direction::Left {
        -1.0
    } else {
        1.0
    };
    let edge_distance = if direction == Direction::Left {
        center.col as f64
    } else {
        cols.saturating_sub(center.col) as f64
    };
    let head_x = (center.col as f64
        + 0.5
        + sign * smooth_step(progress) * (edge_distance + segments.len() as f64 + 2.0))
        * CELL_SIZE;
    let base_y = center.row as f64 * CELL_SIZE;
    let green_step = 223.0 / segments.len().max(1) as f64;
    for index in 0..segments.len() {
        let lag = (segments.len() - 1 - index) as f64 * CELL_SIZE;
        let wave =
            (progress * std::f64::consts::TAU * 4.0 - index as f64 * 0.78).sin() * CELL_SIZE * 0.16;
        context.set_fill_style_str(&format!(
            "rgb(0, {}, 20)",
            (32.0 + green_step * index as f64).min(255.0) as u8
        ));
        context.fill_rect(
            head_x - sign * lag - CELL_SIZE / 2.0,
            base_y + wave,
            CELL_SIZE,
            CELL_SIZE,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_bootstrap_flash(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    camera: Camera,
    cols: usize,
    rows: usize,
    origin: Cell,
    progress: f64,
) {
    let (offset_x, offset_y) = board_screen_offset(layout, camera, cols, rows);
    let center_x = offset_x - camera.pan_x + (origin.col as f64 + 0.5) * CELL_SIZE * camera.zoom;
    let center_y = offset_y - camera.pan_y + (origin.row as f64 + 0.5) * CELL_SIZE * camera.zoom;
    let radius = smooth_step(progress) * layout.width.hypot(layout.height) * 1.15;
    let alpha = if progress < 0.6 {
        (progress / 0.6).clamp(0.0, 1.0)
    } else {
        ((1.0 - progress) / 0.4).clamp(0.0, 1.0)
    };
    context.save();
    reset_screen_transform(context, layout);
    context.set_global_alpha(alpha * 0.96);
    context.set_fill_style_str("#fff8d6");
    context.begin_path();
    context
        .arc(center_x, center_y, radius, 0.0, std::f64::consts::TAU)
        .ok();
    context.fill();
    context.restore();
}

#[cfg(target_arch = "wasm32")]
fn smooth_step(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

#[cfg(target_arch = "wasm32")]
fn draw_effect_anvil(
    context: &CanvasRenderingContext2d,
    center_x: f64,
    center_y: f64,
    flow: Direction,
) {
    let (width, height) = match flow {
        Direction::Left | Direction::Right => (CELL_SIZE * 0.34, CELL_SIZE * 0.48),
        Direction::Up | Direction::Down => (CELL_SIZE * 0.48, CELL_SIZE * 0.34),
    };
    let block_x = center_x - width / 2.0;
    let block_y = center_y - height / 2.0;
    context.set_fill_style_str("#434950");
    context.fill_rect(block_x, block_y, width, height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(1.5);
    context.stroke_rect(block_x + 0.5, block_y + 0.5, width - 1.0, height - 1.0);
}

#[cfg(target_arch = "wasm32")]
fn draw_effect_blade(
    context: &CanvasRenderingContext2d,
    center_x: f64,
    center_y: f64,
    flow: Direction,
    service_side: Direction,
    tooth_drop: f64,
) {
    let (carriage_width, carriage_height) = match flow {
        Direction::Left | Direction::Right => (CELL_SIZE * 0.84, CELL_SIZE * 0.28),
        Direction::Up | Direction::Down => (CELL_SIZE * 0.28, CELL_SIZE * 0.84),
    };
    let x = center_x - carriage_width / 2.0;
    let y = center_y - carriage_height / 2.0;
    context.set_fill_style_str("#b97924");
    context.fill_rect(x, y, carriage_width, carriage_height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(1.5);
    context.stroke_rect(
        x + 0.5,
        y + 0.5,
        carriage_width - 1.0,
        carriage_height - 1.0,
    );

    let (side_x, side_y) = direction_vector(service_side.reversed());
    let tooth_length = CELL_SIZE * (0.22 + tooth_drop * 0.72);
    let tooth_width = CELL_SIZE * 0.24;
    let carriage_half = match service_side {
        Direction::Up | Direction::Down => carriage_height / 2.0,
        Direction::Left | Direction::Right => carriage_width / 2.0,
    };
    let tooth_center_x = center_x + side_x * (carriage_half + tooth_length / 2.0);
    let tooth_center_y = center_y + side_y * (carriage_half + tooth_length / 2.0);
    let (tooth_w, tooth_h) = match service_side {
        Direction::Up | Direction::Down => (tooth_width, tooth_length),
        Direction::Left | Direction::Right => (tooth_length, tooth_width),
    };
    context.set_fill_style_str("#d8dde0");
    context.fill_rect(
        tooth_center_x - tooth_w / 2.0,
        tooth_center_y - tooth_h / 2.0,
        tooth_w,
        tooth_h,
    );
    context.set_stroke_style_str("#7d2724");
    context.stroke_rect(
        tooth_center_x - tooth_w / 2.0 + 0.5,
        tooth_center_y - tooth_h / 2.0 + 0.5,
        tooth_w - 1.0,
        tooth_h - 1.0,
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_cut_flash(
    context: &CanvasRenderingContext2d,
    center_x: f64,
    center_y: f64,
    flow: Direction,
    progress: f64,
) {
    let pulse = 1.0 - ((progress - 0.56).abs() / 0.16).clamp(0.0, 1.0);
    let (flow_x, flow_y) = direction_vector(flow);
    context.set_stroke_style_str("#fff4b0");
    context.set_line_width(1.0 + pulse * 2.0);
    context.begin_path();
    context.move_to(
        center_x - flow_y * CELL_SIZE * 0.34,
        center_y + flow_x * CELL_SIZE * 0.34,
    );
    context.line_to(
        center_x + flow_y * CELL_SIZE * 0.34,
        center_y - flow_x * CELL_SIZE * 0.34,
    );
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_hud(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    hud: HudSnapshot<'_>,
) -> Vec<HudButton> {
    let HudSnapshot {
        game,
        draft,
        selected_station,
        camera,
        population_open,
        population_scroll,
        unlock_queue_open,
        toolbar_collapsed,
        rates,
        cheat,
    } = hud;
    let mut buttons = Vec::new();
    draw_score(context, layout, game, rates, &mut buttons);
    draw_unlock_queue(
        context,
        game,
        unlock_queue_open,
        cheat,
        layout.width,
        &mut buttons,
    );

    if let Some(draft) = draft {
        draw_station_draft_controls(
            context,
            layout,
            draft,
            camera,
            game.cols(),
            game.rows(),
            &mut buttons,
        );
    } else {
        if let Some(index) = selected_station {
            if let Some(station) = game.splitter_stations().get(index).copied() {
                draw_station_card(
                    context,
                    layout,
                    station,
                    camera,
                    game.cols(),
                    game.rows(),
                    &mut buttons,
                );
            }
        }
        if population_open {
            draw_population_menu(context, layout, game, population_scroll, &mut buttons);
        }
        draw_bottom_toolbar(context, layout, game, toolbar_collapsed, &mut buttons);
    }

    buttons
}

#[cfg(target_arch = "wasm32")]
fn draw_score(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    rates: (f64, f64, f64),
    buttons: &mut Vec<HudButton>,
) {
    let apls = game.apls().to_string();
    context.set_font("800 24px ui-monospace, SFMono-Regular, Consolas, monospace");
    context.set_line_width(4.0);
    context.set_stroke_style_str("#000000");
    context.set_fill_style_str("#ffffff");
    let width = measure_text(context, &apls, 15.0);
    let x = (layout.width - width - HUD_MARGIN).max(HUD_MARGIN);
    let _ = context.stroke_text(&apls, x, 32.0);
    let _ = context.fill_text(&apls, x, 32.0);

    if game.juice_discovered() {
        let juice = game.juice().to_string();
        let juice_width = measure_text(context, &juice, 15.0);
        let juice_x = (x - juice_width - 24.0).max(HUD_MARGIN);
        let _ = context.stroke_text(&juice, juice_x, 32.0);
        let _ = context.fill_text(&juice, juice_x, 32.0);
    }

    let snek_count = game.sneks().len();
    if game.population_unlocked() {
        let count = format!("{snek_count} snek");
        context.set_font("800 16px ui-monospace, SFMono-Regular, Consolas, monospace");
        let count_width = measure_text(context, &count, 10.0);
        let count_x = (layout.width - count_width - HUD_MARGIN).max(HUD_MARGIN);
        let _ = context.stroke_text(&count, count_x, 53.0);
        let _ = context.fill_text(&count, count_x, 53.0);
        buttons.push(HudButton {
            kind: HudButtonKind::PopulationToggle,
            x: count_x - 8.0,
            y: 29.0,
            width: count_width + 16.0,
            height: 44.0,
        });
    }
    if game.juicer_unlocked() {
        let mut rate_text = format!("{:.1} apls/sec", rates.0);
        if game.population_unlocked() {
            rate_text.push_str(&format!("  {:.1} snek/sec", rates.1));
        }
        if game.juice_discovered() {
            rate_text.push_str(&format!("  {:.1} juice/sec", rates.2 / 10.0));
        }
        context.set_font("800 11px ui-monospace, SFMono-Regular, Consolas, monospace");
        let rate_width = measure_text(context, &rate_text, 7.0);
        let rate_x = (layout.width - rate_width - HUD_MARGIN).max(HUD_MARGIN);
        let rate_y = if game.population_unlocked() {
            72.0
        } else {
            52.0
        };
        context.set_line_width(3.0);
        let _ = context.stroke_text(&rate_text, rate_x, rate_y);
        let _ = context.fill_text(&rate_text, rate_x, rate_y);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_unlock_queue(
    context: &CanvasRenderingContext2d,
    game: &SnekState,
    open: bool,
    cheat: bool,
    viewport_width: f64,
    buttons: &mut Vec<HudButton>,
) {
    if game.bootstrap_activity().is_some() || (!game.unlock_queue_available() && !cheat) {
        return;
    }

    let x = HUD_MARGIN;
    let y = HUD_MARGIN;
    let width = 290.0_f64.min((viewport_width - HUD_MARGIN * 2.0).max(40.0));
    let header_height = 30.0;
    let apple_visible = game.apple_unlock_visible();
    let auto_visible = !game.auto_snek_unlocked() && game.auto_snek_offer_visible();
    let splitter_visible = game.splitter_unlock_visible();
    let juicer_visible = game.juicer_upgrade_visible();
    context.set_line_width(3.0);
    context.set_stroke_style_str("#000000");
    context.set_fill_style_str("#ffffff");
    context.set_font("800 15px ui-monospace, SFMono-Regular, Consolas, monospace");
    let symbol = if open { "-" } else { "+" };
    let _ = context.stroke_text(symbol, x + 12.0, y + 20.0);
    let _ = context.fill_text(symbol, x + 12.0, y + 20.0);
    buttons.push(HudButton {
        kind: HudButtonKind::UnlockQueueToggle,
        x: x - 7.0,
        y: y - 7.0,
        width: 44.0,
        height: 44.0,
    });

    if !open {
        return;
    }

    let mut row = 0usize;
    if apple_visible {
        let apple = draw_button(
            context,
            HudButtonKind::BuyApple,
            x + 6.0,
            y + header_height + 4.0,
            width - 12.0,
            HUD_BUTTON_HEIGHT,
            &format!("Apple {APPLE_COST} jce"),
        );
        if !game.can_buy_apple() {
            draw_disabled_overlay(context, apple);
        }
        buttons.push(apple);
        row += 1;
    }
    if !game.juicer_unlocked() && game.can_unlock_juicer() {
        let button = draw_button(
            context,
            HudButtonKind::UnlockJuicer,
            x + 6.0,
            y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
            width - 12.0,
            HUD_BUTTON_HEIGHT,
            "When life gives you -10 apls...",
        );
        if !game.can_unlock_juicer() {
            draw_disabled_overlay(context, button);
        }
        buttons.push(button);
        row += 1;
    } else {
        if auto_visible {
            let button = draw_button(
                context,
                HudButtonKind::AutoSnek,
                x + 6.0,
                y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
                width - 12.0,
                HUD_BUTTON_HEIGHT,
                &format!("Auto Snek {AUTO_SNEK_COST} jce"),
            );
            if !game.can_buy_auto_snek() {
                draw_disabled_overlay(context, button);
            }
            buttons.push(button);
            row += 1;
        }
        if juicer_visible && !game.juicer_placement_unlocked() {
            let button = draw_button(
                context,
                HudButtonKind::UnlockJuicerPlacement,
                x + 6.0,
                y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
                width - 12.0,
                HUD_BUTTON_HEIGHT,
                &format!("Juicer {JUICER_PLACEMENT_UNLOCK_COST} jce"),
            );
            if !game.can_unlock_juicer_placement() {
                draw_disabled_overlay(context, button);
            }
            buttons.push(button);
            row += 1;
        }
        if splitter_visible && !game.splitter_unlocked() {
            let button = draw_button(
                context,
                HudButtonKind::BuySplitterUnlock,
                x + 6.0,
                y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
                width - 12.0,
                HUD_BUTTON_HEIGHT,
                &format!("Splitter {SPLITTER_STATION_UNLOCK_COST} jce"),
            );
            if !game.can_unlock_splitter_station() {
                draw_disabled_overlay(context, button);
            }
            buttons.push(button);
            row += 1;
        }
    }

    #[cfg(debug_assertions)]
    if cheat {
        let apls = draw_button(
            context,
            HudButtonKind::DebugAdd10Apls,
            x + 6.0,
            y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
            width - 12.0,
            HUD_BUTTON_HEIGHT,
            "+10 apls",
        );
        buttons.push(apls);
        row += 1;
        if game.juice_discovered() {
            let juice = draw_button(
                context,
                HudButtonKind::DebugAdd10Juice,
                x + 6.0,
                y + header_height + 4.0 + row as f64 * (HUD_BUTTON_HEIGHT + 4.0),
                width - 12.0,
                HUD_BUTTON_HEIGHT,
                "+10 juice",
            );
            buttons.push(juice);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn measure_text(context: &CanvasRenderingContext2d, text: &str, fallback_char_width: f64) -> f64 {
    context
        .measure_text(text)
        .map(|metrics| metrics.width())
        .unwrap_or_else(|_| text.len() as f64 * fallback_char_width)
}

#[cfg(target_arch = "wasm32")]
fn draw_bottom_toolbar(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    collapsed: bool,
    buttons: &mut Vec<HudButton>,
) {
    if !game.toolbelt_available() {
        return;
    }
    draw_station_toolbelt(context, layout, game, collapsed, buttons);
}

#[cfg(target_arch = "wasm32")]
fn draw_station_toolbelt(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    collapsed: bool,
    buttons: &mut Vec<HudButton>,
) {
    let toggle_size = 44.0;
    let toggle_x = layout.width - HUD_MARGIN - toggle_size;
    let toggle_y = layout.height - HUD_MARGIN - toggle_size;
    draw_toolbar_chevron(context, toggle_x, toggle_y, collapsed);
    if collapsed {
        buttons.push(HudButton {
            kind: HudButtonKind::ToolbarToggle,
            x: toggle_x,
            y: toggle_y,
            width: toggle_size,
            height: toggle_size,
        });
        return;
    }

    let available_width = (toggle_x - HUD_MARGIN - TOOLBELT_GAP).max(1.0);
    let padding = 5.0;
    let gaps_width = TOOLBELT_GAP * (TOOLBELT_SLOTS - 1) as f64;
    let slot_size = ((available_width - padding * 2.0 - gaps_width) / TOOLBELT_SLOTS as f64)
        .clamp(4.0, TOOLBELT_SLOT_SIZE);
    let width = padding * 2.0 + slot_size * TOOLBELT_SLOTS as f64 + gaps_width;
    let height = slot_size + padding * 2.0;
    let x = ((toggle_x - width) / 2.0).max(HUD_MARGIN);
    let y = (layout.height - height - HUD_MARGIN).max(HUD_MARGIN);
    context.set_fill_style_str("rgba(255, 255, 255, 0.96)");
    context.fill_rect(x, y, width, height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0);
    buttons.push(HudButton {
        kind: HudButtonKind::PanelBlocker,
        x,
        y,
        width,
        height,
    });

    let mut unlocked = Vec::new();
    if game.juicer_placement_unlocked() {
        unlocked.push(StationDesign::Juicer);
    }
    if game.splitter_unlocked() {
        unlocked.push(StationDesign::Splitter);
    }
    for index in 0..TOOLBELT_SLOTS {
        let button = HudButton {
            kind: HudButtonKind::PanelBlocker,
            x: x + padding + index as f64 * (slot_size + TOOLBELT_GAP),
            y: y + padding,
            width: slot_size,
            height: slot_size,
        };
        if let Some(design) = unlocked.get(index).copied() {
            let option = HudButton {
                kind: HudButtonKind::StationOption(design),
                ..button
            };
            draw_station_option(context, option, design);
            buttons.push(HudButton {
                y: option.y - (44.0 - slot_size) / 2.0,
                height: 44.0,
                ..option
            });
        } else {
            context.set_stroke_style_str("rgba(17, 17, 17, 0.35)");
            context.set_line_width(1.0);
            context.stroke_rect(
                button.x + 0.5,
                button.y + 0.5,
                slot_size - 1.0,
                slot_size - 1.0,
            );
        }
    }
    buttons.push(HudButton {
        kind: HudButtonKind::ToolbarToggle,
        x: toggle_x,
        y: toggle_y,
        width: toggle_size,
        height: toggle_size,
    });
}

#[cfg(target_arch = "wasm32")]
fn draw_toolbar_chevron(context: &CanvasRenderingContext2d, x: f64, y: f64, collapsed: bool) {
    let (tip_y, edge_y) = if collapsed {
        (y + 14.0, y + 28.0)
    } else {
        (y + 30.0, y + 16.0)
    };
    for (color, width) in [("#000000", 6.0), ("#ffffff", 3.0)] {
        context.set_stroke_style_str(color);
        context.set_line_width(width);
        context.begin_path();
        context.move_to(x + 10.0, edge_y);
        context.line_to(x + 22.0, tip_y);
        context.line_to(x + 34.0, edge_y);
        context.stroke();
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_population_menu(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    scroll: f64,
    buttons: &mut Vec<HudButton>,
) {
    let (x, y, width, height) = population_panel_rect(layout, game);
    let groups = snek_length_groups(game);
    let row_height = 38.0;
    let max_snek_len = game.max_snek_len().unwrap_or(MIN_SNEK_LEN + 1);
    context.set_fill_style_str("rgba(255, 255, 255, 0.96)");
    context.fill_rect(x, y, width, height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0);
    buttons.push(HudButton {
        kind: HudButtonKind::PanelBlocker,
        x,
        y,
        width,
        height,
    });

    context.set_fill_style_str("#111111");
    context.set_font("800 13px ui-monospace, SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text("Population Management", x + 10.0, y + 20.0);
    let close = draw_button(
        context,
        HudButtonKind::PopulationClose,
        x + width - 38.0,
        y + 5.0,
        30.0,
        26.0,
        "x",
    );
    buttons.push(HudButton {
        x: close.x - 7.0,
        y: close.y - 9.0,
        width: 44.0,
        height: 44.0,
        ..close
    });
    let max_y = y + 47.0;
    context.set_font("700 12px ui-monospace, SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text(
        &game.max_snek_len().map_or_else(
            || "max unset".to_string(),
            |max| format!("max length {max}"),
        ),
        x + 52.0,
        max_y,
    );
    let down = draw_button(
        context,
        HudButtonKind::MaxLengthDown,
        x + 8.0,
        y + 27.0,
        36.0,
        28.0,
        "v",
    );
    let up = draw_button(
        context,
        HudButtonKind::MaxLengthUp,
        x + width - 44.0,
        y + 27.0,
        36.0,
        28.0,
        "^",
    );
    buttons.extend([down, up]);

    let content_top = y + 62.0;
    let content_bottom = y + height - 8.0;
    let scroll = scroll.clamp(0.0, population_max_scroll(layout, game));
    for (row, length) in (MIN_SNEK_LEN..max_snek_len).rev().enumerate() {
        let row_y = content_top + row as f64 * row_height - scroll;
        if row_y < content_top || row_y + 34.0 > content_bottom {
            continue;
        }
        let current = groups
            .iter()
            .find_map(|(group_len, count)| (*group_len == length).then_some(*count))
            .unwrap_or(0);
        let target = game
            .length_targets()
            .iter()
            .find_map(|item| (item.length() == length).then_some(item.count()))
            .unwrap_or(0);
        let _ = context.fill_text(
            &format!("len {length:<3} {current} now  {target} target"),
            x + 48.0,
            row_y + 19.0,
        );
        let down = draw_button(
            context,
            HudButtonKind::TargetDown(length),
            x + 8.0,
            row_y,
            36.0,
            34.0,
            "-",
        );
        let up = draw_button(
            context,
            HudButtonKind::TargetUp(length),
            x + width - 44.0,
            row_y,
            36.0,
            34.0,
            "+",
        );
        buttons.extend([down, up]);
    }
}

#[cfg(target_arch = "wasm32")]
fn population_panel_rect(layout: SnekLayout, game: &SnekState) -> (f64, f64, f64, f64) {
    let width = (layout.width - HUD_MARGIN * 2.0).min(360.0);
    let x = ((layout.width - width) / 2.0).max(HUD_MARGIN);
    let row_height = 38.0;
    let max_snek_len = game.max_snek_len().unwrap_or(MIN_SNEK_LEN + 1);
    let target_rows = max_snek_len.saturating_sub(MIN_SNEK_LEN);
    let height = (72.0 + row_height * target_rows as f64)
        .min(layout.height - HUD_MARGIN * 2.0 - STATION_OPTION_HEIGHT - 20.0);
    let toolbelt_clearance = if game.juicer_unlocked() {
        STATION_OPTION_HEIGHT + 20.0
    } else {
        ICON_BUTTON_SIZE + 8.0
    };
    let y = (layout.height - height - HUD_MARGIN - toolbelt_clearance).max(HUD_MARGIN);
    (x, y, width, height)
}

#[cfg(target_arch = "wasm32")]
fn population_max_scroll(layout: SnekLayout, game: &SnekState) -> f64 {
    let (_, _, _, height) = population_panel_rect(layout, game);
    let rows = game
        .max_snek_len()
        .unwrap_or(MIN_SNEK_LEN + 1)
        .saturating_sub(MIN_SNEK_LEN);
    (rows as f64 * 38.0 - (height - 70.0)).max(0.0)
}

#[cfg(target_arch = "wasm32")]
fn snek_length_groups(game: &SnekState) -> Vec<(usize, usize)> {
    let mut groups = Vec::<(usize, usize)>::new();
    for snek in game.sneks() {
        if let Some((_, count)) = groups.iter_mut().find(|(len, _)| *len == snek.len()) {
            *count += 1;
        } else {
            groups.push((snek.len(), 1));
        }
    }
    groups.sort_by(|(a, _), (b, _)| b.cmp(a));
    groups
}

#[cfg(target_arch = "wasm32")]
fn draw_disabled_overlay(context: &CanvasRenderingContext2d, button: HudButton) {
    context.set_fill_style_str("rgba(244, 244, 244, 0.58)");
    context.fill_rect(button.x, button.y, button.width, button.height);
    context.set_stroke_style_str("rgba(17, 17, 17, 0.35)");
    context.set_line_width(2.0);
    context.stroke_rect(
        button.x + 1.0,
        button.y + 1.0,
        button.width - 2.0,
        button.height - 2.0,
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_station_draft_controls(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    draft: &StationDraft,
    camera: Camera,
    cols: usize,
    rows: usize,
    buttons: &mut Vec<HudButton>,
) {
    let station_width = station_cols(draft.design, draft.rotation) as f64 * CELL_SIZE * camera.zoom;
    let station_height =
        station_rows(draft.design, draft.rotation) as f64 * CELL_SIZE * camera.zoom;
    let button_gap = 6.0;
    let button_total_width = ICON_BUTTON_SIZE * 3.0 + button_gap * 2.0;
    let (offset_x, offset_y) = board_screen_offset(layout, camera, cols, rows);
    let screen_x = draft.origin.col as f64 * CELL_SIZE * camera.zoom + offset_x - camera.pan_x;
    let screen_y = draft.origin.row as f64 * CELL_SIZE * camera.zoom + offset_y - camera.pan_y;
    let x = screen_x
        .min((layout.width - button_total_width - HUD_MARGIN).max(HUD_MARGIN))
        .max(HUD_MARGIN);
    let mut y = screen_y + station_height + 8.0;
    if y + ICON_BUTTON_SIZE > layout.height - HUD_MARGIN {
        y = (screen_y - ICON_BUTTON_SIZE - 8.0).max(HUD_MARGIN);
    }

    draw_station_tooltip(
        context,
        layout,
        x,
        y - 34.0,
        station_label(draft.design),
        station_width,
    );

    let place = draw_icon_button(context, HudButtonKind::PlaceStation, x, y, "#d9f5de");
    let rotate = draw_icon_button(
        context,
        HudButtonKind::RotateStation,
        x + ICON_BUTTON_SIZE + button_gap,
        y,
        "#fff2bd",
    );
    let cancel = draw_icon_button(
        context,
        HudButtonKind::CancelStation,
        x + (ICON_BUTTON_SIZE + button_gap) * 2.0,
        y,
        "#ffd8d8",
    );
    buttons.extend([place, rotate, cancel]);
}

#[cfg(target_arch = "wasm32")]
fn draw_station_card(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    station: SplitterStation,
    camera: Camera,
    cols: usize,
    rows: usize,
    buttons: &mut Vec<HudButton>,
) {
    let (offset_x, offset_y) = board_screen_offset(layout, camera, cols, rows);
    let station_width = station.width() as f64 * CELL_SIZE * camera.zoom;
    let station_height = station.height() as f64 * CELL_SIZE * camera.zoom;
    let screen_x = station.origin().col as f64 * CELL_SIZE * camera.zoom + offset_x - camera.pan_x;
    let screen_y = station.origin().row as f64 * CELL_SIZE * camera.zoom + offset_y - camera.pan_y;
    draw_station_tooltip(
        context,
        layout,
        screen_x,
        screen_y - 30.0,
        station_label(station.design()),
        station_width,
    );

    let button_gap = 6.0;
    let button_total_width = ICON_BUTTON_SIZE * 2.0 + button_gap;
    let x = screen_x
        .min((layout.width - button_total_width - HUD_MARGIN).max(HUD_MARGIN))
        .max(HUD_MARGIN);
    let mut y = screen_y + station_height + 8.0;
    if y + ICON_BUTTON_SIZE > layout.height - HUD_MARGIN {
        y = (screen_y - ICON_BUTTON_SIZE - 8.0).max(HUD_MARGIN);
    }

    let move_button = draw_icon_button(context, HudButtonKind::MoveStation, x, y, "#dbe9ff");
    let delete = draw_icon_button(
        context,
        HudButtonKind::DeleteStation,
        x + ICON_BUTTON_SIZE + button_gap,
        y,
        "#ffd8d8",
    );
    buttons.extend([move_button, delete]);
}

#[cfg(target_arch = "wasm32")]
fn draw_button(
    context: &CanvasRenderingContext2d,
    kind: HudButtonKind,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    label: &str,
) -> HudButton {
    context.set_fill_style_str("rgba(244, 244, 244, 0.94)");
    context.fill_rect(x, y, width, height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0);
    context.set_fill_style_str("#111111");
    context.set_font("700 13px ui-monospace, SFMono-Regular, Consolas, monospace");
    let measured = context
        .measure_text(label)
        .ok()
        .map(|metrics| metrics.width())
        .unwrap_or(0.0);
    let fitted_size = if measured > width - 20.0 {
        (13.0 * (width - 20.0).max(1.0) / measured).max(1.0)
    } else {
        13.0
    };
    context.set_font(&format!(
        "700 {fitted_size}px ui-monospace, SFMono-Regular, Consolas, monospace"
    ));
    let _ = context.fill_text(label, x + 10.0, y + height / 2.0 + 5.0);

    HudButton {
        kind,
        x,
        y,
        width,
        height,
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_icon_button(
    context: &CanvasRenderingContext2d,
    kind: HudButtonKind,
    x: f64,
    y: f64,
    fill: &str,
) -> HudButton {
    context.set_fill_style_str(fill);
    context.fill_rect(x, y, ICON_BUTTON_SIZE, ICON_BUTTON_SIZE);
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(
        x + 1.0,
        y + 1.0,
        ICON_BUTTON_SIZE - 2.0,
        ICON_BUTTON_SIZE - 2.0,
    );

    context.set_stroke_style_str("#111111");
    context.set_line_width(3.0);
    match kind {
        HudButtonKind::PlaceStation => draw_check_icon(context, x, y),
        HudButtonKind::RotateStation => draw_rotate_icon(context, x, y),
        HudButtonKind::CancelStation => draw_x_icon(context, x, y),
        HudButtonKind::MoveStation => draw_move_icon(context, x, y),
        HudButtonKind::DeleteStation => draw_delete_icon(context, x, y),
        HudButtonKind::PopulationToggle => draw_population_icon(context, x, y),
        _ => {}
    }

    HudButton {
        kind,
        x: x - 5.0,
        y: y - 5.0,
        width: ICON_BUTTON_SIZE + 10.0,
        height: ICON_BUTTON_SIZE + 10.0,
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_population_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.set_fill_style_str("#63ad5b");
    for (dx, dy) in [(9.0, 18.0), (16.0, 11.0), (23.0, 18.0)] {
        context.fill_rect(x + dx - 3.0, y + dy - 3.0, 7.0, 7.0);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_check_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 8.0, y + 18.0);
    context.line_to(x + 14.0, y + 24.0);
    context.line_to(x + 26.0, y + 10.0);
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_rotate_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 24.0, y + 10.0);
    context.line_to(x + 15.0, y + 10.0);
    context.line_to(x + 10.0, y + 15.0);
    context.line_to(x + 10.0, y + 22.0);
    context.line_to(x + 15.0, y + 27.0);
    context.line_to(x + 23.0, y + 27.0);
    context.stroke();
    context.begin_path();
    context.move_to(x + 21.0, y + 6.0);
    context.line_to(x + 27.0, y + 10.0);
    context.line_to(x + 21.0, y + 14.0);
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_x_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 10.0, y + 10.0);
    context.line_to(x + 24.0, y + 24.0);
    context.move_to(x + 24.0, y + 10.0);
    context.line_to(x + 10.0, y + 24.0);
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_move_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 17.0, y + 7.0);
    context.line_to(x + 17.0, y + 27.0);
    context.move_to(x + 7.0, y + 17.0);
    context.line_to(x + 27.0, y + 17.0);
    context.move_to(x + 17.0, y + 7.0);
    context.line_to(x + 13.0, y + 12.0);
    context.move_to(x + 17.0, y + 7.0);
    context.line_to(x + 21.0, y + 12.0);
    context.move_to(x + 17.0, y + 27.0);
    context.line_to(x + 13.0, y + 22.0);
    context.move_to(x + 17.0, y + 27.0);
    context.line_to(x + 21.0, y + 22.0);
    context.move_to(x + 7.0, y + 17.0);
    context.line_to(x + 12.0, y + 13.0);
    context.move_to(x + 7.0, y + 17.0);
    context.line_to(x + 12.0, y + 21.0);
    context.move_to(x + 27.0, y + 17.0);
    context.line_to(x + 22.0, y + 13.0);
    context.move_to(x + 27.0, y + 17.0);
    context.line_to(x + 22.0, y + 21.0);
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_delete_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 10.0, y + 12.0);
    context.line_to(x + 24.0, y + 12.0);
    context.move_to(x + 14.0, y + 12.0);
    context.line_to(x + 14.0, y + 9.0);
    context.line_to(x + 20.0, y + 9.0);
    context.line_to(x + 20.0, y + 12.0);
    context.move_to(x + 12.0, y + 15.0);
    context.line_to(x + 14.0, y + 27.0);
    context.line_to(x + 21.0, y + 27.0);
    context.line_to(x + 23.0, y + 15.0);
    context.move_to(x + 16.0, y + 17.0);
    context.line_to(x + 16.0, y + 24.0);
    context.move_to(x + 19.0, y + 17.0);
    context.line_to(x + 19.0, y + 24.0);
    context.stroke();
}

#[cfg(target_arch = "wasm32")]
fn draw_station_option(
    context: &CanvasRenderingContext2d,
    button: HudButton,
    design: StationDesign,
) {
    context.set_fill_style_str("rgba(255, 247, 184, 0.96)");
    context.fill_rect(button.x, button.y, button.width, button.height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(
        button.x + 1.0,
        button.y + 1.0,
        button.width - 2.0,
        button.height - 2.0,
    );

    let cols = station_cols(design, 0);
    let rows = station_rows(design, 0);
    let icon_cell = ((button.width - 12.0) / cols as f64).min((button.height - 12.0) / rows as f64);
    draw_station_pixels(
        context,
        button.x + (button.width - cols as f64 * icon_cell) / 2.0,
        button.y + (button.height - rows as f64 * icon_cell) / 2.0,
        design,
        0,
        icon_cell,
        1.0,
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_station(
    context: &CanvasRenderingContext2d,
    station: SplitterStation,
    cell_size: f64,
    alpha: f64,
    animation_t: f64,
    juicer_activity: Option<f64>,
) {
    draw_station_pixels(
        context,
        station.origin().col as f64 * CELL_SIZE,
        station.origin().row as f64 * CELL_SIZE,
        station.design(),
        station.rotation(),
        cell_size,
        alpha,
    );
    if station.design() == StationDesign::Juicer {
        draw_juicer_slurry(context, station, animation_t, alpha, juicer_activity);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_juicer_slurry(
    context: &CanvasRenderingContext2d,
    station: SplitterStation,
    animation_t: f64,
    alpha: f64,
    activity: Option<f64>,
) {
    let Some(activity) = activity else {
        return;
    };
    let center = station.origin();
    let (local_col, local_row) = match station.rotation() {
        1 => (7.0, 7.0),
        2 => (2.0, 7.0),
        3 => (2.0, 2.0),
        _ => (7.0, 2.0),
    };
    let center_x = (center.col as f64 + local_col) * CELL_SIZE;
    let center_y = (center.row as f64 + local_row) * CELL_SIZE;
    context.save();
    context.set_global_alpha(alpha);
    context.begin_path();
    context.rect(
        center_x - CELL_SIZE,
        center_y - CELL_SIZE,
        CELL_SIZE * 2.0,
        CELL_SIZE * 2.0,
    );
    context.clip();
    let intensity = (activity * 2.0).min(1.0);
    for index in 0..7 {
        let phase = animation_t * (0.55 + index as f64 * 0.04) + index as f64 * 0.91;
        let orbit = CELL_SIZE * (0.12 + intensity * (0.16 + (index % 3) as f64 * 0.04));
        let lobe = CELL_SIZE * (0.24 + intensity * 0.22 + (phase * 1.7).sin().abs() * 0.08);
        context.set_fill_style_str(match index % 4 {
            0 => "rgba(93, 15, 24, 0.94)",
            1 => "rgba(151, 27, 36, 0.92)",
            2 => "rgba(116, 19, 28, 0.94)",
            _ => "rgba(188, 55, 58, 0.86)",
        });
        context.begin_path();
        context
            .arc(
                center_x + phase.cos() * orbit,
                center_y + phase.sin() * orbit * 0.72,
                lobe,
                0.0,
                std::f64::consts::TAU,
            )
            .ok();
        context.fill();
    }
    for index in 0..3 {
        let phase = -animation_t * 0.8 + index as f64 * 2.1;
        let radius = CELL_SIZE * (0.18 + intensity * 0.38);
        let size = CELL_SIZE * (0.09 + index as f64 * 0.025);
        context.set_fill_style_str(if index == 1 { "#8fc45d" } else { "#4f8f3f" });
        context.fill_rect(
            center_x + phase.cos() * radius - size / 2.0,
            center_y + phase.sin() * radius * 0.7 - size / 2.0,
            size,
            size,
        );
    }
    context.set_fill_style_str("rgba(52, 8, 15, 0.86)");
    context.begin_path();
    context
        .arc(
            center_x,
            center_y + CELL_SIZE * 0.42,
            CELL_SIZE * (0.11 + intensity * 0.05),
            0.0,
            std::f64::consts::TAU,
        )
        .ok();
    context.fill();
    context.restore();
}

#[cfg(target_arch = "wasm32")]
fn draw_construction_packets(
    context: &CanvasRenderingContext2d,
    station: SplitterStation,
    progress: f64,
) {
    let bar_x = station.origin().col as f64 * CELL_SIZE;
    let bar_y = station.origin().row as f64 * CELL_SIZE - 7.0;
    let bar_width = station.width() as f64 * CELL_SIZE;
    context.set_fill_style_str("rgba(17, 17, 17, 0.78)");
    context.fill_rect(bar_x, bar_y, bar_width, 5.0);
    context.set_fill_style_str("#f1c84b");
    context.fill_rect(bar_x + 1.0, bar_y + 1.0, (bar_width - 2.0) * progress, 3.0);

    let packet_count = (progress * 4.0).ceil().clamp(1.0, 4.0) as usize;
    let x = station.origin().col as f64 * CELL_SIZE + CELL_SIZE * 0.35;
    let y = station.origin().row as f64 * CELL_SIZE + CELL_SIZE * 0.35;
    for index in 0..packet_count {
        let px = x + (index % 2) as f64 * CELL_SIZE * 0.72;
        let py = y + (index / 2) as f64 * CELL_SIZE * 0.72;
        let size = CELL_SIZE * 0.62;
        context.set_fill_style_str("#f1c84b");
        context.fill_rect(px, py, size, size);
        context.set_stroke_style_str("#ffffff");
        context.set_line_width(1.5);
        context.begin_path();
        context.move_to(px + size / 2.0, py);
        context.line_to(px + size / 2.0, py + size);
        context.move_to(px, py + size / 2.0);
        context.line_to(px + size, py + size / 2.0);
        context.stroke();
        context.begin_path();
        context
            .arc(
                px + size * 0.38,
                py - 1.0,
                size * 0.16,
                0.0,
                std::f64::consts::TAU,
            )
            .ok();
        context
            .arc(
                px + size * 0.62,
                py - 1.0,
                size * 0.16,
                0.0,
                std::f64::consts::TAU,
            )
            .ok();
        context.stroke();
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_station_pixels(
    context: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    design: StationDesign,
    rotation: u8,
    cell_size: f64,
    alpha: f64,
) {
    let previous_alpha = context.global_alpha();
    context.set_global_alpha(alpha);
    let cols = station_cols(design, rotation);
    let rows = station_rows(design, rotation);
    for row in 0..rows {
        for col in 0..cols {
            let ch = station_pixel(design, row, col, rotation).unwrap_or(' ');
            if ch == ' ' {
                continue;
            }
            let color = match ch {
                '0' => "#9ca4a6",
                '#' => "#343941",
                'G' => "#bd8128",
                'B' => "#9d3431",
                'A' => "#4b5258",
                'S' | 'D' => "#202429",
                _ => "#343941",
            };
            context.set_fill_style_str(color);
            context.fill_rect(
                x + col as f64 * cell_size,
                y + row as f64 * cell_size,
                cell_size,
                cell_size,
            );
            if ch != '0' {
                context.set_stroke_style_str("rgba(17, 17, 17, 0.28)");
                context.set_line_width(1.0);
                context.stroke_rect(
                    x + col as f64 * cell_size + 0.5,
                    y + row as f64 * cell_size + 0.5,
                    cell_size - 1.0,
                    cell_size - 1.0,
                );
            }
            if ch == 'G' {
                context.set_fill_style_str("#f0bd55");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.24,
                    y + row as f64 * cell_size + cell_size * 0.24,
                    cell_size * 0.52,
                    cell_size * 0.52,
                );
                context.set_fill_style_str("#343941");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.4,
                    y + row as f64 * cell_size + cell_size * 0.4,
                    cell_size * 0.2,
                    cell_size * 0.2,
                );
            } else if ch == 'B' {
                context.set_fill_style_str("#d8dde0");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.38,
                    y + row as f64 * cell_size + cell_size * 0.12,
                    cell_size * 0.24,
                    cell_size * 0.76,
                );
            } else if ch == 'A' {
                context.set_fill_style_str("#747d84");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.12,
                    y + row as f64 * cell_size + cell_size * 0.16,
                    cell_size * 0.76,
                    cell_size * 0.22,
                );
            } else if ch == 'D' {
                context.set_stroke_style_str("#8b9498");
                context.set_line_width((cell_size * 0.12).max(1.0));
                context.begin_path();
                context
                    .arc(
                        x + (col as f64 + 0.5) * cell_size,
                        y + (row as f64 + 0.5) * cell_size,
                        cell_size * 0.22,
                        0.0,
                        std::f64::consts::TAU,
                    )
                    .ok();
                context.stroke();
            }
        }
    }
    context.set_global_alpha(previous_alpha);
}

#[cfg(target_arch = "wasm32")]
fn draw_station_invalid_overlay(
    context: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    design: StationDesign,
    rotation: u8,
    cell_size: f64,
) {
    context.set_fill_style_str("rgba(207, 43, 43, 0.58)");
    for row in 0..station_rows(design, rotation) {
        for col in 0..station_cols(design, rotation) {
            if station_pixel(design, row, col, rotation).is_some_and(|tile| tile != ' ') {
                context.fill_rect(
                    x + col as f64 * cell_size,
                    y + row as f64 * cell_size,
                    cell_size - 1.0,
                    cell_size - 1.0,
                );
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn station_pixel(design: StationDesign, row: usize, col: usize, rotation: u8) -> Option<char> {
    station_cell(design, row, col, rotation)
}

#[cfg(target_arch = "wasm32")]
fn station_cols(design: StationDesign, rotation: u8) -> usize {
    SplitterStation::new(Cell::new(0, 0), rotation, design, usize::MAX, usize::MAX).width()
}

#[cfg(target_arch = "wasm32")]
fn station_rows(design: StationDesign, rotation: u8) -> usize {
    SplitterStation::new(Cell::new(0, 0), rotation, design, usize::MAX, usize::MAX).height()
}

#[cfg(target_arch = "wasm32")]
fn direction_vector(direction: Direction) -> (f64, f64) {
    match direction {
        Direction::Up => (0.0, -1.0),
        Direction::Down => (0.0, 1.0),
        Direction::Left => (-1.0, 0.0),
        Direction::Right => (1.0, 0.0),
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_station_access(
    context: &CanvasRenderingContext2d,
    station: SplitterStation,
    activity: Option<SplitterActivity>,
    juicer_active: bool,
) {
    draw_door(
        context,
        station.entry_cell(),
        station.flow_direction().reversed(),
        0.78,
        "#4c9bd6",
    );

    if station.design() == StationDesign::Juicer {
        draw_door(
            context,
            station.old_exit_cell(),
            station.old_exit_direction(),
            f64::from(juicer_active),
            "#55b96b",
        );
        return;
    }

    let (new_exit_open, old_exit_open) =
        activity.map_or((0.0, 0.0), |activity| match activity.phase() {
            SplitterActivityPhase::Feeding | SplitterActivityPhase::Cutting => (0.0, 0.0),
            SplitterActivityPhase::Releasing => (1.0, 1.0),
        });
    draw_door(
        context,
        station.new_exit_cell(),
        station.new_exit_direction(),
        new_exit_open,
        "#55b96b",
    );
    draw_door(
        context,
        station.old_exit_cell(),
        station.old_exit_direction(),
        old_exit_open,
        "#55b96b",
    );
}

#[cfg(target_arch = "wasm32")]
fn draw_door(
    context: &CanvasRenderingContext2d,
    cell: Cell,
    outward: Direction,
    openness: f64,
    color: &str,
) {
    let x = cell.col as f64 * CELL_SIZE;
    let y = cell.row as f64 * CELL_SIZE;
    let thickness = CELL_SIZE * 0.24;
    let leaf = (CELL_SIZE * 0.5 - 1.0) * (1.0 - openness * 0.78);

    let rectangles = match outward {
        Direction::Left => [
            (x - thickness, y, thickness, leaf),
            (x - thickness, y + CELL_SIZE - leaf, thickness, leaf),
        ],
        Direction::Right => [
            (x + CELL_SIZE, y, thickness, leaf),
            (x + CELL_SIZE, y + CELL_SIZE - leaf, thickness, leaf),
        ],
        Direction::Up => [
            (x, y - thickness, leaf, thickness),
            (x + CELL_SIZE - leaf, y - thickness, leaf, thickness),
        ],
        Direction::Down => [
            (x, y + CELL_SIZE, leaf, thickness),
            (x + CELL_SIZE - leaf, y + CELL_SIZE, leaf, thickness),
        ],
    };

    context.set_fill_style_str(color);
    context.set_stroke_style_str("#111111");
    context.set_line_width(1.0);
    for (door_x, door_y, width, height) in rectangles {
        context.fill_rect(door_x, door_y, width, height);
        context.stroke_rect(door_x + 0.5, door_y + 0.5, width - 1.0, height - 1.0);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_station_selection(context: &CanvasRenderingContext2d, station: SplitterStation) {
    context.set_stroke_style_str("#111111");
    context.set_line_width(2.0);
    context.stroke_rect(
        station.origin().col as f64 * CELL_SIZE + 1.0,
        station.origin().row as f64 * CELL_SIZE + 1.0,
        station.width() as f64 * CELL_SIZE - 2.0,
        station.height() as f64 * CELL_SIZE - 2.0,
    );
}

#[cfg(target_arch = "wasm32")]
fn station_label(design: StationDesign) -> &'static str {
    match design {
        StationDesign::Juicer => "Juicer",
        StationDesign::Splitter => "Splitter",
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_station_tooltip(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    x: f64,
    y: f64,
    label: &str,
    min_width: f64,
) {
    let width = (label.len() as f64 * 8.0 + 18.0).max(min_width.min(150.0));
    let height = 24.0;
    let tooltip_x = x.min((layout.width - width - HUD_MARGIN).max(HUD_MARGIN));
    let tooltip_y = y.max(HUD_MARGIN);
    context.set_fill_style_str("rgba(17, 17, 17, 0.92)");
    context.fill_rect(tooltip_x, tooltip_y, width, height);
    context.set_fill_style_str("#ffffff");
    context.set_font("700 12px ui-monospace, SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text(label, tooltip_x + 9.0, tooltip_y + 16.0);
}

#[cfg(target_arch = "wasm32")]
fn station_origin_from_point(x: f64, y: f64, design: StationDesign, rotation: u8) -> Cell {
    let width_px = station_cols(design, rotation) as f64 * CELL_SIZE;
    let height_px = station_rows(design, rotation) as f64 * CELL_SIZE;
    Cell::new(
        ((x - width_px / 2.0).max(0.0) / CELL_SIZE).floor() as usize,
        ((y - height_px / 2.0).max(0.0) / CELL_SIZE).floor() as usize,
    )
}

#[cfg(target_arch = "wasm32")]
fn draw_paused(context: &CanvasRenderingContext2d, layout: SnekLayout) {
    context.set_fill_style_str("rgba(255, 255, 255, 0.68)");
    context.fill_rect(0.0, 0.0, layout.width, layout.height);
    context.set_fill_style_str("#111111");
    context.set_font("700 38px Georgia, 'Times New Roman', serif");
    let text = "paused";
    let width = 118.0;
    let _ = context.fill_text(text, (layout.width - width) / 2.0, layout.height / 2.0);
}

#[cfg(target_arch = "wasm32")]
fn wiggle_offsets(positions: &VecDeque<super::state::Cell>, t: f64) -> Vec<(f64, f64)> {
    let mut offset_d = t;
    let step = std::f64::consts::TAU / 12.0;
    let total_wiggle = 2.0;
    let last = positions.len().saturating_sub(1);

    positions
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index != last {
                offset_d += step;
            }
            if index == last || positions.len() < 3 {
                return (0.0, 0.0);
            }

            let before = if index == 0 {
                positions[index]
            } else {
                positions[index - 1]
            };
            let current = positions[index];
            let after = positions[(index + 1).min(last)];
            if before.col == current.col && current.col == after.col {
                ((offset_d.sin() * total_wiggle).floor(), 0.0)
            } else if before.row == current.row && current.row == after.row {
                (0.0, (offset_d.sin() * total_wiggle).floor())
            } else {
                (0.0, 0.0)
            }
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn load_save() -> Option<SavedSnekGame> {
    let raw = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())?;
    serde_json::from_str(&raw).ok()
}

#[cfg(target_arch = "wasm32")]
fn save_game(game: &SnekState) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let Ok(json) = serde_json::to_string(&game.save()) else {
        return;
    };
    let _ = storage.set_item(STORAGE_KEY, &json);
}

#[cfg(target_arch = "wasm32")]
fn random_seed() -> u64 {
    let random_bits = (js_sys::Math::random() * u64::MAX as f64) as u64;
    random_bits ^ js_sys::Date::now() as u64
}
