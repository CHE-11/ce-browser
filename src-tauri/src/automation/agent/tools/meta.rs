//! Tools about the run itself: questions, finishing, the plan, memory and helper agents.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::automation::{clip, policy};
use crate::settings::{AutomationSettings, sleep_site_key};
use crate::{paths, tabs};

use super::super::memory::{self, MemoryItem};
use super::super::runs::{self, Answer, PlanItem, StartOptions, with_run};
use super::{
    Ctx, Done, Kind, Tool, arg_bool, arg_f64, arg_str, gate, halt, object, step, tool,
};

pub fn tools(auto: &AutomationSettings, helper: bool) -> Vec<Tool> {
    let mut tools = vec![
        tool(
            "update_plan",
            "Set the checklist for this task. Send the whole list each time, marking each item pending, active, done or skipped.",
            object(
                json!({
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string" },
                                "status": { "type": "string", "enum": ["pending", "active", "done", "skipped"] }
                            },
                            "required": ["text", "status"]
                        }
                    }
                }),
                &["items"],
            ),
            Kind::Meta,
            false,
            update_plan,
        ),
        tool(
            "ask_user",
            "Ask the user a question and wait for the answer.",
            object(json!({ "question": { "type": "string" } }), &["question"]),
            Kind::Meta,
            false,
            ask_user,
        ),
        tool(
            "done",
            "Finish. The summary, in Markdown, is shown to the user.",
            object(json!({ "summary": { "type": "string" } }), &["summary"]),
            Kind::Meta,
            false,
            done,
        ),
        tool(
            "fail",
            "Give up, with the reason.",
            object(json!({ "reason": { "type": "string" } }), &["reason"]),
            Kind::Meta,
            false,
            fail,
        ),
    ];
    if auto.agent_memory {
        tools.push(tool(
            "remember",
            "Keep a lasting note for later runs, such as a preference of the user's or how a site works. Never store passwords or codes.",
            object(
                json!({
                    "text": { "type": "string" },
                    "site": { "type": "string", "description": "Host the note is about, such as `github.com`, or `this site` for the current page. Blank for a general note." },
                    "pinned": { "type": "boolean", "description": "Show it in every run." }
                }),
                &["text"],
            ),
            Kind::Meta,
            false,
            remember,
        ));
        tools.push(tool(
            "recall",
            "Look up notes kept in earlier runs.",
            object(json!({ "query": { "type": "string" } }), &[]),
            Kind::Read,
            false,
            recall,
        ));
        tools.push(tool(
            "forget",
            "Delete a note by its id, when it is wrong or out of date.",
            object(json!({ "id": { "type": "string" } }), &["id"]),
            Kind::Meta,
            false,
            forget,
        ));
    }
    if !helper && auto.max_parallel_agents > 0 {
        tools.push(tool(
            "delegate",
            "Start a helper agent on an independent part of the task, in its own background tab. Its result arrives as a message when it finishes.",
            object(
                json!({
                    "goal": { "type": "string", "description": "Everything the helper needs to know: what to do and what to report back." },
                    "url": { "type": "string", "description": "Where the helper starts." }
                }),
                &["goal"],
            ),
            Kind::Meta,
            false,
            delegate,
        ));
        tools.push(tool(
            "wait_agents",
            "Wait until your helpers finish, or until the time runs out.",
            object(
                json!({ "seconds": { "type": "number", "description": "Longest wait. Defaults to 120." } }),
                &[],
            ),
            Kind::Meta,
            false,
            wait_agents,
        ));
    }
    tools
}

fn update_plan(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let items: Vec<PlanItem> = args
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let (text, status) = match item {
                        Value::String(text) => (text.clone(), "pending".to_string()),
                        _ => (
                            item.get("text")?.as_str()?.to_string(),
                            item.get("status")
                                .and_then(Value::as_str)
                                .unwrap_or("pending")
                                .to_string(),
                        ),
                    };
                    let status = match status.as_str() {
                        "active" | "done" | "skipped" => status,
                        "in_progress" | "doing" => "active".into(),
                        "completed" | "complete" => "done".into(),
                        _ => "pending".into(),
                    };
                    let text = clip(text.trim(), 200);
                    (!text.is_empty()).then_some(PlanItem { text, status })
                })
                .take(25)
                .collect()
        })
        .unwrap_or_default();
    let count = items.len();
    with_run(ctx.id, |state| state.run.plan = items);
    runs::emit(ctx.app);
    step("", format!("Plan updated with {count} items."))
}

fn ask_user(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let question = arg_str(args, "question");
    let label = format!("Ask: {}", clip(&question, 120));
    match runs::ask(ctx.app, ctx.id, "question", &question, None, "") {
        Some(Answer::Reply(text)) => step(label, format!("The user answered: {text}")),
        Some(Answer::Allow(_)) => step(label, "The user didn't answer."),
        None => Err("Stopped.".into()),
    }
}

fn done(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let summary = arg_str(args, "summary");
    runs::push_step(ctx.app, ctx.id, "answer", &summary, "");
    Ok(Done::Finish("done", summary))
}

fn fail(_ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let reason = arg_str(args, "reason");
    Ok(Done::Finish(
        "failed",
        if reason.is_empty() {
            "The agent gave up.".into()
        } else {
            reason
        },
    ))
}

fn page_url(ctx: &Ctx) -> String {
    tabs::tab_info(ctx.app, ctx.tab)
        .map(|info| info.url)
        .unwrap_or_default()
}

fn remember(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let text = arg_str(args, "text");
    let site = match arg_str(args, "site").trim() {
        "this site" | "current" | "here" => sleep_site_key(&page_url(ctx)),
        other => other.to_string(),
    };
    let saved = memory::save(
        ctx.app,
        MemoryItem {
            text,
            site,
            pinned: arg_bool(args, "pinned", false),
            ..MemoryItem::default()
        },
    )?;
    step(
        format!("Remember: {}", clip(&saved.text, 80)),
        format!("Saved note {}.", saved.id),
    )
}

fn recall(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let query = arg_str(args, "query");
    let items = memory::recall(ctx.app, &query, &page_url(ctx), 12);
    step(
        if query.trim().is_empty() {
            "Look through notes".to_string()
        } else {
            format!("Recall \"{}\"", clip(&query, 40))
        },
        if items.is_empty() {
            "No notes match.".to_string()
        } else {
            items
                .iter()
                .map(memory::describe)
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

fn forget(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let id = arg_str(args, "id");
    if memory::delete(ctx.app, id.trim()) {
        step(format!("Forget note {id}"), "Deleted.")
    } else {
        Err("There is no note with that id.".into())
    }
}

fn delegate(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let goal = arg_str(args, "goal");
    if goal.trim().is_empty() {
        return Err("Describe what the helper should do.".into());
    }
    let max = ctx.auto.max_parallel_agents as usize;
    let working = runs::active_children(ctx.id);
    if working.len() >= max {
        return Err(format!(
            "{} helpers are already working, the most allowed. Call wait_agents first.",
            working.len()
        ));
    }
    let address = arg_str(args, "url");
    let url = tabs::resolve(
        ctx.app,
        if address.trim().is_empty() {
            paths::NEW_TAB
        } else {
            &address
        },
    );
    gate(ctx, policy::open(ctx.auto, url.as_str()))?;
    halt(ctx)?;
    let opened = tabs::open(
        ctx.app,
        tabs::OpenTab {
            window: ctx.window.to_string(),
            background: true,
            ..tabs::OpenTab::new(url)
        },
    )?;
    with_run(ctx.id, |state| state.opened.push(opened));
    let child = runs::start_with(
        ctx.app,
        opened,
        &goal,
        StartOptions {
            parent: Some(ctx.id),
            background: true,
            ..StartOptions::default()
        },
    )
    .inspect_err(|_| {
        let _ = tabs::close_tab(ctx.app, opened);
    })?;
    step(
        format!("Start helper #{child}: {}", clip(&goal, 80)),
        format!(
            "Helper #{child} is working in tab {opened}. Its result arrives as a message when it finishes."
        ),
    )
}

fn wait_agents(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let limit = Duration::from_secs_f64(arg_f64(args, "seconds").unwrap_or(120.0).clamp(5.0, 600.0));
    let start = Instant::now();
    loop {
        let working = runs::active_children(ctx.id);
        if working.is_empty() {
            return step(
                "Wait for helpers",
                "Every helper has finished. Their results follow as messages.",
            );
        }
        if start.elapsed() >= limit {
            let list: Vec<String> = working
                .iter()
                .map(|(id, goal)| format!("#{id} ({})", clip(goal, 60)))
                .collect();
            return step(
                "Wait for helpers",
                format!("Still working: {}. Finished helpers' results follow as messages.", list.join(", ")),
            );
        }
        runs::pause(ctx.id, Duration::from_millis(800)).map_err(|_| "Stopped.".to_string())?;
    }
}
