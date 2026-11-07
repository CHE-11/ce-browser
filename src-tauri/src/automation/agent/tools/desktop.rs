//! Tools that drive the Windows desktop. Registered only when Settings allows it,
//! and only for a run whose access includes the desktop. Helpers never get them.

use serde_json::{Value, json};

use crate::automation::flows::{self, FlowStep};
use crate::automation::{self, desktop, jpeg_data_url};

use super::super::runs;
use super::{Ctx, Done, Kind, Tool, arg_f64, arg_str, halt, object, step, tool};

pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            "look_screen",
            "Take a screenshot of the primary monitor. click_screen and drag_screen use this image's pixels.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            look_screen,
        ),
        tool(
            "click_screen",
            "Click the primary monitor. x and y are pixels of the latest look_screen image.",
            object(
                json!({ "x": { "type": "number" }, "y": { "type": "number" } }),
                &["x", "y"],
            ),
            Kind::Page,
            false,
            click_screen,
        ),
        tool(
            "drag_screen",
            "Drag on the primary monitor. Coordinates are pixels of the latest look_screen image.",
            object(
                json!({
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "toX": { "type": "number" },
                    "toY": { "type": "number" }
                }),
                &["x", "y", "toX", "toY"],
            ),
            Kind::Page,
            false,
            drag_screen,
        ),
        tool(
            "type_screen",
            "Type text into whichever window is focused, outside or inside the browser.",
            object(json!({ "text": { "type": "string" } }), &["text"]),
            Kind::Page,
            false,
            type_screen,
        ),
        tool(
            "list_windows",
            "List visible windows with their screen rectangles.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            list_windows,
        ),
    ]
}

fn look_screen(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    runs::approve_desktop(ctx.app, ctx.id)?;
    halt(ctx)?;
    let (shot, scale) = desktop::screenshot()?;
    let thumb = crate::automation::driver::shrink_jpeg(&shot.bytes, 480, 60)?;
    runs::set_screen(ctx.app, ctx.id, jpeg_data_url(&thumb.bytes), scale);
    ctx.image = Some(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &shot.bytes,
    ));
    step(
        "Look at the screen",
        format!(
            "Screenshot attached, {}x{} pixels. click_screen uses these pixels.",
            shot.width, shot.height
        ),
    )
}

fn click_screen(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    runs::approve_desktop(ctx.app, ctx.id)?;
    let x = arg_f64(args, "x").ok_or("Missing \"x\".")?;
    let y = arg_f64(args, "y").ok_or("Missing \"y\".")?;
    let (screen_x, screen_y) = screen_point(ctx, x, y);
    halt(ctx)?;
    desktop::click(screen_x, screen_y)?;
    record(
        ctx,
        FlowStep {
            kind: "desktopClick".into(),
            x: Some(screen_x),
            y: Some(screen_y),
            ..flows::new_step()
        },
    );
    step(
        format!("Click the screen at {}, {}", x.round(), y.round()),
        "Clicked.",
    )
}

fn drag_screen(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    runs::approve_desktop(ctx.app, ctx.id)?;
    let x = arg_f64(args, "x").ok_or("Missing \"x\".")?;
    let y = arg_f64(args, "y").ok_or("Missing \"y\".")?;
    let to_x = arg_f64(args, "toX").ok_or("Missing \"toX\".")?;
    let to_y = arg_f64(args, "toY").ok_or("Missing \"toY\".")?;
    let (from_x, from_y) = screen_point(ctx, x, y);
    let (end_x, end_y) = screen_point(ctx, to_x, to_y);
    halt(ctx)?;
    desktop::drag(from_x, from_y, end_x, end_y)?;
    record(
        ctx,
        FlowStep {
            kind: "desktopDrag".into(),
            x: Some(from_x),
            y: Some(from_y),
            to_x: Some(end_x),
            to_y: Some(end_y),
            ..flows::new_step()
        },
    );
    step("Drag on the screen", "Dragged.")
}

fn type_screen(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    runs::approve_desktop(ctx.app, ctx.id)?;
    let text = arg_str(args, "text");
    if text.is_empty() {
        return Err("Missing \"text\".".into());
    }
    halt(ctx)?;
    desktop::type_text(&text)?;
    record(
        ctx,
        FlowStep {
            kind: "desktopType".into(),
            text: text.clone(),
            ..flows::new_step()
        },
    );
    step(
        format!("Type \"{}\" on the desktop", automation::clip(&text, 80)),
        "Typed.",
    )
}

fn list_windows(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    runs::approve_desktop(ctx.app, ctx.id)?;
    let titles = desktop::windows();
    step(
        "List windows",
        if titles.is_empty() {
            "No visible windows.".into()
        } else {
            titles.join("\n")
        },
    )
}

fn screen_point(ctx: &Ctx, x: f64, y: f64) -> (f64, f64) {
    let scale = runs::screen_scale(ctx.id);
    (x * scale, y * scale)
}

fn record(ctx: &mut Ctx, step: FlowStep) {
    ctx.flow.push(step);
}
