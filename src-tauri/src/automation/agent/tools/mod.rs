//! The tools the agent may call. Each one is registered with what it touches and a handler.

mod browser;
mod data;
mod meta;
mod tabs;

use serde_json::{Value, json};
use tauri::AppHandle;

use crate::ai::{ToolCall, ToolSpec};
use crate::automation::driver;
use crate::automation::flows::{FlowStep, FlowTarget};
use crate::automation::policy::{self, Verdict};
use crate::automation::{audit, clip};
use crate::settings::AutomationSettings;

use super::runs::{self, current_tab, halted, push_step, with_run};

/// What a tool does, which decides how many may run in one turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Only reads. Several may run in one turn.
    Read,
    /// Acts on a page. The turn ends after it, since the page may change.
    Page,
    /// Changes the run or browser data without touching the page.
    Meta,
}

pub struct Tool {
    pub spec: ToolSpec,
    pub kind: Kind,
    /// Refused on pages automation may not act on, such as the browser's own.
    pub on_page: bool,
    pub run: fn(&mut Ctx, &Value) -> Result<Done, String>,
}

pub struct Ctx<'a> {
    pub app: &'a AppHandle,
    pub id: u32,
    pub auto: &'a AutomationSettings,
    pub window: &'a str,
    pub tab: u32,
    pub scale: f64,
    /// A screenshot to send with the result, in base64.
    pub image: Option<String>,
    /// Flow steps that repeat what the tool did.
    pub flow: Vec<FlowStep>,
}

pub enum Done {
    /// The label shows in the panel; a blank label adds no step. The detail goes to the model.
    Step { label: String, detail: String },
    Finish(&'static str, String),
}

pub fn step(label: impl Into<String>, detail: impl Into<String>) -> Result<Done, String> {
    Ok(Done::Step {
        label: label.into(),
        detail: detail.into(),
    })
}

pub enum Outcome {
    Continue {
        result: String,
        image: Option<String>,
        page: bool,
    },
    Finish(&'static str, String),
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn reference() -> Value {
    json!({ "type": "integer", "description": "Element ref from the newest page state." })
}

fn tool(
    name: &'static str,
    description: &'static str,
    parameters: Value,
    kind: Kind,
    on_page: bool,
    run: fn(&mut Ctx, &Value) -> Result<Done, String>,
) -> Tool {
    Tool {
        spec: ToolSpec {
            name,
            description,
            parameters,
        },
        kind,
        on_page,
        run,
    }
}

/// The tools a run may use with these settings. Helpers can't start helpers of their own.
pub fn all(auto: &AutomationSettings, helper: bool) -> Vec<Tool> {
    let mut tools = browser::tools(auto);
    tools.extend(tabs::tools());
    tools.extend(data::tools(auto));
    tools.extend(meta::tools(auto, helper));
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_unique_and_gated() {
        let everything = AutomationSettings {
            agent_allow_script: true,
            ..AutomationSettings::default()
        };
        let tools = all(&everything, false);
        let mut names: Vec<&str> = tools.iter().map(|tool| tool.spec.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert!(names.contains(&"run_script"));
        assert!(names.contains(&"delegate"));
        let helper = all(&AutomationSettings::default(), true);
        assert!(!helper.iter().any(|tool| tool.spec.name == "delegate"));
        assert!(!helper.iter().any(|tool| tool.spec.name == "run_script"));
        let forgetful = AutomationSettings {
            agent_memory: false,
            ..AutomationSettings::default()
        };
        assert!(!all(&forgetful, false)
            .iter()
            .any(|tool| tool.spec.name == "remember"));
    }
}

pub fn arg_u32(args: &Value, key: &str) -> Result<u32, String> {
    args.get(key)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_f64().map(|n| n as u64))
                .or_else(|| {
                    value
                        .as_str()
                        .and_then(|s| s.trim().trim_matches(['[', ']']).parse().ok())
                })
        })
        .map(|value| value as u32)
        .ok_or_else(|| format!("Missing \"{key}\"."))
}

pub fn arg_str(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub fn arg_bool(args: &Value, key: &str, fallback: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

pub fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    })
}

pub fn arg_list(args: &Value, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect(),
        Some(Value::String(one)) => vec![one.clone()],
        _ => Vec::new(),
    }
}

pub fn element_label(info: &driver::ElementInfo, element: u32) -> String {
    if info.name.trim().is_empty() {
        format!("{} [{element}]", info.role)
    } else {
        format!("\"{}\"", clip(info.name.trim(), 60))
    }
}

/// How a flow finds this element again, described for AI healing.
pub fn flow_target(ctx: &Ctx, element: u32, label: &str) -> Option<FlowTarget> {
    let value = driver::locators_of(ctx.app, ctx.tab, element).ok()?;
    let mut target: FlowTarget = serde_json::from_value(value).ok()?;
    if target.description.trim().is_empty() {
        target.description = format!("the {} element", label.trim_matches('"'));
    }
    Some(target)
}

/// Runs one tool call and records it in the panel.
pub fn execute(
    app: &AppHandle,
    id: u32,
    auto: &AutomationSettings,
    window: &str,
    scale: f64,
    tools: &[Tool],
    call: &ToolCall,
) -> Outcome {
    if halted(id) {
        return Outcome::Finish("stopped", "Stopped.".into());
    }
    let name = call.name.as_str();
    let Some(tool) = tools.iter().find(|tool| tool.spec.name == name) else {
        let error = format!("There is no tool named \"{name}\".");
        push_step(app, id, "error", &format!("{name} failed"), &error);
        return Outcome::Continue {
            result: format!("Error: {error}"),
            image: None,
            page: false,
        };
    };
    let page = tool.kind == Kind::Page;
    let tab = current_tab(id).unwrap_or(0);
    if tool.on_page {
        let url = crate::tabs::tab_info(app, tab)
            .map(|info| info.url)
            .unwrap_or_default();
        if let Verdict::Block(reason) = policy::page(auto, &url) {
            push_step(app, id, "blocked", &format!("{name} refused"), &reason);
            return Outcome::Continue {
                result: reason,
                image: None,
                page: false,
            };
        }
    }
    let mut ctx = Ctx {
        app,
        id,
        auto,
        window,
        tab,
        scale,
        image: None,
        flow: Vec::new(),
    };
    match (tool.run)(&mut ctx, &call.arguments) {
        Ok(Done::Finish(status, text)) => Outcome::Finish(status, text),
        Ok(Done::Step { label, detail }) => {
            if !label.is_empty() {
                push_step(
                    app,
                    id,
                    if page { "action" } else { "tool" },
                    &label,
                    &clip(&detail, 300),
                );
                audit(app, "agent", tab, name, &label);
            }
            if !ctx.flow.is_empty() {
                with_run(id, |state| {
                    for step in ctx.flow.drain(..) {
                        if state.flow_steps.len() < 500 {
                            state.flow_steps.push(step);
                        }
                    }
                });
            }
            if halted(id) {
                Outcome::Finish("stopped", "Stopped.".into())
            } else {
                Outcome::Continue {
                    result: detail,
                    image: ctx.image,
                    page,
                }
            }
        }
        Err(error) => {
            if halted(id) || error == "Stopped." {
                return Outcome::Finish("stopped", "Stopped.".into());
            }
            push_step(app, id, "error", &format!("{name} failed"), &error);
            Outcome::Continue {
                result: format!("Error: {error}"),
                image: None,
                page,
            }
        }
    }
}

/// Gate for the tool handlers.
pub fn gate(ctx: &Ctx, verdict: Verdict) -> Result<(), String> {
    runs::gate(ctx.app, ctx.id, verdict)
}

pub fn halt(ctx: &Ctx) -> Result<(), String> {
    runs::halt(ctx.id)
}
