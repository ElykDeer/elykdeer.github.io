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
    station_cell, Cell, Direction, Lcg, SavedSnekGame, Snek, SnekGame as SnekState,
    SplitterActivity, SplitterActivityPhase, SplitterStation, StationDesign, APPLE_COST,
    AUTO_SNEK_COST, DEFAULT_COLS, DEFAULT_ROWS, MIN_SNEK_LEN, SPLITTER_STATION_COLS,
    SPLITTER_STATION_ROWS, SPLITTER_STATION_UNLOCK_COST,
};

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "elyk.snek.save.v3";
#[cfg(target_arch = "wasm32")]
const CLEAR_EVENT: &str = "elyk:snek-clear";
#[cfg(target_arch = "wasm32")]
const CELL_SIZE: f64 = 15.0;
#[cfg(target_arch = "wasm32")]
const STEP_MS: f64 = 50.0;
#[cfg(target_arch = "wasm32")]
const SAVE_MS: f64 = 5_000.0;
#[cfg(target_arch = "wasm32")]
const SWIPE_THRESHOLD: f64 = 18.0;
#[cfg(target_arch = "wasm32")]
const HUD_MARGIN: f64 = 14.0;
#[cfg(target_arch = "wasm32")]
const HUD_BUTTON_HEIGHT: f64 = 34.0;
#[cfg(target_arch = "wasm32")]
const STATION_OPTION_WIDTH: f64 = 74.0;
#[cfg(target_arch = "wasm32")]
const STATION_OPTION_HEIGHT: f64 = 54.0;
#[cfg(target_arch = "wasm32")]
const STATION_OPTION_GAP: f64 = 8.0;
#[cfg(target_arch = "wasm32")]
const ICON_BUTTON_SIZE: f64 = 34.0;
#[cfg(target_arch = "wasm32")]
const SHOP_BUTTON_WIDTH: f64 = 92.0;
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
    shop_open: bool,
    population_open: bool,
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
    shop_open: bool,
    population_open: bool,
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
    AutoSnek,
    StationOption(StationDesign),
    PlaceStation,
    RotateStation,
    CancelStation,
    MoveStation,
    DeleteStation,
    BuySnek,
    BuyApple,
    BuySplitterUnlock,
    ShopToggle,
    PopulationToggle,
    DebugAdd10,
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
            shop_open: false,
            population_open: false,
            cheat,
            active_pointers: Vec::new(),
            pinch: None,
        }
    }

    fn update(&mut self, timestamp: f64) {
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

        if self.dirty && timestamp - self.last_save >= SAVE_MS {
            self.save_now();
            self.last_save = timestamp;
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
            draw_station(&context, station, CELL_SIZE, 1.0);
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
        }
        for snek in self.game.sneks() {
            draw_snek(
                &context,
                snek,
                self.game.wiggle_t(),
                self.game.auto_snek_unlocked() && self.game.manual_snek() == Some(snek.id()),
            );
        }
        let splitter_activities = self.game.splitter_activities();
        for activity in splitter_activities.iter().copied() {
            if activity.phase() == SplitterActivityPhase::Cutting {
                draw_split_effect(&context, activity);
            }
        }
        for (index, station) in self.game.splitter_stations().iter().copied().enumerate() {
            if moving_station == Some(index) {
                continue;
            }
            let activity = splitter_activities
                .iter()
                .rev()
                .copied()
                .find(|activity| activity.station() == station);
            draw_station_access(&context, station, activity);
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
                shop_open: self.shop_open,
                population_open: self.population_open,
                cheat: self.cheat,
            },
        );
        if self.paused {
            draw_paused(&context, layout);
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
        self.shop_open = false;
        self.population_open = false;
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
        if let Some(station_index) = self.game.station_index_at(cell) {
            self.selected_station = Some(station_index);
            self.population_open = false;
            self.dirty = true;
            return;
        }

        self.selected_station = None;
        self.game.tap_cell(cell);
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
                        self.selected_station = match draft.source {
                            StationDraftSource::New => {
                                self.game.splitter_stations().len().checked_sub(1)
                            }
                            StationDraftSource::Move { index } => Some(index),
                        };
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
                    self.shop_open = false;
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
                    self.shop_open = true;
                    self.population_open = false;
                    self.dirty = true;
                }
            }
            HudButtonKind::PopulationToggle => {
                self.population_open = !self.population_open;
                if self.population_open {
                    self.shop_open = false;
                }
            }
            HudButtonKind::DebugAdd10 => {
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
            HudButtonKind::BuySnek => {
                if self.game.buy_snek() {
                    self.dirty = true;
                }
            }
            HudButtonKind::BuyApple => {
                if self.game.buy_apple(&mut self.rng) {
                    self.dirty = true;
                }
            }
            HudButtonKind::ShopToggle => {
                self.shop_open = !self.shop_open;
                if self.shop_open {
                    self.population_open = false;
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
            origin: station_origin_from_point(world_x, world_y, 0),
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
            draft.origin = station_origin_from_point(canvas_x, canvas_y, draft.rotation);
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
    let offsets = wiggle_offsets(positions, wiggle_t);
    let green_step = 255.0 / positions.len().max(1) as f64;
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
fn draw_split_effect(context: &CanvasRenderingContext2d, activity: SplitterActivity) {
    let progress = activity.progress();
    let station = activity.station();
    let cell = station.cut_target();
    let flow = station.flow_direction();
    let side = station.old_exit_direction();
    let (side_x, side_y) = direction_vector(side);
    let x = cell.col as f64 * CELL_SIZE;
    let y = cell.row as f64 * CELL_SIZE;
    let center_x = x + CELL_SIZE / 2.0;
    let center_y = y + CELL_SIZE / 2.0;
    let previous_alpha = context.global_alpha();
    context.set_global_alpha(0.98);

    draw_effect_anvil(context, center_x, center_y, flow, side_x, side_y);
    let slam_t = (progress / 0.35).clamp(0.0, 1.0);
    let retract_t = ((progress - 0.7) / 0.3).clamp(0.0, 1.0);
    let blade_offset = CELL_SIZE * (1.15 - slam_t * 0.81 + retract_t * 0.81);
    draw_effect_blade(
        context,
        center_x - side_x * blade_offset,
        center_y - side_y * blade_offset,
        flow,
    );

    context.set_global_alpha(previous_alpha);
}

#[cfg(target_arch = "wasm32")]
fn draw_effect_anvil(
    context: &CanvasRenderingContext2d,
    center_x: f64,
    center_y: f64,
    flow: Direction,
    side_x: f64,
    side_y: f64,
) {
    let (width, height) = match flow {
        Direction::Left | Direction::Right => (CELL_SIZE * 0.72, CELL_SIZE * 0.58),
        Direction::Up | Direction::Down => (CELL_SIZE * 0.58, CELL_SIZE * 0.72),
    };
    let block_x = center_x + side_x * CELL_SIZE * 0.34 - width / 2.0;
    let block_y = center_y + side_y * CELL_SIZE * 0.34 - height / 2.0;
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
) {
    let (width, height) = match flow {
        Direction::Left | Direction::Right => (CELL_SIZE * 0.24, CELL_SIZE * 0.62),
        Direction::Up | Direction::Down => (CELL_SIZE * 0.62, CELL_SIZE * 0.24),
    };
    let x = center_x - width / 2.0;
    let y = center_y - height / 2.0;
    context.set_fill_style_str("#d94b43");
    context.fill_rect(x, y, width, height);
    context.set_stroke_style_str("#111111");
    context.set_line_width(1.5);
    context.stroke_rect(x + 0.5, y + 0.5, width - 1.0, height - 1.0);
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
        shop_open,
        population_open,
        cheat,
    } = hud;
    let mut buttons = Vec::new();
    draw_score(context, layout, game, &mut buttons);

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
        if shop_open {
            draw_upgrade_shop(context, layout, game, cheat, &mut buttons);
        } else if population_open {
            draw_population_menu(context, layout, game, &mut buttons);
        }
        draw_bottom_toolbar(context, layout, shop_open, &mut buttons);
    }

    buttons
}

#[cfg(target_arch = "wasm32")]
fn draw_score(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    buttons: &mut Vec<HudButton>,
) {
    let text = game.apls().to_string();
    context.set_font("800 24px ui-monospace, SFMono-Regular, Consolas, monospace");
    context.set_line_width(4.0);
    context.set_stroke_style_str("#000000");
    context.set_fill_style_str("#ffffff");
    let width = measure_text(context, &text, 15.0);
    let x = (layout.width - width - HUD_MARGIN).max(HUD_MARGIN);
    let _ = context.stroke_text(&text, x, 32.0);
    let _ = context.fill_text(&text, x, 32.0);

    let snek_count = game.sneks().len();
    if snek_count > 1 {
        let count = format!("{snek_count} snek");
        context.set_font("800 16px ui-monospace, SFMono-Regular, Consolas, monospace");
        let count_width = measure_text(context, &count, 10.0);
        let count_x = (x - count_width - 12.0).max(HUD_MARGIN);
        let _ = context.stroke_text(&count, count_x, 31.0);
        let _ = context.fill_text(&count, count_x, 31.0);
        buttons.push(HudButton {
            kind: HudButtonKind::PopulationToggle,
            x: count_x - 4.0,
            y: 10.0,
            width: count_width + 8.0,
            height: 28.0,
        });
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
    shop_open: bool,
    buttons: &mut Vec<HudButton>,
) {
    let x = (layout.width - HUD_MARGIN - ICON_BUTTON_SIZE).max(HUD_MARGIN);
    let y = (layout.height - HUD_MARGIN - ICON_BUTTON_SIZE).max(HUD_MARGIN);
    let shop = draw_icon_button(
        context,
        HudButtonKind::ShopToggle,
        x,
        y,
        if shop_open { "#d9f5de" } else { "#ffffff" },
    );
    buttons.push(shop);
}

#[cfg(target_arch = "wasm32")]
fn draw_upgrade_shop(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    cheat: bool,
    buttons: &mut Vec<HudButton>,
) {
    let width = (layout.width - HUD_MARGIN * 2.0).min(620.0);
    let station_columns = if game.splitter_unlocked() {
        (((width - 20.0 + STATION_OPTION_GAP) / (STATION_OPTION_WIDTH + STATION_OPTION_GAP)).floor()
            as usize)
            .clamp(1, StationDesign::ALL.len())
    } else {
        0
    };
    let station_rows = if station_columns == 0 {
        0
    } else {
        StationDesign::ALL.len().div_ceil(station_columns)
    };
    let station_grid_height = if station_rows == 0 {
        0.0
    } else {
        STATION_OPTION_HEIGHT * station_rows as f64
            + STATION_OPTION_GAP * station_rows.saturating_sub(1) as f64
            + 12.0
    };
    let height = 70.0 + HUD_BUTTON_HEIGHT + station_grid_height;
    let x = ((layout.width - width) / 2.0).max(HUD_MARGIN);
    let y = (layout.height - height - HUD_MARGIN - ICON_BUTTON_SIZE - 8.0).max(HUD_MARGIN);
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
    let _ = context.fill_text("Shop", x + 10.0, y + 20.0);
    if game.pending_snek_orders() > 0 {
        context.set_font("700 11px ui-monospace, SFMono-Regular, Consolas, monospace");
        let _ = context.fill_text(
            &format!("queued {}", game.pending_snek_orders()),
            x + 58.0,
            y + 20.0,
        );
    }

    #[cfg(debug_assertions)]
    if cheat {
        let debug = draw_button(
            context,
            HudButtonKind::DebugAdd10,
            x + width - 72.0,
            y + 4.0,
            62.0,
            22.0,
            "+10",
        );
        buttons.push(debug);
    }

    #[cfg(not(debug_assertions))]
    let _ = cheat;

    let mut button_x = x + 10.0;
    let button_y = y + 34.0;
    if !game.auto_snek_unlocked() {
        let auto = draw_button(
            context,
            HudButtonKind::AutoSnek,
            button_x,
            button_y,
            150.0,
            HUD_BUTTON_HEIGHT,
            &format!("Auto Snek {AUTO_SNEK_COST}"),
        );
        if !game.can_buy_auto_snek() {
            draw_disabled_overlay(context, auto);
        }
        buttons.push(auto);
    } else if !game.splitter_unlocked() {
        let unlock = draw_button(
            context,
            HudButtonKind::BuySplitterUnlock,
            button_x,
            button_y,
            160.0,
            HUD_BUTTON_HEIGHT,
            &format!("Cutter {SPLITTER_STATION_UNLOCK_COST}"),
        );
        if !game.can_unlock_splitter_station() {
            draw_disabled_overlay(context, unlock);
        }
        buttons.push(unlock);
    } else {
        let buy_snek = draw_button(
            context,
            HudButtonKind::BuySnek,
            button_x,
            button_y,
            SHOP_BUTTON_WIDTH,
            HUD_BUTTON_HEIGHT,
            &format!("Snek {MIN_SNEK_LEN}"),
        );
        if !game.can_buy_snek() {
            draw_disabled_overlay(context, buy_snek);
        }
        buttons.push(buy_snek);
        button_x += SHOP_BUTTON_WIDTH + STATION_OPTION_GAP;

        let buy_apple = draw_button(
            context,
            HudButtonKind::BuyApple,
            button_x,
            button_y,
            SHOP_BUTTON_WIDTH,
            HUD_BUTTON_HEIGHT,
            &format!("Apple {APPLE_COST}"),
        );
        if !game.can_buy_apple() {
            draw_disabled_overlay(context, buy_apple);
        }
        buttons.push(buy_apple);

        let station_y = button_y + HUD_BUTTON_HEIGHT + 10.0;
        for (index, design) in StationDesign::ALL.into_iter().enumerate() {
            let col = index % station_columns;
            let row = index / station_columns;
            let option_x = x + 10.0 + col as f64 * (STATION_OPTION_WIDTH + STATION_OPTION_GAP);
            let option_y = station_y + row as f64 * (STATION_OPTION_HEIGHT + STATION_OPTION_GAP);
            let button = HudButton {
                kind: HudButtonKind::StationOption(design),
                x: option_x,
                y: option_y,
                width: STATION_OPTION_WIDTH,
                height: STATION_OPTION_HEIGHT,
            };
            draw_station_option(context, button, design);
            if game.can_place_splitter_station() {
                buttons.push(button);
            } else {
                draw_disabled_overlay(context, button);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_population_menu(
    context: &CanvasRenderingContext2d,
    layout: SnekLayout,
    game: &SnekState,
    buttons: &mut Vec<HudButton>,
) {
    let width = (layout.width - HUD_MARGIN * 2.0).min(360.0);
    let x = ((layout.width - width) / 2.0).max(HUD_MARGIN);
    let groups = snek_length_groups(game);
    let row_height = 22.0;
    let height = 38.0 + row_height * groups.len().max(1) as f64;
    let y = (layout.height - height - HUD_MARGIN - ICON_BUTTON_SIZE - 8.0).max(HUD_MARGIN);
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
    let _ = context.fill_text("Population", x + 10.0, y + 20.0);
    context.set_font("700 12px ui-monospace, SFMono-Regular, Consolas, monospace");
    if groups.is_empty() {
        let _ = context.fill_text("1 x length 3", x + 10.0, y + 44.0);
        return;
    }
    for (row, (len, count)) in groups.into_iter().enumerate() {
        let line_y = y + 44.0 + row as f64 * row_height;
        let _ = context.fill_text(&format!("{count} x length {len}"), x + 10.0, line_y);
    }
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
    let station_width = station_cols(draft.rotation) as f64 * CELL_SIZE * camera.zoom;
    let station_height = station_rows(draft.rotation) as f64 * CELL_SIZE * camera.zoom;
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
        HudButtonKind::ShopToggle => draw_shop_icon(context, x, y),
        _ => {}
    }

    HudButton {
        kind,
        x,
        y,
        width: ICON_BUTTON_SIZE,
        height: ICON_BUTTON_SIZE,
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
fn draw_shop_icon(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.begin_path();
    context.move_to(x + 9.0, y + 14.0);
    context.line_to(x + 25.0, y + 14.0);
    context.line_to(x + 23.0, y + 27.0);
    context.line_to(x + 11.0, y + 27.0);
    context.close_path();
    context.stroke();
    context.begin_path();
    context.move_to(x + 13.0, y + 14.0);
    context.line_to(x + 13.0, y + 9.0);
    context.line_to(x + 21.0, y + 9.0);
    context.line_to(x + 21.0, y + 14.0);
    context.move_to(x + 13.0, y + 20.0);
    context.line_to(x + 21.0, y + 20.0);
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

    let icon_cell = ((button.width - 12.0) / SPLITTER_STATION_COLS as f64)
        .min((button.height - 12.0) / SPLITTER_STATION_ROWS as f64);
    draw_station_pixels(
        context,
        button.x + (button.width - SPLITTER_STATION_COLS as f64 * icon_cell) / 2.0,
        button.y + (button.height - SPLITTER_STATION_ROWS as f64 * icon_cell) / 2.0,
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
    let cols = station_cols(rotation);
    let rows = station_rows(rotation);
    for row in 0..rows {
        for col in 0..cols {
            let ch = station_pixel(design, row, col, rotation).unwrap_or(' ');
            if ch == ' ' {
                continue;
            }
            let color = match ch {
                '1' => "#343941",
                '2' => "#d89a2b",
                '3' => "#a9413d",
                '0' => "#9ca4a6",
                _ => "#f4f4f4",
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
            if ch == '2' {
                context.set_fill_style_str("#f0bd55");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.24,
                    y + row as f64 * cell_size + cell_size * 0.24,
                    cell_size * 0.52,
                    cell_size * 0.52,
                );
            } else if ch == '3' {
                context.set_fill_style_str("#d8dde0");
                context.fill_rect(
                    x + col as f64 * cell_size + cell_size * 0.38,
                    y + row as f64 * cell_size + cell_size * 0.12,
                    cell_size * 0.24,
                    cell_size * 0.76,
                );
            }
        }
    }
    context.set_global_alpha(previous_alpha);
}

#[cfg(target_arch = "wasm32")]
fn station_pixel(design: StationDesign, row: usize, col: usize, rotation: u8) -> Option<char> {
    station_cell(design, row, col, rotation)
}

#[cfg(target_arch = "wasm32")]
fn station_cols(rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        SPLITTER_STATION_COLS
    } else {
        SPLITTER_STATION_ROWS
    }
}

#[cfg(target_arch = "wasm32")]
fn station_rows(rotation: u8) -> usize {
    if rotation.is_multiple_of(2) {
        SPLITTER_STATION_ROWS
    } else {
        SPLITTER_STATION_COLS
    }
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
) {
    draw_door(
        context,
        station.entry_cell(),
        station.flow_direction().reversed(),
        0.78,
        "#4c9bd6",
    );

    let (new_exit_open, old_exit_open) =
        activity.map_or((0.0, 0.0), |activity| match activity.phase() {
            SplitterActivityPhase::Feeding => (door_openness(activity.progress()), 0.0),
            SplitterActivityPhase::Cutting => (1.0, 0.0),
            SplitterActivityPhase::Releasing => (1.0, 1.0),
        });
    draw_door(
        context,
        station.new_exit_cell(),
        station.flow_direction(),
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
fn door_openness(progress: f64) -> f64 {
    (progress * 2.0).clamp(0.0, 1.0)
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
            (x, y, thickness, leaf),
            (x, y + CELL_SIZE - leaf, thickness, leaf),
        ],
        Direction::Right => [
            (x + CELL_SIZE - thickness, y, thickness, leaf),
            (
                x + CELL_SIZE - thickness,
                y + CELL_SIZE - leaf,
                thickness,
                leaf,
            ),
        ],
        Direction::Up => [
            (x, y, leaf, thickness),
            (x + CELL_SIZE - leaf, y, leaf, thickness),
        ],
        Direction::Down => [
            (x, y + CELL_SIZE - thickness, leaf, thickness),
            (
                x + CELL_SIZE - leaf,
                y + CELL_SIZE - thickness,
                leaf,
                thickness,
            ),
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
        StationDesign::Gate => "Gate splitter",
        StationDesign::Saw => "Saw splitter",
        StationDesign::Press => "Press splitter",
        StationDesign::Butterfly => "Butterfly splitter",
        StationDesign::Mantis => "Mantis splitter",
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
fn station_origin_from_point(x: f64, y: f64, rotation: u8) -> Cell {
    let width_px = station_cols(rotation) as f64 * CELL_SIZE;
    let height_px = station_rows(rotation) as f64 * CELL_SIZE;
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
