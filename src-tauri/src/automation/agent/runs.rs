//! Agent runs: their shared state, the controls the panel calls, approvals and finishing.

use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::ai::{self, ChatMessage};
use crate::automation::flows::FlowStep;
use crate::automation::policy::{self, Risk, Verdict};
use crate::automation::{audit, clip, now_ms};
use crate::settings::sleep_site_key;
use crate::{settings, system, tabs};

use super::{run_loop, store};

pub const MAX_RUNS: usize = 100;
/// Older runs drop their thumbnails so each `agent-changed` event stays small.
const THUMB_RUNS: usize = 3;
/// Runs sent with all their steps. Older finished runs send none; `agent_run` loads them.
const FULL_RUNS: usize = 12;
const MAX_STEPS_KEPT: usize = 400;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentStep {
    /// `think`, `action`, `tool`, `approved`, `blocked`, `error`, `user` or `answer`.
    pub kind: String,
    pub text: String,
    pub detail: String,
    pub thumb: Option<String>,
    pub at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentApproval {
    pub id: u32,
    /// `confirm` asks allow or deny. `question` asks for a typed answer.
    pub kind: String,
    pub prompt: String,
    /// What a `confirm` could do: `open`, `send`, `destructive`, `payment`, `upload`, `script`
    /// or `budget`. Blank for questions.
    pub risk: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlanItem {
    pub text: String,
    /// `pending`, `active`, `done` or `skipped`.
    pub status: String,
}

/// What a run may reach. Tools outside it are not offered to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentAccess {
    /// `site` keeps the run on one site. `browser` lets it go anywhere automation may.
    pub sites: String,
    /// The site a `site` run is held to. Filled in from the first web page it works on.
    pub site: String,
    /// Open, switch, read and close other tabs.
    pub tabs: bool,
    /// History, bookmarks, downloads, boards and saved flows.
    pub browser_data: bool,
    /// Save files to the Downloads folder and upload local files to pages.
    pub files: bool,
    pub clipboard: bool,
    /// Read and write the agent's notes.
    pub memory: bool,
    /// Start helper agents in background tabs.
    pub helpers: bool,
    /// Run JavaScript in pages, when Settings also allows it.
    pub scripts: bool,
    pub notifications: bool,
}

impl Default for AgentAccess {
    fn default() -> Self {
        Self {
            sites: "browser".into(),
            site: String::new(),
            tabs: true,
            browser_data: true,
            files: true,
            clipboard: true,
            memory: true,
            helpers: true,
            scripts: false,
            notifications: true,
        }
    }
}

impl AgentAccess {
    pub fn normalize(mut self) -> Self {
        if self.sites != "site" {
            self.sites = "browser".into();
            self.site.clear();
        }
        self.site = clip(self.site.trim(), 200);
        self
    }

    /// The tighter of two access sets, for a helper started by a run.
    fn within(mut self, parent: &AgentAccess) -> Self {
        if parent.sites == "site" {
            self.sites = "site".into();
            self.site = parent.site.clone();
        }
        self.tabs &= parent.tabs;
        self.browser_data &= parent.browser_data;
        self.files &= parent.files;
        self.clipboard &= parent.clipboard;
        self.memory &= parent.memory;
        self.scripts &= parent.scripts;
        self.notifications &= parent.notifications;
        self.helpers = false;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentRun {
    pub id: u32,
    /// `agent` for runs started from the panel, `mcp` for actions from an MCP client.
    pub source: String,
    pub tab_id: u32,
    pub window: String,
    pub goal: String,
    /// `running`, `waiting`, `paused`, `done`, `failed` or `stopped`.
    pub status: String,
    pub steps: Vec<AgentStep>,
    /// Steps the run has. `steps` is empty for older runs sent in a list.
    pub step_count: u32,
    pub approval: Option<AgentApproval>,
    pub result: String,
    pub started: u64,
    pub finished: u64,
    pub actions: u32,
    pub max_steps: u32,
    /// `ask`, `auto` or `full`.
    pub autonomy: String,
    /// Model override for this run. Blank uses the agent model from Settings.
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Tokens the run may use before it asks to keep going. Zero has no limit.
    pub budget: u64,
    pub plan: Vec<PlanItem>,
    /// The run that started this one as a helper.
    pub parent: Option<u32>,
    /// Works in a background tab and notifies when it finishes.
    pub background: bool,
    /// Page actions that can be saved as a flow.
    pub flow_steps: u32,
    /// Address of the tab when the run started.
    pub start_url: String,
    pub access: AgentAccess,
}

impl AgentRun {
    pub fn active(&self) -> bool {
        matches!(self.status.as_str(), "running" | "waiting" | "paused")
    }
}

/// How a run starts, from the panel or a flow. Blank fields use the settings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StartOptions {
    pub autonomy: String,
    pub model: String,
    pub background: bool,
    /// Other tabs whose text the agent reads before it starts.
    pub context_tabs: Vec<u32>,
    pub budget: u64,
    /// Guidance for the model that the panel doesn't show as part of the goal.
    pub instructions: String,
    /// Blank uses the default access.
    pub access: Option<AgentAccess>,
    #[serde(skip)]
    pub parent: Option<u32>,
}

pub(super) enum Answer {
    Allow(bool),
    Reply(String),
}

pub(super) struct RunState {
    pub run: AgentRun,
    pub stop: bool,
    /// The next MCP `session` call fails once, then a later call may start a new session.
    /// `stop` itself stays set so an action already in flight still refuses to touch the page.
    pub reject_next: bool,
    pub paused: bool,
    pub answer: Option<(u32, Answer)>,
    pub pending_risk: Option<Risk>,
    pub notes: Vec<String>,
    /// Last activity, for ending idle MCP sessions.
    pub touched: u64,
    /// The model conversation, kept so a finished run can take a follow-up.
    pub transcript: Vec<ChatMessage>,
    pub flow_steps: Vec<FlowStep>,
    /// Tabs this run opened, which it may close.
    pub opened: Vec<u32>,
    pub context_tabs: Vec<u32>,
    pub instructions: String,
    /// The follow-up that restarted a finished run.
    pub follow_up: Option<String>,
    /// The run's thread is still going, perhaps winding down after a stop.
    pub working: bool,
}

impl RunState {
    fn new(run: AgentRun) -> Self {
        Self {
            run,
            stop: false,
            reject_next: false,
            paused: false,
            answer: None,
            pending_risk: None,
            notes: Vec::new(),
            touched: now_ms(),
            transcript: Vec::new(),
            flow_steps: Vec::new(),
            opened: Vec::new(),
            context_tabs: Vec::new(),
            instructions: String::new(),
            follow_up: None,
            working: false,
        }
    }
}

#[derive(Default)]
pub(super) struct Agents {
    pub runs: Vec<RunState>,
    pub next_run: u32,
    pub next_approval: u32,
}

impl Agents {
    pub fn get(&mut self, id: u32) -> Option<&mut RunState> {
        self.runs.iter_mut().find(|state| state.run.id == id)
    }
}

fn shared() -> &'static (Mutex<Agents>, Condvar) {
    static AGENTS: OnceLock<(Mutex<Agents>, Condvar)> = OnceLock::new();
    AGENTS.get_or_init(|| (Mutex::new(Agents::default()), Condvar::new()))
}

pub(super) fn lock() -> MutexGuard<'static, Agents> {
    shared().0.lock().expect("agents lock")
}

pub(super) fn with_run<T>(id: u32, change: impl FnOnce(&mut RunState) -> T) -> Option<T> {
    let mut agents = lock();
    agents.get(id).map(change)
}

pub(super) fn wake() {
    shared().1.notify_all();
}

/// Loads saved runs. Runs that were working when the browser closed count as stopped.
pub fn init(app: &AppHandle) {
    let saved = store::load(app);
    let mut agents = lock();
    for item in saved {
        if agents.runs.iter().any(|state| state.run.id == item.run.id) {
            continue;
        }
        let mut run = item.run;
        if run.active() || run.finished == 0 {
            run.status = "stopped".into();
            run.finished = run.finished.max(run.started);
            run.approval = None;
            if run.result.is_empty() {
                run.result = "The browser closed during this run.".into();
            }
        }
        agents.next_run = agents.next_run.max(run.id);
        let mut state = RunState::new(run);
        state.transcript = item.transcript;
        state.flow_steps = item.flow_steps;
        agents.runs.push(state);
    }
    agents.runs.sort_by_key(|state| state.run.id);
}

fn listed(state: &RunState, full: bool) -> AgentRun {
    let mut run = if full || state.run.active() {
        state.run.clone()
    } else {
        AgentRun {
            steps: Vec::new(),
            ..state.run.clone()
        }
    };
    run.step_count = state.run.steps.len() as u32;
    run.flow_steps = state.flow_steps.len() as u32;
    run
}

/// Every run, newest first. Older finished runs come without their steps.
pub fn runs() -> Vec<AgentRun> {
    let agents = lock();
    let count = agents.runs.len();
    agents
        .runs
        .iter()
        .enumerate()
        .rev()
        .map(|(index, state)| listed(state, index + FULL_RUNS >= count))
        .collect()
}

/// One run with all its steps.
pub fn run_detail(id: u32) -> Option<AgentRun> {
    lock()
        .runs
        .iter()
        .find(|state| state.run.id == id)
        .map(|state| listed(state, true))
}

pub(super) fn emit(app: &AppHandle) {
    let _ = app.emit("agent-changed", runs());
}

/// The model's reply as it is written. Empty text clears it.
pub(super) fn emit_delta(app: &AppHandle, id: u32, text: &str, thinking: &str) {
    let _ = app.emit(
        "agent-delta",
        json!({ "runId": id, "text": clip(text, 6000), "thinking": clip(thinking, 6000) }),
    );
}

/// Writes runs to `agent-runs.json` on another thread.
pub(super) fn save(app: &AppHandle) {
    let snapshot = {
        let agents = lock();
        store::snapshot(&agents.runs)
    };
    store::save(app, snapshot);
}

/// The run that is driving `tab`, if any.
pub fn run_for_tab(tab: u32) -> Option<u32> {
    lock()
        .runs
        .iter()
        .find(|state| state.run.tab_id == tab && state.run.active())
        .map(|state| state.run.id)
}

fn check_ready(app: &AppHandle, current: &settings::BrowserSettings) -> Result<(), String> {
    if !current.automation.enabled {
        return Err("Automation is turned off in Settings.".into());
    }
    if !ai::available(app, current) {
        return Err("Set up an AI provider in Settings, under AI, to use the agent.".into());
    }
    Ok(())
}

pub fn start_with(
    app: &AppHandle,
    tab: u32,
    goal: &str,
    options: StartOptions,
) -> Result<u32, String> {
    let goal = goal.trim();
    if goal.is_empty() {
        return Err("Describe what the agent should do.".into());
    }
    let current = settings::get(app);
    check_ready(app, &current)?;
    let info = tabs::tab_info(app, tab).ok_or("That tab is closed.")?;
    if current.ai_blocked(&info.url) {
        return Err(settings::AI_SITE_BLOCKED.into());
    }
    if run_for_tab(tab).is_some() {
        return Err("An agent is already working in this tab.".into());
    }
    let mut autonomy = policy::autonomy(if options.autonomy.trim().is_empty() {
        &current.automation.default_autonomy
    } else {
        &options.autonomy
    });
    let mut model = clip(options.model.trim(), 120);
    let mut access = options.access.clone().unwrap_or_default().normalize();
    if access.sites == "site" && access.site.is_empty() {
        access.site = sleep_site_key(&info.url);
    }
    if let Some(parent) = options.parent
        && let Some((level, parent_model, parent_access)) = with_run(parent, |state| {
            (
                state.run.autonomy.clone(),
                state.run.model.clone(),
                state.run.access.clone(),
            )
        })
    {
        autonomy = policy::lower_autonomy(autonomy, &level);
        if model.is_empty() {
            model = parent_model;
        }
        access = access.within(&parent_access);
    }
    let context_tabs: Vec<u32> = options
        .context_tabs
        .iter()
        .copied()
        .filter(|id| {
            *id != tab && tabs::tab_info(app, *id).is_some_and(|other| other.window == info.window)
        })
        .take(6)
        .collect();
    let id = {
        let mut agents = lock();
        agents.next_run += 1;
        let id = agents.next_run;
        let mut state = RunState::new(AgentRun {
            id,
            source: "agent".into(),
            tab_id: tab,
            window: info.window.clone(),
            goal: clip(goal, 4000),
            status: "running".into(),
            started: now_ms(),
            max_steps: current.automation.max_steps,
            autonomy: autonomy.into(),
            model,
            budget: options.budget,
            parent: options.parent,
            background: options.background,
            start_url: info.url.clone(),
            access,
            ..AgentRun::default()
        });
        state.context_tabs = context_tabs;
        state.instructions = clip(options.instructions.trim(), 2000);
        state.working = true;
        agents.runs.push(state);
        trim_runs(&mut agents);
        id
    };
    tabs::set_automated(app, tab, true);
    emit(app);
    run_loop::spawn(app, id);
    Ok(id)
}

/// Sends a follow-up to a run. A finished run picks up where it stopped, with its history.
/// `tab` is where to continue when the run's own tab has closed.
pub fn continue_run(app: &AppHandle, id: u32, text: &str, tab: Option<u32>) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let (active, source, own_tab, working) = with_run(id, |state| {
        (
            state.run.active(),
            state.run.source.clone(),
            state.run.tab_id,
            state.working,
        )
    })
    .ok_or("That run was removed.")?;
    if active {
        return reply(app, id, text);
    }
    if working {
        return Err("The agent is still stopping. Try again in a moment.".into());
    }
    if source != "agent" {
        return Err("MCP sessions can't take follow-ups. Start a new task.".into());
    }
    let current = settings::get(app);
    check_ready(app, &current)?;
    let tab = if tabs::tab_info(app, own_tab).is_some() {
        own_tab
    } else {
        tab.filter(|tab| tabs::tab_info(app, *tab).is_some())
            .ok_or("The run's tab is closed. Open a tab for it to work in.")?
    };
    if run_for_tab(tab).is_some_and(|other| other != id) {
        return Err("An agent is already working in this tab.".into());
    }
    if tabs::tab_info(app, tab).is_some_and(|info| current.ai_blocked(&info.url)) {
        return Err(settings::AI_SITE_BLOCKED.into());
    }
    let restarted = with_run(id, |state| {
        if state.run.active() || state.working {
            return false;
        }
        state.working = true;
        let earlier = std::mem::take(&mut state.run.result);
        state.follow_up = Some(if state.transcript.is_empty() && !earlier.is_empty() {
            format!(
                "{text}\n\n(Your earlier answer was: {})",
                clip(&earlier, 3000)
            )
        } else {
            text.to_string()
        });
        state.run.status = "running".into();
        state.run.finished = 0;
        state.run.approval = None;
        state.run.tab_id = tab;
        state.run.max_steps = current.automation.max_steps;
        state.stop = false;
        state.paused = false;
        state.answer = None;
        state.notes.clear();
        add_step(&mut state.run, step("user", text, ""));
        true
    })
    .unwrap_or(false);
    if !restarted {
        return reply(app, id, text);
    }
    tabs::set_automated(app, tab, true);
    emit(app);
    run_loop::spawn(app, id);
    Ok(())
}

/// The run that shows an MCP client's actions, moved to `tab`. Starts one when none is active.
pub fn session(app: &AppHandle, tab: u32, client: &str) -> Result<u32, String> {
    let existing = {
        let mut agents = lock();
        agents
            .runs
            .iter_mut()
            .find(|state| state.run.source == "mcp" && (state.run.active() || state.reject_next))
            .map(|state| {
                state.touched = now_ms();
                (
                    state.run.id,
                    state.stop || state.reject_next,
                    state.run.tab_id,
                )
            })
    };
    if let Some((id, stopped, current)) = existing {
        if stopped {
            with_run(id, |state| state.reject_next = false);
            finish(app, id, "stopped", "Stopped by the user.");
            return Err("The user stopped this MCP session in the agent panel. Call again to start a new one.".into());
        }
        if current != tab && tab != 0 {
            switch_to(app, id, current, tab);
        }
        return Ok(id);
    }
    let window = tabs::tab_info(app, tab)
        .map(|info| info.window)
        .unwrap_or_else(|| tabs::focused_window(app));
    let id = {
        let mut agents = lock();
        agents.next_run += 1;
        let id = agents.next_run;
        agents.runs.push(RunState::new(AgentRun {
            id,
            source: "mcp".into(),
            tab_id: tab,
            window,
            goal: if client.trim().is_empty() {
                "MCP client".into()
            } else {
                clip(client.trim(), 120)
            },
            status: "running".into(),
            started: now_ms(),
            autonomy: "ask".into(),
            ..AgentRun::default()
        }));
        trim_runs(&mut agents);
        id
    };
    if tab != 0 {
        tabs::set_automated(app, tab, true);
    }
    emit(app);
    Ok(id)
}

/// Waits while the user has taken over an MCP session. Errors once they stop it.
pub fn session_checkpoint(id: u32) -> Result<(), String> {
    checkpoint(id).map_err(|_| "The user stopped this MCP session.".to_string())
}

/// Records one MCP action in the panel.
pub fn session_step(app: &AppHandle, id: u32, ok: bool, text: &str, detail: &str) {
    with_run(id, |state| {
        state.touched = now_ms();
        state.run.actions += 1;
        add_step(
            &mut state.run,
            step(if ok { "action" } else { "error" }, text, detail),
        );
    });
    emit(app);
}

/// Applies a policy verdict to an MCP action, asking the user in the agent panel when needed.
pub fn session_gate(app: &AppHandle, id: u32, verdict: Verdict) -> Result<(), String> {
    if let Verdict::Ask { .. } = &verdict {
        let window = with_run(id, |state| state.run.window.clone()).unwrap_or_default();
        if !window.is_empty() {
            let _ = crate::automation::panel::toggle(app, &window, Some(true));
        }
    }
    gate(app, id, verdict)
}

/// Ends MCP sessions with no calls for `idle_ms`.
pub fn end_idle_sessions(app: &AppHandle, idle_ms: u64) {
    let now = now_ms();
    let idle: Vec<u32> = lock()
        .runs
        .iter()
        .filter(|state| {
            state.run.source == "mcp"
                && state.run.active()
                && state.run.approval.is_none()
                && !state.paused
                && now.saturating_sub(state.touched) > idle_ms
        })
        .map(|state| state.run.id)
        .collect();
    for id in idle {
        finish(app, id, "done", "The MCP client went quiet.");
    }
}

fn trim_runs(agents: &mut Agents) {
    while agents.runs.len() > MAX_RUNS {
        let Some(index) = agents.runs.iter().position(|state| !state.run.active()) else {
            break;
        };
        agents.runs.remove(index);
    }
    let count = agents.runs.len();
    for state in agents
        .runs
        .iter_mut()
        .take(count.saturating_sub(THUMB_RUNS))
    {
        for step in &mut state.run.steps {
            step.thumb = None;
        }
    }
}

fn children(id: u32) -> Vec<u32> {
    lock()
        .runs
        .iter()
        .filter(|state| state.run.parent == Some(id) && state.run.active())
        .map(|state| state.run.id)
        .collect()
}

/// Active helpers this run started.
pub(super) fn active_children(id: u32) -> Vec<(u32, String)> {
    lock()
        .runs
        .iter()
        .filter(|state| state.run.parent == Some(id) && state.run.active())
        .map(|state| (state.run.id, state.run.goal.clone()))
        .collect()
}

pub fn stop(app: &AppHandle, id: u32) {
    let active = with_run(id, |state| {
        if !state.run.active() {
            return false;
        }
        state.stop = true;
        state.paused = false;
        if state.run.source == "mcp" {
            state.reject_next = true;
        }
        true
    })
    .unwrap_or(false);
    wake();
    if active {
        finish(app, id, "stopped", "Stopped.");
    }
}

/// Stops every active run in the focused window. Escape uses this so a key aimed at the page
/// wins over a click or keystroke the model already chose.
pub fn stop_focused(app: &AppHandle) -> bool {
    stop_window(app, &tabs::focused_window(app))
}

fn stop_window(app: &AppHandle, window: &str) -> bool {
    let ids: Vec<u32> = lock()
        .runs
        .iter()
        .filter(|state| state.run.window == window && state.run.active())
        .map(|state| state.run.id)
        .collect();
    if ids.is_empty() {
        return false;
    }
    for id in ids {
        stop(app, id);
    }
    true
}

pub fn answer(app: &AppHandle, id: u32, approval: u32, allow: bool) {
    with_run(id, |state| {
        if state
            .run
            .approval
            .as_ref()
            .is_some_and(|item| item.id == approval)
        {
            state.answer = Some((approval, Answer::Allow(allow)));
        }
    });
    wake();
    emit(app);
}

/// Answers the agent's question, or adds an instruction to a running agent.
pub fn reply(app: &AppHandle, id: u32, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let tab = with_run(id, |state| state.run.tab_id).ok_or("That run has finished.")?;
    if tabs::tab_info(app, tab).is_some_and(|info| settings::get(app).ai_blocked(&info.url)) {
        return Err(settings::AI_SITE_BLOCKED.into());
    }
    with_run(id, |state| {
        if !state.run.active() {
            return Err("That run has finished.".to_string());
        }
        match &state.run.approval {
            Some(approval) if approval.kind == "question" => {
                state.answer = Some((approval.id, Answer::Reply(text.to_string())));
            }
            _ => state.notes.push(format!("Message from the user: {text}")),
        }
        add_step(&mut state.run, step("user", text, ""));
        Ok(())
    })
    .ok_or("That run has finished.")??;
    wake();
    emit(app);
    Ok(())
}

/// Changes how much a run may do without asking. A waiting approval the new level covers is
/// allowed at once.
pub fn set_autonomy(app: &AppHandle, id: u32, level: &str) -> Result<(), String> {
    let level = policy::autonomy(level);
    with_run(id, |state| {
        if state.run.source != "agent" {
            return Err("MCP sessions always ask.".to_string());
        }
        state.run.autonomy = level.into();
        if let (Some(approval), Some(risk)) = (&state.run.approval, state.pending_risk)
            && approval.kind == "confirm"
            && policy::autonomy_allows(level, risk)
        {
            state.answer = Some((approval.id, Answer::Allow(true)));
        }
        Ok(())
    })
    .ok_or("That run was removed.")??;
    wake();
    emit(app);
    Ok(())
}

/// Changes what a run may reach. Holding a run to one site uses the site of its current tab.
pub fn set_access(app: &AppHandle, id: u32, access: AgentAccess) -> Result<(), String> {
    let mut access = access.normalize();
    let tab = with_run(id, |state| state.run.tab_id).ok_or("That run was removed.")?;
    if access.sites == "site" && access.site.is_empty() {
        access.site = tabs::tab_info(app, tab)
            .map(|info| sleep_site_key(&info.url))
            .unwrap_or_default();
    }
    with_run(id, |state| {
        if state.run.source != "agent" {
            return Err("MCP sessions use the MCP server's settings.".to_string());
        }
        state.run.access = access;
        Ok(())
    })
    .ok_or("That run was removed.")??;
    emit(app);
    Ok(())
}

/// What the run may reach now.
pub(super) fn access(id: u32) -> AgentAccess {
    with_run(id, |state| state.run.access.clone()).unwrap_or_default()
}

/// Holds a `site` run to its site. The first web page fills the site in when it is blank.
pub(super) fn site_allowed(id: u32, url: &str) -> Result<(), String> {
    let host = sleep_site_key(url);
    if host.is_empty() {
        return Ok(());
    }
    with_run(id, |state| {
        let access = &mut state.run.access;
        if access.sites != "site" {
            return Ok(());
        }
        if access.site.is_empty() {
            access.site = host.clone();
            return Ok(());
        }
        if host == access.site || host.ends_with(&format!(".{}", access.site)) {
            Ok(())
        } else {
            Err(format!(
                "This run may only work on {}, and {host} is another site. Go back, or finish and tell the user. They can allow the whole browser from the lock menu.",
                access.site
            ))
        }
    })
    .unwrap_or(Ok(()))
}

/// Pauses the agent before its next action so the user can use the page.
pub fn take_over(app: &AppHandle, id: u32) {
    with_run(id, |state| {
        if state.run.active() {
            state.paused = true;
            state.run.status = "paused".into();
        }
    });
    emit(app);
}

pub fn resume(app: &AppHandle, id: u32) {
    with_run(id, |state| {
        if state.paused {
            state.paused = false;
            state.run.status = "running".into();
            state.notes.push(
                "The user took over the page for a while and may have changed it. Look at the new page state before acting.".into(),
            );
        }
    });
    wake();
    emit(app);
}

/// Forgets finished runs.
pub fn clear(app: &AppHandle) {
    lock().runs.retain(|state| state.run.active());
    save(app);
    emit(app);
}

/// Forgets one run, stopping it first.
pub fn delete(app: &AppHandle, id: u32) {
    stop(app, id);
    lock().runs.retain(|state| state.run.id != id);
    save(app);
    emit(app);
}

/// Stops runs whose tab closed.
pub fn tab_closed(app: &AppHandle, tab: u32) {
    if let Some(id) = run_for_tab(tab) {
        stop(app, id);
    }
    let mut agents = lock();
    for state in &mut agents.runs {
        state.opened.retain(|opened| *opened != tab);
    }
}

pub(super) fn step(kind: &str, text: &str, detail: &str) -> AgentStep {
    AgentStep {
        kind: kind.into(),
        text: clip(text, 4000),
        detail: clip(detail, 4000),
        thumb: None,
        at: now_ms(),
    }
}

fn add_step(run: &mut AgentRun, step: AgentStep) {
    run.steps.push(step);
    if run.steps.len() > MAX_STEPS_KEPT {
        run.steps.remove(0);
    }
}

pub(super) fn push_step(app: &AppHandle, id: u32, kind: &str, text: &str, detail: &str) {
    with_run(id, |state| {
        add_step(&mut state.run, step(kind, text, detail))
    });
    emit(app);
}

pub(super) fn set_last_thumb(id: u32, thumb: String) {
    with_run(id, |state| {
        if let Some(last) = state
            .run
            .steps
            .iter_mut()
            .rev()
            .find(|step| step.kind == "action")
            && last.thumb.is_none()
        {
            last.thumb = Some(thumb);
        }
    });
}

pub(super) struct Stopped;

/// Waits while the user has taken over. Errors once the run is stopped.
pub(super) fn checkpoint(id: u32) -> Result<(), Stopped> {
    let (mutex, condvar) = shared();
    let mut agents = mutex.lock().expect("agents lock");
    loop {
        let Some(state) = agents.get(id) else {
            return Err(Stopped);
        };
        if state.stop || state.run.finished != 0 {
            return Err(Stopped);
        }
        if !state.paused {
            return Ok(());
        }
        agents = condvar
            .wait_timeout(agents, Duration::from_millis(500))
            .expect("agents lock")
            .0;
    }
}

/// Sleeps, returning as soon as the user stops the run.
pub(super) fn pause(id: u32, total: Duration) -> Result<(), Stopped> {
    let slice = Duration::from_millis(100);
    let mut left = total;
    loop {
        checkpoint(id)?;
        if left.is_zero() {
            return Ok(());
        }
        let step = if left < slice { left } else { slice };
        std::thread::sleep(step);
        left = left.saturating_sub(step);
    }
}

pub(super) fn halted(id: u32) -> bool {
    with_run(id, |state| {
        state.stop || state.run.finished != 0 || !state.run.active()
    })
    .unwrap_or(true)
}

pub(super) fn halt(id: u32) -> Result<(), String> {
    if halted(id) {
        Err("Stopped.".into())
    } else {
        Ok(())
    }
}

pub fn risk_name(risk: Risk) -> &'static str {
    match risk {
        Risk::Open => "open",
        Risk::Send => "send",
        Risk::Destructive => "destructive",
        Risk::Payment => "payment",
        Risk::Upload => "upload",
        Risk::Script => "script",
    }
}

fn panel_hidden(app: &AppHandle, window: &str) -> bool {
    tabs::agent_panel_width(app, window) <= 0.0
}

/// Shows a prompt in the agent panel and waits for the user. `None` when the run stopped.
pub(super) fn ask(
    app: &AppHandle,
    id: u32,
    kind: &str,
    prompt: &str,
    risk: Option<Risk>,
    label: &str,
) -> Option<Answer> {
    let (approval, notify) = {
        let mut agents = lock();
        if agents
            .get(id)
            .is_none_or(|state| state.stop || state.run.finished != 0)
        {
            return None;
        }
        agents.next_approval += 1;
        let approval = agents.next_approval;
        let state = agents.get(id)?;
        state.answer = None;
        state.pending_risk = risk;
        state.run.status = "waiting".into();
        state.run.approval = Some(AgentApproval {
            id: approval,
            kind: kind.into(),
            prompt: clip(prompt, 600),
            risk: label.into(),
        });
        let notify = (state.run.source == "agent").then(|| {
            (
                state.run.background,
                state.run.window.clone(),
                state.run.goal.clone(),
            )
        });
        (approval, notify)
    };
    emit(app);
    if let Some((background, window, goal)) = notify
        && settings::get(app).automation.agent_notify
        && (background || panel_hidden(app, &window))
    {
        system::toast(
            &format!("Agent needs you: {}", clip(&goal, 60)),
            &clip(prompt, 200),
            "",
        );
    }
    let (mutex, condvar) = shared();
    let mut agents = mutex.lock().expect("agents lock");
    let answer = loop {
        let Some(state) = agents.get(id) else {
            break None;
        };
        if state.stop || state.run.finished != 0 {
            break None;
        }
        if let Some((answered, _)) = &state.answer
            && *answered == approval
        {
            break state.answer.take().map(|(_, answer)| answer);
        }
        agents = condvar
            .wait_timeout(agents, Duration::from_millis(500))
            .expect("agents lock")
            .0;
    };
    if let Some(state) = agents.get(id) {
        state.run.approval = None;
        state.pending_risk = None;
        if state.run.status == "waiting" && !state.stop && state.run.finished == 0 {
            state.run.status = if state.paused {
                "paused".into()
            } else {
                "running".into()
            };
        }
    }
    drop(agents);
    emit(app);
    answer
}

/// Runs or refuses an action after the policy, the run's autonomy and the user decide.
pub(super) fn gate(app: &AppHandle, id: u32, verdict: Verdict) -> Result<(), String> {
    match verdict {
        Verdict::Allow => Ok(()),
        Verdict::Block(reason) => Err(reason),
        Verdict::Ask { prompt, risk } => {
            let level = with_run(id, |state| state.run.autonomy.clone()).unwrap_or_default();
            if policy::autonomy_allows(&level, risk) {
                push_step(
                    app,
                    id,
                    "approved",
                    &prompt,
                    &format!("Went ahead without asking ({level} autonomy)."),
                );
                return Ok(());
            }
            match ask(app, id, "confirm", &prompt, Some(risk), risk_name(risk)) {
                Some(Answer::Allow(true)) => Ok(()),
                Some(_) => Err(
                    "The user declined this action. Don't try it again. Find another way or finish."
                        .into(),
                ),
                None => Err("Stopped.".into()),
            }
        }
    }
}

/// Asks whether to go on past a limit. True to keep going.
pub(super) fn ask_more(app: &AppHandle, id: u32, prompt: &str) -> bool {
    matches!(
        ask(app, id, "confirm", prompt, None, "budget"),
        Some(Answer::Allow(true))
    )
}

pub(super) fn current_tab(id: u32) -> Option<u32> {
    with_run(id, |state| state.run.tab_id)
}

pub(super) fn finish(app: &AppHandle, id: u32, status: &str, result: &str) {
    let closed = with_run(id, |state| {
        if state.run.finished != 0 {
            return None;
        }
        // A stop already requested wins over a model reply that finishes in the same moment.
        let status = if state.stop { "stopped" } else { status };
        let result = if state.stop {
            "Stopped.".to_string()
        } else {
            clip(result, 8000)
        };
        state.run.status = status.into();
        state.run.result = result.clone();
        state.run.finished = now_ms();
        state.run.approval = None;
        state.pending_risk = None;
        state.paused = false;
        Some((
            state.run.tab_id,
            state.run.source.clone(),
            status.to_string(),
            result,
            state.run.parent,
            state.run.background,
            state.run.window.clone(),
            state.run.goal.clone(),
        ))
    });
    let Some(Some((tab, source, status, result, parent, background, window, goal))) = closed else {
        return;
    };
    tabs::set_automated(app, tab, false);
    audit(app, &source, tab, &status, &result);
    for child in children(id) {
        stop(app, child);
    }
    if let Some(parent) = parent {
        with_run(parent, |state| {
            state.notes.push(format!(
                "Helper #{id} ({}) finished with status {status}: {}",
                clip(&goal, 120),
                clip(&result, 3000)
            ));
        });
    }
    if source == "agent"
        && parent.is_none()
        && status != "stopped"
        && settings::get(app).automation.agent_notify
        && (background || panel_hidden(app, &window))
    {
        let title = if status == "done" {
            "Agent finished"
        } else {
            "Agent couldn't finish"
        };
        system::toast(
            &format!("{title}: {}", clip(&goal, 60)),
            &clip(&result, 200),
            "",
        );
    }
    emit_delta(app, id, "", "");
    wake();
    emit(app);
    if source == "agent" {
        save(app);
    }
}

pub(super) fn switch_to(app: &AppHandle, id: u32, from: u32, to: u32) {
    with_run(id, |state| state.run.tab_id = to);
    if from != to {
        tabs::set_automated(app, from, false);
        tabs::set_automated(app, to, true);
    }
    emit(app);
}
