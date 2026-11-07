//! One agent run: look at the tab, ask the model, run its tools, and repeat.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::ai::{self, ChatMessage, ChatPart, StreamPiece, ToolReply, ToolSpec};
use crate::automation::clip;
use crate::settings::{self, BrowserSettings};
use crate::tabs;

use super::runs::{self, checkpoint, emit, finish, pause, push_step, set_last_thumb, with_run};
use super::tools::{self, Outcome};
use super::{observe, prompt, store};

const MODEL_TIMEOUT: Duration = Duration::from_secs(180);
const SUMMARY_TIMEOUT: Duration = Duration::from_secs(90);
/// Messages before older turns are condensed into one summary.
const SUMMARY_AFTER: usize = 48;
/// Newest messages kept word for word when summarizing.
const SUMMARY_KEEP: usize = 12;
const DELTA_EVERY: Duration = Duration::from_millis(60);

/// A tool result waiting to go back to the model with the next page state.
struct Pending {
    call_id: String,
    result: String,
    image: Option<String>,
}

pub(super) fn spawn(app: &AppHandle, id: u32) {
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("agent-run".into())
        .spawn(move || run(handle, id));
    if spawned.is_err() {
        with_run(id, |state| state.working = false);
        finish(app, id, "failed", "Couldn't start the agent.");
    }
}

fn run(app: AppHandle, id: u32) {
    let mut messages =
        with_run(id, |state| std::mem::take(&mut state.transcript)).unwrap_or_default();
    let mut pending = Vec::new();
    let outcome = drive(&app, id, &mut messages, &mut pending);
    flush(&mut messages, &mut pending, Vec::new());
    observe::prune(&mut messages);
    store::strip_images(&mut messages);
    let already_finished = with_run(id, |state| {
        state.transcript = messages;
        state.follow_up = None;
        state.working = false;
        state.run.finished != 0
    })
    .unwrap_or(true);
    match outcome {
        Ok((status, result)) => finish(&app, id, status, &result),
        Err(error) => {
            let (text, detail) = error.split_once('\n').unwrap_or((error.as_str(), ""));
            push_step(&app, id, "error", text, detail);
            finish(&app, id, "failed", &error);
        }
    }
    if already_finished {
        runs::save(&app);
    }
}

/// Sends the waiting tool results. `parts`, usually the new page state, go with the last one,
/// or in a user message when nothing is waiting.
fn flush(messages: &mut Vec<ChatMessage>, pending: &mut Vec<Pending>, parts: Vec<ChatPart>) {
    let count = pending.len();
    let mut parts = Some(parts);
    for (index, item) in pending.drain(..).enumerate() {
        let mut content = vec![ChatPart::Text(item.result)];
        if let Some(image) = item.image {
            content.push(ChatPart::Image {
                media_type: "image/jpeg".into(),
                data: image,
            });
        }
        if index + 1 == count {
            content.extend(parts.take().unwrap_or_default());
        }
        messages.push(ChatMessage::ToolResult {
            call_id: item.call_id,
            parts: content,
        });
    }
    if let Some(parts) = parts
        && !parts.is_empty()
    {
        messages.push(ChatMessage::User(parts));
    }
}

fn stopped() -> Result<(&'static str, String), String> {
    Ok(("stopped", "Stopped.".into()))
}

struct Setup {
    goal: String,
    window: String,
    model: String,
    follow_up: Option<String>,
    context_tabs: Vec<u32>,
    instructions: String,
    budget: u64,
    helper: bool,
}

fn drive(
    app: &AppHandle,
    id: u32,
    messages: &mut Vec<ChatMessage>,
    pending: &mut Vec<Pending>,
) -> Result<(&'static str, String), String> {
    let current = settings::get(app);
    let auto = current.automation.clone();
    let setup = with_run(id, |state| Setup {
        goal: state.run.goal.clone(),
        window: state.run.window.clone(),
        model: state.run.model.clone(),
        follow_up: state.follow_up.clone(),
        context_tabs: state.context_tabs.clone(),
        instructions: state.instructions.clone(),
        budget: state.run.budget,
        helper: state.run.parent.is_some(),
    })
    .ok_or("The run was removed.")?;
    let model = if setup.model.trim().is_empty() {
        auto.agent_model.clone()
    } else {
        setup.model.clone()
    };
    let mut opening = Some(match &setup.follow_up {
        Some(text) if !messages.is_empty() => format!("Follow-up from the user: {text}"),
        Some(text) => format!("Task: {}\n\nFollow-up from the user: {text}", setup.goal),
        None => {
            let mut text = format!("Task: {}", setup.goal);
            if !setup.instructions.is_empty() {
                text.push_str(&format!("\n\nHow to go about it: {}", setup.instructions));
            }
            text.push_str(&observe::context_tabs(app, &setup.context_tabs));
            text
        }
    });
    let tools = tools::all(&auto, setup.helper);
    let specs: Vec<ToolSpec> = tools.iter().map(|tool| tool.spec.clone()).collect();
    let mut limit = auto.max_steps.max(1);
    let mut turns = 0u32;
    let mut budget = setup.budget;
    loop {
        if checkpoint(id).is_err() {
            return stopped();
        }
        if turns >= limit {
            let more = runs::ask_more(
                app,
                id,
                &format!(
                    "The agent has taken {turns} turns without finishing. Let it keep going for {} more?",
                    auto.max_steps
                ),
            );
            if !more {
                return Ok((
                    "failed",
                    format!("Stopped after {turns} steps without finishing."),
                ));
            }
            limit += auto.max_steps.max(1);
        }
        if budget > 0 {
            let used = with_run(id, |state| state.run.tokens_in + state.run.tokens_out)
                .unwrap_or(0);
            if used >= budget {
                let more = runs::ask_more(
                    app,
                    id,
                    &format!(
                        "The agent used {used} tokens, its budget of {budget}. Let it use {} more?",
                        setup.budget
                    ),
                );
                if !more {
                    return Ok(("failed", "Stopped at its token budget.".into()));
                }
                budget += setup.budget;
                with_run(id, |state| state.run.budget = budget);
            }
        }
        let tab = runs::current_tab(id).ok_or("The run was removed.")?;
        if tabs::tab_info(app, tab).is_none() {
            return Ok(("stopped", "The tab was closed.".into()));
        }
        let observation = observe::observe(app, tab, &setup.window, auto.screenshots_to_model);
        if checkpoint(id).is_err() {
            return stopped();
        }
        if let Some(thumb) = observation.thumb.clone() {
            set_last_thumb(id, thumb);
            emit(app);
        }
        let mut parts = Vec::new();
        if let Some(text) = opening.take() {
            parts.push(ChatPart::Text(text));
        }
        parts.push(ChatPart::Text(observation.text.clone()));
        if let Some(image) = observation.image.clone() {
            parts.push(ChatPart::Image {
                media_type: "image/jpeg".into(),
                data: image,
            });
        }
        flush(messages, pending, parts);
        let notes = with_run(id, |state| std::mem::take(&mut state.notes)).unwrap_or_default();
        for note in notes {
            messages.push(ChatMessage::User(vec![ChatPart::Text(note)]));
        }
        summarize(app, id, &current, &model, &setup.goal, messages);
        observe::prune(messages);
        let autonomy = with_run(id, |state| state.run.autonomy.clone()).unwrap_or_default();
        let system = prompt::system_prompt(app, &auto, &autonomy, &observation.url, setup.helper);
        let reply = match ask_twice(app, id, &current, &model, &system, messages, &specs) {
            Ok(reply) => reply,
            Err(AskError::Stopped) => return stopped(),
            Err(AskError::Failed(error)) => return Err(error),
        };
        with_run(id, |state| {
            state.run.tokens_in += reply.usage.input;
            state.run.tokens_out += reply.usage.output;
        });
        runs::emit_delta(app, id, "", "");
        if checkpoint(id).is_err() {
            return stopped();
        }
        messages.push(ChatMessage::Assistant {
            text: reply.text.clone(),
            calls: reply.calls.clone(),
        });
        if !reply.text.is_empty() {
            push_step(app, id, "think", &reply.text, "");
        } else {
            emit(app);
        }
        if reply.calls.is_empty() {
            let text = if reply.text.is_empty() {
                "Finished.".to_string()
            } else {
                reply.text
            };
            return Ok(("done", text));
        }
        turns += 1;
        let mut page_done = false;
        for (index, call) in reply.calls.iter().enumerate() {
            if page_done {
                pending.push(Pending {
                    call_id: call.id.clone(),
                    result: "Skipped. Only one page action runs per turn; look at the new page state first.".into(),
                    image: None,
                });
                continue;
            }
            match tools::execute(
                app,
                id,
                &auto,
                &setup.window,
                observation.scale,
                &tools,
                call,
            ) {
                Outcome::Continue {
                    result,
                    image,
                    page,
                } => {
                    with_run(id, |state| state.run.actions += 1);
                    pending.push(Pending {
                        call_id: call.id.clone(),
                        result,
                        image,
                    });
                    page_done = page;
                }
                Outcome::Finish(status, text) => {
                    pending.push(Pending {
                        call_id: call.id.clone(),
                        result: "Finished.".into(),
                        image: None,
                    });
                    for rest in &reply.calls[index + 1..] {
                        pending.push(Pending {
                            call_id: rest.id.clone(),
                            result: "Not run; the task finished.".into(),
                            image: None,
                        });
                    }
                    return Ok((status, text));
                }
            }
        }
        if pause(id, Duration::from_millis(350)).is_err() {
            return stopped();
        }
    }
}

/// Condenses older turns into one note once the conversation grows long, keeping exact facts.
fn summarize(
    app: &AppHandle,
    id: u32,
    current: &BrowserSettings,
    model: &str,
    goal: &str,
    messages: &mut Vec<ChatMessage>,
) {
    if messages.len() <= SUMMARY_AFTER {
        return;
    }
    let mut split = messages.len() - SUMMARY_KEEP;
    while split > 1 && !matches!(messages[split], ChatMessage::Assistant { .. }) {
        split -= 1;
    }
    if split <= 1 {
        return;
    }
    let history = render(&messages[..split]);
    let summary = ai::complete(
        app,
        current,
        model,
        prompt::SUMMARY_SYSTEM,
        &history,
        false,
        SUMMARY_TIMEOUT,
    )
    .map(|text| clip(text.trim(), 1800))
    .unwrap_or_else(|_| {
        with_run(id, |state| {
            state
                .run
                .steps
                .iter()
                .filter(|step| step.kind == "action" || step.kind == "tool")
                .map(|step| format!("- {}", clip(&step.text, 100)))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|text| clip(&text, 1800))
        .unwrap_or_default()
    });
    let head = ChatMessage::User(vec![ChatPart::Text(format!(
        "Task: {}\n\nPROGRESS SO FAR (older turns, summarized):\n{summary}",
        clip(goal, 600)
    ))]);
    messages.splice(0..split, [head]);
    push_step(
        app,
        id,
        "tool",
        "Summarized older steps to keep the conversation short",
        "",
    );
}

fn part_texts(parts: &[ChatPart], max: usize) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            ChatPart::Text(text) => Some(clip(text, max)),
            ChatPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for message in messages {
        let line = match message {
            ChatMessage::User(parts) => format!("USER: {}", part_texts(parts, 1500)),
            ChatMessage::Assistant { text, calls } => {
                let calls: Vec<String> = calls
                    .iter()
                    .map(|call| format!("{}({})", call.name, clip(&call.arguments.to_string(), 200)))
                    .collect();
                format!("AGENT: {} {}", clip(text, 600), calls.join(", "))
            }
            ChatMessage::ToolResult { parts, .. } => {
                format!("RESULT: {}", part_texts(parts, 800))
            }
        };
        out.push_str(&line);
        out.push('\n');
    }
    clip(&out, 40_000)
}

enum AskError {
    Stopped,
    Failed(String),
}

/// Asks the model, trying once more after a short wait when the first request fails.
fn ask_twice(
    app: &AppHandle,
    id: u32,
    current: &BrowserSettings,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Result<ToolReply, AskError> {
    match ask_model(app, id, current, model, system, messages, tools) {
        Err(AskError::Failed(first)) => {
            if pause(id, Duration::from_secs(2)).is_err() {
                return Err(AskError::Stopped);
            }
            match ask_model(app, id, current, model, system, messages, tools) {
                Err(AskError::Failed(_)) => Err(AskError::Failed(first)),
                other => other,
            }
        }
        other => other,
    }
}

/// Asks the model on another thread. Returns as soon as the user stops the run, and drops a
/// reply that arrives after that, so the model cannot click or type over the user.
fn ask_model(
    app: &AppHandle,
    id: u32,
    current: &BrowserSettings,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Result<ToolReply, AskError> {
    if checkpoint(id).is_err() {
        return Err(AskError::Stopped);
    }
    let (tx, rx) = mpsc::channel();
    let app = app.clone();
    let current = current.clone();
    let model = model.to_string();
    let system = system.to_string();
    let messages = messages.to_vec();
    let tools = tools.to_vec();
    let spawned = thread::Builder::new()
        .name("agent-model".into())
        .spawn(move || {
            let reply = if current.automation.stream_replies {
                let mut text = String::new();
                let mut thinking = String::new();
                let mut last = Instant::now();
                let mut on_delta = |piece: StreamPiece, so_far: &str| {
                    if runs::halted(id) {
                        return false;
                    }
                    match piece {
                        StreamPiece::Text => text = so_far.to_string(),
                        StreamPiece::Thinking => thinking = so_far.to_string(),
                    }
                    if last.elapsed() >= DELTA_EVERY {
                        runs::emit_delta(&app, id, &text, &thinking);
                        last = Instant::now();
                    }
                    true
                };
                ai::complete_tools_stream(
                    &app,
                    &current,
                    &model,
                    &system,
                    &messages,
                    &tools,
                    MODEL_TIMEOUT,
                    &mut on_delta,
                )
            } else {
                ai::complete_tools(
                    &app,
                    &current,
                    &model,
                    &system,
                    &messages,
                    &tools,
                    MODEL_TIMEOUT,
                )
            };
            let _ = tx.send(reply);
        });
    if spawned.is_err() {
        return Err(AskError::Failed("Couldn't reach the assistant.".into()));
    }
    loop {
        if checkpoint(id).is_err() {
            return Err(AskError::Stopped);
        }
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(Ok(reply)) => {
                if checkpoint(id).is_err() {
                    return Err(AskError::Stopped);
                }
                return Ok(reply);
            }
            Ok(Err(error)) => return Err(AskError::Failed(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AskError::Failed("The assistant returned nothing.".into()));
            }
        }
    }
}
