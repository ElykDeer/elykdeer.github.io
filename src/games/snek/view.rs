use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
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
};

#[cfg(target_arch = "wasm32")]
use super::state::{
    Direction, Lcg, SavedSnekGame, SnekGame as SnekState, DEFAULT_COLS, DEFAULT_ROWS,
};

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "elyk.snek.save.v1";
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
#[component]
pub fn SnekGame(#[prop(default = false)] _cheat: bool) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let runner = Rc::new(RefCell::new(SnekRunner::new()));
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

    let click_runner = Rc::clone(&runner);
    let handle_click = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            click_runner.borrow_mut().render(&canvas);
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
            let _ = canvas.focus();
        }
        pointer_down_runner
            .borrow_mut()
            .pointer_down(ev.client_x() as f64, ev.client_y() as f64);
    };

    let pointer_up_runner = Rc::clone(&runner);
    let handle_pointer_up = move |ev: PointerEvent| {
        ev.prevent_default();
        pointer_up_runner
            .borrow_mut()
            .pointer_up(ev.client_x() as f64, ev.client_y() as f64);
    };

    view! {
        <section class="terminal-game snek-game" aria-label="Snek">
            <canvas
                class="terminal-game-canvas snek-game-canvas"
                style="height:clamp(24rem,64vh,38rem);background:#ffffff;cursor:pointer;image-rendering:pixelated;"
                node_ref=canvas_ref
                tabindex="0"
                role="application"
                aria-label="Snek game canvas. Move with arrow keys, WASD, or swipe. Pause with Space."
                on:click=handle_click
                on:keydown=handle_keydown
                on:pointerdown=handle_pointer_down
                on:pointerup=handle_pointer_up
                on:pointercancel=move |_| runner.borrow_mut().pointer_cancel()
            />
            <div class="visually-hidden" role="status" aria-live="polite">
                {move || status_text.get()}
            </div>
        </section>
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn SnekGame(#[prop(default = false)] _cheat: bool) -> impl IntoView {
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
            runner.disable_persistence();
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
}

#[cfg(target_arch = "wasm32")]
impl SnekRunner {
    fn new() -> Self {
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
        }
    }

    fn update(&mut self, timestamp: f64) {
        if self.last_step == 0.0 {
            self.last_step = timestamp;
        }

        if !self.paused {
            let mut steps = 0;
            while timestamp - self.last_step >= STEP_MS && steps < 6 {
                self.game.step(&mut self.rng);
                self.last_step += STEP_MS;
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
        self.game.resize(layout.cols, layout.rows);
        let Some(context) = canvas_context(canvas) else {
            return;
        };

        context.set_fill_style_str("#ffffff");
        context.fill_rect(0.0, 0.0, layout.width, layout.height);

        draw_apple(&context, layout, self.game.apple());
        draw_snek(&context, layout, &self.game);
        draw_score(&context, layout, self.game.score());
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

    fn pointer_down(&mut self, x: f64, y: f64) {
        self.pointer_start = Some((x, y));
    }

    fn pointer_up(&mut self, x: f64, y: f64) {
        let Some((start_x, start_y)) = self.pointer_start.take() else {
            return;
        };
        let dx = x - start_x;
        let dy = y - start_y;
        if dx.abs().max(dy.abs()) < SWIPE_THRESHOLD {
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

    fn pointer_cancel(&mut self) {
        self.pointer_start = None;
    }

    fn disable_persistence(&mut self) {
        self.save_enabled = false;
        self.dirty = false;
    }

    fn status_text(&self) -> String {
        if self.paused {
            format!("Snek paused. Score {}.", self.game.score())
        } else {
            format!("Snek score {}.", self.game.score())
        }
    }

    fn save_now(&self) {
        if !self.save_enabled {
            return;
        }
        save_game(&self.game);
    }
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
    let width = canvas.client_width().max(300) as f64;
    let height = canvas.client_height().max(300) as f64;
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
fn draw_apple(context: &CanvasRenderingContext2d, layout: SnekLayout, apple: super::state::Cell) {
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
    draw_grid_mask(context, layout);
}

#[cfg(target_arch = "wasm32")]
fn draw_snek(context: &CanvasRenderingContext2d, layout: SnekLayout, game: &SnekState) {
    let positions = game.positions();
    let offsets = wiggle_offsets(positions, game.wiggle_t());
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

    draw_grid_mask(context, layout);
}

#[cfg(target_arch = "wasm32")]
fn draw_grid_mask(context: &CanvasRenderingContext2d, layout: SnekLayout) {
    let grid_width = layout.cols as f64 * CELL_SIZE;
    let grid_height = layout.rows as f64 * CELL_SIZE;
    context.set_fill_style_str("#f4f4f4");
    if grid_width < layout.width {
        context.fill_rect(grid_width, 0.0, layout.width - grid_width, layout.height);
    }
    if grid_height < layout.height {
        context.fill_rect(0.0, grid_height, layout.width, layout.height - grid_height);
    }
}

#[cfg(target_arch = "wasm32")]
fn draw_score(context: &CanvasRenderingContext2d, layout: SnekLayout, score: usize) {
    let text = score.to_string();
    context.set_fill_style_str("#111111");
    context.set_font("700 46px Georgia, 'Times New Roman', serif");
    let width = text.len() as f64 * 28.0;
    let _ = context.fill_text(&text, layout.width - width - 8.0, 46.0);
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
fn wiggle_offsets(positions: &[super::state::Cell], t: f64) -> Vec<(f64, f64)> {
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
