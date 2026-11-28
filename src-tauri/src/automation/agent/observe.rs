//! What the model sees each turn: the page state, a screenshot and the window's tabs.

use std::thread;
use std::time::Duration;

use base64::Engine;
use tauri::AppHandle;

use crate::ai::{ChatMessage, ChatPart};
use crate::automation::driver::{self, SHOT_WIDTH};
use crate::automation::{clip, jpeg_data_url};
use crate::tabs;

pub const OBSERVATION: &str = "PAGE STATE";

pub struct Observation {
    pub text: String,
    pub image: Option<String>,
    pub thumb: Option<String>,
    /// CSS pixels per screenshot pixel, for `click_xy`.
    pub scale: f64,
    pub url: String,
}

/// Whether the user's ask is likely to need a page screenshot (layout, images, unlabeled UI).
/// Text-only tasks (summarize, extract, search) get the DOM/text page state only.
pub fn task_wants_screenshot(goal: &str, instructions: &str, follow_up: Option<&str>) -> bool {
    let mut text = String::new();
    text.push_str(goal);
    text.push('\n');
    text.push_str(instructions);
    if let Some(follow_up) = follow_up {
        text.push('\n');
        text.push_str(follow_up);
    }
    let text = text.to_lowercase();
    if text.trim().is_empty() {
        return false;
    }
    // Strong "use your eyes" asks.
    const VISUAL: &[&str] = &[
        "screenshot",
        "screen shot",
        "look like",
        "looks like",
        "what do you see",
        "what you see",
        "visually",
        "visual",
        "on screen",
        "on the screen",
        "image",
        "picture",
        "photo",
        "icon",
        "logo",
        "banner",
        "color",
        "colour",
        "layout",
        "design",
        "appearance",
        "chart",
        "graph",
        "map",
        "canvas",
        "thumbnail",
        "where is the",
        "find the button",
        "red button",
        "blue button",
        "green button",
        "click the image",
        "click on the image",
        "unlabeled",
        "without a label",
        "pixel",
    ];
    if VISUAL.iter().any(|needle| text.contains(needle)) {
        return true;
    }
    // Text / data tasks that the DOM state already covers.
    const TEXT_ONLY: &[&str] = &[
        "summarize",
        "summary",
        "extract",
        "read this",
        "read the",
        "what does",
        "what is this page",
        "explain",
        "translate",
        "list the",
        "find in the text",
        "search for",
        "how many",
        "price of",
        "bookmark",
        "history",
        "remember",
        "markdown",
        "copy the",
        "save as",
    ];
    if TEXT_ONLY.iter().any(|needle| text.contains(needle)) {
        return false;
    }
    false
}

pub fn observe(app: &AppHandle, tab: u32, window: &str, with_shot: bool) -> Observation {
    let url = tabs::tab_info(app, tab)
        .map(|info| info.url)
        .unwrap_or_default();
    if crate::settings::get(app).ai_blocked(&url) {
        return Observation {
            text: format!("{OBSERVATION}\n{}\n", crate::settings::AI_SITE_BLOCKED),
            image: None,
            thumb: None,
            scale: 1.0,
            url,
        };
    }
    driver::wait_load(app, tab, Duration::from_secs(8));
    let snap = driver::snapshot(app, tab).or_else(|_| {
        thread::sleep(Duration::from_millis(700));
        driver::snapshot(app, tab)
    });
    let url = tabs::tab_info(app, tab)
        .map(|info| info.url)
        .unwrap_or_default();
    let mut text = format!("{OBSERVATION}\nCurrent tab id: {tab}\n");
    let mut css_width = 0.0;
    match &snap {
        Ok(snap) => {
            css_width = snap.width;
            text.push_str(&snap.render());
            let excerpt = driver::text(app, tab, 1500, false).unwrap_or_default();
            if !excerpt.trim().is_empty() {
                text.push_str("\n\nPage text (start):\n");
                text.push_str(&excerpt);
            }
            if snap.count == 0
                && excerpt.trim().len() < 40
                && let Ok(seen) = driver::ocr(app, tab)
                && !seen.trim().is_empty()
            {
                text.push_str("\n\nText read from the screen:\n");
                text.push_str(&clip(&seen, 2000));
            }
        }
        Err(error) => text.push_str(&format!("URL: {url}\nCouldn't read the page: {error}")),
    }
    let others: Vec<String> = tabs::snapshot(app)
        .tabs
        .into_iter()
        .filter(|info| info.window == window)
        .take(15)
        .map(|info| {
            format!(
                "- {}: {} <{}>{}",
                info.id,
                clip(&info.title, 60),
                clip(&info.url, 80),
                if info.id == tab { " (current)" } else { "" }
            )
        })
        .collect();
    if others.len() > 1 {
        text.push_str("\n\nTabs in this window:\n");
        text.push_str(&others.join("\n"));
    }
    if let Some(pending) = crate::automation::dialog::current()
        && pending.tab == tab
    {
        text.push_str(&format!(
            "\n\nA JavaScript {} dialog is open: \"{}\"\nCall answer_dialog before doing anything else.\n",
            pending.kind,
            clip(&pending.message, 300)
        ));
    }
    if !with_shot {
        text.push_str(
            "\n\nNo screenshot attached. Use the element list and page text. Call look only if you need to see layout, images, colors, or something without a ref.\n",
        );
        return Observation {
            text,
            image: None,
            thumb: None,
            scale: 1.0,
            url,
        };
    }
    let mut observation = Observation {
        text,
        image: None,
        thumb: None,
        scale: 1.0,
        url,
    };
    if let Ok(shot) = driver::screenshot(app, tab, true, SHOT_WIDTH) {
        if css_width > 0.0 && shot.width > 0 {
            observation.scale = css_width / shot.width as f64;
        }
        observation.thumb = driver::shrink_jpeg(&shot.bytes, 480, 60)
            .ok()
            .map(|thumb| jpeg_data_url(&thumb.bytes));
        observation.image = Some(base64::engine::general_purpose::STANDARD.encode(&shot.bytes));
    }
    observation
}

/// A screenshot of `tab` with element labels, for the `look` tool.
pub fn look(app: &AppHandle, tab: u32) -> Result<String, String> {
    let url = tabs::tab_info(app, tab)
        .map(|info| info.url)
        .unwrap_or_default();
    if crate::settings::get(app).ai_blocked(&url) {
        return Err(crate::settings::AI_SITE_BLOCKED.into());
    }
    let shot = driver::screenshot(app, tab, true, SHOT_WIDTH)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&shot.bytes))
}

/// The text of other tabs the user pointed the agent at, for its first turn.
pub fn context_tabs(app: &AppHandle, ids: &[u32]) -> String {
    let mut out = String::new();
    for id in ids {
        let Some(info) = tabs::tab_info(app, *id) else {
            continue;
        };
        if crate::settings::get(app).ai_blocked(&info.url) {
            out.push_str(&format!(
                "\n\nCONTEXT TAB {id}: {}\n",
                crate::settings::AI_SITE_BLOCKED
            ));
            continue;
        }
        let body = driver::text(app, *id, 4000, true)
            .unwrap_or_else(|error| format!("(Couldn't read this tab: {error})"));
        out.push_str(&format!(
            "\n\nCONTEXT TAB {id}: {} <{}>\n{}",
            clip(&info.title, 80),
            clip(&info.url, 160),
            body
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::task_wants_screenshot;

    #[test]
    fn text_tasks_skip_screenshots() {
        assert!(!task_wants_screenshot(
            "Summarize this page and list the key facts",
            "",
            None
        ));
        assert!(!task_wants_screenshot(
            "Extract the prices from the table",
            "",
            None
        ));
        assert!(!task_wants_screenshot(
            "What does this article say?",
            "",
            None
        ));
    }

    #[test]
    fn visual_tasks_request_screenshots() {
        assert!(task_wants_screenshot(
            "What color is the primary button?",
            "",
            None
        ));
        assert!(task_wants_screenshot(
            "Describe the layout of this page",
            "",
            None
        ));
        assert!(task_wants_screenshot(
            "Book a flight",
            "",
            Some("Look at the seat map image")
        ));
    }
}

/// Drops screenshots and page states from all but the newest turn so the request stays small.
pub fn prune(messages: &mut [ChatMessage]) {
    let count = messages.len();
    for (index, message) in messages.iter_mut().enumerate() {
        if index + 2 >= count {
            continue;
        }
        let parts = match message {
            ChatMessage::User(parts) => parts,
            ChatMessage::ToolResult { parts, .. } => parts,
            ChatMessage::Assistant { .. } => continue,
        };
        parts.retain(|part| !matches!(part, ChatPart::Image { .. }));
        for part in parts.iter_mut() {
            if let ChatPart::Text(text) = part {
                if text.starts_with(OBSERVATION) {
                    let url = text
                        .lines()
                        .find(|line| line.starts_with("URL: "))
                        .unwrap_or("")
                        .to_string();
                    *text = format!("(Older page state omitted. {url})");
                } else if text.chars().count() > 2500 {
                    *text = clip(text, 2500);
                }
            }
        }
    }
}
