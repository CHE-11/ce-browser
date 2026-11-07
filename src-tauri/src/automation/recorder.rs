//! Records what the user does in one tab as flow steps.
//! A script in the page posts each click, field value, choice and key press with a token.

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::AppHandle;

use crate::tabs;

use super::flows::{self, Flow, FlowStep, FlowTarget};
use super::{clip, driver, now_ms};

const SCRIPT: &str = include_str!("recorder.js");
const LOCATORS: &str = include_str!("locators.js");
const MAX_STEPS: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRecording {
    pub tab_id: u32,
    pub steps: Vec<FlowStep>,
    pub started: u64,
    /// Flow the steps are added to when saved. Empty saves a new flow.
    pub flow_id: String,
}

struct Recording {
    tab: u32,
    token: String,
    start_url: String,
    script: Option<String>,
    steps: Vec<FlowStep>,
    started: u64,
    flow_id: String,
}

fn state() -> &'static Mutex<Option<Recording>> {
    static STATE: OnceLock<Mutex<Option<Recording>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub fn current() -> Option<FlowRecording> {
    state()
        .lock()
        .expect("recorder lock")
        .as_ref()
        .map(|recording| FlowRecording {
            tab_id: recording.tab,
            steps: recording.steps.clone(),
            started: recording.started,
            flow_id: recording.flow_id.clone(),
        })
}

pub fn recording_tab() -> Option<u32> {
    state()
        .lock()
        .expect("recorder lock")
        .as_ref()
        .map(|recording| recording.tab)
}

fn script(token: &str) -> String {
    format!(
        "(function () {{\n{LOCATORS}\n{}\n}})();",
        SCRIPT.replace("__CE_RECORD_TOKEN__", token)
    )
}

fn new_token() -> String {
    let mut raw = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Starts recording in `tab`. A recording in another tab is saved first. With `flow_id`, saving
/// adds the steps to the end of that flow instead of making a new one.
pub fn start(app: &AppHandle, tab: u32, flow_id: Option<&str>) -> Result<(), String> {
    let info = tabs::tab_info(app, tab).ok_or("That tab is closed.")?;
    let flow_id = flow_id.map(str::trim).unwrap_or_default().to_string();
    if !flow_id.is_empty() && flows::flow(app, &flow_id).is_none() {
        return Err("That flow no longer exists.".into());
    }
    if !info.url.starts_with("http://")
        && !info.url.starts_with("https://")
        && !info.url.starts_with("file:")
    {
        return Err("Open a web page to record a flow.".into());
    }
    if recording_tab().is_some() {
        stop(app, true)?;
    }
    let token = new_token();
    let source = script(&token);
    let identifier = driver::cdp(
        app,
        tab,
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": source }),
    )
    .ok()
    .and_then(|value| {
        value
            .get("identifier")
            .and_then(Value::as_str)
            .map(String::from)
    });
    *state().lock().expect("recorder lock") = Some(Recording {
        tab,
        token,
        start_url: info.url.clone(),
        script: identifier,
        steps: Vec::new(),
        started: now_ms(),
        flow_id,
    });
    driver::evaluate_main(app, tab, &source).map_err(|error| {
        *state().lock().expect("recorder lock") = None;
        error
    })?;
    flows::emit(app);
    Ok(())
}

/// Ends the recording. With `save` and at least one step, stores the steps and returns the id of
/// the flow that holds them.
pub fn stop(app: &AppHandle, save: bool) -> Result<Option<String>, String> {
    let Some((tab, token, script)) =
        state()
            .lock()
            .expect("recorder lock")
            .as_ref()
            .map(|recording| {
                (
                    recording.tab,
                    recording.token.clone(),
                    recording.script.clone(),
                )
            })
    else {
        return Ok(None);
    };
    if tabs::tab_info(app, tab).is_some() {
        if let Some(identifier) = &script {
            let _ = driver::cdp(
                app,
                tab,
                "Page.removeScriptToEvaluateOnNewDocument",
                json!({ "identifier": identifier }),
            );
        }
        let _ = driver::evaluate_main(
            app,
            tab,
            &format!("window.dispatchEvent(new Event('ce-record-stop-{token}'))"),
        );
        thread::sleep(Duration::from_millis(200));
    }
    let Some(recording) = state().lock().expect("recorder lock").take() else {
        return Ok(None);
    };
    let steps = recording.steps;
    flows::emit(app);
    if !save || steps.is_empty() {
        return Ok(None);
    }
    let existing = if recording.flow_id.is_empty() {
        None
    } else {
        flows::flow(app, &recording.flow_id)
    };
    if let Some(mut flow) = existing {
        if flow.start_url.trim().is_empty() {
            flow.start_url = recording.start_url;
        }
        flow.steps.extend(steps);
        let saved = flows::save(app, flow)?;
        return Ok(Some(saved.id));
    }
    let host = url::Url::parse(&recording.start_url)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.trim_start_matches("www.").to_string())
        })
        .unwrap_or_else(|| "page".into());
    let flow = Flow {
        name: format!("Recorded on {host}"),
        start_url: recording.start_url,
        steps,
        ..Flow::default()
    };
    let saved = flows::save(app, flow)?;
    Ok(Some(saved.id))
}

/// Starts or stops recording from a shortcut. Stopping saves and opens the flow.
pub fn toggle(app: &AppHandle, tab: u32) -> Result<(), String> {
    let handle = app.clone();
    thread::spawn(move || {
        let result = if recording_tab().is_some() {
            stop(&handle, true).and_then(|saved| match saved {
                Some(id) => flows::open_page(
                    &handle,
                    &tabs::focused_window(&handle),
                    Some(&format!("flow={id}")),
                ),
                None => Ok(()),
            })
        } else {
            start(&handle, tab, None)
        };
        if let Err(error) = result {
            crate::system::toast("Recording", &error, "");
        }
    });
    Ok(())
}

/// A step posted by the page script. Messages without the recording's token are ignored.
pub fn on_message(app: &AppHandle, tab: u32, message: &Value) {
    let token = message.get("token").and_then(Value::as_str).unwrap_or("");
    let Some(raw) = message.get("step") else {
        return;
    };
    let step = parse_step(raw);
    let changed = {
        let mut guard = state().lock().expect("recorder lock");
        let Some(recording) = guard.as_mut() else {
            return;
        };
        if recording.tab != tab || recording.token != token || step.kind.is_empty() {
            return;
        }
        push(&mut recording.steps, step)
    };
    if changed {
        flows::emit(app);
    }
}

/// Drops a recorded step before the recording is saved.
pub fn remove_step(app: &AppHandle, index: usize) -> Result<(), String> {
    {
        let mut guard = state().lock().expect("recorder lock");
        let recording = guard.as_mut().ok_or("Nothing is being recorded.")?;
        if index >= recording.steps.len() {
            return Err("That step is no longer in the recording.".into());
        }
        recording.steps.remove(index);
    }
    flows::emit(app);
    Ok(())
}

/// The user loaded an address from the address bar in the recording tab.
pub fn note_navigate(app: &AppHandle, tab: u32, url: &str) {
    let changed = {
        let mut guard = state().lock().expect("recorder lock");
        let Some(recording) = guard.as_mut().filter(|recording| recording.tab == tab) else {
            return;
        };
        let step = FlowStep {
            kind: "navigate".into(),
            url: url.to_string(),
            ..flows::new_step()
        };
        push(&mut recording.steps, step)
    };
    if changed {
        flows::emit(app);
    }
}

pub fn tab_closed(app: &AppHandle, tab: u32) {
    if recording_tab() == Some(tab) {
        let handle = app.clone();
        thread::spawn(move || {
            let _ = stop(&handle, true);
        });
    }
}

fn parse_step(raw: &Value) -> FlowStep {
    let text = |key: &str| {
        raw.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let target = raw
        .get("target")
        .filter(|value| value.is_object())
        .map(|value| {
            let field = |key: &str| clip(value.get(key).and_then(Value::as_str).unwrap_or(""), 400);
            FlowTarget {
                role: field("role"),
                name: field("name"),
                text: field("text"),
                test_id: field("testId"),
                css: field("css"),
                xpath: field("xpath"),
                tag: field("tag"),
                description: field("description"),
            }
        });
    let kind = match text("kind").as_str() {
        kind @ ("click" | "type" | "press" | "select" | "upload") => kind.to_string(),
        _ => String::new(),
    };
    FlowStep {
        kind,
        target,
        text: clip(&text("text"), 4000),
        keys: clip(&text("keys"), 60),
        values: raw
            .get("values")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|item| clip(item, 400))
                    .take(50)
                    .collect()
            })
            .unwrap_or_default(),
        note: clip(&text("note"), 300),
        ..flows::new_step()
    }
}

fn same_target(a: &Option<FlowTarget>, b: &Option<FlowTarget>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.css == b.css && a.xpath == b.xpath,
        _ => false,
    }
}

/// Adds a step, folding repeats: later typing into the same field replaces the earlier value.
fn push(steps: &mut Vec<FlowStep>, step: FlowStep) -> bool {
    if let Some(last) = steps.last_mut() {
        if step.kind == "type" && last.kind == "type" && same_target(&last.target, &step.target) {
            last.text = step.text;
            return true;
        }
        if step.kind == "navigate" && last.kind == "navigate" {
            last.url = step.url;
            return true;
        }
    }
    if steps.len() >= MAX_STEPS {
        return false;
    }
    steps.push(step);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_folds_into_one_step() {
        let target = Some(FlowTarget {
            css: "#q".into(),
            ..FlowTarget::default()
        });
        let mut steps = Vec::new();
        push(
            &mut steps,
            FlowStep {
                kind: "type".into(),
                target: target.clone(),
                text: "a".into(),
                ..flows::new_step()
            },
        );
        push(
            &mut steps,
            FlowStep {
                kind: "type".into(),
                target,
                text: "abc".into(),
                ..flows::new_step()
            },
        );
        push(
            &mut steps,
            FlowStep {
                kind: "press".into(),
                keys: "Enter".into(),
                ..flows::new_step()
            },
        );
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].text, "abc");
    }

    #[test]
    fn script_is_scoped() {
        let source = script("abc");
        assert!(source.starts_with("(function () {"));
        assert!(source.contains("\"abc\""));
        assert!(!source.contains("__CE_RECORD_TOKEN__"));
    }
}
