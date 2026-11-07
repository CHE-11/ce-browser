//! The AI agent: looks at a tab, asks the model what to do, runs its tools, and repeats.
//! Runs keep their conversation, so a finished run can take a follow-up.

pub mod memory;
mod observe;
mod prompt;
mod run_loop;
mod runs;
mod store;
mod tools;

use tauri::AppHandle;

use crate::automation::clip;
use crate::automation::flows::{self, Flow};

pub use runs::{
    AgentAccess, AgentRun, StartOptions, answer, clear, clear_dialog, continue_run, delete,
    end_idle_sessions, init, note_dialog, reply, resume, run_detail, run_for_tab, run_tab, runs,
    session, session_checkpoint, session_gate, session_step, set_access, set_autonomy, start_with,
    stop, stop_focused, tab_closed, take_over,
};

/// Saves the page actions of a run as a new flow. Returns the flow's id.
pub fn save_flow(app: &AppHandle, id: u32) -> Result<String, String> {
    let (goal, start_url, steps) = runs::with_run(id, |state| {
        (
            state.run.goal.clone(),
            state.run.start_url.clone(),
            state.flow_steps.clone(),
        )
    })
    .ok_or("That run was removed.")?;
    if steps.is_empty() {
        return Err("This run has no page actions to save.".into());
    }
    let starts_with_navigate = steps.first().is_some_and(|step| step.kind == "navigate");
    let web = start_url.starts_with("http://") || start_url.starts_with("https://");
    let flow = Flow {
        name: format!("Agent: {}", clip(goal.lines().next().unwrap_or(""), 60)),
        description: clip(&goal, 300),
        start_url: if starts_with_navigate || !web {
            String::new()
        } else {
            start_url
        },
        steps,
        ..Flow::default()
    };
    flows::save(app, flow).map(|flow| flow.id)
}
