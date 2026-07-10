use std::cell::RefCell;
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::rc::Rc;

use leptos::html::Canvas;
use leptos::prelude::*;
use leptos::reactive::owner::on_cleanup;
use send_wrapper::SendWrapper;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent};

use super::state::{
    score_breakdown, Board, BubbleColor, BubbleGame, CellCoord, Resolution, SavedBubbleGame,
    ShopUpgrade, COLS, ROWS,
};

const HIGH_SCORE_STORAGE_KEY: &str = "elyk.bubbles.high-score";
const NORMAL_SAVE_STORAGE_KEY: &str = "elyk.bubbles.save.normal";
const HARD_SAVE_STORAGE_KEY: &str = "elyk.bubbles.save.hard";
const PROGRESS_STORAGE_KEY: &str = "elyk.bubbles.progress.v1";
const RESUME_OFFER_MOVES: u8 = 5;
const CURRENT_PROGRESS_VERSION: u8 = 2;
const DROP_RESET_COST: u64 = 800;

/// Canvas implementation for the terminal bubbles game.
#[component]
pub fn BubblesGame(
    #[prop(default = false)] hard: bool,
    #[prop(default = false)] cheat: bool,
) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let high_score = load_high_score();
    let saved_game = load_saved_game(hard, high_score);
    let resume_offer_visible = RwSignal::new(saved_game.is_some());
    let runner = Rc::new(RefCell::new(CanvasRunner::new(
        high_score, hard, saved_game,
    )));
    let status_text = RwSignal::new(runner.borrow().status_text());
    let ability_snapshot = RwSignal::new(runner.borrow().ability_snapshot());
    let ability_tooltip = RwSignal::new(None::<String>);
    let pending_ability = RwSignal::new(None::<AbilityAction>);
    let shop_visible = RwSignal::new(runner.borrow().shop_visible());
    let animation = Rc::new(RefCell::new(AnimationLoop::default()));

    let start_runner = Rc::clone(&runner);
    let start_animation = Rc::clone(&animation);
    let frame_resume_offer_visible = resume_offer_visible;
    let frame_status_text = status_text;
    let frame_ability_snapshot = ability_snapshot;
    let frame_shop_visible = shop_visible;
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
                let resume_visible = runner.resume_offer_visible();
                if frame_resume_offer_visible.get_untracked() != resume_visible {
                    frame_resume_offer_visible.set(resume_visible);
                }
                let status = runner.status_text();
                if frame_status_text.get_untracked() != status {
                    frame_status_text.set(status);
                }
                let abilities = runner.ability_snapshot();
                if frame_ability_snapshot.get_untracked() != abilities {
                    frame_ability_snapshot.set(abilities);
                }
                let shop_is_visible = runner.shop_visible();
                if frame_shop_visible.get_untracked() != shop_is_visible {
                    frame_shop_visible.set(shop_is_visible);
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
    on_cleanup(move || {
        cleanup_animation.borrow_mut().cancel();
    });

    let move_runner = Rc::clone(&runner);
    let handle_mouse_move = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            move_runner.borrow_mut().pointer_move(
                ev.client_x() as f64,
                ev.client_y() as f64,
                &canvas,
            );
        }
    };

    let shoot_runner = Rc::clone(&runner);
    let handle_click = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            // Hit-test before focusing: focus() can scroll the canvas into view
            // and shift its rect out from under the click coordinates.
            let mut runner = shoot_runner.borrow_mut();
            runner.tap(ev.client_x() as f64, ev.client_y() as f64, &canvas);
            runner.render(&canvas);
            let _ = canvas.focus();
        }
    };

    let touch_move_runner = Rc::clone(&runner);
    let handle_touch_move = move |ev: TouchEvent| {
        ev.prevent_default();
        let Some(touch) = ev.touches().item(0) else {
            return;
        };
        if let Some(canvas) = canvas_ref.get() {
            touch_move_runner.borrow_mut().pointer_move(
                touch.client_x() as f64,
                touch.client_y() as f64,
                &canvas,
            );
        }
    };

    let touch_start_runner = Rc::clone(&runner);
    let handle_touch_start = move |ev: TouchEvent| {
        ev.prevent_default();
        let Some(touch) = ev.touches().item(0) else {
            return;
        };
        if let Some(canvas) = canvas_ref.get() {
            touch_start_runner.borrow_mut().pointer_move(
                touch.client_x() as f64,
                touch.client_y() as f64,
                &canvas,
            );
        }
    };

    let touch_shoot_runner = Rc::clone(&runner);
    let handle_touch_end = move |ev: TouchEvent| {
        ev.prevent_default();
        let Some(touch) = ev.changed_touches().item(0) else {
            return;
        };
        if let Some(canvas) = canvas_ref.get() {
            let mut runner = touch_shoot_runner.borrow_mut();
            runner.tap(touch.client_x() as f64, touch.client_y() as f64, &canvas);
            runner.render(&canvas);
        }
    };

    let hold_runner = Rc::clone(&runner);
    let handle_context_menu = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            let mut runner = hold_runner.borrow_mut();
            runner.hold();
            runner.render(&canvas);
        }
    };

    let key_runner = Rc::clone(&runner);
    let handle_keydown = move |ev: KeyboardEvent| match ev.key().as_str() {
        "ArrowLeft" => {
            ev.prevent_default();
            key_runner.borrow_mut().nudge_aim(-0.055);
        }
        "ArrowRight" => {
            ev.prevent_default();
            key_runner.borrow_mut().nudge_aim(0.055);
        }
        " " | "Enter" => {
            ev.prevent_default();
            key_runner.borrow_mut().shoot_or_restart();
        }
        "1" => {
            ev.prevent_default();
            key_runner.borrow_mut().select_shop_option(0);
        }
        "2" => {
            ev.prevent_default();
            key_runner.borrow_mut().select_shop_option(1);
        }
        "3" => {
            ev.prevent_default();
            key_runner.borrow_mut().select_shop_option(2);
        }
        "c" | "C" => {
            ev.prevent_default();
            key_runner.borrow_mut().call_color();
        }
        "b" | "B" => {
            ev.prevent_default();
            key_runner.borrow_mut().load_bomb_shot();
        }
        "w" | "W" => {
            ev.prevent_default();
            key_runner.borrow_mut().load_wild_shot();
        }
        "l" | "L" => {
            ev.prevent_default();
            key_runner.borrow_mut().load_lightning_shot();
        }
        "d" | "D" => {
            ev.prevent_default();
            key_runner.borrow_mut().load_drill_shot();
        }
        "x" | "X" => {
            ev.prevent_default();
            key_runner.borrow_mut().use_drop_reset();
        }
        "h" | "H" => {
            ev.prevent_default();
            key_runner.borrow_mut().hold();
        }
        "r" | "R" => {
            ev.prevent_default();
            key_runner.borrow_mut().resume_saved();
        }
        _ => {}
    };

    let resume_runner = Rc::clone(&runner);
    let handle_resume = move |ev: MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(canvas) = canvas_ref.get() {
            let mut runner = resume_runner.borrow_mut();
            runner.resume_saved();
            resume_offer_visible.set(runner.resume_offer_visible());
            runner.render(&canvas);
            status_text.set(runner.status_text());
            let _ = canvas.focus();
        }
    };

    let call_runner = Rc::clone(&runner);
    let handle_color_call = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::ColorCall,
            ev,
            Rc::clone(&call_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    let bomb_runner = Rc::clone(&runner);
    let handle_bomb_shot = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::BombShot,
            ev,
            Rc::clone(&bomb_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    let wild_runner = Rc::clone(&runner);
    let handle_wild_shot = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::WildShot,
            ev,
            Rc::clone(&wild_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    let lightning_runner = Rc::clone(&runner);
    let handle_lightning_shot = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::Lightning,
            ev,
            Rc::clone(&lightning_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    let drill_runner = Rc::clone(&runner);
    let handle_drill_shot = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::Drill,
            ev,
            Rc::clone(&drill_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    let reset_runner = Rc::clone(&runner);
    let handle_drop_reset = move |ev: MouseEvent| {
        handle_ability_click(
            AbilityAction::DropReset,
            ev,
            Rc::clone(&reset_runner),
            canvas_ref,
            status_text,
            ability_snapshot,
            ability_tooltip,
            pending_ability,
        );
    };

    #[cfg(debug_assertions)]
    let debug_controls = if cheat {
        let debug_shop_runner = Rc::clone(&runner);
        let debug_score_runner = Rc::clone(&runner);
        let debug_jump_to_shop = move |ev: MouseEvent| {
            ev.prevent_default();
            if let Some(canvas) = canvas_ref.get() {
                let mut runner = debug_shop_runner.borrow_mut();
                runner.debug_jump_to_shop();
                resume_offer_visible.set(runner.resume_offer_visible());
                shop_visible.set(runner.shop_visible());
                status_text.set(runner.status_text());
                ability_snapshot.set(runner.ability_snapshot());
                runner.render(&canvas);
                let _ = canvas.focus();
            }
        };
        let debug_add_score = move |ev: MouseEvent| {
            ev.prevent_default();
            if let Some(canvas) = canvas_ref.get() {
                let mut runner = debug_score_runner.borrow_mut();
                runner.debug_add_score();
                shop_visible.set(runner.shop_visible());
                status_text.set(runner.status_text());
                runner.render(&canvas);
                let _ = canvas.focus();
            }
        };

        view! {
            <div
                class="bubbles-debug-controls"
                class:bubbles-debug-controls-shop=move || shop_visible.get()
                aria-label="Debug controls"
            >
                <button
                    type="button"
                    class:bubbles-debug-control-hidden=move || shop_visible.get()
                    on:click=debug_jump_to_shop
                >"Shop"</button>
                <button
                    type="button"
                    class:bubbles-debug-control-hidden=move || !shop_visible.get()
                    on:click=debug_add_score
                >"+10k"</button>
            </div>
        }
        .into_any()
    } else {
        view! {}.into_any()
    };
    #[cfg(not(debug_assertions))]
    let debug_controls = {
        let _ = cheat;
        view! {}.into_any()
    };

    view! {
        <section class="terminal-game bubbles-game" aria-label="Bubbles">
            <canvas
                class="terminal-game-canvas"
                node_ref=canvas_ref
                tabindex="0"
                role="application"
                aria-label="Bubbles game canvas. Aim with mouse, touch, or arrow keys. Shoot with click, tap, Space, or Enter. Hold with H, right click, or the hold slot."
                on:mousemove=handle_mouse_move
                on:click=handle_click
                on:touchmove=handle_touch_move
                on:touchstart=handle_touch_start
                on:touchend=handle_touch_end
                on:contextmenu=handle_context_menu
                on:keydown=handle_keydown
            />
            <div class="visually-hidden" role="status" aria-live="polite">
                {move || status_text.get()}
            </div>
            <div class="bubbles-ability-strip" aria-label="Active upgrades">
                <div class="bubbles-ability-cluster bubbles-ability-cluster-left">
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        let abilities = ability_snapshot.get();
                        shop_visible.get() || !abilities.color_call_enabled || abilities.color_call_charges == 0
                    }
                    aria-label="Color Call"
                    on:click=handle_color_call
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <circle cx="12" cy="12" r="7" fill="none" stroke="currentColor" stroke-width="2" />
                        <circle cx="12" cy="12" r="2.5" fill="currentColor" />
                        <path d="M12 2v3M12 19v3M2 12h3M19 12h3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().color_call_charges}</span>
                </button>
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        shop_visible.get() || ability_snapshot.get().bomb_shot_charges == 0
                    }
                    aria-label="Bomb Shot"
                    on:click=handle_bomb_shot
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <circle cx="11" cy="13" r="6.5" fill="none" stroke="currentColor" stroke-width="2" />
                        <path d="M15.5 7.5 18 5M18 5l1.5 1.5M18 5l-1.5-1.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().bomb_shot_charges}</span>
                </button>
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        shop_visible.get() || ability_snapshot.get().wild_shot_charges == 0
                    }
                    aria-label="Wild Shot"
                    on:click=handle_wild_shot
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <circle cx="12" cy="12" r="7" fill="none" stroke="currentColor" stroke-width="2" />
                        <path d="M12 5v14M5 12h14M7.5 7.5l9 9M16.5 7.5l-9 9" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().wild_shot_charges}</span>
                </button>
                </div>
                <div class="bubbles-ability-cluster bubbles-ability-cluster-right">
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        shop_visible.get() || ability_snapshot.get().lightning_charges == 0
                    }
                    aria-label="Lightning Shot"
                    on:click=handle_lightning_shot
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <path d="M13 2 5 13h6l-1 9 8-12h-6l1-8Z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().lightning_charges}</span>
                </button>
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        shop_visible.get() || ability_snapshot.get().drill_charges == 0
                    }
                    aria-label="Drill Shot"
                    on:click=handle_drill_shot
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <path d="M12 3l6 6-7 12-5-5L12 3Z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
                        <path d="M9 14l3 3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().drill_charges}</span>
                </button>
                <button
                    type="button"
                    class="bubbles-ability-bubble"
                    class:bubbles-ability-bubble-hidden=move || {
                        shop_visible.get() || ability_snapshot.get().drop_reset_charges == 0
                    }
                    aria-label="Drop Reset"
                    on:click=handle_drop_reset
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                        <path d="M7 7h7a5 5 0 1 1-4.4 7.4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                        <path d="M7 7v5h5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                    <span class="bubbles-ability-count">{move || ability_snapshot.get().drop_reset_charges}</span>
                </button>
                </div>
            </div>
            <div
                class="bubbles-ability-tooltip"
                class:bubbles-ability-tooltip-hidden=move || ability_tooltip.get().is_none() || shop_visible.get()
            >
                {move || ability_tooltip.get().unwrap_or_default()}
            </div>
            {debug_controls}
            <button
                type="button"
                class="bubbles-resume-button"
                class:bubbles-resume-button-hidden=move || !resume_offer_visible.get() || shop_visible.get()
                aria-hidden=move || (!resume_offer_visible.get() || shop_visible.get()).to_string()
                disabled=move || !resume_offer_visible.get() || shop_visible.get()
                tabindex=move || if resume_offer_visible.get() && !shop_visible.get() { "0" } else { "-1" }
                on:click=handle_resume
            >
                <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                    <path
                        d="M20 12a8 8 0 1 1-2.35-5.65"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                    />
                    <path
                        d="M20 4v5h-5"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    />
                </svg>
                <span>"Resume last?"</span>
            </button>
        </section>
    }
}

fn handle_ability_click(
    action: AbilityAction,
    ev: MouseEvent,
    runner: Rc<RefCell<CanvasRunner>>,
    canvas_ref: NodeRef<Canvas>,
    status_text: RwSignal<String>,
    ability_snapshot: RwSignal<AbilitySnapshot>,
    ability_tooltip: RwSignal<Option<String>>,
    pending_ability: RwSignal<Option<AbilityAction>>,
) {
    ev.prevent_default();
    ev.stop_propagation();

    if pending_ability.get_untracked() != Some(action) {
        pending_ability.set(Some(action));
        ability_tooltip.set(Some(action.help(ability_snapshot.get_untracked())));
        fade_ability_tooltip(action, ability_tooltip, pending_ability);
        return;
    }

    pending_ability.set(None);
    ability_tooltip.set(None);
    let Some(canvas) = canvas_ref.get() else {
        return;
    };
    {
        let mut runner = runner.borrow_mut();
        match action {
            AbilityAction::ColorCall => runner.call_color(),
            AbilityAction::BombShot => runner.load_bomb_shot(),
            AbilityAction::WildShot => runner.load_wild_shot(),
            AbilityAction::Lightning => runner.load_lightning_shot(),
            AbilityAction::Drill => runner.load_drill_shot(),
            AbilityAction::DropReset => runner.use_drop_reset(),
        }
        runner.render(&canvas);
        status_text.set(runner.status_text());
        ability_snapshot.set(runner.ability_snapshot());
    }
    let _ = canvas.focus();
}

fn fade_ability_tooltip(
    action: AbilityAction,
    ability_tooltip: RwSignal<Option<String>>,
    pending_ability: RwSignal<Option<AbilityAction>>,
) {
    let callback = Closure::once(move || {
        if pending_ability.get_untracked() == Some(action) {
            pending_ability.set(None);
            ability_tooltip.set(None);
        }
    });

    if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            2_800,
        );
        callback.forget();
    }
}

#[derive(Default)]
struct AnimationLoop {
    callback: Option<Closure<dyn FnMut(f64)>>,
    request_id: Option<i32>,
}

impl AnimationLoop {
    fn is_running(&self) -> bool {
        self.callback.is_some()
    }

    fn cancel(&mut self) {
        if let (Some(window), Some(request_id)) = (web_sys::window(), self.request_id.take()) {
            let _ = window.cancel_animation_frame(request_id);
        }
        self.callback = None;
    }
}

fn request_animation_frame(animation: &Rc<RefCell<AnimationLoop>>) -> Option<i32> {
    let animation = animation.borrow();
    let window = web_sys::window()?;
    let callback = animation.callback.as_ref()?;
    window
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .ok()
}

#[derive(Clone, Copy, Debug)]
struct BubbleLayout {
    width: f64,
    height: f64,
    top: f64,
    radius: f64,
    row_gap: f64,
    left: f64,
    cannon_x: f64,
    cannon_y: f64,
}

impl BubbleLayout {
    fn for_canvas(width: f64, height: f64) -> Self {
        let top = 36.0;
        let bottom = 104.0;
        let radius_by_width = (width - 28.0).max(280.0) / (COLS as f64 * 2.0 + 1.0);
        let radius_by_height =
            (height - top - bottom).max(260.0) / (2.0 + (ROWS - 1) as f64 * 1.72);
        let radius = radius_by_width.min(radius_by_height).clamp(8.5, 19.0);
        let row_gap = radius * 1.72;
        let grid_width = radius * (COLS as f64 * 2.0 + 1.0);
        let left = ((width - grid_width) / 2.0).max(8.0);

        Self {
            width,
            height,
            top,
            radius,
            row_gap,
            left,
            cannon_x: width / 2.0,
            cannon_y: height - 58.0,
        }
    }

    fn center(&self, coord: CellCoord, parity: usize) -> (f64, f64) {
        let stagger = if (coord.row + parity) % 2 == 0 {
            0.0
        } else {
            self.radius
        };
        (
            self.left + self.radius + stagger + coord.col as f64 * self.radius * 2.0,
            self.top + self.radius + coord.row as f64 * self.row_gap,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Projectile {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    color: BubbleColor,
}

/// A short-lived bit of animation: a pop shard or a falling dropped bubble.
#[derive(Clone, Copy, Debug)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    radius: f64,
    color: BubbleColor,
    kind: ParticleKind,
    delay: f64,
    life: f64,
    max_life: f64,
}

#[derive(Clone, Copy, Debug)]
enum ParticleKind {
    Bubble,
    Spark,
    Ring,
    LightningArc,
    ScreenFlash,
}

const GRAVITY: f64 = 660.0;
const DROP_SLIDE: f64 = 0.20;
/// Center distance (in radii) at which a shot attaches. Edge contact is 2.0, so
/// less than that lets a shot slip a bit further into a gap before snapping.
const COLLISION_TOLERANCE: f64 = 1.55;
const SHOT_CAST_STEP_RADIUS: f64 = 0.42;
const DEFAULT_TRACE_DISTANCE_RADIUS: f64 = 22.0;
const LEVEL_ONE_TRACE_MULTIPLIER: f64 = 2.0;
const BANK_DISTANCE_PER_LEVEL_RADIUS: f64 = 4.0;
const BASE_TRACE_BOUNCES: u8 = 1;
const FULL_TRACE_BOUNCES: u8 = 80;

#[derive(Debug)]
struct CanvasRunner {
    game: BubbleGame,
    hard: bool,
    resume_offer: ResumeOffer,
    progress: BubbleProgress,
    aim_angle: f64,
    projectile: Option<Projectile>,
    particles: Vec<Particle>,
    shop_tab: ShopTab,
    shop_kit_page: usize,
    drop_slide: f64,
    last_timestamp: Option<f64>,
    frame_dt: f64,
    message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct BubbleProgress {
    version: u8,
    #[serde(default)]
    best_round: u32,
}

impl BubbleProgress {
    fn normalized(mut self) -> Self {
        self.version = CURRENT_PROGRESS_VERSION;
        self.best_round = self.best_round.min(99);
        self
    }
}

#[derive(Clone, Debug)]
struct ShopOption {
    upgrade: ShopUpgrade,
    title: &'static str,
    cost: u64,
    level: u32,
    active_level: u32,
    max_level: Option<u32>,
    owned: bool,
    enabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct ShopKitInfo {
    title: &'static str,
    upgrades: &'static [ShopUpgrade],
}

const SCOUT_KIT_UPGRADES: [ShopUpgrade; 5] = [
    ShopUpgrade::QueueSight,
    ShopUpgrade::BankSight,
    ShopUpgrade::Reticle,
    ShopUpgrade::LandingDot,
    ShopUpgrade::TraceSight,
];
const RISK_KIT_UPGRADES: [ShopUpgrade; 5] = [
    ShopUpgrade::SoftCeiling,
    ShopUpgrade::DropHaste,
    ShopUpgrade::PopDrop,
    ShopUpgrade::CleanStart,
    ShopUpgrade::ExtraRows,
];
const BREACH_KIT_UPGRADES: [ShopUpgrade; 4] = [
    ShopUpgrade::BombDrop,
    ShopUpgrade::BlastRadius,
    ShopUpgrade::LightningPower,
    ShopUpgrade::DrillPower,
];
const LUCK_KIT_UPGRADES: [ShopUpgrade; 4] = [
    ShopUpgrade::WildOrb,
    ShopUpgrade::WildCache,
    ShopUpgrade::PrizeBubbles,
    ShopUpgrade::PrizeQuality,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AbilitySnapshot {
    color_call_charges: u32,
    color_call_enabled: bool,
    color_sense_level: u8,
    color_sense_enabled: bool,
    bomb_shot_charges: u32,
    bomb_radius_level: u8,
    bomb_radius_enabled: bool,
    wild_shot_charges: u32,
    wild_cache_level: u8,
    wild_cache_enabled: bool,
    lightning_charges: u32,
    storm_focus_level: u8,
    storm_focus_enabled: bool,
    lightning_power_level: u8,
    lightning_power_enabled: bool,
    drill_charges: u32,
    drill_power_level: u8,
    drill_power_enabled: bool,
    drop_reset_charges: u32,
    drop_reserve_level: u8,
    drop_reserve_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbilityAction {
    ColorCall,
    BombShot,
    WildShot,
    Lightning,
    Drill,
    DropReset,
}

impl AbilityAction {
    fn help(self, ability: AbilitySnapshot) -> String {
        match self {
            Self::ColorCall => color_call_ability_help(ability),
            Self::BombShot => bomb_shot_ability_help(ability),
            Self::WildShot => wild_shot_ability_help(ability),
            Self::Lightning => lightning_ability_help(ability),
            Self::Drill => drill_ability_help(ability),
            Self::DropReset => drop_reset_ability_help(ability),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShopHit {
    Buy(usize),
    ToggleOwned(usize),
    BuyConsumable(usize),
    SelectTab(ShopTab),
    SelectKit(usize),
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShopTab {
    Upgrades,
    Owned,
    Consumables,
}

#[derive(Debug)]
struct ResumeOffer {
    saved_game: Option<SavedBubbleGame>,
    moves_remaining: u8,
}

impl ResumeOffer {
    fn new(saved_game: Option<SavedBubbleGame>) -> Self {
        Self {
            moves_remaining: saved_game.as_ref().map(|_| RESUME_OFFER_MOVES).unwrap_or(0),
            saved_game,
        }
    }

    fn is_visible(&self) -> bool {
        self.saved_game.is_some() && self.moves_remaining > 0
    }

    fn record_move(&mut self) {
        if self.saved_game.is_none() || self.moves_remaining == 0 {
            return;
        }
        self.moves_remaining -= 1;
        if self.moves_remaining == 0 {
            self.saved_game = None;
        }
    }

    fn take(&mut self) -> Option<SavedBubbleGame> {
        self.moves_remaining = 0;
        self.saved_game.take()
    }
}

/// A fresh seed each page load so the starting board is random every time.
fn random_seed() -> u64 {
    let bits = (js_sys::Math::random() * u64::MAX as f64) as u64;
    bits ^ (js_sys::Date::now() as u64)
}

impl CanvasRunner {
    fn new(high_score: u64, hard: bool, saved_game: Option<SavedBubbleGame>) -> Self {
        let progress = load_progress();
        Self {
            game: if hard {
                BubbleGame::hard(random_seed(), high_score)
            } else {
                BubbleGame::new(random_seed(), high_score)
            },
            hard,
            resume_offer: ResumeOffer::new(saved_game),
            progress,
            aim_angle: -PI / 2.0,
            projectile: None,
            particles: Vec::new(),
            shop_tab: ShopTab::Upgrades,
            shop_kit_page: 0,
            drop_slide: 0.0,
            last_timestamp: None,
            frame_dt: 1.0 / 60.0,
            message: "Aim and shoot. Match three or more.".to_string(),
        }
    }

    fn resume_offer_visible(&self) -> bool {
        self.resume_offer.is_visible()
    }

    fn status_text(&self) -> String {
        let held = self.game.held.map(color_name).unwrap_or("empty");
        format!(
            "Score {}. Best {}. Drop in {}. Current bubble {}. Held {}. {}",
            self.game.score,
            self.game.high_score,
            self.game.shots_until_drop,
            color_name(self.game.current_color()),
            held,
            self.message
        )
    }

    fn ability_snapshot(&self) -> AbilitySnapshot {
        AbilitySnapshot {
            color_call_charges: self.game.run.color_call_charges,
            color_call_enabled: self.game.run.color_call_enabled,
            color_sense_level: self.game.run.color_sense_level,
            color_sense_enabled: self.game.run.color_sense_enabled,
            bomb_shot_charges: self.game.run.bomb_shot_charges,
            bomb_radius_level: self.game.run.active_bomb_radius_level(),
            bomb_radius_enabled: self.game.run.active_bomb_radius_level() > 0,
            wild_shot_charges: self.game.run.wild_shot_charges,
            wild_cache_level: self.game.run.active_wild_cache_level(),
            wild_cache_enabled: self.game.run.active_wild_cache_level() > 0,
            lightning_charges: self.game.run.lightning_charges,
            storm_focus_level: self.game.run.storm_focus_level,
            storm_focus_enabled: self.game.run.storm_focus_enabled,
            lightning_power_level: self.game.run.active_lightning_power_level(),
            lightning_power_enabled: self.game.run.active_lightning_power_level() > 0,
            drill_charges: self.game.run.drill_charges,
            drill_power_level: self.game.run.active_drill_power_level(),
            drill_power_enabled: self.game.run.active_drill_power_level() > 0,
            drop_reset_charges: self.game.run.drop_reset_charges,
            drop_reserve_level: self.game.run.drop_reserve_level,
            drop_reserve_enabled: self.game.run.drop_reserve_enabled,
        }
    }

    fn visible_next_colors(&self) -> Vec<BubbleColor> {
        let mut next = self.game.next_colors();
        let queue_level = self.game.run.active_queue_sight_level();
        let scout_bonus = queue_level > 0
            && self.game.run.active_bank_sight_level() > 0
            && self.game.run.active_reticle_level() > 0
            && self.game.run.active_landing_dot_level() > 0
            && self.game.run.active_trace_sight_level() > 0;
        let visible = 2 + queue_level.min(3) as usize + usize::from(scout_bonus);
        next.truncate(visible);
        next
    }

    fn resume_saved(&mut self) {
        let high_score = load_high_score();
        let Some(saved_game) = self.resume_offer.take() else {
            return;
        };

        let stored_game = load_raw_saved_game(self.hard);
        if !stored_save_allows_resume(stored_game.as_ref(), &saved_game, &self.game.save_state()) {
            self.message = "Saved board is no longer available.".to_string();
            self.game.high_score = high_score.max(self.game.score);
            return;
        }

        let Some(game) = BubbleGame::from_save(saved_game, high_score, self.hard) else {
            return;
        };

        self.game = game;
        self.projectile = None;
        self.particles.clear();
        self.drop_slide = 0.0;
        self.progress = load_progress();
        self.message = "Resumed saved board.".to_string();
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
    }

    fn finish_move(&mut self) {
        self.resume_offer.record_move();
        self.game.high_score = load_high_score().max(self.game.score);
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
    }

    fn hold(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        self.game.hold();
        self.message = "Held a bubble. Tap hold, right-click, or H to swap.".to_string();
        self.finish_move();
    }

    /// A tap on the hold slot swaps the held bubble (the only way to hold on a
    /// touchscreen); anywhere else aims and shoots.
    fn tap(&mut self, client_x: f64, client_y: f64, canvas: &HtmlCanvasElement) {
        let layout = layout_for_canvas(canvas);
        let rect = canvas.get_bounding_client_rect();
        let x = client_x - rect.left();
        let y = client_y - rect.top();

        if self.shop_visible() {
            let offer_count = self.shop_kit_options().len();
            self.handle_shop_hit(shop_hit(
                layout,
                x,
                y,
                offer_count,
                self.owned_shop_options().len(),
                self.consumable_shop_options().len(),
                self.shop_tab,
                self.shop_kit_page,
            ));
            return;
        }

        let controls = cannon_controls(layout);
        if distance(x, y, controls.hold_x, controls.hold_y) <= hold_hit_radius(controls) {
            self.hold();
        } else {
            self.aim_from_client(client_x, client_y, canvas);
            self.shoot_or_restart();
        }
    }

    fn aim_from_client(&mut self, client_x: f64, client_y: f64, canvas: &HtmlCanvasElement) {
        let layout = layout_for_canvas(canvas);
        let rect = canvas.get_bounding_client_rect();
        let x = client_x - rect.left();
        let y = client_y - rect.top();
        let angle = (y - layout.cannon_y).atan2(x - layout.cannon_x);
        self.aim_angle = clamp_aim(angle);
    }

    fn pointer_move(&mut self, client_x: f64, client_y: f64, canvas: &HtmlCanvasElement) {
        let layout = layout_for_canvas(canvas);
        let rect = canvas.get_bounding_client_rect();
        let x = client_x - rect.left();
        let y = client_y - rect.top();

        if self.shop_visible() {
            return;
        }

        let angle = (y - layout.cannon_y).atan2(x - layout.cannon_x);
        self.aim_angle = clamp_aim(angle);
    }

    fn nudge_aim(&mut self, delta: f64) {
        self.aim_angle = clamp_aim(self.aim_angle + delta);
    }

    fn shoot_or_restart(&mut self) {
        if self.shop_visible() {
            self.continue_from_shop();
            return;
        }

        if self.game.game_over {
            let high_score = self.game.high_score;
            self.progress = load_progress();
            self.game.restart();
            self.game.high_score = high_score;
            self.projectile = None;
            self.message = "New board. Aim and shoot.".to_string();
            self.finish_move();
            return;
        }

        if self.projectile.is_some() {
            return;
        }

        let speed = 560.0;
        // Take this shot's color and advance the queue now, so the cannon shows
        // the next bubble while this one is still in flight.
        let color = self.game.pop_next();
        self.projectile = Some(Projectile {
            x: 0.0,
            y: 0.0,
            vx: self.aim_angle.cos() * speed,
            vy: self.aim_angle.sin() * speed,
            color,
        });
    }

    fn shop_visible(&self) -> bool {
        self.game.game_over && self.game.board.is_cleared()
    }

    fn ensure_shop_reward(&mut self) {
        if !self.shop_visible() {
            return;
        }

        let Some((stage, deposited)) = self.game.award_shop_for_current_round() else {
            return;
        };

        if stage > self.progress.best_round {
            self.progress.best_round = stage;
            save_progress(&self.progress);
        }

        self.message = format!("Shop is open. Banked {} score.", deposited);
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
    }

    fn select_shop_option(&mut self, index: usize) {
        if !self.shop_visible() {
            return;
        }
        self.ensure_shop_reward();
        let Some(option) = self.shop_kit_options().into_iter().nth(index) else {
            return;
        };
        if option
            .max_level
            .is_some_and(|max_level| option.level >= max_level)
        {
            self.message = format!(
                "{} is already maxed. Tap its chip to change it.",
                option.title
            );
            return;
        }
        if self.game.shop_score() < option.cost {
            self.message = format!("Need {} score for {}.", option.cost, option.title);
            return;
        }

        if self
            .game
            .purchase_upgrade(option.upgrade, option.cost)
            .is_none()
        {
            return;
        }
        save_game(self.hard, &self.game);
    }

    fn handle_shop_hit(&mut self, hit: Option<ShopHit>) {
        match hit {
            Some(ShopHit::Buy(index)) => self.select_shop_option(index),
            Some(ShopHit::ToggleOwned(index)) => self.toggle_owned_shop_option(index),
            Some(ShopHit::BuyConsumable(index)) => self.buy_consumable_shop_option(index),
            Some(ShopHit::SelectTab(tab)) => {
                self.shop_tab = tab;
            }
            Some(ShopHit::SelectKit(index)) => {
                self.shop_tab = ShopTab::Upgrades;
                self.shop_kit_page = index.min(shop_kits().len().saturating_sub(1));
            }
            Some(ShopHit::Continue) => self.continue_from_shop(),
            None => {}
        }
    }

    fn toggle_owned_shop_option(&mut self, index: usize) {
        if !self.shop_visible() {
            return;
        }
        let Some(option) = self.owned_shop_options().into_iter().nth(index) else {
            return;
        };
        self.game.cycle_upgrade_level(option.upgrade);
        save_game(self.hard, &self.game);
    }

    fn buy_consumable_shop_option(&mut self, index: usize) {
        if !self.shop_visible() {
            return;
        }
        self.ensure_shop_reward();
        let Some(option) = self.consumable_shop_options().into_iter().nth(index) else {
            return;
        };
        if self.game.shop_score() < option.cost {
            self.message = format!("Need {} score for {}.", option.cost, option.title);
            return;
        }
        if self
            .game
            .purchase_upgrade(option.upgrade, option.cost)
            .is_none()
        {
            return;
        }
        save_game(self.hard, &self.game);
    }

    fn continue_from_shop(&mut self) {
        if !self.shop_visible() {
            return;
        }
        self.ensure_shop_reward();
        self.game.start_next_round();
        self.projectile = None;
        self.particles.clear();
        self.drop_slide = 0.0;
        self.aim_angle = -PI / 2.0;
        self.message = "New board. Aim and shoot.".to_string();
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
    }

    fn call_color(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if !self.game.run.color_call_enabled || self.game.run.color_call_charges == 0 {
            self.message = "No Color Call ready.".to_string();
            return;
        }
        match self.game.call_color() {
            Some(color) => {
                self.message = format!(
                    "Color call loaded {}. {} charges left.",
                    color_name(color),
                    self.game.run.color_call_charges
                );
                save_game(self.hard, &self.game);
            }
            None => {
                self.message = "No Color Call charges.".to_string();
            }
        }
    }

    fn load_bomb_shot(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if self.game.load_bomb_shot() {
            self.message = format!(
                "Bomb shot loaded. {} left.",
                self.game.run.bomb_shot_charges
            );
            save_game(self.hard, &self.game);
        } else {
            self.message = "No bomb shots ready.".to_string();
        }
    }

    fn load_wild_shot(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if self.game.load_wild_shot() {
            self.message = format!(
                "Wild shot loaded. {} left.",
                self.game.run.wild_shot_charges
            );
            save_game(self.hard, &self.game);
        } else {
            self.message = "No wild shots ready.".to_string();
        }
    }

    fn load_lightning_shot(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if self.game.load_lightning_shot() {
            self.message = format!(
                "Lightning loaded. {} left.",
                self.game.run.lightning_charges
            );
            save_game(self.hard, &self.game);
        } else {
            self.message = "No lightning shots ready.".to_string();
        }
    }

    fn load_drill_shot(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if self.game.load_drill_shot() {
            self.message = format!("Drill loaded. {} left.", self.game.run.drill_charges);
            save_game(self.hard, &self.game);
        } else {
            self.message = "No drill shots ready.".to_string();
        }
    }

    fn use_drop_reset(&mut self) {
        if self.game.game_over || self.projectile.is_some() {
            return;
        }
        if self.game.reset_drop_counter() {
            self.message = format!(
                "Drop reset. Drop in {}. {} left.",
                self.game.shots_until_drop, self.game.run.drop_reset_charges
            );
            save_game(self.hard, &self.game);
        } else {
            self.message = "No drop resets ready.".to_string();
        }
    }

    #[cfg(debug_assertions)]
    fn debug_jump_to_shop(&mut self) {
        self.game.board = Board::empty();
        self.game.game_over = true;
        self.projectile = None;
        self.particles.clear();
        self.drop_slide = 0.0;
        self.ensure_shop_reward();
        self.message = "Debug: jumped to shop.".to_string();
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
    }

    #[cfg(debug_assertions)]
    fn debug_add_score(&mut self) {
        self.game.add_shop_score(10_000);
        save_game(self.hard, &self.game);
        save_high_score(self.game.high_score);
        self.message = format!("Debug: score {}.", self.game.shop_score());
    }

    fn shop_kit_options(&self) -> Vec<ShopOption> {
        kit_shop_upgrades()
            .into_iter()
            .map(|upgrade| self.shop_option(upgrade))
            .collect()
    }

    fn owned_shop_options(&self) -> Vec<ShopOption> {
        permanent_shop_upgrades()
            .into_iter()
            .map(|upgrade| self.shop_option(upgrade))
            .filter(|option| option.owned)
            .collect()
    }

    fn consumable_shop_options(&self) -> Vec<ShopOption> {
        [
            ShopUpgrade::ColorCall,
            ShopUpgrade::DropReset,
            ShopUpgrade::BombShot,
            ShopUpgrade::WildShot,
            ShopUpgrade::LightningBolt,
            ShopUpgrade::DrillShot,
            ShopUpgrade::Revival,
        ]
        .into_iter()
        .map(|upgrade| self.shop_option(upgrade))
        .collect()
    }

    fn shop_option(&self, upgrade: ShopUpgrade) -> ShopOption {
        match upgrade {
            ShopUpgrade::ColorCall => ShopOption {
                upgrade,
                title: "Color Call",
                cost: 400,
                level: self.game.run.color_call_charges,
                active_level: self.game.run.color_call_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.color_call_charges > 0,
                enabled: self.game.run.color_call_enabled,
            },
            ShopUpgrade::WildOrb => ShopOption {
                upgrade,
                title: "Queue Wilds",
                cost: 1200 + self.game.run.wild_orb_level as u64 * 600,
                level: self.game.run.wild_orb_level as u32,
                active_level: self.game.run.active_wild_orb_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.wild_orb_level > 0,
                enabled: self.game.run.active_wild_orb_level() > 0,
            },
            ShopUpgrade::WildCache => ShopOption {
                upgrade,
                title: "Wild Spread",
                cost: 1700 + self.game.run.wild_cache_level as u64 * 800,
                level: self.game.run.wild_cache_level as u32,
                active_level: self.game.run.active_wild_cache_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.wild_cache_level > 0,
                enabled: self.game.run.active_wild_cache_level() > 0,
            },
            ShopUpgrade::BombDrop => ShopOption {
                upgrade,
                title: "Fuse Rows",
                cost: 1400 + self.game.run.bomb_drop_level as u64 * 600,
                level: self.game.run.bomb_drop_level as u32,
                active_level: self.game.run.active_bomb_drop_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.bomb_drop_level > 0,
                enabled: self.game.run.active_bomb_drop_level() > 0,
            },
            ShopUpgrade::BlastRadius => ShopOption {
                upgrade,
                title: "Blast Radius",
                cost: 1800 + self.game.run.bomb_radius_level as u64 * 900,
                level: self.game.run.bomb_radius_level as u32,
                active_level: self.game.run.active_bomb_radius_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.bomb_radius_level > 0,
                enabled: self.game.run.active_bomb_radius_level() > 0,
            },
            ShopUpgrade::BombShot => ShopOption {
                upgrade,
                title: "Bomb Shot",
                cost: 900,
                level: self.game.run.bomb_shot_charges,
                active_level: self.game.run.bomb_shot_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.bomb_shot_charges > 0,
                enabled: self.game.run.bomb_shot_charges > 0,
            },
            ShopUpgrade::WildShot => ShopOption {
                upgrade,
                title: "Wild Shot",
                cost: 1000,
                level: self.game.run.wild_shot_charges,
                active_level: self.game.run.wild_shot_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.wild_shot_charges > 0,
                enabled: self.game.run.wild_shot_charges > 0,
            },
            ShopUpgrade::LightningBolt => ShopOption {
                upgrade,
                title: "Lightning",
                cost: 1400,
                level: self.game.run.lightning_charges,
                active_level: self.game.run.lightning_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.lightning_charges > 0,
                enabled: self.game.run.lightning_charges > 0,
            },
            ShopUpgrade::DrillShot => ShopOption {
                upgrade,
                title: "Drill Shot",
                cost: 1600,
                level: self.game.run.drill_charges,
                active_level: self.game.run.drill_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.drill_charges > 0,
                enabled: self.game.run.drill_charges > 0,
            },
            ShopUpgrade::DropReset => ShopOption {
                upgrade,
                title: "Drop Reset",
                cost: DROP_RESET_COST,
                level: self.game.run.drop_reset_charges,
                active_level: self.game.run.drop_reset_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.drop_reset_charges > 0,
                enabled: self.game.run.drop_reset_charges > 0,
            },
            ShopUpgrade::SoftCeiling => ShopOption {
                upgrade,
                title: "Soft Ceiling",
                cost: 900 + self.game.run.drop_delay_level as u64 * 400,
                level: self.game.run.drop_delay_level as u32,
                active_level: self.game.run.active_drop_delay_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.drop_delay_level > 0,
                enabled: self.game.run.active_drop_delay_level() > 0,
            },
            ShopUpgrade::DropHaste => ShopOption {
                upgrade,
                title: "Fast Drops",
                cost: 900 + self.game.run.drop_haste_level as u64 * 450,
                level: self.game.run.drop_haste_level as u32,
                active_level: self.game.run.active_drop_haste_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.drop_haste_level > 0,
                enabled: self.game.run.active_drop_haste_level() > 0,
            },
            ShopUpgrade::PopDrop => ShopOption {
                upgrade,
                title: "Pop Pressure",
                cost: 1600,
                level: self.game.run.pop_drop_level as u32,
                active_level: self.game.run.active_pop_drop_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.pop_drop_level > 0,
                enabled: self.game.run.active_pop_drop_level() > 0,
            },
            ShopUpgrade::CleanStart => ShopOption {
                upgrade,
                title: "Clean Start",
                cost: 1500 + self.game.run.clean_start_level as u64 * 800,
                level: self.game.run.clean_start_level as u32,
                active_level: self.game.run.active_clean_start_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.clean_start_level > 0,
                enabled: self.game.run.active_clean_start_level() > 0,
            },
            ShopUpgrade::ExtraRows => ShopOption {
                upgrade,
                title: "More Rows",
                cost: 1200 + self.game.run.extra_rows_level as u64 * 700,
                level: self.game.run.extra_rows_level as u32,
                active_level: self.game.run.active_extra_rows_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.extra_rows_level > 0,
                enabled: self.game.run.active_extra_rows_level() > 0,
            },
            ShopUpgrade::QueueSight => ShopOption {
                upgrade,
                title: "Queue Sight",
                cost: 650 + self.game.run.queue_sight_level as u64 * 350,
                level: self.game.run.queue_sight_level as u32,
                active_level: self.game.run.active_queue_sight_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.queue_sight_level > 0,
                enabled: self.game.run.active_queue_sight_level() > 0,
            },
            ShopUpgrade::TraceSight => ShopOption {
                upgrade,
                title: "Ghost Bubble",
                cost: 950,
                level: self.game.run.trace_sight_level as u32,
                active_level: self.game.run.active_trace_sight_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.trace_sight_level > 0,
                enabled: self.game.run.active_trace_sight_level() > 0,
            },
            ShopUpgrade::Reticle => ShopOption {
                upgrade,
                title: "Aim Guide",
                cost: 700 + self.game.run.reticle_level as u64 * 450,
                level: self.game.run.reticle_level as u32,
                active_level: self.game.run.active_reticle_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.reticle_level > 0,
                enabled: self.game.run.active_reticle_level() > 0,
            },
            ShopUpgrade::LandingDot => ShopOption {
                upgrade,
                title: "Landing Dot",
                cost: 850,
                level: self.game.run.landing_dot_level as u32,
                active_level: self.game.run.active_landing_dot_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.landing_dot_level > 0,
                enabled: self.game.run.active_landing_dot_level() > 0,
            },
            ShopUpgrade::PrizeBubbles => ShopOption {
                upgrade,
                title: "Prize Bubbles",
                cost: 1500 + self.game.run.prize_bubble_level as u64 * 650,
                level: self.game.run.prize_bubble_level as u32,
                active_level: self.game.run.active_prize_bubble_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.prize_bubble_level > 0,
                enabled: self.game.run.active_prize_bubble_level() > 0,
            },
            ShopUpgrade::PrizeQuality => ShopOption {
                upgrade,
                title: "Prize Quality",
                cost: 1300 + self.game.run.prize_quality_level as u64 * 700,
                level: self.game.run.prize_quality_level as u32,
                active_level: self.game.run.active_prize_quality_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.prize_quality_level > 0,
                enabled: self.game.run.active_prize_quality_level() > 0,
            },
            ShopUpgrade::Revival => ShopOption {
                upgrade,
                title: "Second Breath",
                cost: 2000,
                level: self.game.run.revive_charges,
                active_level: self.game.run.revive_charges,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.revive_charges > 0,
                enabled: self.game.run.revive_enabled,
            },
            ShopUpgrade::LightningPower => ShopOption {
                upgrade,
                title: "Storm Channel",
                cost: 1700 + self.game.run.lightning_power_level as u64 * 800,
                level: self.game.run.lightning_power_level as u32,
                active_level: self.game.run.active_lightning_power_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.lightning_power_level > 0,
                enabled: self.game.run.active_lightning_power_level() > 0,
            },
            ShopUpgrade::DrillPower => ShopOption {
                upgrade,
                title: "Deep Drill",
                cost: 1500 + self.game.run.drill_power_level as u64 * 700,
                level: self.game.run.drill_power_level as u32,
                active_level: self.game.run.active_drill_power_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.drill_power_level > 0,
                enabled: self.game.run.active_drill_power_level() > 0,
            },
            ShopUpgrade::BankSight => ShopOption {
                upgrade,
                title: "Bank Preview",
                cost: 700 + self.game.run.bank_sight_level as u64 * 300,
                level: self.game.run.bank_sight_level as u32,
                active_level: self.game.run.active_bank_sight_level() as u32,
                max_level: upgrade.max_level().map(u32::from),
                owned: self.game.run.bank_sight_level > 0,
                enabled: self.game.run.active_bank_sight_level() > 0,
            },
        }
    }

    fn update(&mut self, timestamp: f64) {
        if let Some(last_timestamp) = self.last_timestamp {
            self.frame_dt = ((timestamp - last_timestamp) / 1000.0).clamp(1.0 / 120.0, 1.0 / 30.0);
        }
        self.last_timestamp = Some(timestamp);
    }

    fn render(&mut self, canvas: &HtmlCanvasElement) {
        let layout = resize_canvas(canvas);
        if let Some(projectile) = &mut self.projectile {
            if projectile.x == 0.0 && projectile.y == 0.0 {
                projectile.x = layout.cannon_x;
                projectile.y = layout.cannon_y;
            }
        }

        self.step_projectile(layout);
        self.step_animations();
        self.ensure_shop_reward();

        let Some(context) = canvas_context(canvas) else {
            return;
        };

        // Slide newly dropped rows down from one row up as the animation eases out.
        let row_offset = -layout.row_gap * (self.drop_slide / DROP_SLIDE);

        let shop_visible = self.shop_visible();

        draw_background(&context, layout);
        draw_header(&context, layout, &self.game, &self.message, !shop_visible);
        draw_aim(
            &context,
            layout,
            &self.game.board,
            self.aim_angle,
            self.game.current_color(),
            self.game.run.active_trace_sight_level(),
            1 + self.game.run.active_reticle_level(),
            self.game.run.active_landing_dot_level(),
            self.game.run.active_bank_sight_level(),
        );
        draw_board(&context, layout, &self.game.board, row_offset);
        draw_particles(&context, layout, &self.particles);
        draw_cannon(
            &context,
            layout,
            self.aim_angle,
            self.game.current_color(),
            self.visible_next_colors(),
            self.game.held,
        );

        if let Some(projectile) = self.projectile {
            draw_bubble(
                &context,
                projectile.x,
                projectile.y,
                layout.radius,
                projectile.color,
                1.0,
            );
        }

        if self.game.game_over {
            if self.game.board.is_cleared() {
                draw_shop_overlay(
                    &context,
                    layout,
                    &self.game,
                    &self.progress,
                    self.shop_kit_options(),
                    self.owned_shop_options(),
                    self.consumable_shop_options(),
                    self.shop_tab,
                    self.shop_kit_page,
                );
            } else {
                draw_overlay(&context, layout, false, self.game.score);
            }
        }
    }

    fn step_animations(&mut self) {
        let dt = self.frame_dt;
        self.drop_slide = (self.drop_slide - dt).max(0.0);
        for particle in &mut self.particles {
            if particle.delay > 0.0 {
                particle.delay -= dt;
                continue;
            }
            if matches!(particle.kind, ParticleKind::Bubble) {
                particle.vy += GRAVITY * dt;
            }
            particle.x += particle.vx * dt;
            particle.y += particle.vy * dt;
            particle.life -= dt;
        }
        self.particles
            .retain(|particle| particle.life > 0.0 || particle.delay > 0.0);
    }

    fn step_projectile(&mut self, layout: BubbleLayout) {
        let Some(mut projectile) = self.projectile.take() else {
            return;
        };

        let dt = self.frame_dt;
        let speed = (projectile.vx.powi(2) + projectile.vy.powi(2)).sqrt();
        let substeps = ((speed * dt) / (layout.radius * SHOT_CAST_STEP_RADIUS))
            .ceil()
            .max(1.0) as usize;
        let step_dt = dt / substeps as f64;

        for _ in 0..substeps {
            projectile.x += projectile.vx * step_dt;
            projectile.y += projectile.vy * step_dt;

            let left_wall = layout.left + layout.radius;
            let right_wall = layout.width - layout.left - layout.radius;
            if projectile.x <= left_wall {
                projectile.x = left_wall;
                projectile.vx = projectile.vx.abs();
            } else if projectile.x >= right_wall {
                projectile.x = right_wall;
                projectile.vx = -projectile.vx.abs();
            }

            if projectile.y <= layout.top + layout.radius {
                self.attach_projectile(projectile, layout, AttachmentTarget::Ceiling);
                return;
            }

            if let Some(hit) =
                hit_existing_bubble(&self.game.board, layout, projectile.x, projectile.y)
            {
                self.attach_projectile(projectile, layout, AttachmentTarget::Bubble(hit));
                return;
            }
        }

        self.projectile = Some(projectile);
    }

    fn attach_projectile(
        &mut self,
        projectile: Projectile,
        layout: BubbleLayout,
        target: AttachmentTarget,
    ) {
        let drill_cells = if projectile.color == BubbleColor::Drill {
            let anchor = match target {
                AttachmentTarget::Bubble(anchor) => Some(anchor),
                AttachmentTarget::Ceiling => None,
            };
            Some(drill_path_cells(
                &self.game.board,
                layout,
                projectile.x,
                projectile.y,
                projectile.vx,
                projectile.vy,
                anchor,
                self.game.run.drill_depth(),
            ))
        } else {
            None
        };
        // No room to land: discard the shot rather than ending the game. Losing
        // only happens when a dropped row overflows past the bottom.
        let coord = if drill_cells.is_some() {
            None
        } else {
            match attachment_cell(&self.game.board, layout, projectile.x, projectile.y, target) {
                Some(coord) => Some(coord),
                None => {
                    self.projectile = None;
                    self.message = "No room to land. Shot discarded.".to_string();
                    self.game.finish_discarded_shot();
                    self.finish_move();
                    return;
                }
            }
        };

        let parity_before = self.game.board.parity();
        let before_score = self.game.score;
        let resolution = if let Some(cells) = drill_cells {
            self.game.drill_through(cells)
        } else {
            self.game.attach_color(
                coord.expect("non-drill shot has an attachment cell"),
                projectile.color,
            )
        };
        let gained = self.game.score.saturating_sub(before_score);

        self.spawn_pop_particles(layout, &resolution);
        // A parity flip means a new row was pushed in: ease it down.
        if self.game.board.parity() != parity_before {
            self.drop_slide = DROP_SLIDE;
        }

        self.message = if resolution.popped.is_empty() {
            format!("Attached. Drop in {} shots.", self.game.shots_until_drop)
        } else {
            let score = score_breakdown(&resolution);
            format!(
                "Popped {} ({}); Dropped {} ({}); +{}{}",
                resolution.popped.len(),
                score.pop,
                resolution.dropped.len(),
                score.drop,
                gained,
                prize_message(&resolution.prizes)
            )
        };

        self.projectile = None;
        self.finish_move();
    }

    fn spawn_pop_particles(&mut self, layout: BubbleLayout, resolution: &Resolution) {
        // Popped bubbles burst into shards; dropped clusters fall away whole.
        if resolution
            .popped
            .iter()
            .any(|(_, color)| *color == BubbleColor::Lightning)
        {
            self.spawn_lightning_path(layout, resolution);
        }
        for (coord, color) in &resolution.popped {
            let (x, y) = layout.center(*coord, self.game.board.parity());
            if *color != BubbleColor::Lightning {
                for shard in 0..6 {
                    let angle = shard as f64 / 6.0 * std::f64::consts::TAU;
                    let speed = 90.0 + (shard as f64 * 17.0) % 60.0;
                    self.particles.push(Particle {
                        x,
                        y,
                        vx: angle.cos() * speed,
                        vy: angle.sin() * speed - 40.0,
                        radius: layout.radius * 0.34,
                        color: *color,
                        kind: ParticleKind::Bubble,
                        delay: 0.0,
                        life: 0.50,
                        max_life: 0.50,
                    });
                }
            }
            match *color {
                BubbleColor::Bomb => self.spawn_bomb_burst(layout, x, y),
                BubbleColor::Lightning => self.spawn_lightning_burst(layout, x, y),
                BubbleColor::Drill => self.spawn_drill_burst(layout, x, y),
                BubbleColor::Wild | BubbleColor::Prize => {
                    self.spawn_glint_burst(layout, x, y, *color)
                }
                _ => {}
            }
        }
        for (coord, color) in &resolution.dropped {
            let (x, y) = layout.center(*coord, self.game.board.parity());
            self.particles.push(Particle {
                x,
                y,
                vx: 0.0,
                vy: 40.0,
                radius: layout.radius,
                color: *color,
                kind: ParticleKind::Bubble,
                delay: 0.0,
                life: 0.82,
                max_life: 0.82,
            });
        }
    }

    fn spawn_bomb_burst(&mut self, layout: BubbleLayout, x: f64, y: f64) {
        self.particles.push(Particle {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            radius: layout.radius * 1.75,
            color: BubbleColor::Bomb,
            kind: ParticleKind::Ring,
            delay: 0.0,
            life: 0.82,
            max_life: 0.82,
        });
        for shard in 0..12 {
            let angle = shard as f64 / 12.0 * std::f64::consts::TAU;
            let speed = 170.0 + (shard as f64 % 4.0) * 24.0;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                radius: layout.radius * 0.28,
                color: BubbleColor::Gold,
                kind: ParticleKind::Spark,
                delay: 0.0,
                life: 0.58,
                max_life: 0.58,
            });
        }
    }

    fn spawn_lightning_burst(&mut self, layout: BubbleLayout, x: f64, y: f64) {
        self.particles.push(Particle {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            radius: layout.radius * 1.2,
            color: BubbleColor::Lightning,
            kind: ParticleKind::Ring,
            delay: 0.0,
            life: 0.72,
            max_life: 0.72,
        });
        for shard in 0..8 {
            let angle = shard as f64 / 8.0 * std::f64::consts::TAU;
            let speed = 150.0 + (shard as f64 % 3.0) * 30.0;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                radius: layout.radius * 0.34,
                color: BubbleColor::Lightning,
                kind: ParticleKind::Spark,
                delay: 0.0,
                life: 0.62,
                max_life: 0.62,
            });
        }
    }

    fn spawn_drill_burst(&mut self, layout: BubbleLayout, x: f64, y: f64) {
        self.particles.push(Particle {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            radius: layout.radius,
            color: BubbleColor::Drill,
            kind: ParticleKind::Ring,
            delay: 0.0,
            life: 0.58,
            max_life: 0.58,
        });
        for shard in 0..7 {
            let angle = -std::f64::consts::FRAC_PI_2 + (shard as f64 - 3.0) * 0.28;
            let speed = 120.0 + shard as f64 * 12.0;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                radius: layout.radius * 0.24,
                color: BubbleColor::Drill,
                kind: ParticleKind::Spark,
                delay: 0.0,
                life: 0.48,
                max_life: 0.48,
            });
        }
    }

    fn spawn_lightning_path(&mut self, layout: BubbleLayout, resolution: &Resolution) {
        self.particles.push(Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            radius: 0.0,
            color: BubbleColor::Lightning,
            kind: ParticleKind::ScreenFlash,
            delay: 0.0,
            life: 0.26,
            max_life: 0.26,
        });

        let cells = ordered_lightning_cells(
            &self.game.board,
            resolution
                .popped
                .iter()
                .find_map(|(coord, color)| (*color == BubbleColor::Lightning).then_some(*coord)),
            resolution.popped.iter().map(|(coord, _)| *coord).collect(),
        );

        for (index, pair) in cells.windows(2).enumerate() {
            let (x, y) = layout.center(pair[0], self.game.board.parity());
            let (target_x, target_y) = layout.center(pair[1], self.game.board.parity());
            let delay = index as f64 * 0.032;
            self.particles.push(Particle {
                x,
                y,
                vx: target_x,
                vy: target_y,
                radius: layout.radius,
                color: BubbleColor::Lightning,
                kind: ParticleKind::LightningArc,
                delay,
                life: 0.46,
                max_life: 0.46,
            });
            for spark in 0..3 {
                let t = (spark as f64 + 1.0) / 4.0;
                let spark_x = x + (target_x - x) * t;
                let spark_y = y + (target_y - y) * t;
                let angle = (target_y - y).atan2(target_x - x)
                    + std::f64::consts::FRAC_PI_2
                    + (spark as f64 - 1.0) * 0.72;
                let speed = 125.0 + spark as f64 * 22.0;
                self.particles.push(Particle {
                    x: spark_x,
                    y: spark_y,
                    vx: angle.cos() * speed,
                    vy: angle.sin() * speed,
                    radius: layout.radius * 0.16,
                    color: BubbleColor::Lightning,
                    kind: ParticleKind::Spark,
                    delay: delay + 0.012,
                    life: 0.30,
                    max_life: 0.30,
                });
            }
        }
    }

    fn spawn_glint_burst(&mut self, layout: BubbleLayout, x: f64, y: f64, color: BubbleColor) {
        for shard in 0..5 {
            let angle = shard as f64 / 5.0 * std::f64::consts::TAU;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * 95.0,
                vy: angle.sin() * 95.0,
                radius: layout.radius * 0.18,
                color,
                kind: ParticleKind::Spark,
                delay: 0.0,
                life: 0.36,
                max_life: 0.36,
            });
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum AttachmentTarget {
    Ceiling,
    Bubble(CellCoord),
}

fn ordered_lightning_cells(
    board: &Board,
    start: Option<CellCoord>,
    cells: Vec<CellCoord>,
) -> Vec<CellCoord> {
    let mut remaining = cells.into_iter().collect::<BTreeSet<_>>();
    let Some(mut current) = start.filter(|coord| remaining.contains(coord)).or_else(|| {
        remaining
            .iter()
            .max_by_key(|coord| (coord.row, coord.col))
            .copied()
    }) else {
        return Vec::new();
    };

    let mut ordered = Vec::with_capacity(remaining.len());
    remaining.remove(&current);
    ordered.push(current);

    while !remaining.is_empty() {
        let next = board
            .neighbors(current)
            .into_iter()
            .filter(|coord| remaining.contains(coord))
            .min_by_key(|coord| lightning_neighbor_rank(current, *coord))
            .or_else(|| {
                remaining
                    .iter()
                    .copied()
                    .min_by_key(|coord| lightning_fallback_rank(current, *coord))
            })
            .expect("remaining lightning cells are non-empty");
        remaining.remove(&next);
        ordered.push(next);
        current = next;
    }

    ordered
}

fn lightning_neighbor_rank(current: CellCoord, candidate: CellCoord) -> (u8, usize, usize, usize) {
    let direction = if candidate.row < current.row {
        0
    } else if candidate.row == current.row {
        1
    } else {
        2
    };
    (
        direction,
        candidate.row,
        current.col.abs_diff(candidate.col),
        candidate.col,
    )
}

fn lightning_fallback_rank(current: CellCoord, candidate: CellCoord) -> (u8, usize, usize, usize) {
    let direction = if candidate.row < current.row {
        0
    } else if candidate.row == current.row {
        1
    } else {
        2
    };
    (
        direction,
        current.row.abs_diff(candidate.row) + current.col.abs_diff(candidate.col),
        candidate.row,
        candidate.col,
    )
}

fn hit_existing_bubble(board: &Board, layout: BubbleLayout, x: f64, y: f64) -> Option<CellCoord> {
    board
        .occupied_cells()
        .filter_map(|coord| {
            let (cx, cy) = layout.center(coord, board.parity());
            let hit_distance = distance(x, y, cx, cy);
            // Below 2r (edge contact) so a shot can squeeze a little into gaps.
            (hit_distance <= layout.radius * COLLISION_TOLERANCE).then_some((coord, hit_distance))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(coord, _)| coord)
}

fn drill_path_cells(
    board: &Board,
    layout: BubbleLayout,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    anchor: Option<CellCoord>,
    depth: usize,
) -> Vec<CellCoord> {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed <= f64::EPSILON {
        return anchor.into_iter().collect();
    }

    let dir_x = vx / speed;
    let dir_y = vy / speed;
    let limit = depth.max(1);
    let overlap_radius = layout.radius * COLLISION_TOLERANCE;
    let step = (layout.radius * 0.22).max(2.0);
    let max_distance = (layout.width + layout.height) * 1.6;
    let mut seen = BTreeSet::new();
    let mut cells = Vec::new();
    if let Some(anchor) = anchor {
        seen.insert(anchor);
        cells.push(anchor);
    }

    let mut px = x;
    let mut py = y;
    let mut traveled = 0.0;
    while cells.len() < limit && traveled <= max_distance {
        px += dir_x * step;
        py += dir_y * step;
        traveled += step;

        let mut overlaps = board
            .occupied_cells()
            .filter(|coord| !seen.contains(coord))
            .filter_map(|coord| {
                let (cx, cy) = layout.center(coord, board.parity());
                let hit_distance = distance(px, py, cx, cy);
                (hit_distance <= overlap_radius).then_some((hit_distance, coord))
            })
            .collect::<Vec<_>>();
        overlaps.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.row.cmp(&right.1.row))
                .then_with(|| left.1.col.cmp(&right.1.col))
        });

        for (_, coord) in overlaps {
            if seen.insert(coord) {
                cells.push(coord);
            }
            if cells.len() >= limit {
                break;
            }
        }

        if px < layout.left - layout.radius * 3.0
            || px > layout.width - layout.left + layout.radius * 3.0
            || py < layout.top - layout.radius * 3.0
            || py > layout.height + layout.radius * 3.0
        {
            break;
        }
    }
    cells
}

fn attachment_cell(
    board: &Board,
    layout: BubbleLayout,
    x: f64,
    y: f64,
    target: AttachmentTarget,
) -> Option<CellCoord> {
    match target {
        AttachmentTarget::Ceiling => nearest_empty_top_cell(board, layout, x),
        AttachmentTarget::Bubble(anchor) => nearest_empty_neighbor(board, layout, anchor, x, y),
    }
}

fn nearest_empty_top_cell(board: &Board, layout: BubbleLayout, x: f64) -> Option<CellCoord> {
    let mut best = None;
    let mut best_distance = f64::MAX;

    for col in 0..COLS {
        let coord = CellCoord { row: 0, col };
        if !board.is_empty(coord) {
            continue;
        }
        let (cx, _) = layout.center(coord, board.parity());
        let candidate_distance = (x - cx).abs();
        if candidate_distance < best_distance {
            best = Some(coord);
            best_distance = candidate_distance;
        }
    }

    best
}

fn nearest_empty_neighbor(
    board: &Board,
    layout: BubbleLayout,
    anchor: CellCoord,
    x: f64,
    y: f64,
) -> Option<CellCoord> {
    board
        .empty_attachment_neighbors(anchor)
        .into_iter()
        .min_by(|left, right| {
            let (left_x, left_y) = layout.center(*left, board.parity());
            let (right_x, right_y) = layout.center(*right, board.parity());
            distance(x, y, left_x, left_y).total_cmp(&distance(x, y, right_x, right_y))
        })
}

fn clamp_aim(angle: f64) -> f64 {
    angle.clamp(-PI + 0.18, -0.18)
}

fn resize_canvas(canvas: &HtmlCanvasElement) -> BubbleLayout {
    let width = canvas.client_width().max(320) as f64;
    let height = canvas.client_height().max(470) as f64;
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

    BubbleLayout::for_canvas(width, height)
}

fn layout_for_canvas(canvas: &HtmlCanvasElement) -> BubbleLayout {
    BubbleLayout::for_canvas(
        canvas.client_width().max(320) as f64,
        canvas.client_height().max(470) as f64,
    )
}

fn canvas_context(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas
        .get_context("2d")
        .ok()
        .flatten()?
        .dyn_into::<CanvasRenderingContext2d>()
        .ok()
}

fn draw_background(context: &CanvasRenderingContext2d, layout: BubbleLayout) {
    context.set_fill_style_str("#08100d");
    context.fill_rect(0.0, 0.0, layout.width, layout.height);
    context.set_fill_style_str("#0e1713");
    context.fill_rect(0.0, 0.0, layout.width, layout.top - 13.0);
    context.set_stroke_style_str("rgba(172, 215, 178, 0.22)");
    context.stroke_rect(0.5, 0.5, layout.width - 1.0, layout.height - 1.0);
}

fn draw_header(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    game: &BubbleGame,
    message: &str,
    show_top_hud: bool,
) {
    // Monospace at 14px is ~8.4px per glyph — accurate enough to center/right-align.
    let hud_text_width = |text: &str| text.chars().count() as f64 * 8.4;

    if show_top_hud {
        context.set_font("14px SFMono-Regular, Consolas, monospace");

        context.set_fill_style_str("#acd7b2");
        let _ = context.fill_text(
            &format!("score {}  best {}", game.score, game.high_score),
            16.0,
            28.0,
        );

        context.set_fill_style_str("rgba(199, 223, 201, 0.82)");
        let drop_text = format!("drop {}", game.shots_until_drop);
        let _ = context.fill_text(
            &drop_text,
            layout.width - 16.0 - hud_text_width(&drop_text),
            28.0,
        );
    }
    if !show_top_hud {
        return;
    }

    let prize_split = message.split_once("; Prize: ");
    let status_h = if prize_split.is_some() { 44.0 } else { 28.0 };
    context.set_fill_style_str("rgba(8, 16, 13, 0.88)");
    context.fill_rect(
        1.0,
        layout.height - status_h - 1.0,
        layout.width - 2.0,
        status_h,
    );
    let mobile = layout.width < 420.0;
    let message_font = if mobile {
        "12px SFMono-Regular, Consolas, monospace"
    } else {
        "14px SFMono-Regular, Consolas, monospace"
    };
    let char_width = if mobile { 7.4 } else { 8.4 };
    let message_x = if mobile { 12.0 } else { 16.0 };
    context.set_font(message_font);
    context.set_fill_style_str("rgba(199, 223, 201, 0.82)");
    if let Some((score_text, prize_text)) = prize_split {
        let prize = format!("Prize: {}", prize_text);
        context.set_fill_style_str("#e9d18e");
        let prize_text = truncate_text(
            &prize,
            (layout.width - message_x - if mobile { 12.0 } else { 18.0 }).max(72.0),
            char_width,
        );
        let _ = context.fill_text(&prize_text, message_x, layout.height - 28.0);
        context.set_fill_style_str("rgba(199, 223, 201, 0.82)");
        let score_text = truncate_text(
            score_text,
            (layout.width - message_x - if mobile { 12.0 } else { 18.0 }).max(72.0),
            char_width,
        );
        let _ = context.fill_text(&score_text, message_x, layout.height - 10.0);
    } else {
        let max_message_width =
            (layout.width - message_x - if mobile { 18.0 } else { 24.0 }).max(72.0);
        let message = truncate_text(message, max_message_width, char_width);
        let _ = context.fill_text(&message, message_x, layout.height - 13.0);
    }
}

fn draw_board(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    board: &Board,
    row_offset: f64,
) {
    for row in 0..ROWS {
        for col in 0..COLS {
            let coord = CellCoord { row, col };
            let (x, y) = layout.center(coord, board.parity());
            context.set_stroke_style_str("rgba(172, 215, 178, 0.08)");
            context.begin_path();
            let _ = context.arc(x, y, layout.radius * 0.92, 0.0, PI * 2.0);
            context.stroke();

            if let Some(color) = board.get(coord) {
                draw_bubble(context, x, y + row_offset, layout.radius, color, 1.0);
            }
        }
    }
}

fn draw_particles(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    particles: &[Particle],
) {
    for particle in particles {
        if particle.delay > 0.0 {
            continue;
        }
        let alpha = (particle.life / particle.max_life).clamp(0.0, 1.0);
        match particle.kind {
            ParticleKind::Bubble => draw_bubble(
                context,
                particle.x,
                particle.y,
                particle.radius,
                particle.color,
                alpha,
            ),
            ParticleKind::Spark => draw_spark(context, particle, alpha),
            ParticleKind::Ring => draw_blast_ring(context, particle, alpha),
            ParticleKind::LightningArc => draw_lightning_arc(context, particle, alpha),
            ParticleKind::ScreenFlash => draw_screen_flash(context, layout, alpha),
        }
    }
}

fn draw_screen_flash(context: &CanvasRenderingContext2d, layout: BubbleLayout, alpha: f64) {
    context.set_global_alpha((alpha * 0.34).min(0.34));
    context.set_fill_style_str("#d8fbff");
    context.fill_rect(0.0, 0.0, layout.width, layout.height);
    context.set_global_alpha((alpha * 0.22).min(0.22));
    context.set_fill_style_str("#ffe66d");
    context.fill_rect(0.0, 0.0, layout.width, layout.height);
    context.set_global_alpha(1.0);
}

fn draw_spark(context: &CanvasRenderingContext2d, particle: &Particle, alpha: f64) {
    let (_, stroke, _) = color_hex(particle.color);
    context.set_global_alpha(alpha);
    context.set_stroke_style_str(stroke);
    context.set_line_width(2.0);
    context.begin_path();
    context.move_to(particle.x, particle.y);
    context.line_to(
        particle.x - particle.vx * 0.035,
        particle.y - particle.vy * 0.035,
    );
    context.stroke();
    context.set_line_width(1.0);
    context.set_global_alpha(1.0);
}

fn draw_blast_ring(context: &CanvasRenderingContext2d, particle: &Particle, alpha: f64) {
    let progress = 1.0 - alpha;
    context.set_global_alpha((alpha * 1.25).min(1.0));
    context.set_stroke_style_str(match particle.color {
        BubbleColor::Lightning | BubbleColor::Drill => "#9cf0ff",
        _ => "#ffb35c",
    });
    context.set_line_width(4.0);
    context.begin_path();
    let _ = context.arc(
        particle.x,
        particle.y,
        particle.radius * (0.85 + progress * 2.35),
        0.0,
        PI * 2.0,
    );
    context.stroke();
    context.set_line_width(1.0);
    context.set_global_alpha(1.0);
}

fn draw_lightning_arc(context: &CanvasRenderingContext2d, particle: &Particle, alpha: f64) {
    let dx = particle.vx - particle.x;
    let dy = particle.vy - particle.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let normal_x = -dy / len;
    let normal_y = dx / len;
    let segments = 7;
    let mut points = Vec::with_capacity(segments + 1);
    for index in 0..=segments {
        let t = index as f64 / segments as f64;
        let wave = match index {
            0 | 7 => 0.0,
            _ => {
                let sign = if index % 2 == 0 { 1.0 } else { -1.0 };
                sign * particle.radius * (0.22 + (index % 3) as f64 * 0.07)
            }
        };
        points.push((
            particle.x + dx * t + normal_x * wave,
            particle.y + dy * t + normal_y * wave,
        ));
    }

    context.set_global_alpha(alpha);
    context.set_stroke_style_str("#9cf0ff");
    context.set_line_width(6.0);
    context.begin_path();
    if let Some((x, y)) = points.first().copied() {
        context.move_to(x, y);
    }
    for (x, y) in points.iter().copied().skip(1) {
        context.line_to(x, y);
    }
    context.stroke();

    context.set_stroke_style_str("#ffe66d");
    context.set_line_width(2.0);
    context.begin_path();
    if let Some((x, y)) = points.first().copied() {
        context.move_to(x, y);
    }
    for (x, y) in points.iter().copied().skip(1) {
        context.line_to(x, y);
    }
    context.stroke();
    context.set_line_width(2.0);
    context.set_global_alpha(1.0);
}

fn draw_aim(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    board: &Board,
    angle: f64,
    color: BubbleColor,
    ghost_level: u8,
    guide_level: u8,
    dot_level: u8,
    bank_sight_level: u8,
) {
    let guide_trace = cast_aim_trace(board, layout, angle, guide_level, bank_sight_level);
    if guide_trace.points.len() >= 2 {
        context.set_stroke_style_str("rgba(233, 209, 142, 0.42)");
        context.set_line_width(2.0);
        context.set_line_cap("round");
        context.begin_path();
        let (start_x, start_y) = guide_trace.points[0];
        context.move_to(start_x, start_y);
        for (x, y) in guide_trace.points.iter().copied().skip(1) {
            context.line_to(x, y);
        }
        context.stroke();
        context.set_line_width(1.0);
        context.set_line_cap("butt");

        if dot_level > 0 {
            if let Some((x, y)) = guide_trace.points.last().copied() {
                context.set_fill_style_str("rgba(156, 240, 255, 0.78)");
                context.begin_path();
                let _ = context.arc(x, y, 3.1, 0.0, PI * 2.0);
                context.fill();
                context.set_stroke_style_str("rgba(255, 230, 109, 0.72)");
                context.begin_path();
                let _ = context.arc(x, y, 5.0, 0.0, PI * 2.0);
                context.stroke();
            }
        }
    }

    if ghost_level > 0 {
        let ghost_trace = cast_aim_trace(board, layout, angle, 3, bank_sight_level);
        if let Some(coord) = ghost_trace.landing {
            let (x, y) = layout.center(coord, board.parity());
            draw_bubble(context, x, y, layout.radius, color, 0.34);
        }
    }
}

struct AimTrace {
    points: Vec<(f64, f64)>,
    landing: Option<CellCoord>,
}

fn aim_trace_config(reticle_level: u8, bank_sight_level: u8) -> (Option<f64>, u8) {
    let reticle_level = reticle_level.min(2);
    let bank_level = bank_sight_level.min(3);
    let infinite_trace = reticle_level >= 2 || bank_level == 3;
    let bounces = if infinite_trace {
        FULL_TRACE_BOUNCES
    } else {
        BASE_TRACE_BOUNCES + bank_level
    };
    let distance = if infinite_trace {
        None
    } else {
        let reticle_multiplier = if reticle_level >= 1 {
            LEVEL_ONE_TRACE_MULTIPLIER
        } else {
            1.0
        };
        Some(
            DEFAULT_TRACE_DISTANCE_RADIUS * reticle_multiplier
                + f64::from(bank_level) * BANK_DISTANCE_PER_LEVEL_RADIUS,
        )
    };
    (distance, bounces)
}

fn cast_aim_trace(
    board: &Board,
    layout: BubbleLayout,
    angle: f64,
    reticle_level: u8,
    bank_sight_level: u8,
) -> AimTrace {
    let mut x = layout.cannon_x;
    let mut y = layout.cannon_y;
    let mut vx = angle.cos();
    let vy = angle.sin();
    let (trace_distance_radius, mut bounces_left) =
        aim_trace_config(reticle_level, bank_sight_level);
    let max_distance = trace_distance_radius
        .map(|distance| layout.radius * distance)
        .unwrap_or_else(|| (layout.width + layout.height) * 4.0);
    let step = layout.radius * SHOT_CAST_STEP_RADIUS;
    let max_steps = (max_distance / step).ceil().max(1.0) as usize;
    let mut points = Vec::with_capacity(max_steps.min(192) + 1);
    points.push((x, y));

    for _ in 0..max_steps {
        x += vx * step;
        y += vy * step;

        let left_wall = layout.left + layout.radius;
        let right_wall = layout.width - layout.left - layout.radius;
        if x <= left_wall {
            if bounces_left == 0 {
                x = left_wall;
                points.push((x, y));
                return AimTrace {
                    points,
                    landing: None,
                };
            }
            x = left_wall;
            vx = vx.abs();
            bounces_left -= 1;
        } else if x >= right_wall {
            if bounces_left == 0 {
                x = right_wall;
                points.push((x, y));
                return AimTrace {
                    points,
                    landing: None,
                };
            }
            x = right_wall;
            vx = -vx.abs();
            bounces_left -= 1;
        }

        if y <= layout.top + layout.radius {
            y = layout.top + layout.radius;
            points.push((x, y));
            return AimTrace {
                points,
                landing: nearest_empty_top_cell(board, layout, x),
            };
        }

        if let Some(hit) = hit_existing_bubble(board, layout, x, y) {
            points.push((x, y));
            return AimTrace {
                points,
                landing: nearest_empty_neighbor(board, layout, hit, x, y),
            };
        }

        points.push((x, y));
    }

    AimTrace {
        points,
        landing: None,
    }
}

#[derive(Clone, Copy, Debug)]
struct CannonControls {
    hold_x: f64,
    hold_y: f64,
    next_x: f64,
    radius: f64,
    gap: f64,
}

const CANNON_SIDE_SLOT_MARGIN: f64 = 30.0;
const HOLD_HIT_RADIUS_MIN: f64 = 28.0;
const HOLD_HIT_RADIUS_SCALE: f64 = 2.6;

fn cannon_controls(layout: BubbleLayout) -> CannonControls {
    let radius = layout.radius * 0.62;
    let gap = radius * 2.0 + 6.0;
    let hold_x = layout.cannon_x - layout.radius * 1.05 - CANNON_SIDE_SLOT_MARGIN - radius;
    CannonControls {
        hold_x,
        hold_y: layout.cannon_y,
        next_x: layout.cannon_x + layout.radius * 1.05 + CANNON_SIDE_SLOT_MARGIN + radius,
        radius,
        gap,
    }
}

fn hold_hit_radius(controls: CannonControls) -> f64 {
    (controls.radius * HOLD_HIT_RADIUS_SCALE).max(HOLD_HIT_RADIUS_MIN)
}

fn draw_cannon(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    angle: f64,
    color: BubbleColor,
    next: Vec<BubbleColor>,
    held: Option<BubbleColor>,
) {
    let muzzle_x = layout.cannon_x + angle.cos() * 34.0;
    let muzzle_y = layout.cannon_y + angle.sin() * 34.0;
    context.set_stroke_style_str("#c7dfc9");
    context.set_line_width(5.0);
    context.begin_path();
    context.move_to(layout.cannon_x, layout.cannon_y);
    context.line_to(muzzle_x, muzzle_y);
    context.stroke();
    context.set_line_width(1.0);
    draw_bubble(
        context,
        layout.cannon_x,
        layout.cannon_y,
        layout.radius * 1.05,
        color,
        1.0,
    );

    // Upcoming bubbles queued up beside the cannon so the next shots are obvious.
    let controls = cannon_controls(layout);
    let queue_radius = controls.radius;
    let queue_x = controls.next_x;
    context.set_fill_style_str("rgba(199, 223, 201, 0.66)");
    context.set_font("11px SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text(
        "next",
        queue_x - queue_radius,
        layout.cannon_y - queue_radius - 6.0,
    );
    for (index, color) in next.iter().enumerate() {
        draw_bubble(
            context,
            queue_x + index as f64 * controls.gap,
            layout.cannon_y,
            queue_radius,
            *color,
            if index == 0 { 1.0 } else { 0.78 },
        );
    }

    // Held bubble (tap / right-click / H to swap) sits to the left of the cannon.
    let hold_x = controls.hold_x;
    context.set_fill_style_str("rgba(199, 223, 201, 0.66)");
    let _ = context.fill_text(
        "hold",
        hold_x - queue_radius,
        layout.cannon_y - queue_radius - 6.0,
    );
    match held {
        Some(color) => draw_bubble(context, hold_x, layout.cannon_y, queue_radius, color, 1.0),
        None => {
            context.set_stroke_style_str("rgba(199, 223, 201, 0.4)");
            context.begin_path();
            let _ = context.arc(hold_x, layout.cannon_y, queue_radius, 0.0, PI * 2.0);
            context.stroke();
        }
    }
}

fn draw_bubble(
    context: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    radius: f64,
    color: BubbleColor,
    alpha: f64,
) {
    let (fill, stroke, shine) = color_hex(color);
    context.set_global_alpha(alpha);
    context.set_fill_style_str(fill);
    context.begin_path();
    let _ = context.arc(x, y, radius, 0.0, PI * 2.0);
    context.fill();
    context.set_stroke_style_str(stroke);
    context.set_line_width(2.0);
    context.stroke();
    context.set_fill_style_str(shine);
    context.begin_path();
    let _ = context.arc(
        x - radius * 0.32,
        y - radius * 0.35,
        radius * 0.26,
        0.0,
        PI * 2.0,
    );
    context.fill();
    if color == BubbleColor::Prize {
        context.set_fill_style_str("#e9d18e");
        context.set_font(&format!(
            "700 {}px SFMono-Regular, Consolas, monospace",
            (radius * 1.05).round()
        ));
        context.set_text_align("center");
        context.set_text_baseline("middle");
        let _ = context.fill_text("?", x, y + radius * 0.05);
        context.set_text_align("start");
        context.set_text_baseline("alphabetic");
    }
    context.set_global_alpha(1.0);
    context.set_line_width(1.0);
}

fn draw_overlay(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    cleared: bool,
    score: u64,
) {
    context.set_fill_style_str("rgba(8, 12, 10, 0.74)");
    context.fill_rect(0.0, layout.top, layout.width, layout.height - layout.top);
    context.set_fill_style_str("#f3f1ea");
    context.set_font("22px SFMono-Regular, Consolas, monospace");
    let title = if cleared {
        "board cleared"
    } else {
        "game over"
    };
    let _ = context.fill_text(title, layout.width / 2.0 - 78.0, layout.height / 2.0 - 12.0);
    context.set_font("14px SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text(
        &format!("score {} - click, tap, Space, or Enter to restart", score),
        layout.width / 2.0 - 190.0,
        layout.height / 2.0 + 18.0,
    );
}

fn draw_shop_overlay(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    game: &BubbleGame,
    _progress: &BubbleProgress,
    options: Vec<ShopOption>,
    owned_options: Vec<ShopOption>,
    consumables: Vec<ShopOption>,
    shop_tab: ShopTab,
    shop_kit_page: usize,
) {
    let (panel_x, panel_y, panel_w, panel_h) = shop_panel(layout);
    context.set_fill_style_str("rgba(8, 12, 10, 0.80)");
    context.fill_rect(0.0, 0.0, layout.width, layout.height);
    context.set_fill_style_str("rgba(14, 23, 19, 0.96)");
    context.fill_rect(panel_x, panel_y, panel_w, panel_h);

    context.set_font("700 18px SFMono-Regular, Consolas, monospace");
    context.set_fill_style_str("#f3f1ea");
    let pad = 14.0;
    let _ = context.fill_text("Shop", panel_x + pad, panel_y + 27.0);

    let score = format!("score {}", game.shop_score());
    let score_w = text_width(&score, 12.0);
    context.set_font("12px SFMono-Regular, Consolas, monospace");
    context.set_fill_style_str("rgba(199, 223, 201, 0.82)");
    let _ = context.fill_text(&score, panel_x + panel_w - 8.0 - score_w, panel_y + 27.0);

    draw_shop_tabs(context, layout, shop_tab);
    match shop_tab {
        ShopTab::Upgrades => {
            draw_mobile_shop_kit_page(context, layout, game, &options, shop_kit_page)
        }
        ShopTab::Owned => draw_owned_shop_toggles(context, layout, &owned_options, true),
        ShopTab::Consumables => {
            draw_consumable_shop_items(context, layout, game, &consumables, true)
        }
    }

    let (button_x, button_y, button_w, button_h) = shop_continue_rect(layout);
    context.set_fill_style_str("rgba(233, 209, 142, 0.14)");
    context.fill_rect(button_x, button_y, button_w, button_h);
    context.set_stroke_style_str("rgba(233, 209, 142, 0.68)");
    context.stroke_rect(
        button_x + 0.5,
        button_y + 0.5,
        button_w - 1.0,
        button_h - 1.0,
    );
    context.set_font("700 13px SFMono-Regular, Consolas, monospace");
    context.set_fill_style_str("#e9d18e");
    context.set_text_baseline("middle");
    let label = "Play!";
    let label_w = label.chars().count() as f64 * 7.8;
    let group_w = 18.0 + label_w;
    let group_x = button_x + (button_w - group_w) / 2.0;
    draw_play_triangle(context, group_x + 5.0, button_y + button_h / 2.0);
    let _ = context.fill_text(label, group_x + 18.0, button_y + button_h / 2.0);
    context.set_text_baseline("alphabetic");
}

fn draw_owned_shop_toggles(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    owned_options: &[ShopOption],
    tabbed: bool,
) {
    let (panel_x, panel_y, _, _) = shop_panel(layout);
    let tabbed_layout = tabbed;
    if owned_options.is_empty() && tabbed {
        context.set_font("11px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str("rgba(199, 223, 201, 0.42)");
        let _ = context.fill_text("none yet", panel_x + 14.0, panel_y + 106.0);
    }

    for (index, option) in owned_options.iter().enumerate() {
        let (chip_x, chip_y, chip_w, chip_h) = owned_toggle_rect(layout, index, tabbed);
        let border = if option.enabled {
            "#e9d18e"
        } else {
            "rgba(172, 215, 178, 0.48)"
        };
        context.set_fill_style_str(if option.enabled {
            "rgba(233, 209, 142, 0.14)"
        } else {
            "rgba(199, 223, 201, 0.06)"
        });
        context.fill_rect(chip_x, chip_y, chip_w, chip_h);
        context.set_stroke_style_str(border);
        context.stroke_rect(chip_x + 0.5, chip_y + 0.5, chip_w - 1.0, chip_h - 1.0);
        context.set_fill_style_str(if option.enabled {
            "#e9d18e"
        } else {
            "rgba(199, 223, 201, 0.82)"
        });
        let level_label = owned_active_level_label(option);
        if tabbed_layout {
            context.set_text_baseline("alphabetic");
            context.set_font("700 10px SFMono-Regular, Consolas, monospace");
            let title = truncate_text(option.title, chip_w - 56.0, 5.6);
            let _ = context.fill_text(&title, chip_x + 7.0, chip_y + 17.0);
            context.set_text_align("right");
            let _ = context.fill_text(&level_label, chip_x + chip_w - 7.0, chip_y + 17.0);
            context.set_text_align("start");
            context.set_font("8px SFMono-Regular, Consolas, monospace");
            context.set_fill_style_str("rgba(199, 223, 201, 0.62)");
            let help = truncate_text(shop_upgrade_micro_help(option), chip_w - 14.0, 4.8);
            let _ = context.fill_text(&help, chip_x + 7.0, chip_y + 35.0);
        } else {
            context.set_text_baseline("middle");
            context.set_font("700 10px SFMono-Regular, Consolas, monospace");
            let title = if chip_w < 126.0 {
                compact_owned_chip_label(option.upgrade, chip_w)
            } else {
                option.title
            };
            let _ = context.fill_text(title, chip_x + 7.0, chip_y + chip_h / 2.0);
            context.set_text_align("right");
            let _ = context.fill_text(&level_label, chip_x + chip_w - 7.0, chip_y + chip_h / 2.0);
            context.set_text_align("start");
            context.set_text_baseline("alphabetic");
        }
    }
}

fn draw_consumable_shop_items(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    game: &BubbleGame,
    consumables: &[ShopOption],
    tabbed: bool,
) {
    let tabbed_layout = tabbed;
    for (index, option) in consumables.iter().enumerate() {
        let (item_x, item_y, item_w, item_h) = consumable_rect(layout, index, tabbed);
        let affordable = game.shop_score() >= option.cost;
        context.set_fill_style_str(if affordable {
            "rgba(233, 209, 142, 0.13)"
        } else {
            "rgba(255, 255, 255, 0.025)"
        });
        context.fill_rect(item_x, item_y, item_w, item_h);
        context.set_stroke_style_str(if affordable {
            "rgba(233, 209, 142, 0.52)"
        } else {
            "rgba(172, 215, 178, 0.28)"
        });
        context.stroke_rect(item_x + 0.5, item_y + 0.5, item_w - 1.0, item_h - 1.0);
        context.set_fill_style_str(if affordable {
            "#e9d18e"
        } else {
            "rgba(199, 223, 201, 0.48)"
        });
        let title = consumable_display_title(option.upgrade, item_w);
        context.set_font("700 10px SFMono-Regular, Consolas, monospace");
        let text_x = if tabbed_layout { 8.0 } else { 7.0 };
        let title_y = if tabbed_layout {
            14.0
        } else if item_h < 25.0 {
            10.0
        } else {
            13.0
        };
        let label_y = if tabbed_layout {
            31.0
        } else if item_h < 25.0 {
            20.0
        } else {
            25.0
        };
        let _ = context.fill_text(title, item_x + text_x, item_y + title_y);
        context.set_font("10px SFMono-Regular, Consolas, monospace");
        let label = if option.level > 0 {
            format!("x{}  {}", option.level, option.cost)
        } else {
            format!("{}", option.cost)
        };
        if tabbed_layout {
            let label_w = text_width(&label, 6.0);
            let _ = context.fill_text(&label, item_x + item_w - text_x - label_w, item_y + title_y);
            context.set_fill_style_str(if affordable {
                "rgba(233, 209, 142, 0.74)"
            } else {
                "rgba(199, 223, 201, 0.44)"
            });
            let help = truncate_text(shop_upgrade_micro_help(option), item_w - text_x * 2.0, 4.8);
            let _ = context.fill_text(&help, item_x + text_x, item_y + label_y);
        } else {
            let _ = context.fill_text(&label, item_x + text_x, item_y + label_y);
        }
    }
}

fn owned_active_level_label(option: &ShopOption) -> String {
    match option.max_level {
        Some(_) if option.active_level == 0 => format!("off/{}", option.level),
        Some(_) if option.active_level < option.level => {
            format!("lvl {}/{}", option.active_level, option.level)
        }
        Some(_) => format!("lvl {}", option.level),
        None if option.level > 0 => format!("x{}", option.level),
        None => "off".to_string(),
    }
}

fn draw_play_triangle(context: &CanvasRenderingContext2d, x: f64, y: f64) {
    context.set_fill_style_str("#e9d18e");
    context.begin_path();
    context.move_to(x - 3.0, y - 6.0);
    context.line_to(x - 3.0, y + 6.0);
    context.line_to(x + 7.0, y);
    context.close_path();
    context.fill();
}

fn draw_shop_tabs(context: &CanvasRenderingContext2d, layout: BubbleLayout, active: ShopTab) {
    for tab in [ShopTab::Upgrades, ShopTab::Owned, ShopTab::Consumables] {
        let (x, y, w, h) = shop_tab_rect(layout, tab);
        let selected = tab == active;
        context.set_fill_style_str(if selected {
            "rgba(233, 209, 142, 0.16)"
        } else {
            "rgba(255, 255, 255, 0.032)"
        });
        context.fill_rect(x, y, w, h);
        context.set_stroke_style_str(if selected {
            "rgba(233, 209, 142, 0.62)"
        } else {
            "rgba(172, 215, 178, 0.24)"
        });
        context.stroke_rect(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        context.set_font("700 10px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str(if selected {
            "#e9d18e"
        } else {
            "rgba(199, 223, 201, 0.68)"
        });
        let title = shop_tab_title(tab);
        let title_w = text_width(title, 6.2);
        let _ = context.fill_text(title, x + (w - title_w) / 2.0, y + 17.0);
    }
}

fn draw_mobile_shop_kit_page(
    context: &CanvasRenderingContext2d,
    layout: BubbleLayout,
    game: &BubbleGame,
    options: &[ShopOption],
    active_kit: usize,
) {
    let kits = shop_kits();
    let active_kit = active_kit.min(kits.len().saturating_sub(1));
    draw_mobile_kit_tabs(context, layout, active_kit);

    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let kit = kits[active_kit];
    let content_x = panel_x + 14.0;
    let content_w = panel_w - 28.0;

    context.set_fill_style_str("#f3f1ea");
    context.set_font("700 16px SFMono-Regular, Consolas, monospace");
    let _ = context.fill_text(kit.title, content_x, panel_y + 119.0);

    for (item, upgrade) in kit.upgrades.iter().enumerate() {
        let Some(index) = kit_upgrade_index(*upgrade) else {
            continue;
        };
        let Some(option) = options.get(index) else {
            continue;
        };
        let (card_x, card_y, card_w, card_h) = mobile_kit_upgrade_rect(layout, active_kit, item);
        let complete = option
            .max_level
            .is_some_and(|max_level| option.level >= max_level);
        let affordable = game.shop_score() >= option.cost && !complete;
        let border = if complete {
            "rgba(199, 223, 201, 0.36)"
        } else if option.enabled {
            "#e9d18e"
        } else if affordable {
            "rgba(233, 209, 142, 0.62)"
        } else {
            "rgba(172, 215, 178, 0.28)"
        };
        context.set_fill_style_str(if complete {
            "rgba(199, 223, 201, 0.05)"
        } else if affordable {
            "rgba(233, 209, 142, 0.12)"
        } else {
            "rgba(255, 255, 255, 0.026)"
        });
        context.fill_rect(card_x, card_y, card_w, card_h);
        context.set_stroke_style_str(border);
        context.stroke_rect(card_x + 0.5, card_y + 0.5, card_w - 1.0, card_h - 1.0);

        context.set_font("700 10px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str(if affordable || complete {
            "#e9d18e"
        } else {
            "rgba(199, 223, 201, 0.52)"
        });
        let cost_label = match option.max_level {
            Some(_) if complete => "max".to_string(),
            Some(_) => compact_score(option.cost),
            None if option.level > 0 => format!("x{}", option.level),
            None => compact_score(option.cost),
        };
        let cost_w = text_width(&cost_label, 6.0);
        let title = truncate_text(option.title, card_w - cost_w - 22.0, 6.0);
        let _ = context.fill_text(&title, card_x + 8.0, card_y + 20.0);
        context.set_text_align("right");
        let _ = context.fill_text(&cost_label, card_x + card_w - 8.0, card_y + 20.0);
        context.set_text_align("start");

        context.set_font("8px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str(if affordable || complete {
            "rgba(233, 209, 142, 0.74)"
        } else {
            "rgba(199, 223, 201, 0.46)"
        });
        draw_wrapped_text(
            context,
            shop_upgrade_card_help(option),
            card_x + 8.0,
            card_y + 39.0,
            card_w - 16.0,
            4.7,
            11.0,
            ((card_h - 58.0) / 11.0).floor().max(1.0) as usize,
        );

        let level = match option.max_level {
            Some(max_level) if complete => format!("lvl {} max", max_level),
            Some(max_level) => format!("lvl {}/{}", option.level, max_level),
            None if option.level > 0 => format!("owned x{}", option.level),
            None => "one use".to_string(),
        };
        context.set_font("9px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str("rgba(199, 223, 201, 0.58)");
        let _ = context.fill_text(&level, card_x + 8.0, card_y + card_h - 13.0);
    }

    context.set_stroke_style_str("rgba(172, 215, 178, 0.12)");
    context.begin_path();
    context.move_to(content_x, panel_y + 134.0);
    context.line_to(content_x + content_w, panel_y + 134.0);
    context.stroke();
}

fn draw_mobile_kit_tabs(context: &CanvasRenderingContext2d, layout: BubbleLayout, active: usize) {
    for (index, kit) in shop_kits().iter().enumerate() {
        let (x, y, w, h) = mobile_kit_tab_rect(layout, index);
        let selected = index == active;
        context.set_fill_style_str(if selected {
            "rgba(233, 209, 142, 0.15)"
        } else {
            "rgba(255, 255, 255, 0.026)"
        });
        context.fill_rect(x, y, w, h);
        context.set_stroke_style_str(if selected {
            "rgba(233, 209, 142, 0.58)"
        } else {
            "rgba(172, 215, 178, 0.22)"
        });
        context.stroke_rect(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        context.set_font("700 9px SFMono-Regular, Consolas, monospace");
        context.set_fill_style_str(if selected {
            "#e9d18e"
        } else {
            "rgba(199, 223, 201, 0.66)"
        });
        let title_w = text_width(kit.title, 5.5);
        let _ = context.fill_text(kit.title, x + (w - title_w) / 2.0, y + 15.0);
    }
}

fn compact_score(score: u64) -> String {
    if score < 1_000 {
        return score.to_string();
    }
    if score % 1_000 == 0 {
        format!("{}k", score / 1_000)
    } else {
        format!("{:.1}k", score as f64 / 1_000.0)
    }
}

fn prize_message(prizes: &[&'static str]) -> String {
    if prizes.is_empty() {
        String::new()
    } else {
        let prizes = prizes
            .iter()
            .map(|prize| match *prize {
                "Color Call" => "Call",
                "Bomb Shot" => "Bomb",
                "Lightning" => "Bolt",
                "Drill Shot" => "Drill",
                "Drop Reset" => "Reset",
                "Wild Shot" => "Wild",
                "Second Breath" => "Breath",
                "Bomb + Lightning" => "Bomb+Bolt",
                prize => prize,
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("; Prize: {}", prizes)
    }
}

fn shop_upgrade_card_help(option: &ShopOption) -> &'static str {
    match option.upgrade {
        ShopUpgrade::ColorCall => "Loads the largest active color.",
        ShopUpgrade::WildOrb => match option.level {
            0 => "5% chance for wild bubbles in the queue.",
            1 => "10% chance for wild bubbles in the queue.",
            _ => "20% chance for wild bubbles in the queue.",
        },
        ShopUpgrade::WildCache => {
            if option.level >= 1 {
                "Wilds match every touched color."
            } else {
                "Upgrades wild bubbles."
            }
        }
        ShopUpgrade::BombDrop => match option.level {
            0 => "Bombs in drops.",
            1 => "More bombs in drops.",
            _ => "Even more bombs in drops.",
        },
        ShopUpgrade::BlastRadius => "Bombs reach one more ring.",
        ShopUpgrade::BombShot => "Loads one radial blast bubble.",
        ShopUpgrade::WildShot => "Loads one wild bubble shot.",
        ShopUpgrade::LightningBolt => "Loads a shot that chains upward.",
        ShopUpgrade::DrillShot => "Loads one punch-through drill shot.",
        ShopUpgrade::DropReset => "Refills the drop counter once.",
        ShopUpgrade::SoftCeiling => match option.level {
            0 => "More shots before drops.",
            1 => "Even more shots.",
            _ => "Max extra shots.",
        },
        ShopUpgrade::DropHaste => match option.level {
            0 => "Rows drop sooner.",
            1 => "Rows drop even sooner.",
            _ => "Fastest drops.",
        },
        ShopUpgrade::PopDrop => "Pops advance drops.",
        ShopUpgrade::CleanStart => "1 fewer start row.",
        ShopUpgrade::ExtraRows => "1 more start row.",
        ShopUpgrade::QueueSight => match option.level {
            0 => "+1 queue bubble.",
            1 => "More queue sight.",
            _ => "Even more queue sight.",
        },
        ShopUpgrade::TraceSight => "Shows a ghost landing bubble.",
        ShopUpgrade::Reticle => match option.level {
            0 => "Doubles aim guide.",
            _ => "Full aim guide.",
        },
        ShopUpgrade::LandingDot => "Shows path endpoint dot.",
        ShopUpgrade::PrizeBubbles => match option.level {
            0 => "10% chance for prize bubbles in drops.",
            1 => "20% chance for prize bubbles in drops.",
            _ => "30% chance for prize bubbles in drops.",
        },
        ShopUpgrade::PrizeQuality => match option.level {
            0 => "Rewards: Call, Bomb, Lightning, Drill, Reset, Wild.",
            1 => "Rewards: Call, Bomb, Lightning, Drill, Reset, Wild, Breath.",
            _ => "Rewards: Call, Bomb, Lightning, Drill, Reset, Wild, Breath, Bundle.",
        },
        ShopUpgrade::Revival => "Clears space once on overflow.",
        ShopUpgrade::LightningPower => match option.level {
            0 => "Lightning splashes.",
            1 => "Lightning opens a wide beam.",
            _ => "Lightning opens a wider beam.",
        },
        ShopUpgrade::DrillPower => match option.level {
            0 => "Drill depth 7.",
            1 => "Drill depth 9.",
            _ => "Drill depth 11.",
        },
        ShopUpgrade::BankSight => match option.level {
            0 => "Preview 2 bounces.",
            1 => "Preview 3 bounces.",
            _ => "Preview all bounces.",
        },
    }
}

fn shop_upgrade_micro_help(option: &ShopOption) -> &'static str {
    match option.upgrade {
        ShopUpgrade::ColorCall => "Loads largest color.",
        ShopUpgrade::WildOrb => match option.level {
            0 => "5% queue wilds.",
            1 => "10% queue wilds.",
            _ => "20% queue wilds.",
        },
        ShopUpgrade::WildCache => {
            if option.level >= 1 {
                "Wilds match all touched colors."
            } else {
                "Improves wild shots."
            }
        }
        ShopUpgrade::BombDrop => "Bombs can drop in rows.",
        ShopUpgrade::BlastRadius => "Bombs reach farther.",
        ShopUpgrade::BombShot => "One bomb shot.",
        ShopUpgrade::WildShot => "One wild shot.",
        ShopUpgrade::LightningBolt => "One lightning shot.",
        ShopUpgrade::DrillShot => "One drill shot.",
        ShopUpgrade::DropReset => "Refills drop counter.",
        ShopUpgrade::SoftCeiling => "More shots before drops.",
        ShopUpgrade::DropHaste => "Rows drop sooner.",
        ShopUpgrade::PopDrop => "Pops count toward drops.",
        ShopUpgrade::CleanStart => "-1 start row.",
        ShopUpgrade::ExtraRows => "+1 start row.",
        ShopUpgrade::QueueSight => "Shows more queue.",
        ShopUpgrade::TraceSight => "Ghost landing bubble.",
        ShopUpgrade::Reticle => "Longer aim line.",
        ShopUpgrade::LandingDot => "Endpoint dot.",
        ShopUpgrade::PrizeBubbles => match option.level {
            0 => "10% prize drops.",
            1 => "20% prize drops.",
            _ => "30% prize drops.",
        },
        ShopUpgrade::PrizeQuality => match option.level {
            0 => "Adds Wild rewards.",
            1 => "Adds Breath rewards.",
            _ => "Adds Bundle rewards.",
        },
        ShopUpgrade::Revival => "One overflow save.",
        ShopUpgrade::LightningPower => match option.level {
            0 => "Lightning splashes.",
            1 => "Wide beam.",
            _ => "Wider beam.",
        },
        ShopUpgrade::DrillPower => match option.level {
            0 => "Depth 7.",
            1 => "Depth 9.",
            _ => "Depth 11.",
        },
        ShopUpgrade::BankSight => match option.level {
            0 => "Preview 2 bounces.",
            1 => "Preview 3 bounces.",
            _ => "Preview all bounces.",
        },
    }
}

fn draw_wrapped_text(
    context: &CanvasRenderingContext2d,
    text: &str,
    x: f64,
    y: f64,
    max_width: f64,
    char_width: f64,
    line_height: f64,
    max_lines: usize,
) {
    let mut line = String::new();
    let mut lines = Vec::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", line, word)
        };
        if text_width(&candidate, char_width) <= max_width {
            line = candidate;
        } else {
            if !line.is_empty() {
                lines.push(line);
            }
            line = word.to_string();
            if lines.len() + 1 >= max_lines {
                break;
            }
        }
    }
    if !line.is_empty() && lines.len() < max_lines {
        lines.push(line);
    }

    for (index, line) in lines.iter().enumerate() {
        let line = if index + 1 == max_lines {
            truncate_text(line, max_width, char_width)
        } else {
            line.clone()
        };
        let _ = context.fill_text(&line, x, y + index as f64 * line_height);
    }
}

fn truncate_text(text: &str, max_width: f64, char_width: f64) -> String {
    if max_width <= char_width * 4.0 {
        return String::new();
    }
    if text_width(text, char_width) <= max_width {
        return text.to_string();
    }
    let max_chars = (max_width / char_width).floor().max(1.0) as usize;
    let mut truncated = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn text_width(text: &str, char_width: f64) -> f64 {
    text.chars().count() as f64 * char_width
}

fn shop_hit(
    layout: BubbleLayout,
    x: f64,
    y: f64,
    offer_count: usize,
    owned_count: usize,
    consumable_count: usize,
    shop_tab: ShopTab,
    shop_kit_page: usize,
) -> Option<ShopHit> {
    for tab in [ShopTab::Upgrades, ShopTab::Owned, ShopTab::Consumables] {
        let (tab_x, tab_y, tab_w, tab_h) = shop_tab_rect(layout, tab);
        if x >= tab_x && x <= tab_x + tab_w && y >= tab_y && y <= tab_y + tab_h {
            return Some(ShopHit::SelectTab(tab));
        }
    }

    match shop_tab {
        ShopTab::Upgrades => {
            for index in 0..shop_kits().len() {
                let (tab_x, tab_y, tab_w, tab_h) = mobile_kit_tab_rect(layout, index);
                if x >= tab_x && x <= tab_x + tab_w && y >= tab_y && y <= tab_y + tab_h {
                    return Some(ShopHit::SelectKit(index));
                }
            }

            let kits = shop_kits();
            let kit_index = shop_kit_page.min(kits.len().saturating_sub(1));
            for (item, upgrade) in kits[kit_index].upgrades.iter().enumerate() {
                let Some(index) = kit_upgrade_index(*upgrade) else {
                    continue;
                };
                if index >= offer_count {
                    continue;
                }
                let (node_x, node_y, node_w, node_h) =
                    mobile_kit_upgrade_rect(layout, kit_index, item);
                if x >= node_x && x <= node_x + node_w && y >= node_y && y <= node_y + node_h {
                    return Some(ShopHit::Buy(index));
                }
            }
        }
        ShopTab::Owned => {
            for index in 0..owned_count {
                let (chip_x, chip_y, chip_w, chip_h) = owned_toggle_rect(layout, index, true);
                if x >= chip_x && x <= chip_x + chip_w && y >= chip_y && y <= chip_y + chip_h {
                    return Some(ShopHit::ToggleOwned(index));
                }
            }
        }
        ShopTab::Consumables => {
            for index in 0..consumable_count {
                let (item_x, item_y, item_w, item_h) = consumable_rect(layout, index, true);
                if x >= item_x && x <= item_x + item_w && y >= item_y && y <= item_y + item_h {
                    return Some(ShopHit::BuyConsumable(index));
                }
            }
        }
    }

    let (button_x, button_y, button_w, button_h) = shop_continue_rect(layout);
    (x >= button_x && x <= button_x + button_w && y >= button_y && y <= button_y + button_h)
        .then_some(ShopHit::Continue)
}

fn shop_panel(layout: BubbleLayout) -> (f64, f64, f64, f64) {
    (1.0, 1.0, layout.width - 2.0, layout.height - 2.0)
}

fn shop_tab_rect(layout: BubbleLayout, tab: ShopTab) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let pad = 14.0;
    let gap = 5.0;
    let tab_w = (panel_w - pad * 2.0 - gap * 2.0) / 3.0;
    let index = match tab {
        ShopTab::Upgrades => 0,
        ShopTab::Owned => 1,
        ShopTab::Consumables => 2,
    };
    (
        panel_x + pad + index as f64 * (tab_w + gap),
        panel_y + 38.0,
        tab_w,
        24.0,
    )
}

fn shop_tab_title(tab: ShopTab) -> &'static str {
    match tab {
        ShopTab::Upgrades => "Upgrades",
        ShopTab::Owned => "Owned",
        ShopTab::Consumables => "Items",
    }
}

fn owned_toggle_rect(layout: BubbleLayout, index: usize, tabbed: bool) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let tabbed_layout = tabbed;
    let gap = if tabbed_layout { 8.0 } else { 6.0 };
    let cols = if tabbed_layout && panel_w < 720.0 {
        2
    } else if tabbed_layout {
        3
    } else if panel_w < 500.0 {
        5
    } else {
        6
    };
    let pad = if tabbed_layout { 14.0 } else { 18.0 };
    let chip_w = (panel_w - pad * 2.0 - gap * (cols as f64 - 1.0)) / cols as f64;
    let chip_h = if tabbed_layout { 48.0 } else { 22.0 };
    let col = index % cols;
    let row = index / cols;
    (
        panel_x + pad + col as f64 * (chip_w + gap),
        panel_y
            + if tabbed_layout {
                76.0 + row as f64 * (chip_h + 7.0)
            } else {
                (if panel_w < 500.0 { 84.0 } else { 88.0 }) + row as f64 * (chip_h + 3.0)
            },
        chip_w,
        chip_h,
    )
}

fn mobile_kit_tab_rect(layout: BubbleLayout, index: usize) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let pad = 14.0;
    let gap = 5.0;
    let tab_w = (panel_w - pad * 2.0 - gap * 3.0) / 4.0;
    (
        panel_x + pad + index as f64 * (tab_w + gap),
        panel_y + 70.0,
        tab_w,
        22.0,
    )
}

fn mobile_kit_upgrade_rect(
    layout: BubbleLayout,
    kit_index: usize,
    item: usize,
) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let (_, button_y, _, _) = shop_continue_rect(layout);
    let gap = 8.0;
    let start_x = panel_x + 14.0;
    let start_y = panel_y + 144.0;
    let risk_layout = shop_kits()
        .get(kit_index)
        .is_some_and(|kit| kit.title == "Risk");
    let cols = if risk_layout {
        2
    } else if panel_w >= 720.0 {
        3
    } else {
        2
    };
    let card_w = (panel_w - 28.0 - gap * (cols as f64 - 1.0)) / cols as f64;
    let item_count = shop_kits()
        .get(kit_index)
        .map(|kit| kit.upgrades.len())
        .unwrap_or(4);
    let rows = if risk_layout {
        3
    } else {
        item_count.div_ceil(cols).max(1)
    };
    let available_h = (button_y - 12.0 - start_y).max(150.0);
    let card_h =
        ((available_h - gap * (rows.saturating_sub(1)) as f64) / rows as f64).clamp(76.0, 118.0);
    let (col, row) = if risk_layout {
        match item {
            0 => (0, 0),
            1 => (0, 1),
            2 => (0, 2),
            3 => (1, 0),
            4 => (1, 1),
            _ => (item % cols, item / cols),
        }
    } else {
        (item % cols, item / cols)
    };
    (
        start_x + col as f64 * (card_w + gap),
        start_y + row as f64 * (card_h + gap),
        card_w,
        card_h,
    )
}

fn consumable_strip_rect(layout: BubbleLayout) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, panel_h) = shop_panel(layout);
    if panel_w < 500.0 {
        (
            panel_x + 18.0,
            panel_y + panel_h - 98.0,
            panel_w - 36.0,
            51.0,
        )
    } else {
        (
            panel_x + 18.0,
            panel_y + panel_h - 92.0,
            panel_w - 36.0,
            28.0,
        )
    }
}

fn consumable_rect(layout: BubbleLayout, index: usize, tabbed: bool) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, _) = shop_panel(layout);
    let (strip_x, strip_y, strip_w, cols, gap, item_h) = if tabbed {
        (
            panel_x + 14.0,
            panel_y + 76.0,
            panel_w - 28.0,
            if panel_w < 500.0 { 2 } else { 4 },
            8.0,
            54.0,
        )
    } else {
        let (strip_x, strip_y, strip_w, _) = consumable_strip_rect(layout);
        (
            strip_x,
            strip_y,
            strip_w,
            if panel_w < 500.0 { 3 } else { 7 },
            if panel_w < 500.0 { 6.0 } else { 8.0 },
            if panel_w < 500.0 { 23.0 } else { 28.0 },
        )
    };
    let item_w = ((strip_w - gap * (cols as f64 - 1.0)) / cols as f64).max(if panel_w < 500.0 {
        72.0
    } else {
        52.0
    });
    let col = index % cols;
    let row = index / cols;
    (
        strip_x + col as f64 * (item_w + gap),
        strip_y + row as f64 * (item_h + 5.0),
        item_w,
        item_h,
    )
}

fn shop_continue_rect(layout: BubbleLayout) -> (f64, f64, f64, f64) {
    let (panel_x, panel_y, panel_w, panel_h) = shop_panel(layout);
    let mobile = panel_w < 500.0;
    let button_w = 96.0_f64.min(panel_w - 36.0);
    (
        panel_x + panel_w - button_w - 18.0,
        panel_y + panel_h - if mobile { 36.0 } else { 42.0 },
        button_w,
        if mobile { 26.0 } else { 28.0 },
    )
}

fn color_hex(color: BubbleColor) -> (&'static str, &'static str, &'static str) {
    match color {
        BubbleColor::Rose => ("#f04f72", "#ffb4c1", "rgba(255,255,255,0.5)"),
        BubbleColor::Gold => ("#e6b84e", "#ffe2a0", "rgba(255,255,255,0.45)"),
        BubbleColor::Sky => ("#54a8f2", "#b8dcff", "rgba(255,255,255,0.5)"),
        BubbleColor::Mint => ("#4ecf8d", "#b9f0ce", "rgba(255,255,255,0.45)"),
        BubbleColor::Violet => ("#9b6df0", "#d5c2ff", "rgba(255,255,255,0.48)"),
        BubbleColor::Coral => ("#f07a4f", "#ffc4a8", "rgba(255,255,255,0.45)"),
        BubbleColor::Wild => ("#f3f1ea", "#9cf0ff", "rgba(155,109,240,0.42)"),
        BubbleColor::Prize => ("#10261f", "#e9d18e", "rgba(255,255,255,0.46)"),
        BubbleColor::Bomb => ("#25292c", "#ffb35c", "rgba(255,255,255,0.28)"),
        BubbleColor::Lightning => ("#ffe66d", "#9cf0ff", "rgba(255,255,255,0.62)"),
        BubbleColor::Drill => ("#3d5960", "#9cf0ff", "rgba(255,255,255,0.42)"),
    }
}

fn color_name(color: BubbleColor) -> &'static str {
    match color {
        BubbleColor::Rose => "rose",
        BubbleColor::Gold => "gold",
        BubbleColor::Sky => "sky",
        BubbleColor::Mint => "mint",
        BubbleColor::Violet => "violet",
        BubbleColor::Coral => "coral",
        BubbleColor::Wild => "wild",
        BubbleColor::Prize => "prize",
        BubbleColor::Bomb => "bomb",
        BubbleColor::Lightning => "lightning",
        BubbleColor::Drill => "drill",
    }
}

fn color_call_ability_help(ability: AbilitySnapshot) -> String {
    let extra = if ability.color_sense_enabled && ability.color_sense_level > 0 {
        format!(
            " Color Sense adds {} per round.",
            ability.color_sense_level.min(3)
        )
    } else {
        String::new()
    };
    format!("Color Call: loads the largest active color.{}", extra)
}

fn bomb_shot_ability_help(ability: AbilitySnapshot) -> String {
    let radius = if ability.bomb_radius_enabled && ability.bomb_radius_level > 0 {
        " Upgraded blast radius active."
    } else {
        ""
    };
    format!(
        "Bomb Shot: loads a controlled radial blast bubble.{}",
        radius
    )
}

fn wild_shot_ability_help(ability: AbilitySnapshot) -> String {
    let match_text = if ability.wild_cache_enabled && ability.wild_cache_level >= 2 {
        " Pops every touched match."
    } else {
        " Pops the best touched match."
    };
    format!("Wild Shot: loads a wild bubble.{}", match_text)
}

fn lightning_ability_help(ability: AbilitySnapshot) -> String {
    let extra = if ability.storm_focus_enabled && ability.storm_focus_level > 0 {
        format!(
            " Storm Focus adds {} per round.",
            ability.storm_focus_level.min(2)
        )
    } else {
        String::new()
    };
    let power = if ability.lightning_power_enabled && ability.lightning_power_level > 0 {
        match ability.lightning_power_level.min(3) {
            1 => " Splashes adjacent bubbles.",
            2 => " Splashes and opens a wide upward beam.",
            _ => " Splashes and opens the widest upward beam.",
        }
    } else {
        ""
    };
    format!(
        "Lightning: chains through connected bubbles, then upward.{}{}",
        power, extra
    )
}

fn drill_ability_help(ability: AbilitySnapshot) -> String {
    let depth = 5 + ability.drill_power_level.min(3) as usize * 2;
    let power = if ability.drill_power_enabled && ability.drill_power_level > 0 {
        format!(" Active depth is {} bubbles.", depth)
    } else {
        " Base depth is 5 bubbles.".to_string()
    };
    format!(
        "Drill: punches through every bubble the shot overlaps until its depth runs out.{}",
        power
    )
}

fn drop_reset_ability_help(ability: AbilitySnapshot) -> String {
    let extra = if ability.drop_reserve_enabled && ability.drop_reserve_level > 0 {
        format!(
            " Drop Reserve adds {} per round.",
            ability.drop_reserve_level.min(3)
        )
    } else {
        String::new()
    };
    format!("Drop Reset: refills the drop counter.{}", extra)
}

fn permanent_shop_upgrades() -> [ShopUpgrade; 18] {
    kit_shop_upgrades()
}

fn kit_shop_upgrades() -> [ShopUpgrade; 18] {
    [
        ShopUpgrade::QueueSight,
        ShopUpgrade::BankSight,
        ShopUpgrade::Reticle,
        ShopUpgrade::LandingDot,
        ShopUpgrade::TraceSight,
        ShopUpgrade::SoftCeiling,
        ShopUpgrade::DropHaste,
        ShopUpgrade::PopDrop,
        ShopUpgrade::CleanStart,
        ShopUpgrade::ExtraRows,
        ShopUpgrade::BombDrop,
        ShopUpgrade::BlastRadius,
        ShopUpgrade::LightningPower,
        ShopUpgrade::DrillPower,
        ShopUpgrade::WildOrb,
        ShopUpgrade::WildCache,
        ShopUpgrade::PrizeBubbles,
        ShopUpgrade::PrizeQuality,
    ]
}

fn kit_upgrade_index(upgrade: ShopUpgrade) -> Option<usize> {
    kit_shop_upgrades()
        .iter()
        .position(|candidate| *candidate == upgrade)
}

fn shop_kits() -> [ShopKitInfo; 4] {
    [
        ShopKitInfo {
            title: "Scout",
            upgrades: &SCOUT_KIT_UPGRADES,
        },
        ShopKitInfo {
            title: "Risk",
            upgrades: &RISK_KIT_UPGRADES,
        },
        ShopKitInfo {
            title: "Breach",
            upgrades: &BREACH_KIT_UPGRADES,
        },
        ShopKitInfo {
            title: "Luck",
            upgrades: &LUCK_KIT_UPGRADES,
        },
    ]
}

fn consumable_display_title(upgrade: ShopUpgrade, width: f64) -> &'static str {
    let narrow = width < 96.0;
    match upgrade {
        ShopUpgrade::ColorCall if narrow => "Call",
        ShopUpgrade::ColorCall => "Color Call",
        ShopUpgrade::TraceSight => "Ghost",
        ShopUpgrade::Reticle => "Guide",
        ShopUpgrade::LandingDot => "Dot",
        ShopUpgrade::DropReset if narrow => "Reset",
        ShopUpgrade::DropReset => "Drop Reset",
        ShopUpgrade::CleanStart => "Clean",
        ShopUpgrade::ExtraRows => "Rows+",
        ShopUpgrade::DropHaste => "Fast",
        ShopUpgrade::PopDrop => "Pop Drop",
        ShopUpgrade::BombShot if narrow => "Bomb",
        ShopUpgrade::BombShot => "Bomb Shot",
        ShopUpgrade::WildShot if narrow => "Wild",
        ShopUpgrade::WildShot => "Wild Shot",
        ShopUpgrade::WildCache => "Focus",
        ShopUpgrade::LightningBolt if narrow => "Bolt",
        ShopUpgrade::LightningBolt => "Lightning",
        ShopUpgrade::DrillShot if narrow => "Drill",
        ShopUpgrade::DrillShot => "Drill Shot",
        ShopUpgrade::Revival if narrow => "Breath",
        ShopUpgrade::Revival => "2nd Breath",
        ShopUpgrade::WildOrb => "Wild",
        ShopUpgrade::BombDrop => "Fuse",
        ShopUpgrade::BlastRadius => "Blast",
        ShopUpgrade::SoftCeiling => "Soft",
        ShopUpgrade::QueueSight => "Queue",
        ShopUpgrade::PrizeBubbles => "Prize",
        ShopUpgrade::PrizeQuality => "Quality",
        ShopUpgrade::BankSight => "Bank",
        ShopUpgrade::LightningPower => "Storm",
        ShopUpgrade::DrillPower => "Drill+",
    }
}

fn short_upgrade_label(upgrade: ShopUpgrade) -> &'static str {
    match upgrade {
        ShopUpgrade::WildOrb => "wild",
        ShopUpgrade::WildCache => "focus",
        ShopUpgrade::BombDrop => "fuse",
        ShopUpgrade::SoftCeiling => "soft",
        ShopUpgrade::DropHaste => "fast",
        ShopUpgrade::PopDrop => "pressure",
        ShopUpgrade::CleanStart => "clean",
        ShopUpgrade::ExtraRows => "rows+",
        ShopUpgrade::QueueSight => "queue",
        ShopUpgrade::TraceSight => "preview",
        ShopUpgrade::Reticle => "guide",
        ShopUpgrade::LandingDot => "dot",
        ShopUpgrade::PrizeBubbles => "prize",
        ShopUpgrade::BankSight => "bank",
        ShopUpgrade::ColorCall => "call",
        ShopUpgrade::BombShot => "bomb",
        ShopUpgrade::WildShot => "wild",
        ShopUpgrade::LightningBolt => "bolt",
        ShopUpgrade::DrillShot => "drill",
        ShopUpgrade::DropReset => "reset",
        ShopUpgrade::Revival => "breath",
        ShopUpgrade::BlastRadius => "blast",
        ShopUpgrade::PrizeQuality => "quality",
        ShopUpgrade::LightningPower => "storm",
        ShopUpgrade::DrillPower => "deep",
    }
}

fn compact_owned_chip_label(upgrade: ShopUpgrade, width: f64) -> &'static str {
    if width >= 58.0 {
        return short_upgrade_label(upgrade);
    }

    match upgrade {
        ShopUpgrade::WildOrb => "wild",
        ShopUpgrade::WildCache => "wld+",
        ShopUpgrade::BombDrop => "fuse",
        ShopUpgrade::SoftCeiling => "soft",
        ShopUpgrade::DropHaste => "fast",
        ShopUpgrade::PopDrop => "pop",
        ShopUpgrade::CleanStart => "cln",
        ShopUpgrade::ExtraRows => "row+",
        ShopUpgrade::QueueSight => "q",
        ShopUpgrade::TraceSight => "ghst",
        ShopUpgrade::Reticle => "guid",
        ShopUpgrade::LandingDot => "dot",
        ShopUpgrade::PrizeBubbles => "?",
        ShopUpgrade::BankSight => "bank",
        ShopUpgrade::ColorCall => "call",
        ShopUpgrade::BombShot => "bomb",
        ShopUpgrade::WildShot => "wild",
        ShopUpgrade::LightningBolt => "bolt",
        ShopUpgrade::DrillShot => "drill",
        ShopUpgrade::DropReset => "rst",
        ShopUpgrade::Revival => "2nd",
        ShopUpgrade::BlastRadius => "boom",
        ShopUpgrade::PrizeQuality => "+?",
        ShopUpgrade::LightningPower => "zap+",
        ShopUpgrade::DrillPower => "drl+",
    }
}

fn distance(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

fn load_high_score() -> u64 {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(HIGH_SCORE_STORAGE_KEY).ok().flatten())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn save_high_score(score: u64) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(HIGH_SCORE_STORAGE_KEY, &score.to_string());
    }
}

fn load_progress() -> BubbleProgress {
    let progress = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(PROGRESS_STORAGE_KEY).ok().flatten())
        .and_then(|raw| serde_json::from_str::<BubbleProgress>(&raw).ok())
        .unwrap_or_default();

    if progress.version == CURRENT_PROGRESS_VERSION {
        progress.normalized()
    } else {
        BubbleProgress::default().normalized()
    }
}

fn save_progress(progress: &BubbleProgress) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let Ok(json) = serde_json::to_string(&progress.clone().normalized()) else {
        return;
    };
    let _ = storage.set_item(PROGRESS_STORAGE_KEY, &json);
}

fn save_storage_key(hard: bool) -> &'static str {
    if hard {
        HARD_SAVE_STORAGE_KEY
    } else {
        NORMAL_SAVE_STORAGE_KEY
    }
}

fn load_saved_game(hard: bool, high_score: u64) -> Option<SavedBubbleGame> {
    let saved_game = load_raw_saved_game(hard)?;
    BubbleGame::from_save(saved_game.clone(), high_score, hard).map(|_| saved_game)
}

fn load_raw_saved_game(hard: bool) -> Option<SavedBubbleGame> {
    let raw = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(save_storage_key(hard)).ok().flatten())?;

    serde_json::from_str::<SavedBubbleGame>(&raw).ok()
}

fn stored_save_allows_resume(
    stored_game: Option<&SavedBubbleGame>,
    saved_game: &SavedBubbleGame,
    current_game: &SavedBubbleGame,
) -> bool {
    stored_game == Some(saved_game) || stored_game == Some(current_game)
}

fn save_game(hard: bool, game: &BubbleGame) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let Ok(json) = serde_json::to_string(&game.save_state()) else {
        return;
    };
    let _ = storage.set_item(save_storage_key(hard), &json);
}

pub fn clear_high_score() {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.remove_item(HIGH_SCORE_STORAGE_KEY);
        let _ = storage.remove_item(NORMAL_SAVE_STORAGE_KEY);
        let _ = storage.remove_item(HARD_SAVE_STORAGE_KEY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reticle_level_one_doubles_default_aim_guide() {
        let (default_distance, default_bounces) = aim_trace_config(0, 0);
        let (level_one_distance, level_one_bounces) = aim_trace_config(1, 0);

        assert_eq!(default_distance, Some(DEFAULT_TRACE_DISTANCE_RADIUS));
        assert_eq!(
            level_one_distance,
            Some(DEFAULT_TRACE_DISTANCE_RADIUS * LEVEL_ONE_TRACE_MULTIPLIER)
        );
        assert_eq!(default_bounces, BASE_TRACE_BOUNCES);
        assert_eq!(level_one_bounces, BASE_TRACE_BOUNCES);
    }

    #[test]
    fn reticle_level_two_has_full_aim_guide() {
        let (distance, bounces) = aim_trace_config(2, 0);

        assert_eq!(distance, None);
        assert_eq!(bounces, FULL_TRACE_BOUNCES);
    }

    #[test]
    fn stored_save_allows_original_or_current_game_only() {
        let original = BubbleGame::new(11, 0).save_state();

        let mut current_game = BubbleGame::new(22, 0);
        current_game.hold();
        let current = current_game.save_state();

        let other = BubbleGame::new(33, 0).save_state();

        assert!(stored_save_allows_resume(
            Some(&original),
            &original,
            &current
        ));
        assert!(stored_save_allows_resume(
            Some(&current),
            &original,
            &current
        ));
        assert!(!stored_save_allows_resume(
            Some(&other),
            &original,
            &current
        ));
        assert!(!stored_save_allows_resume(None, &original, &current));
    }
}
