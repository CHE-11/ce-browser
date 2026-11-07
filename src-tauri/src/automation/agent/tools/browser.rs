//! Tools that look at or act on the current page.

use std::time::Duration;

use serde_json::{Value, json};

use crate::automation::driver;
use crate::automation::flows::{self, FlowStep};
use crate::automation::policy::{self, Risk, Verdict};
use crate::automation::{clip, fill_secrets, mask_secrets};
use crate::settings::AutomationSettings;
use crate::{media, print, tabs};

use super::super::{observe, runs};
use super::{
    Ctx, Done, Kind, Tool, arg_bool, arg_f64, arg_list, arg_str, arg_u32, element_label,
    flow_target, gate, halt, object, reference, site_gate, step, tool,
};

pub fn tools(auto: &AutomationSettings) -> Vec<Tool> {
    let mut tools = vec![
        tool(
            "click",
            "Click an element with a real mouse click.",
            object(
                json!({
                    "ref": reference(),
                    "button": { "type": "string", "enum": ["left", "right", "middle"] },
                    "double": { "type": "boolean", "description": "Double-click." }
                }),
                &["ref"],
            ),
            Kind::Page,
            true,
            click,
        ),
        tool(
            "click_xy",
            "Click a point on the screenshot, in screenshot pixels. Only for targets without a ref.",
            object(
                json!({ "x": { "type": "number" }, "y": { "type": "number" } }),
                &["x", "y"],
            ),
            Kind::Page,
            true,
            click_xy,
        ),
        tool(
            "type",
            "Type text into a field. Without ref, types into the focused element.",
            object(
                json!({
                    "ref": reference(),
                    "text": { "type": "string" },
                    "clear": { "type": "boolean", "description": "Replace the current value. Defaults to true." },
                    "submit": { "type": "boolean", "description": "Press Enter after typing." }
                }),
                &["text"],
            ),
            Kind::Page,
            true,
            type_text,
        ),
        tool(
            "press",
            "Press a key or chord, such as Enter, Escape, Tab, ArrowDown or Control+A.",
            object(json!({ "keys": { "type": "string" } }), &["keys"]),
            Kind::Page,
            true,
            press,
        ),
        tool(
            "scroll",
            "Scroll the page, or the scrollable element with ref.",
            object(
                json!({
                    "direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                    "screens": { "type": "number", "description": "How far, in screens. Defaults to 1." },
                    "ref": reference()
                }),
                &["direction"],
            ),
            Kind::Page,
            true,
            scroll,
        ),
        tool(
            "select",
            "Choose options in a <select> list box by their text or value.",
            object(
                json!({ "ref": reference(), "values": { "type": "array", "items": { "type": "string" } } }),
                &["ref", "values"],
            ),
            Kind::Page,
            true,
            select,
        ),
        tool(
            "hover",
            "Move the mouse over an element, for menus that open on hover.",
            object(json!({ "ref": reference() }), &["ref"]),
            Kind::Page,
            true,
            hover,
        ),
        tool(
            "navigate",
            "Load an address or a search in the current tab.",
            object(json!({ "url": { "type": "string" } }), &["url"]),
            Kind::Page,
            false,
            navigate,
        ),
        tool(
            "back",
            "Go back one page in the current tab.",
            object(json!({}), &[]),
            Kind::Page,
            false,
            back,
        ),
        tool(
            "wait",
            "Wait for some seconds, or until text appears on the page.",
            object(
                json!({ "seconds": { "type": "number" }, "text": { "type": "string" } }),
                &[],
            ),
            Kind::Page,
            false,
            wait,
        ),
        tool(
            "read_text",
            "Read the whole visible text of the page, with link addresses.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            read_text,
        ),
        tool(
            "read_markdown",
            "Read the page's main content as Markdown, with headings, lists, tables and links. Best for articles and documentation.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            read_markdown,
        ),
        tool(
            "ocr",
            "Read text drawn on screen, such as text inside images or canvases.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            ocr,
        ),
        tool(
            "look",
            "Take a fresh screenshot of the current tab with element labels, to check what the page looks like.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            look,
        ),
        tool(
            "upload",
            "Put local files into a file input. The user always approves this.",
            object(
                json!({ "ref": reference(), "paths": { "type": "array", "items": { "type": "string" } } }),
                &["ref", "paths"],
            ),
            Kind::Page,
            true,
            upload,
        ),
    ];
    if auto.agent_allow_script {
        tools.push(tool(
            "run_script",
            "Run JavaScript in the page and get the value of its last expression as JSON. For reading data the page doesn't show, or actions no other tool can do.",
            object(json!({ "expression": { "type": "string" } }), &["expression"]),
            Kind::Page,
            true,
            run_script,
        ));
    }
    tools
}

fn record(ctx: &mut Ctx, step: FlowStep) {
    ctx.flow.push(step);
}

fn click(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let element = arg_u32(args, "ref")?;
    let info = driver::element_info(ctx.app, ctx.tab, element)?;
    let shown = element_label(&info, element);
    gate(ctx, policy::click(ctx.auto, &info))?;
    let button = arg_str(args, "button");
    let target = flow_target(ctx, element, &shown);
    halt(ctx)?;
    driver::click(
        ctx.app,
        ctx.tab,
        element,
        &button,
        arg_bool(args, "double", false),
    )?;
    record(
        ctx,
        FlowStep {
            kind: "click".into(),
            target,
            keys: if button == "left" {
                String::new()
            } else {
                button
            },
            ..flows::new_step()
        },
    );
    step(format!("Click {shown}"), "Clicked.")
}

fn click_xy(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let x = arg_f64(args, "x").ok_or("Missing \"x\".")?;
    let y = arg_f64(args, "y").ok_or("Missing \"y\".")?;
    halt(ctx)?;
    driver::click_at(
        ctx.app,
        ctx.tab,
        x * ctx.scale,
        y * ctx.scale,
        "left",
        false,
    )?;
    step(format!("Click at {}, {}", x.round(), y.round()), "Clicked.")
}

fn type_text(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let raw = arg_str(args, "text");
    let (text, secret) = fill_secrets(ctx.app, &raw)?;
    let element = arg_u32(args, "ref").ok();
    let submit = arg_bool(args, "submit", false);
    let info = match element {
        Some(element) => Some(driver::element_info(ctx.app, ctx.tab, element)?),
        None => driver::focused_info(ctx.app, ctx.tab)?,
    };
    if let Some(info) = &info {
        gate(ctx, policy::typing(ctx.auto, info, secret))?;
        if submit {
            gate(ctx, policy::submit(ctx.auto, Some(info)))?;
        }
    }
    let shown = mask_secrets(&raw);
    let (into, target) = match (&info, element) {
        (Some(info), Some(element)) => {
            let label = element_label(info, element);
            (format!(" into {label}"), flow_target(ctx, element, &label))
        }
        _ => (String::new(), None),
    };
    halt(ctx)?;
    driver::type_text(
        ctx.app,
        ctx.tab,
        element,
        &text,
        arg_bool(args, "clear", true),
        submit,
    )?;
    record(
        ctx,
        FlowStep {
            kind: "type".into(),
            target,
            text: raw,
            ..flows::new_step()
        },
    );
    if submit {
        record(
            ctx,
            FlowStep {
                kind: "press".into(),
                keys: "Enter".into(),
                ..flows::new_step()
            },
        );
    }
    step(
        format!("Type \"{}\"{into}", clip(&shown, 80)),
        if submit {
            "Typed and pressed Enter."
        } else {
            "Typed."
        },
    )
}

fn press(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let keys = arg_str(args, "keys");
    if keys.to_ascii_lowercase().ends_with("enter") {
        let focused = driver::focused_info(ctx.app, ctx.tab)?;
        gate(ctx, policy::submit(ctx.auto, focused.as_ref()))?;
    }
    halt(ctx)?;
    driver::press(ctx.app, ctx.tab, &keys)?;
    record(
        ctx,
        FlowStep {
            kind: "press".into(),
            keys: keys.clone(),
            ..flows::new_step()
        },
    );
    step(format!("Press {keys}"), "Pressed.")
}

fn scroll(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let direction = arg_str(args, "direction");
    let screens = arg_f64(args, "screens").unwrap_or(1.0).clamp(0.1, 10.0);
    let (amount, horizontal) = match direction.as_str() {
        "up" => (-screens, false),
        "left" => (-screens, true),
        "right" => (screens, true),
        _ => (screens, false),
    };
    let element = arg_u32(args, "ref").ok();
    halt(ctx)?;
    let result = driver::scroll(ctx.app, ctx.tab, element, amount, horizontal)?;
    if element.is_none() {
        record(
            ctx,
            FlowStep {
                kind: "scroll".into(),
                text: direction.clone(),
                seconds: screens,
                ..flows::new_step()
            },
        );
    }
    step(format!("Scroll {direction}"), result)
}

fn select(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let element = arg_u32(args, "ref")?;
    let values = arg_list(args, "values");
    let target = driver::element_info(ctx.app, ctx.tab, element)
        .ok()
        .and_then(|info| flow_target(ctx, element, &element_label(&info, element)));
    halt(ctx)?;
    let picked = driver::select(ctx.app, ctx.tab, element, &values)?;
    record(
        ctx,
        FlowStep {
            kind: "select".into(),
            target,
            values: picked.clone(),
            ..flows::new_step()
        },
    );
    step(
        format!("Choose {}", picked.join(", ")),
        format!("Selected {}.", picked.join(", ")),
    )
}

fn hover(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let element = arg_u32(args, "ref")?;
    let target = driver::element_info(ctx.app, ctx.tab, element)
        .ok()
        .and_then(|info| flow_target(ctx, element, &element_label(&info, element)));
    halt(ctx)?;
    driver::hover(ctx.app, ctx.tab, element)?;
    record(
        ctx,
        FlowStep {
            kind: "hover".into(),
            target,
            ..flows::new_step()
        },
    );
    step(format!("Hover [{element}]"), "Done.")
}

fn navigate(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let input = arg_str(args, "url");
    let url = tabs::resolve(ctx.app, &input);
    site_gate(ctx, url.as_str())?;
    gate(ctx, policy::open(ctx.auto, url.as_str()))?;
    halt(ctx)?;
    driver::navigate(ctx.app, ctx.tab, url.as_str())?;
    record(
        ctx,
        FlowStep {
            kind: "navigate".into(),
            url: url.to_string(),
            ..flows::new_step()
        },
    );
    step(format!("Go to {}", clip(url.as_str(), 100)), "Loaded.")
}

fn back(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    halt(ctx)?;
    driver::back(ctx.app, ctx.tab)?;
    record(
        ctx,
        FlowStep {
            kind: "back".into(),
            ..flows::new_step()
        },
    );
    step("Go back", "Went back.")
}

fn wait(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let seconds = arg_f64(args, "seconds").unwrap_or(2.0).clamp(0.5, 30.0);
    let text = arg_str(args, "text");
    if text.is_empty() {
        runs::pause(ctx.id, Duration::from_secs_f64(seconds))
            .map_err(|_| "Stopped.".to_string())?;
        record(
            ctx,
            FlowStep {
                kind: "wait".into(),
                seconds,
                ..flows::new_step()
            },
        );
        return step(format!("Wait {seconds}s"), "Waited.");
    }
    halt(ctx)?;
    let query = driver::WaitQuery {
        text: text.clone(),
        ..Default::default()
    };
    let seen = driver::wait_for(
        ctx.app,
        ctx.tab,
        &query,
        Duration::from_secs_f64(seconds.max(5.0)),
    )?;
    record(
        ctx,
        FlowStep {
            kind: "waitFor".into(),
            text: text.clone(),
            seconds: seconds.max(5.0),
            ..flows::new_step()
        },
    );
    step(
        format!("Wait for \"{}\"", clip(&text, 60)),
        if seen {
            "The text appeared."
        } else {
            "Timed out; the text did not appear."
        },
    )
}

fn read_text(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    let text = driver::text(ctx.app, ctx.tab, 12_000, true)?;
    step(
        "Read the page text",
        if text.is_empty() {
            "The page has no text.".into()
        } else {
            text
        },
    )
}

fn read_markdown(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    let page = print::extract_page(ctx.app, ctx.tab)?;
    let markdown = media::html_to_markdown(&page.title, &page.html);
    step(
        "Read the page as Markdown",
        if markdown.trim().is_empty() {
            "The page has no readable content.".into()
        } else {
            clip(&markdown, 16_000)
        },
    )
}

fn ocr(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    let text = driver::ocr(ctx.app, ctx.tab)?;
    step(
        "Read the screen",
        if text.trim().is_empty() {
            "No text found on screen.".into()
        } else {
            clip(&text, 6000)
        },
    )
}

fn look(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    ctx.image = Some(observe::look(ctx.app, ctx.tab)?);
    step("Look at the page", "Screenshot attached.")
}

fn upload(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let element = arg_u32(args, "ref")?;
    let paths = arg_list(args, "paths");
    if paths.is_empty() {
        return Err("Name the files to upload.".into());
    }
    gate(
        ctx,
        Verdict::ask(
            Risk::Upload,
            format!("Upload {} to this page?", paths.join(", ")),
        ),
    )?;
    let target = flow_target(ctx, element, "file input");
    halt(ctx)?;
    driver::upload(ctx.app, ctx.tab, element, &paths)?;
    record(
        ctx,
        FlowStep {
            kind: "upload".into(),
            target,
            values: paths.clone(),
            ..flows::new_step()
        },
    );
    step(format!("Upload {}", paths.join(", ")), "Files attached.")
}

fn run_script(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let expression = arg_str(args, "expression");
    if expression.trim().is_empty() {
        return Err("Write the script to run.".into());
    }
    gate(
        ctx,
        Verdict::ask(
            Risk::Script,
            format!("Run this script on the page?\n{}", clip(&expression, 400)),
        ),
    )?;
    halt(ctx)?;
    let value = driver::evaluate_main(ctx.app, ctx.tab, &expression)?;
    step(
        format!("Run a script ({} characters)", expression.chars().count()),
        clip(&value.to_string(), 20_000),
    )
}
