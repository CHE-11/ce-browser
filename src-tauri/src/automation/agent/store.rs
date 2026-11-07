//! `agent-runs.json`: finished agent runs with their conversations, so threads survive a restart.

use std::sync::Mutex;
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::ai::{ChatMessage, ChatPart};
use crate::automation::flows::FlowStep;
use crate::{files, paths};

use super::observe;
use super::runs::{AgentRun, RunState};

pub const FILE: &str = "agent-runs.json";
/// Newest runs that keep their model conversation. Older ones keep only their steps.
const TRANSCRIPT_RUNS: usize = 30;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Stored {
    runs: Vec<StoredRun>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredRun {
    pub run: AgentRun,
    pub transcript: Vec<ChatMessage>,
    pub flow_steps: Vec<FlowStep>,
}

pub fn load(app: &AppHandle) -> Vec<StoredRun> {
    let stored: Stored = files::read_json(&paths::data_file(app, FILE));
    stored.runs
}

/// What to save for these runs: no thumbnails, no images, and only recent conversations.
pub fn snapshot(runs: &[RunState]) -> Vec<StoredRun> {
    let count = runs.len();
    runs.iter()
        .enumerate()
        .filter(|(_, state)| state.run.source == "agent")
        .map(|(index, state)| {
            let mut run = state.run.clone();
            for step in &mut run.steps {
                step.thumb = None;
            }
            run.approval = None;
            let transcript = if index + TRANSCRIPT_RUNS >= count {
                let mut transcript = state.transcript.clone();
                strip_images(&mut transcript);
                observe::prune(&mut transcript);
                transcript
            } else {
                Vec::new()
            };
            StoredRun {
                run,
                transcript,
                flow_steps: state.flow_steps.clone(),
            }
        })
        .collect()
}

pub fn strip_images(messages: &mut [ChatMessage]) {
    for message in messages {
        let parts = match message {
            ChatMessage::User(parts) => parts,
            ChatMessage::ToolResult { parts, .. } => parts,
            ChatMessage::Assistant { .. } => continue,
        };
        parts.retain(|part| !matches!(part, ChatPart::Image { .. }));
    }
}

/// The save number written last, so a slow older save never replaces a newer one.
static WRITTEN: Mutex<u64> = Mutex::new(0);
static QUEUED: Mutex<u64> = Mutex::new(0);

pub fn save(app: &AppHandle, runs: Vec<StoredRun>) {
    let number = {
        let mut queued = QUEUED.lock().expect("agent store lock");
        *queued += 1;
        *queued
    };
    let path = paths::data_file(app, FILE);
    thread::spawn(move || {
        let mut written = WRITTEN.lock().expect("agent store lock");
        if number <= *written {
            return;
        }
        if let Err(error) = files::write_json(&path, &Stored { runs }) {
            eprintln!("agent runs: {error}");
        }
        *written = number;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_files_load() {
        let stored: Stored =
            serde_json::from_str(r#"{"runs":[{"run":{"id":3,"goal":"x"}}]}"#).expect("parse");
        assert_eq!(stored.runs[0].run.id, 3);
        assert!(stored.runs[0].transcript.is_empty());
        let empty: Stored = serde_json::from_str("{}").expect("parse");
        assert!(empty.runs.is_empty());
    }
}
