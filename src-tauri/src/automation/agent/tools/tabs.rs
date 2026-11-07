//! Tools that open, read, switch and close tabs.

use std::time::Duration;

use serde_json::{Value, json};

use crate::automation::driver;
use crate::automation::flows::{self, FlowStep};
use crate::automation::{clip, policy};
use crate::tabs;

use super::super::runs::{self, with_run};
use super::{
    Ctx, Done, Kind, Tool, arg_bool, arg_str, arg_u32, gate, halt, object, site_gate, step, tool,
};

pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            "open_tab",
            "Open an address in a new tab. By default you continue there; with background, you stay in the current tab and can read the new one with read_tab.",
            object(
                json!({
                    "url": { "type": "string" },
                    "background": { "type": "boolean", "description": "Open without switching to it." }
                }),
                &["url"],
            ),
            Kind::Page,
            false,
            open_tab,
        ),
        tool(
            "switch_tab",
            "Continue in another tab of this window.",
            object(json!({ "tabId": { "type": "integer" } }), &["tabId"]),
            Kind::Page,
            false,
            switch_tab,
        ),
        tool(
            "list_tabs",
            "List the tabs in this window with their ids, titles and addresses.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            list_tabs,
        ),
        tool(
            "read_tab",
            "Read the text of another tab in this window without switching to it.",
            object(json!({ "tabId": { "type": "integer" } }), &["tabId"]),
            Kind::Read,
            false,
            read_tab,
        ),
        tool(
            "close_tab",
            "Close a tab you opened during this task.",
            object(json!({ "tabId": { "type": "integer" } }), &["tabId"]),
            Kind::Meta,
            false,
            close_tab,
        ),
    ]
}

fn same_window(ctx: &Ctx, tab: u32) -> Result<tabs::TabInfo, String> {
    let info = tabs::tab_info(ctx.app, tab).ok_or("There is no tab with that id.")?;
    if info.window != ctx.window {
        return Err("That tab is in another window.".into());
    }
    Ok(info)
}

fn open_tab(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let input = arg_str(args, "url");
    let url = tabs::resolve(ctx.app, &input);
    let background = arg_bool(args, "background", false);
    site_gate(ctx, url.as_str())?;
    gate(ctx, policy::open(ctx.auto, url.as_str()))?;
    halt(ctx)?;
    let opened = tabs::open(
        ctx.app,
        tabs::OpenTab {
            window: ctx.window.to_string(),
            background: true,
            ..tabs::OpenTab::new(url.clone())
        },
    )?;
    with_run(ctx.id, |state| state.opened.push(opened));
    if background {
        return step(
            format!("Open {} in the background", clip(url.as_str(), 100)),
            format!(
                "Opened tab {opened} in the background. Read it with read_tab, or switch_tab to work in it."
            ),
        );
    }
    let _ = tabs::activate(ctx.app, opened);
    runs::switch_to(ctx.app, ctx.id, ctx.tab, opened);
    driver::wait_load(ctx.app, opened, Duration::from_secs(20));
    ctx.flow.push(FlowStep {
        kind: "navigate".into(),
        url: url.to_string(),
        ..flows::new_step()
    });
    step(
        format!("Open {} in a new tab", clip(url.as_str(), 100)),
        format!("Opened tab {opened}. You are now in it."),
    )
}

fn switch_tab(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let target = arg_u32(args, "tabId")?;
    let info = same_window(ctx, target)?;
    if let Some(other) = runs::run_for_tab(target)
        && other != ctx.id
    {
        return Err("Another agent is working in that tab.".into());
    }
    if crate::settings::get(ctx.app).ai_blocked(&info.url) {
        return Err(crate::settings::AI_SITE_BLOCKED.into());
    }
    if let policy::Verdict::Block(reason) = policy::page(ctx.auto, &info.url) {
        return Err(reason);
    }
    halt(ctx)?;
    tabs::activate(ctx.app, target)?;
    runs::switch_to(ctx.app, ctx.id, ctx.tab, target);
    step(
        format!("Switch to \"{}\"", clip(&info.title, 60)),
        format!("Now in tab {target}."),
    )
}

fn list_tabs(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    let lines: Vec<String> = tabs::snapshot(ctx.app)
        .tabs
        .into_iter()
        .filter(|info| info.window == ctx.window)
        .map(|info| {
            format!(
                "- {}: {} <{}>{}{}",
                info.id,
                clip(&info.title, 80),
                clip(&info.url, 160),
                if info.id == ctx.tab { " (current)" } else { "" },
                if info.automated && info.id != ctx.tab {
                    " (an agent is working here)"
                } else {
                    ""
                }
            )
        })
        .collect();
    step("List tabs", lines.join("\n"))
}

fn read_tab(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let target = arg_u32(args, "tabId")?;
    let info = same_window(ctx, target)?;
    if crate::settings::get(ctx.app).ai_blocked(&info.url) {
        return Err(crate::settings::AI_SITE_BLOCKED.into());
    }
    if let policy::Verdict::Block(reason) = policy::page(ctx.auto, &info.url) {
        return Err(reason);
    }
    driver::wait_load(ctx.app, target, Duration::from_secs(10));
    let text = driver::text(ctx.app, target, 12_000, true)?;
    step(
        format!("Read \"{}\"", clip(&info.title, 60)),
        format!(
            "Tab {target}: {} <{}>\n\n{}",
            info.title,
            info.url,
            if text.is_empty() {
                "The page has no text.".to_string()
            } else {
                text
            }
        ),
    )
}

fn close_tab(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let target = arg_u32(args, "tabId")?;
    let owned = with_run(ctx.id, |state| state.opened.contains(&target)).unwrap_or(false);
    if !owned {
        return Err("You can only close tabs you opened during this task.".into());
    }
    if target == ctx.tab {
        return Err("Switch to another tab before closing this one.".into());
    }
    if runs::run_for_tab(target).is_some() {
        return Err("An agent is working in that tab.".into());
    }
    tabs::close_tab(ctx.app, target)?;
    with_run(ctx.id, |state| state.opened.retain(|id| *id != target));
    step(format!("Close tab {target}"), "Closed.")
}
