use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use leptos::ev::SubmitEvent;
use leptos::html::{Div, Textarea};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{FocusEvent, KeyboardEvent, MouseEvent};

use crate::commands;
use crate::game::{BubblesGame, TextropolisGame};
use crate::nano_editor::NanoEditorPanel;
use crate::save_manager::SaveManagerPanel;
use crate::storage::{load_profile, save_profile};
use crate::terminal::{
    common_prefix, complete_terminal_tab, parse_ansi_fragments, parse_line, AnsiFragment,
    ConsolePipe, ConsoleScriptInvocation, FetchInvocation, OutputBlock, ParseError, ParsedCommand,
    PythonFileInvocation, TerminalContext,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TerminalEntry {
    id: usize,
    prompt: String,
    output: Vec<OutputBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryDirection {
    Previous,
    Next,
}

#[component]
pub fn App() -> impl IntoView {
    let mut initial_context = TerminalContext::new();
    initial_context.set_command_metadata(commands::command_metadata());
    let (ctx, set_ctx) = signal(initial_context);
    let command_input = RwSignal::new(String::new());
    let history_cursor = RwSignal::new(None::<usize>);
    let history_draft = RwSignal::new(String::new());
    let transcript = RwSignal::new(Vec::<TerminalEntry>::new());
    let next_id = RwSignal::new(0usize);
    let profile_ready = RwSignal::new(false);

    // IndexedDB is async; hold command input until the stored profile has been
    // loaded so early commands cannot be overwritten by the arriving profile.
    spawn_local(async move {
        let error = match load_profile().await {
            Ok(profile) => {
                let mut error = None;
                set_ctx.update(|ctx| {
                    if let Err(err) = ctx.replace_profile(profile) {
                        error = Some(err.to_string());
                    }
                });
                error
            }
            Err(err) => Some(err.to_string()),
        };
        profile_ready.set(true);
        if let Some(message) = error {
            append_entry(
                transcript,
                next_id,
                "system".to_string(),
                vec![OutputBlock::Error(format!(
                    "Stored profile could not be loaded: {message}"
                ))],
            );
        }
    });
    let terminal_window_ref = NodeRef::<Div>::new();
    let command_input_ref = NodeRef::<Textarea>::new();
    let python_mode = RwSignal::new(false);
    let rc_ran = RwSignal::new(false);
    // While true, submitted commands run without echoing their prompt or being
    // recorded in history (used for the auto-run `.rc`).
    let quiet = RwSignal::new(false);

    Effect::new(move |_| {
        let _ = transcript.get();
        // Defer until after the new output has been laid out so we scroll to its
        // real height rather than the previous frame's.
        request_animation_frame(move || {
            if let Some(window) = terminal_window_ref.get_untracked() {
                window.set_scroll_top(window.scroll_height());
            }
        });
    });

    // Push the input signal to the textarea only when it actually differs (e.g.
    // history recall, clear, completion). Writing on every keystroke would reset
    // the caret to the end and break editing inside a multiline block.
    Effect::new(move |_| {
        let value = command_input.get();
        if let Some(textarea) = command_input_ref.get() {
            if textarea.value() != value {
                textarea.set_value(&value);
            }
        }
    });

    let submit_command = move || {
        // The textarea holds the whole (possibly multiline) command; keep it raw
        // so Python sees real indentation.
        let raw = command_input.get();
        let line = raw.trim().to_string();
        let in_python = python_mode.get_untracked();

        if line.is_empty() {
            return;
        }

        history_cursor.set(None);
        history_draft.set(String::new());

        if in_python {
            command_input.set(String::new());
            submit_python(
                ctx,
                set_ctx,
                transcript,
                next_id,
                python_mode,
                command_input,
                raw,
            );
            return;
        }

        command_input.set(String::new());
        let quiet_run = quiet.get_untracked();
        let prompt = if quiet_run {
            String::new()
        } else {
            ctx.with(|ctx| format!("{} {}", format_prompt(ctx.cwd()), line))
        };
        let mut output = Vec::new();
        let mut clear_requested = false;
        let mut python_requested = false;
        let mut pip_request = None;
        let mut console_script_request = None;
        let mut fetch_request = None;
        let mut python_file_request = None;
        let mut console_pipe_request = None;
        let mut profile = None;

        set_ctx.update(|ctx| {
            if !quiet_run {
                ctx.record_history(&line);
            }
            output = run_shell_line(ctx, &line);
            ctx.sync_profile_from_runtime();
            clear_requested = ctx.take_clear_requested();
            python_requested = ctx.take_python_requested();
            pip_request = ctx.take_pip_request();
            console_script_request = ctx.take_console_script_request();
            fetch_request = ctx.take_fetch_request();
            python_file_request = ctx.take_python_file_request();
            console_pipe_request = ctx.take_console_pipe_request();
            profile = Some(ctx.profile().clone());
        });

        if output_launches_game(&output) {
            close_open_games(transcript);
            blur_command_input(command_input_ref);
        }

        if let Some(profile) = profile {
            persist_profile(profile, transcript, next_id);
        }

        if let Some(packages) = pip_request {
            run_pip_install(ctx, set_ctx, transcript, next_id, packages);
        }

        if let Some(invocation) = console_script_request {
            let entry_id = append_entry(transcript, next_id, prompt, output);
            run_console_script(ctx, set_ctx, transcript, next_id, entry_id, invocation);
            return;
        }

        if let Some(invocation) = fetch_request {
            let entry_id = append_entry(transcript, next_id, prompt, output);
            run_fetch(set_ctx, transcript, next_id, entry_id, invocation);
            return;
        }

        if let Some(invocation) = python_file_request {
            let entry_id = append_entry(transcript, next_id, prompt, output);
            run_python_file(ctx, set_ctx, transcript, next_id, entry_id, invocation);
            return;
        }

        if let Some(pipe) = console_pipe_request {
            let entry_id = append_entry(transcript, next_id, prompt, output);
            run_console_pipe(ctx, set_ctx, transcript, next_id, entry_id, pipe);
            return;
        }

        if clear_requested {
            transcript.set(Vec::new());
            if output.is_empty() {
                return;
            }
        }

        if python_requested {
            python_mode.set(true);
            // Pre-warm the runtime so the first prompt isn't blocked on the download.
            spawn_local(async {
                let _ = crate::python::ensure_loaded().await;
            });
        }

        // A quiet command (from `.rc`) with no output adds nothing to the screen.
        if quiet_run && output.is_empty() {
            return;
        }
        append_entry(transcript, next_id, prompt, output);
    };

    // Run /root/.rc on the first click into the terminal (not on load): each line
    // is run like typed input, so it supports pipes, python3, and package scripts.
    let focus_terminal_input = move |ev: MouseEvent| {
        if should_focus_terminal_input(&ev) {
            focus_command_input(command_input_ref);
        }
        if rc_ran.get_untracked() || !profile_ready.get_untracked() {
            return;
        }
        rc_ran.set(true);
        let rc = ctx.with_untracked(|ctx| {
            ctx.vfs()
                .read_file("/", "/root/.rc")
                .ok()
                .map(|file| file.content)
        });
        if let Some(content) = rc {
            quiet.set(true);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                command_input.set(line.to_string());
                submit_command();
            }
            quiet.set(false);
        }
    };

    let handle_command_keydown = move |ev: KeyboardEvent| {
        let key = ev.key();

        // Enter submits; Shift+Enter falls through to the textarea's own newline.
        if key == "Enter" && !ev.shift_key() && !ev.ctrl_key() && !ev.alt_key() && !ev.meta_key() {
            ev.prevent_default();
            submit_command();
            return;
        }

        if python_mode.get_untracked()
            && key.eq_ignore_ascii_case("d")
            && ev.ctrl_key()
            && !ev.alt_key()
            && !ev.meta_key()
        {
            ev.prevent_default();
            command_input.set(String::new());
            history_cursor.set(None);
            history_draft.set(String::new());
            python_mode.set(false);
            append_entry(
                transcript,
                next_id,
                ">>> ^D".to_string(),
                vec![OutputBlock::Text("Returned to shell.".to_string())],
            );
            return;
        }

        // Ctrl-C in the REPL clears the half-typed block but stays in Python.
        if python_mode.get_untracked()
            && key.eq_ignore_ascii_case("c")
            && ev.ctrl_key()
            && !ev.alt_key()
            && !ev.meta_key()
        {
            ev.prevent_default();
            let typed = command_input.get_untracked();
            command_input.set(String::new());
            append_entry(
                transcript,
                next_id,
                format!(">>> {typed}^C"),
                vec![OutputBlock::Text("KeyboardInterrupt".to_string())],
            );
            return;
        }

        // Arrows recall history only at the top/bottom line of the textarea;
        // otherwise they move the cursor between lines as usual.
        if (key == "ArrowUp" || key == "Up")
            && !has_native_navigation_modifier(&ev)
            && cursor_on_first_line(command_input_ref)
        {
            ev.prevent_default();
            recall_history(
                HistoryDirection::Previous,
                ctx,
                command_input,
                history_cursor,
                history_draft,
                python_mode,
            );
            return;
        }

        if (key == "ArrowDown" || key == "Down")
            && !has_native_navigation_modifier(&ev)
            && cursor_on_last_line(command_input_ref)
        {
            ev.prevent_default();
            recall_history(
                HistoryDirection::Next,
                ctx,
                command_input,
                history_cursor,
                history_draft,
                python_mode,
            );
            return;
        }

        if key.eq_ignore_ascii_case("l") && ev.ctrl_key() && !ev.alt_key() && !ev.meta_key() {
            ev.prevent_default();
            history_cursor.set(None);
            history_draft.set(String::new());
            transcript.set(Vec::new());
            return;
        }

        if key.eq_ignore_ascii_case("c") && ev.ctrl_key() && !ev.alt_key() && !ev.meta_key() {
            ev.prevent_default();
            let line = command_input.get_untracked();
            command_input.set(String::new());
            history_cursor.set(None);
            history_draft.set(String::new());
            let prompt = ctx.with(|ctx| format_interrupt_prompt(ctx.cwd(), &line));
            append_entry(transcript, next_id, prompt, Vec::new());
            return;
        }

        if key.eq_ignore_ascii_case("r") && ev.ctrl_key() {
            ev.prevent_default();
            let query = command_input.get_untracked();
            let in_python = python_mode.get_untracked();
            let match_from_history = ctx.with(|ctx| {
                active_history(ctx, in_python)
                    .iter()
                    .rev()
                    .find(|line| query.is_empty() || line.contains(&query))
                    .cloned()
            });

            if let Some(line) = match_from_history {
                history_cursor.set(None);
                command_input.set(line);
            }
            return;
        }

        if key == "Tab" && python_mode.get_untracked() {
            ev.prevent_default();
            let line = command_input.get_untracked();
            spawn_local(async move {
                let Ok(completion) = crate::python::complete(&line).await else {
                    return;
                };
                complete_python_tab(command_input, transcript, next_id, &line, completion);
            });
            return;
        }

        if key == "Tab" {
            ev.prevent_default();
            let line = command_input.get_untracked();
            if let Some(action) = ctx.with(|ctx| complete_terminal_tab(ctx, &line)) {
                history_cursor.set(None);
                if let Some(completed) = action.replacement {
                    command_input.set(completed);
                }
                if let Some(output) = action.output {
                    let prompt = ctx.with(|ctx| format!("{} {}", format_prompt(ctx.cwd()), line));
                    append_entry(transcript, next_id, prompt, vec![output]);
                }
            }
        }
    };

    let handle_history_previous_focus = move |ev: FocusEvent| {
        ev.prevent_default();
        recall_history(
            HistoryDirection::Previous,
            ctx,
            command_input,
            history_cursor,
            history_draft,
            python_mode,
        );
        focus_command_input(command_input_ref);
    };

    let handle_history_next_focus = move |ev: FocusEvent| {
        ev.prevent_default();
        recall_history(
            HistoryDirection::Next,
            ctx,
            command_input,
            history_cursor,
            history_draft,
            python_mode,
        );
        focus_command_input(command_input_ref);
    };

    view! {
        <main class="site-shell">
            <section class="terminal-surface" aria-label="Terminal" on:click=focus_terminal_input>
                <div class="terminal-window" role="log" aria-live="polite" node_ref=terminal_window_ref>
                    <For
                        each={move || transcript.get()}
                        key=|entry| entry.id
                        children={move |entry| {
                            let entry_id = entry.id;
                            // Read prompt and output live from the transcript: a multiline
                            // block grows its prompt and the REPL fills output in later, and
                            // a keyed `For` won't refresh a child from a captured clone.
                            let prompt = move || {
                                transcript.with(|entries| {
                                    entries
                                        .iter()
                                        .find(|candidate| candidate.id == entry_id)
                                        .map(|candidate| candidate.prompt.clone())
                                        .unwrap_or_default()
                                })
                            };
                            let output = move || {
                                transcript.with(|entries| {
                                    entries
                                        .iter()
                                        .find(|candidate| candidate.id == entry_id)
                                        .map(|candidate| candidate.output.clone())
                                        .unwrap_or_default()
                                })
                            };
                            view! {
                                <article class="terminal-entry">
                                    <div class="terminal-prompt">{prompt}</div>
                                    <div class="terminal-output">
                                        <For
                                            each={move || keyed_output_blocks(output())}
                                            key=|(key, _)| key.clone()
                                            children={move |(_, block)| {
                                                view! {
                                                    <OutputBlockView
                                                        block=block
                                                        ctx=ctx
                                                        set_ctx=set_ctx
                                                        transcript=transcript
                                                        next_id=next_id
                                                    />
                                                }
                                            }}
                                        />
                                    </div>
                                </article>
                            }
                        }}
                    />
                    <form class="terminal-input-row" on:submit={move |ev: SubmitEvent| {
                        ev.prevent_default();
                        submit_command();
                    }}>
                        <label class="terminal-input-label" for="terminal-command">
                            {move || {
                                if python_mode.get() {
                                    ">>>".to_string()
                                } else {
                                    ctx.with(|ctx| format_prompt(ctx.cwd()))
                                }
                            }}
                        </label>
                        <input
                            class="terminal-history-sentinel"
                            type="text"
                            tabindex="0"
                            inputmode="none"
                            autocomplete="off"
                            aria-label="Previous command"
                            on:focus=handle_history_previous_focus
                        />
                        <textarea
                            id="terminal-command"
                            class="terminal-input"
                            rows={move || command_input.with(|value| value.matches('\n').count() + 1).to_string()}
                            autocomplete="off"
                            autocapitalize="none"
                            enterkeyhint="enter"
                            spellcheck="false"
                            disabled={move || !profile_ready.get()}
                            node_ref=command_input_ref
                            on:input={move |ev| {
                                history_cursor.set(None);
                                command_input.set(event_target_value(&ev));
                            }}
                            on:keydown=handle_command_keydown
                        ></textarea>
                        <input
                            class="terminal-history-sentinel"
                            type="text"
                            tabindex="0"
                            inputmode="none"
                            autocomplete="off"
                            aria-label="Next command"
                            on:focus=handle_history_next_focus
                        />
                        <button class="terminal-submit" type="submit" disabled={move || !profile_ready.get()}>"Run"</button>
                    </form>
                </div>
            </section>

            <section
                class={move || {
                    if ctx.with(|ctx| ctx.overlay_visible()) {
                        "profile-overlay"
                    } else {
                        "profile-overlay profile-overlay-hidden"
                    }
                }}
                aria-labelledby="profile-name"
                aria-hidden={move || (!ctx.with(|ctx| ctx.overlay_visible())).to_string()}
            >
                <h1 id="profile-name">"Kyle Martin"</h1>
                <p class="profile-role">"Computer Engineer"</p>

                <nav class="contact-links" aria-label="Contact links">
                    <a
                        aria-label="GitHub"
                        title="GitHub"
                        href="https://github.com/ElykDeer"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <svg class="contact-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path
                                fill="currentColor"
                                d="M12 0.7C5.8 0.7 0.8 5.7 0.8 11.9c0 5 3.2 9.2 7.7 10.7 0.6 0.1 0.8-0.2 0.8-0.6v-2c-3.1 0.7-3.8-1.3-3.8-1.3-0.5-1.3-1.3-1.7-1.3-1.7-1-0.7 0.1-0.7 0.1-0.7 1.1 0.1 1.7 1.2 1.7 1.2 1 1.7 2.6 1.2 3.3 0.9 0.1-0.7 0.4-1.2 0.7-1.5-2.5-0.3-5.1-1.2-5.1-5.5 0-1.2 0.4-2.2 1.2-3-0.1-0.3-0.5-1.4 0.1-3 0 0 0.9-0.3 3.1 1.2 0.9-0.3 1.8-0.4 2.8-0.4s1.9 0.1 2.8 0.4c2.1-1.4 3.1-1.2 3.1-1.2 0.6 1.6 0.2 2.7 0.1 3 0.7 0.8 1.2 1.8 1.2 3 0 4.3-2.6 5.2-5.1 5.5 0.4 0.3 0.8 1 0.8 2v3.1c0 0.3 0.2 0.7 0.8 0.6 4.5-1.5 7.7-5.7 7.7-10.7C23.2 5.7 18.2 0.7 12 0.7Z"
                            />
                        </svg>
                    </a>
                    <a
                        aria-label="Bluesky"
                        title="Bluesky"
                        href="https://bsky.app/profile/elykdeer.bsky.social"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <svg class="contact-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path
                                fill="currentColor"
                                d="M5.1 3.1c2.8 2.1 5.8 6.4 6.9 8.7 1.1-2.3 4.1-6.6 6.9-8.7 2-1.5 5.1-2.7 5.1 1 0 0.7-0.4 6.1-0.7 7-0.9 3.3-4.1 4.1-7 3.6 5 0.9 6.3 3.9 3.5 6.9-5.3 5.5-7.6-1.4-8.2-3.2-0.1-0.3-0.2-0.5-0.2-0.4 0 0-0.1 0.2-0.2 0.4-0.6 1.8-2.9 8.7-8.2 3.2-2.8-3-1.5-6 3.5-6.9-2.9 0.5-6.1-0.3-7-3.6-0.2-0.9-0.7-6.3-0.7-7 0-3.7 3.1-2.5 5.1-1Z"
                            />
                        </svg>
                    </a>
                    <a
                        aria-label="Twitter"
                        title="Twitter"
                        href="https://twitter.com/elykdeer"
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <svg class="contact-icon contact-icon-expanded" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path
                                fill="currentColor"
                                d="M20.8 6.1c-0.6 0.3-1.3 0.5-2 0.6 0.7-0.4 1.2-1.1 1.5-1.9-0.7 0.4-1.4 0.7-2.2 0.8-0.6-0.7-1.5-1.1-2.5-1.1-1.9 0-3.4 1.5-3.4 3.4 0 0.3 0 0.5 0.1 0.8-2.8-0.1-5.3-1.5-7-3.6-0.3 0.5-0.5 1.1-0.5 1.7 0 1.2 0.6 2.2 1.5 2.8-0.6 0-1.1-0.2-1.5-0.4v0.1c0 1.6 1.2 3 2.7 3.3-0.3 0.1-0.6 0.1-0.9 0.1-0.2 0-0.4 0-0.6-0.1 0.4 1.4 1.7 2.3 3.2 2.4-1.2 0.9-2.7 1.5-4.3 1.5-0.3 0-0.6 0-0.8-0.1 1.5 1 3.4 1.6 5.3 1.6 6.4 0 9.9-5.3 9.9-9.9v-0.4c0.6-0.5 1.1-1 1.5-1.6Z"
                            />
                        </svg>
                    </a>
                    <a aria-label="Email" title="Email" href="mailto:martinrkyle@gmail.com">
                        <svg class="contact-icon contact-icon-expanded" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                            <path
                                d="M3.5 6.5h17v11h-17v-11Z"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linejoin="miter"
                            />
                            <path
                                d="M4.5 7.5 12 13l7.5-5.5"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linejoin="miter"
                            />
                        </svg>
                    </a>
                </nav>
            </section>
        </main>
    }
}

fn format_prompt(cwd: &str) -> String {
    if cwd == "/root" {
        ">".to_string()
    } else {
        format!("{cwd} >")
    }
}

fn format_interrupt_prompt(cwd: &str, input: &str) -> String {
    let prompt = format_prompt(cwd);
    if input.is_empty() {
        format!("{prompt} ^C")
    } else {
        format!("{prompt} {input}^C")
    }
}

fn run_shell_line(ctx: &mut TerminalContext, line: &str) -> Vec<OutputBlock> {
    match parse_line(line) {
        Ok(parsed) if parsed.stdout_redirect.is_some() && parsed.pipeline.stages.len() == 1 => {
            run_redirected_command(
                ctx,
                parsed.pipeline.stages.into_iter().next().unwrap(),
                parsed.stdout_redirect.unwrap(),
            )
        }
        Ok(parsed) if parsed.stdout_redirect.is_some() => vec![OutputBlock::Error(
            "Redirection only supports a single built-in command.".to_string(),
        )],
        Ok(parsed) if parsed.pipeline.stages.len() == 1 => {
            commands::run_parsed(ctx, parsed.pipeline.stages.into_iter().next().unwrap())
        }
        Ok(parsed) if parsed.pipeline.stages.len() == 2 => {
            let mut stages = parsed.pipeline.stages.into_iter();
            let first = stages.next().unwrap();
            let second = stages.next().unwrap();
            run_text_pipe(ctx, first, second)
        }
        Ok(_) => vec![OutputBlock::Error(
            "Only one pipe is supported right now.".to_string(),
        )],
        Err(ParseError::Empty) => Vec::new(),
        Err(ParseError::TrailingEscape) => {
            vec![OutputBlock::Error(
                "Command ended with an unfinished escape.".to_string(),
            )]
        }
        Err(ParseError::UnterminatedQuote(quote)) => vec![OutputBlock::Error(format!(
            "Unterminated {} quote.",
            quote_name(quote)
        ))],
        Err(ParseError::EmptyPipelineStage) => {
            vec![OutputBlock::Error("Empty pipeline stage.".to_string())]
        }
        Err(ParseError::EmptyRedirectTarget) => vec![OutputBlock::Error(
            "Output redirection needs exactly one target path.".to_string(),
        )],
        Err(ParseError::UnsupportedRedirect) => vec![OutputBlock::Error(
            "Only simple '>' output redirection to one file is supported.".to_string(),
        )],
    }
}

fn run_redirected_command(
    ctx: &mut TerminalContext,
    command: ParsedCommand,
    path: String,
) -> Vec<OutputBlock> {
    let cwd = ctx.cwd().to_string();
    let output = commands::run_parsed(ctx, command);
    if ctx.take_console_script_request().is_some()
        || ctx.take_fetch_request().is_some()
        || ctx.take_pip_request().is_some()
        || ctx.take_python_requested()
    {
        return vec![OutputBlock::Error(
            "Only built-in text commands can be redirected.".to_string(),
        )];
    }

    let text = match redirect_text(output) {
        Ok(text) => text,
        Err(block) => return vec![block],
    };

    match ctx.vfs_mut().write_file(&cwd, &path, text) {
        Ok(()) => {
            ctx.sync_profile_from_runtime();
            Vec::new()
        }
        Err(err) => vec![OutputBlock::Error(err.to_string())],
    }
}

fn run_text_pipe(
    ctx: &mut TerminalContext,
    first: ParsedCommand,
    second: ParsedCommand,
) -> Vec<OutputBlock> {
    let output = commands::run_parsed(ctx, first);
    // An async producer (`cowsay hi | lolcat`, `python3 g.py | lolcat`) defers the
    // whole pipe to the async layer, feeding its stdout into the consumer.
    if let Some(producer) = ctx.take_console_script_request() {
        ctx.request_console_pipe(crate::terminal::PipeProducer::Script(producer), second);
        return Vec::new();
    }
    if let Some(file) = ctx.take_python_file_request() {
        ctx.request_console_pipe(crate::terminal::PipeProducer::File(file), second);
        return Vec::new();
    }
    if ctx.take_fetch_request().is_some()
        || ctx.take_pip_request().is_some()
        || ctx.take_python_requested()
    {
        return vec![OutputBlock::Error(
            "That command can't feed a pipe.".to_string(),
        )];
    }

    let stdin = match pipe_text(output) {
        Ok(stdin) => stdin,
        Err(block) => return vec![block],
    };

    if ctx.has_console_script(&second.name) {
        ctx.request_console_script(second.name, second.args, Some(stdin));
        return Vec::new();
    }

    if commands::find_command(&commands::registry(), &second.name).is_some() {
        ctx.set_stdin(stdin);
        return commands::run_parsed(ctx, second);
    }

    vec![OutputBlock::Error(format!(
        "Unknown command '{}'. Type 'help' to see available commands.",
        second.name
    ))]
}

fn pipe_text(output: Vec<OutputBlock>) -> Result<String, OutputBlock> {
    let mut parts = Vec::new();
    for block in output {
        match block {
            OutputBlock::Text(text) => parts.push(text),
            OutputBlock::Error(message) => return Err(OutputBlock::Error(message)),
            _ => {
                return Err(OutputBlock::Error(
                    "That output cannot be piped yet.".to_string(),
                ));
            }
        }
    }

    let mut stdin = parts.join("\n");
    if !stdin.is_empty() && !stdin.ends_with('\n') {
        stdin.push('\n');
    }
    Ok(stdin)
}

fn redirect_text(output: Vec<OutputBlock>) -> Result<String, OutputBlock> {
    let mut parts = Vec::new();
    let mut saw_text = false;

    for block in output {
        match block {
            OutputBlock::Text(text) => {
                saw_text = true;
                parts.push(text);
            }
            OutputBlock::Error(message) => return Err(OutputBlock::Error(message)),
            _ => {
                return Err(OutputBlock::Error(
                    "That output cannot be redirected yet.".to_string(),
                ));
            }
        }
    }

    if !saw_text {
        return Err(OutputBlock::Error(
            "Only built-in text commands can be redirected.".to_string(),
        ));
    }

    Ok(parts.join("\n"))
}

fn quote_name(quote: char) -> &'static str {
    match quote {
        '\'' => "single",
        '"' => "double",
        _ => "quoted",
    }
}

fn has_native_navigation_modifier(ev: &KeyboardEvent) -> bool {
    ev.ctrl_key() || ev.alt_key() || ev.meta_key()
}

fn blur_command_input(command_input_ref: NodeRef<Textarea>) {
    if let Some(input) = command_input_ref.get_untracked() {
        let element = input.unchecked_ref::<web_sys::HtmlElement>();
        let _ = element.blur();
    }
}

fn output_launches_game(output: &[OutputBlock]) -> bool {
    output.iter().any(|block| match block {
        OutputBlock::Panel { body, .. } => output_launches_game(body),
        OutputBlock::LaunchGame { .. } => true,
        _ => false,
    })
}

fn close_open_games(transcript: RwSignal<Vec<TerminalEntry>>) {
    transcript.update(|entries| {
        for entry in entries {
            close_game_blocks(&mut entry.output);
        }
    });
}

fn close_game_blocks(blocks: &mut [OutputBlock]) {
    for block in blocks {
        match block {
            OutputBlock::LaunchGame { game_id, .. } => {
                *block = OutputBlock::Text(format!("[{game_id} closed]"));
            }
            OutputBlock::Panel { body, .. } => close_game_blocks(body),
            _ => {}
        }
    }
}

/// Whether the textarea caret sits on its first line (so ArrowUp should recall
/// history instead of moving the caret up a line).
fn cursor_on_first_line(command_input_ref: NodeRef<Textarea>) -> bool {
    let Some(textarea) = command_input_ref.get_untracked() else {
        return true;
    };
    let value = textarea.value();
    let pos = (textarea.selection_start().ok().flatten().unwrap_or(0) as usize).min(value.len());
    !value.is_char_boundary(pos) || !value[..pos].contains('\n')
}

/// Whether the caret sits on the textarea's last line (for ArrowDown).
fn cursor_on_last_line(command_input_ref: NodeRef<Textarea>) -> bool {
    let Some(textarea) = command_input_ref.get_untracked() else {
        return true;
    };
    let value = textarea.value();
    let len = value.len() as u32;
    let pos = (textarea.selection_start().ok().flatten().unwrap_or(len) as usize).min(value.len());
    !value.is_char_boundary(pos) || !value[pos..].contains('\n')
}

fn should_focus_terminal_input(ev: &MouseEvent) -> bool {
    let Some(target) = ev.target() else {
        return true;
    };

    let Ok(element) = target.dyn_into::<web_sys::Element>() else {
        return true;
    };

    element
        .closest(".terminal-game, input, textarea, button, a, canvas, [contenteditable='true']")
        .ok()
        .flatten()
        .is_none()
}

/// Apply a REPL completion: a lone match (or a longer shared prefix) replaces
/// the typed fragment; several matches with nothing more in common are listed.
fn complete_python_tab(
    command_input: RwSignal<String>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    line: &str,
    completion: crate::python::Completion,
) {
    let crate::python::Completion { fragment, matches } = completion;
    let head = line.strip_suffix(&fragment).unwrap_or(line);

    match matches.as_slice() {
        [] => {}
        [only] => command_input.set(format!("{head}{only}")),
        _ => {
            let common = common_prefix(&matches).unwrap_or_default();
            if common.len() > fragment.len() {
                command_input.set(format!("{head}{common}"));
            } else {
                // List just the trailing names (drop the shared `obj.` prefix).
                let base = &fragment[..fragment.rfind('.').map_or(0, |dot| dot + 1)];
                let names = matches
                    .iter()
                    .map(|name| name.strip_prefix(base).unwrap_or(name))
                    .collect::<Vec<_>>()
                    .join("  ");
                append_entry(
                    transcript,
                    next_id,
                    format!(">>> {line}"),
                    vec![OutputBlock::Text(names)],
                );
            }
        }
    }
}

/// Save the profile to storage (async), reporting a failure as a transcript
/// entry. Saves are optimistic — the in-memory state is already updated.
fn persist_profile(
    profile: crate::storage::ProfileV1,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
) {
    spawn_local(async move {
        if let Err(err) = save_profile(&profile).await {
            append_entry(
                transcript,
                next_id,
                "system".to_string(),
                vec![OutputBlock::Error(format!("Profile save failed: {err}"))],
            );
        }
    });
}

fn append_entry(
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    prompt: String,
    output: Vec<OutputBlock>,
) -> usize {
    let id = next_id.get_untracked();
    next_id.set(id + 1);
    transcript.update(|entries| {
        entries.push(TerminalEntry { id, prompt, output });
    });
    id
}

/// The history the up/down arrows and Ctrl-R browse — the Python REPL's own
/// history while it's active, otherwise the shell's.
fn active_history(ctx: &TerminalContext, in_python: bool) -> Vec<String> {
    if in_python {
        ctx.python_history()
    } else {
        ctx.terminal_history()
    }
}

fn recall_history(
    direction: HistoryDirection,
    ctx: ReadSignal<TerminalContext>,
    command_input: RwSignal<String>,
    history_cursor: RwSignal<Option<usize>>,
    history_draft: RwSignal<String>,
    python_mode: RwSignal<bool>,
) -> bool {
    match direction {
        HistoryDirection::Previous => {
            let history = ctx.with(|ctx| active_history(ctx, python_mode.get_untracked()));
            if history.is_empty() {
                return false;
            }

            let next_cursor = match history_cursor.get_untracked() {
                Some(index) if index > 0 => index - 1,
                Some(index) => index,
                None => {
                    history_draft.set(command_input.get_untracked());
                    history.len() - 1
                }
            };

            history_cursor.set(Some(next_cursor));
            if let Some(entry) = history.get(next_cursor) {
                command_input.set(entry.clone());
            }
            true
        }
        HistoryDirection::Next => {
            let Some(cursor) = history_cursor.get_untracked() else {
                return false;
            };
            let history = ctx.with(|ctx| active_history(ctx, python_mode.get_untracked()));

            if cursor + 1 < history.len() {
                let next_cursor = cursor + 1;
                history_cursor.set(Some(next_cursor));
                if let Some(entry) = history.get(next_cursor) {
                    command_input.set(entry.clone());
                }
            } else {
                history_cursor.set(None);
                command_input.set(history_draft.get_untracked());
            }
            true
        }
    }
}

fn focus_command_input(command_input_ref: NodeRef<Textarea>) {
    let Some(textarea) = command_input_ref.get_untracked() else {
        return;
    };
    let _ = textarea.focus();
    let len = textarea.value().len() as u32;
    let _ = textarea.set_selection_range(len, len);

    request_animation_frame(move || {
        let _ = textarea.focus();
        let len = textarea.value().len() as u32;
        let _ = textarea.set_selection_range(len, len);
    });
}

fn is_repl_exit(line: &str) -> bool {
    matches!(line.trim(), "exit" | "quit" | "exit()" | "quit()")
}

fn set_entry_output(transcript: RwSignal<Vec<TerminalEntry>>, id: usize, output: Vec<OutputBlock>) {
    transcript.update(|entries| {
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.output = output;
        }
    });
}

/// The `>>> `/`... ` echo for a (possibly multiline) REPL submission.
fn repl_echo(source: &str) -> String {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                format!(">>> {line}")
            } else {
                format!("... {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Evaluate a whole (possibly multiline) Python submission. An incomplete block
/// is put back in the input with a trailing newline so the user keeps editing
/// it; a complete one is echoed and run, sharing the terminal's filesystem.
#[allow(clippy::too_many_arguments)]
fn submit_python(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    python_mode: RwSignal<bool>,
    command_input: RwSignal<String>,
    source: String,
) {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return;
    }
    if is_repl_exit(trimmed) {
        python_mode.set(false);
        append_entry(
            transcript,
            next_id,
            repl_echo(trimmed),
            vec![OutputBlock::Text("Returned to shell.".to_string())],
        );
        return;
    }

    let (files, dirs, cwd) = ctx.with_untracked(|ctx| {
        (
            ctx.vfs().managed_files(),
            ctx.vfs().managed_dirs(),
            ctx.cwd().to_string(),
        )
    });

    spawn_local(async move {
        if let Err(err) = crate::python::sync_to_pyodide(&files, &dirs, &cwd).await {
            append_entry(
                transcript,
                next_id,
                repl_echo(&source),
                vec![OutputBlock::Error(err)],
            );
            return;
        }

        match crate::python::run_source(&source).await {
            // Not finished yet: hand it back to the input (with a fresh line) so
            // the user keeps composing the block.
            Ok(crate::python::Eval::Incomplete) => {
                command_input.set(format!("{}\n", source.trim_end_matches('\n')));
            }
            Ok(crate::python::Eval::Complete(text)) => {
                set_ctx.update(|ctx| ctx.record_python_history(source.trim()));
                persist_profile(
                    ctx.with_untracked(|ctx| ctx.profile().clone()),
                    transcript,
                    next_id,
                );
                apply_python_fs_changes(set_ctx, transcript, next_id).await;
                let output = if text.trim_end().is_empty() {
                    Vec::new()
                } else {
                    vec![OutputBlock::Text(text.trim_end_matches('\n').to_string())]
                };
                append_entry(transcript, next_id, repl_echo(&source), output);
            }
            Err(err) => {
                set_ctx.update(|ctx| ctx.record_python_history(source.trim()));
                persist_profile(
                    ctx.with_untracked(|ctx| ctx.profile().clone()),
                    transcript,
                    next_id,
                );
                append_entry(
                    transcript,
                    next_id,
                    repl_echo(&source),
                    vec![OutputBlock::Error(err)],
                );
            }
        }
    });
}

/// Install packages, then fold mirrored Python filesystem changes into the
/// overlay so they appear in the terminal and persist. The captured pip log is
/// appended as its own transcript entry when it resolves.
fn run_pip_install(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    packages: Vec<String>,
) {
    let (files, dirs, cwd) = ctx.with_untracked(|ctx| {
        (
            ctx.vfs().managed_files(),
            ctx.vfs().managed_dirs(),
            ctx.cwd().to_string(),
        )
    });

    spawn_local(async move {
        if let Err(err) = crate::python::ensure_loaded().await {
            append_entry(
                transcript,
                next_id,
                String::new(),
                vec![OutputBlock::Error(err)],
            );
            return;
        }
        if let Err(err) = crate::python::sync_to_pyodide(&files, &dirs, &cwd).await {
            append_entry(
                transcript,
                next_id,
                String::new(),
                vec![OutputBlock::Error(err)],
            );
            return;
        }

        let block = match crate::python::pip_install(&packages).await {
            Ok(log) => OutputBlock::Text(log),
            Err(log) => OutputBlock::Error(log),
        };
        // Surface installed files regardless of outcome (a partial install still
        // wrote some), then report the log.
        apply_python_fs_changes(set_ctx, transcript, next_id).await;
        append_entry(transcript, next_id, String::new(), vec![block]);
    });
}

/// Execute a Python package console script discovered from installed package
/// metadata, then fold any filesystem changes back into the terminal overlay.
fn run_console_script(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    entry_id: usize,
    invocation: ConsoleScriptInvocation,
) {
    let (files, dirs, cwd) = ctx.with_untracked(|ctx| {
        (
            ctx.vfs().managed_files(),
            ctx.vfs().managed_dirs(),
            ctx.cwd().to_string(),
        )
    });

    spawn_local(async move {
        if let Err(err) = crate::python::ensure_loaded().await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }
        if let Err(err) = crate::python::sync_to_pyodide(&files, &dirs, &cwd).await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }

        let block = match crate::python::run_console_script(
            &invocation.name,
            &invocation.args,
            invocation.stdin.as_deref(),
        )
        .await
        {
            Ok(output) if output.is_empty() => None,
            Ok(output) => Some(OutputBlock::Text(output)),
            Err(output) => Some(OutputBlock::Error(output)),
        };

        apply_python_fs_changes(set_ctx, transcript, next_id).await;
        set_entry_output(transcript, entry_id, block.into_iter().collect());
    });
}

fn run_python_file(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    entry_id: usize,
    invocation: PythonFileInvocation,
) {
    let (files, dirs, cwd) = ctx.with_untracked(|ctx| {
        (
            ctx.vfs().managed_files(),
            ctx.vfs().managed_dirs(),
            ctx.cwd().to_string(),
        )
    });

    spawn_local(async move {
        if let Err(err) = crate::python::ensure_loaded().await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }
        if let Err(err) = crate::python::sync_to_pyodide(&files, &dirs, &cwd).await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }

        let block = match crate::python::run_python_file(&invocation.path, &invocation.args).await {
            Ok(output) if output.is_empty() => None,
            Ok(output) => Some(OutputBlock::Text(output)),
            Err(output) => Some(OutputBlock::Error(output)),
        };

        apply_python_fs_changes(set_ctx, transcript, next_id).await;
        set_entry_output(transcript, entry_id, block.into_iter().collect());
    });
}

/// Run a `<console-script> | <command>` pipe asynchronously: run the producer
/// console script, then feed its stdout as stdin into the consumer (another
/// console script, or a built-in via `set_stdin`).
fn run_console_pipe(
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    entry_id: usize,
    pipe: ConsolePipe,
) {
    let (files, dirs, cwd) = ctx.with_untracked(|ctx| {
        (
            ctx.vfs().managed_files(),
            ctx.vfs().managed_dirs(),
            ctx.cwd().to_string(),
        )
    });

    spawn_local(async move {
        if let Err(err) = crate::python::ensure_loaded().await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }
        if let Err(err) = crate::python::sync_to_pyodide(&files, &dirs, &cwd).await {
            set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
            return;
        }

        let ConsolePipe { producer, consumer } = pipe;
        let produced = match producer {
            crate::terminal::PipeProducer::Script(s) => {
                crate::python::run_console_script(&s.name, &s.args, s.stdin.as_deref()).await
            }
            crate::terminal::PipeProducer::File(f) => {
                crate::python::run_python_file(&f.path, &f.args).await
            }
        };
        let mut stdin = match produced {
            Ok(text) => text,
            Err(err) => {
                set_entry_output(transcript, entry_id, vec![OutputBlock::Error(err)]);
                return;
            }
        };
        if !stdin.is_empty() && !stdin.ends_with('\n') {
            stdin.push('\n');
        }

        let consumer_is_script = ctx.with_untracked(|ctx| ctx.has_console_script(&consumer.name));
        let blocks = if consumer_is_script {
            match crate::python::run_console_script(&consumer.name, &consumer.args, Some(&stdin))
                .await
            {
                Ok(text) if text.is_empty() => Vec::new(),
                Ok(text) => vec![OutputBlock::Text(text)],
                Err(text) => vec![OutputBlock::Error(text)],
            }
        } else {
            let mut blocks = Vec::new();
            set_ctx.update(|ctx| {
                ctx.set_stdin(stdin.clone());
                blocks = commands::run_parsed(ctx, consumer.clone());
            });
            blocks
        };

        apply_python_fs_changes(set_ctx, transcript, next_id).await;
        set_entry_output(transcript, entry_id, blocks);
    });
}

fn run_fetch(
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
    entry_id: usize,
    invocation: FetchInvocation,
) {
    spawn_local(async move {
        let block = match crate::net::fetch_text(&invocation.url).await {
            Ok(text) => {
                if let Some(path) = invocation.output_path {
                    let result = set_ctx.try_update(|ctx| {
                        ctx.vfs_mut()
                            .write_file(&invocation.cwd, &path, text)
                            .map(|()| {
                                ctx.sync_profile_from_runtime();
                                ctx.profile().clone()
                            })
                            .map_err(|err| err.to_string())
                    });
                    match result {
                        Some(Ok(profile)) => {
                            persist_profile(profile, transcript, next_id);
                            OutputBlock::Text(format!("saved {}", path))
                        }
                        Some(Err(err)) => OutputBlock::Error(err),
                        None => OutputBlock::Error("terminal context is unavailable".to_string()),
                    }
                } else {
                    OutputBlock::Text(text)
                }
            }
            Err(err) => OutputBlock::Error(format!("{}: {}", invocation.url, err)),
        };
        set_entry_output(transcript, entry_id, vec![block]);
    });
}

/// Read back any filesystem changes Python made and apply them to the overlay,
/// then persist. A persistence failure is surfaced as its own transcript entry.
async fn apply_python_fs_changes(
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
) {
    let changes = match crate::python::sync_from_pyodide().await {
        Ok(changes) => changes,
        Err(err) => {
            append_entry(
                transcript,
                next_id,
                "fs sync".to_string(),
                vec![OutputBlock::Error(err)],
            );
            return;
        }
    };
    if changes.is_empty() {
        return;
    }

    let mut profile = None;
    set_ctx.update(|ctx| {
        for change in changes {
            match change {
                crate::python::FsChange::Mkdir(path) => ctx.vfs_mut().apply_external_mkdir(&path),
                crate::python::FsChange::Write(path, content) => {
                    ctx.vfs_mut().apply_external_write(&path, content)
                }
                crate::python::FsChange::Remove(path) => ctx.vfs_mut().apply_external_remove(&path),
            }
        }
        ctx.sync_profile_from_runtime();
        profile = Some(ctx.profile().clone());
    });

    if let Some(profile) = profile {
        persist_profile(profile, transcript, next_id);
    }
}

#[component]
fn OutputBlockView(
    block: OutputBlock,
    ctx: ReadSignal<TerminalContext>,
    set_ctx: WriteSignal<TerminalContext>,
    transcript: RwSignal<Vec<TerminalEntry>>,
    next_id: RwSignal<usize>,
) -> impl IntoView {
    match block {
        OutputBlock::Text(text) => render_terminal_text(text, false).into_any(),
        OutputBlock::Error(message) => render_terminal_text(message, true).into_any(),
        OutputBlock::Panel { title, body } => view! {
            <section class="terminal-panel">
                <h2>{title}</h2>
                <For
                    each={move || keyed_output_blocks(body.clone())}
                    key=|(key, _)| key.clone()
                    children={move |(_, child)| {
                        view! {
                            <OutputBlockView
                                block=child
                                ctx=ctx
                                set_ctx=set_ctx
                                transcript=transcript
                                next_id=next_id
                            />
                        }
                    }}
                />
            </section>
        }
        .into_any(),
        OutputBlock::FileView { path, content } => view! {
            <section class="terminal-file">
                <div class="terminal-file-title">{path}</div>
                <pre>{content}</pre>
            </section>
        }
        .into_any(),
        OutputBlock::SaveManager => {
            let on_system_error = Callback::new(move |message: String| {
                append_entry(
                    transcript,
                    next_id,
                    "system".to_string(),
                    vec![OutputBlock::Error(message)],
                );
            });
            view! {
                <SaveManagerPanel
                    ctx=ctx
                    set_ctx=set_ctx
                    on_system_error=on_system_error
                />
            }
            .into_any()
        }
        OutputBlock::NanoEditor { path, content } => {
            let on_error = Callback::new(move |message: String| {
                append_entry(
                    transcript,
                    next_id,
                    String::new(),
                    vec![OutputBlock::Error(message)],
                );
            });
            let on_profile_changed =
                Callback::new(move |profile| persist_profile(profile, transcript, next_id));
            view! {
                <NanoEditorPanel
                    path=path
                    initial_content=content
                    set_ctx=set_ctx
                    on_error=on_error
                    on_profile_changed=on_profile_changed
                />
            }
            .into_any()
        }
        OutputBlock::LaunchGame { game_id, hard } if game_id == "bubbles" => {
            view! { <BubblesGame hard=hard /> }.into_any()
        }
        OutputBlock::LaunchGame { game_id, .. } if game_id == "textropolis" => {
            view! { <TextropolisGame /> }.into_any()
        }
        OutputBlock::LaunchGame { game_id, .. } => view! {
            <section class="terminal-game-placeholder">
                <h2>{game_id.clone()}</h2>
                <p>{format!("No renderer is registered for game_id '{}'.", game_id)}</p>
            </section>
        }
        .into_any(),
    }
}

fn render_terminal_text(text: String, is_error: bool) -> impl IntoView {
    let fragments = parse_ansi_fragments(&text);
    let class = if is_error {
        "terminal-text terminal-error"
    } else {
        "terminal-text"
    };

    view! {
        <pre class=class>
            <For
                each={move || fragments.clone().into_iter().enumerate().collect::<Vec<_>>()}
                key=|(index, _)| *index
                children={move |(_, fragment)| render_ansi_fragment(fragment)}
            />
        </pre>
    }
}

fn keyed_output_blocks(output: Vec<OutputBlock>) -> Vec<(String, OutputBlock)> {
    output
        .into_iter()
        .enumerate()
        .map(|(index, block)| {
            let mut hasher = DefaultHasher::new();
            block.hash(&mut hasher);
            (format!("{index}:{}", hasher.finish()), block)
        })
        .collect()
}

fn render_ansi_fragment(fragment: AnsiFragment) -> impl IntoView {
    let style = fragment.style.to_css();
    let text = fragment.text;

    match style {
        Some(style) => view! { <span style=style>{text}</span> }.into_any(),
        None => view! { <span>{text}</span> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_prompt_omits_path() {
        assert_eq!(format_prompt("/root"), ">");
        assert_eq!(format_prompt("/scratch"), "/scratch >");
    }

    #[test]
    fn interrupt_prompt_keeps_typed_input_without_executing_shape() {
        assert_eq!(
            format_interrupt_prompt("/root", "cat draft.md"),
            "> cat draft.md^C"
        );
        assert_eq!(format_interrupt_prompt("/scratch", ""), "/scratch > ^C");
    }

    fn install_lolcat_metadata(ctx: &mut TerminalContext) {
        ctx.vfs_mut().apply_external_mkdir("/python/site-packages");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/lolcat-1.0.dist-info");
        ctx.vfs_mut().apply_external_write(
            "/python/site-packages/lolcat-1.0.dist-info/entry_points.txt",
            "[console_scripts]\nlolcat = lolcat:main\n".to_string(),
        );
        ctx.sync_profile_from_runtime();
    }

    #[test]
    fn echo_pipe_requests_console_script_with_stdin() {
        let mut ctx = TerminalContext::new();
        install_lolcat_metadata(&mut ctx);

        let output = run_shell_line(&mut ctx, r#"echo "hi there" | lolcat --seed 7"#);

        assert!(output.is_empty());
        assert_eq!(
            ctx.take_console_script_request(),
            Some(ConsoleScriptInvocation {
                name: "lolcat".to_string(),
                args: vec!["--seed".to_string(), "7".to_string()],
                stdin: Some("hi there\n".to_string()),
            })
        );
    }

    #[test]
    fn pipe_rejects_non_text_output() {
        let mut ctx = TerminalContext::new();
        install_lolcat_metadata(&mut ctx);

        assert_eq!(
            run_shell_line(&mut ctx, "nano draft.md | lolcat"),
            vec![OutputBlock::Error(
                "That output cannot be piped yet.".to_string()
            )]
        );
    }

    #[test]
    fn redirect_writes_text_output_to_user_file() {
        let mut ctx = TerminalContext::new();

        let output = run_shell_line(&mut ctx, r#"echo "hello world" > test.txt"#);

        assert!(output.is_empty());
        assert_eq!(
            ctx.vfs().read_file("/root", "test.txt").unwrap().content,
            "hello world"
        );
    }

    #[test]
    fn redirect_rejects_non_text_output() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_shell_line(&mut ctx, "nano draft.md > out.txt"),
            vec![OutputBlock::Error(
                "That output cannot be redirected yet.".to_string()
            )]
        );
        assert!(!ctx.vfs().has_file("/root/out.txt"));
    }

    #[test]
    fn redirect_requires_single_target_path() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_shell_line(&mut ctx, "echo hello >"),
            vec![OutputBlock::Error(
                "Output redirection needs exactly one target path.".to_string()
            )]
        );
        assert_eq!(
            run_shell_line(&mut ctx, "echo hello > out.txt extra"),
            vec![OutputBlock::Error(
                "Only simple '>' output redirection to one file is supported.".to_string()
            )]
        );
    }

    #[test]
    fn close_game_blocks_replaces_nested_launches() {
        let mut blocks = vec![
            OutputBlock::Text("keep".to_string()),
            OutputBlock::LaunchGame {
                game_id: "bubbles".to_string(),
                hard: true,
            },
            OutputBlock::Panel {
                title: "panel".to_string(),
                body: vec![OutputBlock::LaunchGame {
                    game_id: "textropolis".to_string(),
                    hard: false,
                }],
            },
        ];

        close_game_blocks(&mut blocks);

        assert_eq!(
            blocks,
            vec![
                OutputBlock::Text("keep".to_string()),
                OutputBlock::Text("[bubbles closed]".to_string()),
                OutputBlock::Panel {
                    title: "panel".to_string(),
                    body: vec![OutputBlock::Text("[textropolis closed]".to_string())],
                },
            ]
        );
    }

    #[test]
    fn keyed_output_blocks_changes_key_when_block_changes() {
        let old_key = keyed_output_blocks(vec![OutputBlock::LaunchGame {
            game_id: "bubbles".to_string(),
            hard: false,
        }])
        .remove(0)
        .0;
        let new_key = keyed_output_blocks(vec![OutputBlock::Text("[bubbles closed]".to_string())])
            .remove(0)
            .0;

        assert_ne!(old_key, new_key);
    }
}
