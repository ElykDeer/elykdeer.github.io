use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

use leptos::html::Canvas;
use leptos::prelude::*;
use leptos::reactive::owner::on_cleanup;
use send_wrapper::SendWrapper;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent};

use super::core::{
    Board, BubbleColor, BubbleGame, CellCoord, Resolution, SavedBubbleGame, COLS, ROWS,
};

const HIGH_SCORE_STORAGE_KEY: &str = "elyk.bubbles.high-score";
const NORMAL_SAVE_STORAGE_KEY: &str = "elyk.bubbles.save.normal";
const HARD_SAVE_STORAGE_KEY: &str = "elyk.bubbles.save.hard";
const RESUME_OFFER_MOVES: u8 = 5;

/// Canvas implementation for the terminal bubbles game.
#[component]
pub fn BubblesGame(#[prop(default = false)] hard: bool) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let high_score = load_high_score();
    let saved_game = load_saved_game(hard, high_score);
    let resume_offer_visible = RwSignal::new(saved_game.is_some());
    let runner = Rc::new(RefCell::new(CanvasRunner::new(
        high_score, hard, saved_game,
    )));
    let status_text = RwSignal::new(runner.borrow().status_text());
    let animation = Rc::new(RefCell::new(AnimationLoop::default()));

    let start_runner = Rc::clone(&runner);
    let start_animation = Rc::clone(&animation);
    let frame_resume_offer_visible = resume_offer_visible;
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
                let resume_visible = runner.resume_offer_visible();
                if frame_resume_offer_visible.get_untracked() != resume_visible {
                    frame_resume_offer_visible.set(resume_visible);
                }
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
    on_cleanup(move || {
        cleanup_animation.borrow_mut().cancel();
    });

    let move_runner = Rc::clone(&runner);
    let handle_mouse_move = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            move_runner.borrow_mut().aim_from_client(
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
            touch_move_runner.borrow_mut().aim_from_client(
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
            touch_start_runner.borrow_mut().aim_from_client(
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
        "h" | "H" => {
            ev.prevent_default();
            key_runner.borrow_mut().hold();
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
            <button
                type="button"
                class="bubbles-resume-button"
                class:bubbles-resume-button-hidden=move || !resume_offer_visible.get()
                aria-hidden=move || (!resume_offer_visible.get()).to_string()
                disabled=move || !resume_offer_visible.get()
                tabindex=move || if resume_offer_visible.get() { "0" } else { "-1" }
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
        let top = 74.0;
        let bottom = 76.0;
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
            cannon_y: height - 37.0,
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
    life: f64,
    max_life: f64,
}

const GRAVITY: f64 = 660.0;
const DROP_SLIDE: f64 = 0.20;
/// Center distance (in radii) at which a shot attaches. Edge contact is 2.0, so
/// less than that lets a shot slip a bit further into a gap before snapping.
const COLLISION_TOLERANCE: f64 = 1.7;

#[derive(Debug)]
struct CanvasRunner {
    game: BubbleGame,
    hard: bool,
    resume_offer: ResumeOffer,
    aim_angle: f64,
    projectile: Option<Projectile>,
    particles: Vec<Particle>,
    drop_slide: f64,
    last_timestamp: Option<f64>,
    frame_dt: f64,
    message: String,
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
        Self {
            game: if hard {
                BubbleGame::hard(random_seed(), high_score)
            } else {
                BubbleGame::new(random_seed(), high_score)
            },
            hard,
            resume_offer: ResumeOffer::new(saved_game),
            aim_angle: -PI / 2.0,
            projectile: None,
            particles: Vec::new(),
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
        let (hold_x, hold_y) = hold_slot_center(layout);
        if distance(
            client_x - rect.left(),
            client_y - rect.top(),
            hold_x,
            hold_y,
        ) <= layout.radius * 1.5
        {
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

    fn nudge_aim(&mut self, delta: f64) {
        self.aim_angle = clamp_aim(self.aim_angle + delta);
    }

    fn shoot_or_restart(&mut self) {
        if self.game.game_over {
            let high_score = self.game.high_score;
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

        let Some(context) = canvas_context(canvas) else {
            return;
        };

        // Slide newly dropped rows down from one row up as the animation eases out.
        let row_offset = -layout.row_gap * (self.drop_slide / DROP_SLIDE);

        draw_background(&context, layout);
        draw_header(&context, layout, &self.game, &self.message);
        draw_aim(&context, layout, &self.game.board, self.aim_angle);
        draw_board(&context, layout, &self.game.board, row_offset);
        draw_particles(&context, &self.particles);
        draw_cannon(
            &context,
            layout,
            self.aim_angle,
            self.game.current_color(),
            self.game.next_colors(),
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
            draw_overlay(
                &context,
                layout,
                self.game.board.is_cleared(),
                self.game.score,
            );
        }
    }

    fn step_animations(&mut self) {
        let dt = self.frame_dt;
        self.drop_slide = (self.drop_slide - dt).max(0.0);
        for particle in &mut self.particles {
            particle.vy += GRAVITY * dt;
            particle.x += particle.vx * dt;
            particle.y += particle.vy * dt;
            particle.life -= dt;
        }
        self.particles.retain(|particle| particle.life > 0.0);
    }

    fn step_projectile(&mut self, layout: BubbleLayout) {
        let Some(mut projectile) = self.projectile.take() else {
            return;
        };

        let dt = self.frame_dt;
        projectile.x += projectile.vx * dt;
        projectile.y += projectile.vy * dt;

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

        if let Some(hit) = self.hit_existing_bubble(projectile, layout) {
            self.attach_projectile(projectile, layout, AttachmentTarget::Bubble(hit));
            return;
        }

        self.projectile = Some(projectile);
    }

    fn hit_existing_bubble(
        &self,
        projectile: Projectile,
        layout: BubbleLayout,
    ) -> Option<CellCoord> {
        self.game
            .board
            .occupied_cells()
            .filter_map(|coord| {
                let (x, y) = layout.center(coord, self.game.board.parity());
                let hit_distance = distance(projectile.x, projectile.y, x, y);
                // Below 2r (edge contact) so a shot can squeeze a little into gaps.
                (hit_distance <= layout.radius * COLLISION_TOLERANCE)
                    .then_some((coord, hit_distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(coord, _)| coord)
    }

    fn attach_projectile(
        &mut self,
        projectile: Projectile,
        layout: BubbleLayout,
        target: AttachmentTarget,
    ) {
        // No room to land: discard the shot rather than ending the game. Losing
        // only happens when a dropped row overflows past the bottom.
        let Some(coord) =
            attachment_cell(&self.game.board, layout, projectile.x, projectile.y, target)
        else {
            self.projectile = None;
            self.message = "No room to land. Shot discarded.".to_string();
            self.game.finish_discarded_shot();
            self.finish_move();
            return;
        };

        let before_score = self.game.score;
        let parity_before = self.game.board.parity();
        let resolution = self.game.attach_color(coord, projectile.color);
        let gained = self.game.score.saturating_sub(before_score);

        self.spawn_pop_particles(layout, &resolution);
        // A parity flip means a new row was pushed in: ease it down.
        if self.game.board.parity() != parity_before {
            self.drop_slide = DROP_SLIDE;
        }

        self.message = if resolution.popped.is_empty() {
            format!("Attached. Drop in {} shots.", self.game.shots_until_drop)
        } else {
            format!(
                "Popped {}. Dropped {}. +{}",
                resolution.popped.len(),
                resolution.dropped.len(),
                gained
            )
        };

        self.projectile = None;
        self.finish_move();
    }

    fn spawn_pop_particles(&mut self, layout: BubbleLayout, resolution: &Resolution) {
        // Popped bubbles burst into shards; dropped clusters fall away whole.
        for (coord, color) in &resolution.popped {
            let (x, y) = layout.center(*coord, self.game.board.parity());
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
                    life: 0.50,
                    max_life: 0.50,
                });
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
                life: 0.82,
                max_life: 0.82,
            });
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum AttachmentTarget {
    Ceiling,
    Bubble(CellCoord),
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
) {
    // Monospace at 14px is ~8.4px per glyph — accurate enough to center/right-align.
    let text_width = |text: &str| text.chars().count() as f64 * 8.4;

    context.set_font("14px SFMono-Regular, Consolas, monospace");

    // One header line: score/best on the left, the title centered, drop on the right.
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
        layout.width - 16.0 - text_width(&drop_text),
        28.0,
    );

    context.set_fill_style_str("#e9d18e");
    let _ = context.fill_text(
        "bubbles",
        (layout.width - text_width("bubbles")) / 2.0,
        28.0,
    );

    context.set_fill_style_str("rgba(199, 223, 201, 0.82)");
    let _ = context.fill_text(message, 16.0, layout.height - 13.0);
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

fn draw_particles(context: &CanvasRenderingContext2d, particles: &[Particle]) {
    for particle in particles {
        let alpha = (particle.life / particle.max_life).clamp(0.0, 1.0);
        draw_bubble(
            context,
            particle.x,
            particle.y,
            particle.radius,
            particle.color,
            alpha,
        );
    }
}

fn draw_aim(context: &CanvasRenderingContext2d, layout: BubbleLayout, board: &Board, angle: f64) {
    let mut x = layout.cannon_x;
    let mut y = layout.cannon_y;
    let mut vx = angle.cos();
    let vy = angle.sin();
    let step = layout.radius * 0.86;

    context.set_fill_style_str("rgba(233, 209, 142, 0.72)");
    for _ in 0..55 {
        x += vx * step;
        y += vy * step;

        let left_wall = layout.left + layout.radius;
        let right_wall = layout.width - layout.left - layout.radius;
        if x <= left_wall {
            x = left_wall;
            vx = vx.abs();
        } else if x >= right_wall {
            x = right_wall;
            vx = -vx.abs();
        }

        context.begin_path();
        let _ = context.arc(x, y, 1.9, 0.0, PI * 2.0);
        context.fill();

        if y <= layout.top + layout.radius
            || board.occupied_cells().any(|coord| {
                let (cx, cy) = layout.center(coord, board.parity());
                distance(x, y, cx, cy) <= layout.radius * COLLISION_TOLERANCE
            })
        {
            break;
        }
    }
}

/// Center of the on-screen hold slot, shared by the renderer and the tap hit-test.
fn hold_slot_center(layout: BubbleLayout) -> (f64, f64) {
    let queue_radius = layout.radius * 0.62;
    (
        layout.cannon_x - layout.radius * 1.05 - 18.0 - queue_radius,
        layout.cannon_y,
    )
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
    let queue_radius = layout.radius * 0.62;
    let queue_gap = queue_radius * 2.0 + 6.0;
    let queue_x = layout.cannon_x + layout.radius * 1.05 + 18.0 + queue_radius;
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
            queue_x + index as f64 * queue_gap,
            layout.cannon_y,
            queue_radius,
            *color,
            if index == 0 { 1.0 } else { 0.78 },
        );
    }

    // Held bubble (tap / right-click / H to swap) sits to the left of the cannon.
    let (hold_x, _) = hold_slot_center(layout);
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

fn color_hex(color: BubbleColor) -> (&'static str, &'static str, &'static str) {
    match color {
        BubbleColor::Rose => ("#f04f72", "#ffb4c1", "rgba(255,255,255,0.5)"),
        BubbleColor::Gold => ("#e6b84e", "#ffe2a0", "rgba(255,255,255,0.45)"),
        BubbleColor::Sky => ("#54a8f2", "#b8dcff", "rgba(255,255,255,0.5)"),
        BubbleColor::Mint => ("#4ecf8d", "#b9f0ce", "rgba(255,255,255,0.45)"),
        BubbleColor::Violet => ("#9b6df0", "#d5c2ff", "rgba(255,255,255,0.48)"),
        BubbleColor::Coral => ("#f07a4f", "#ffc4a8", "rgba(255,255,255,0.45)"),
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
