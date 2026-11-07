//! Saved flows: recorded or written steps that replay in a tab by hand or on a trigger.
//! Flows, run history, dataset metadata and trigger state live in `automations.json`.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter};

use crate::settings::{self, AutomationSettings};
use crate::{ai, files, paths, system, tabs};

use super::driver::{self, WaitQuery};
use super::extract::{self, DatasetInfo, ExtractSpec};
use super::policy::{self, Verdict};
use super::recorder::{self, FlowRecording};
use super::scheduler;
use super::{agent, audit, clip, fill_secrets, mask_secrets, now_ms, today};

pub const FILE: &str = "automations.json";
const MAX_RUNS: usize = 300;
const MAX_FLOW_RUNS: usize = 25;
const MAX_FLOWS: usize = 500;
const MAX_STEPS: usize = 500;
const FIND_TIMEOUT: Duration = Duration::from_secs(10);
const AI_STEP_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowTarget {
    pub role: String,
    pub name: String,
    pub text: String,
    pub test_id: String,
    pub css: String,
    pub xpath: String,
    pub tag: String,
    pub description: String,
}

impl FlowTarget {
    fn describe(&self) -> String {
        if !self.description.trim().is_empty() {
            return self.description.trim().to_string();
        }
        if !self.name.trim().is_empty() {
            return format!(
                "the \"{}\" {}",
                clip(self.name.trim(), 60),
                if self.role.is_empty() {
                    &self.tag
                } else {
                    &self.role
                }
            );
        }
        if !self.text.trim().is_empty() {
            return format!("the element with text \"{}\"", clip(self.text.trim(), 60));
        }
        format!("the element at {}", clip(&self.css, 80))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowStep {
    pub id: String,
    pub kind: String,
    pub target: Option<FlowTarget>,
    pub url: String,
    pub text: String,
    pub keys: String,
    pub values: Vec<String>,
    pub seconds: f64,
    pub prompt: String,
    pub spec: Option<ExtractSpec>,
    /// Pages an `extract` step follows through the list's Next link.
    pub pages: u32,
    pub optional: bool,
    /// Skipped by runs and left out of exports, without deleting the step.
    pub disabled: bool,
    pub note: String,
    /// How much an `ai` step's agent may do without asking: `ask`, `auto` or `full`.
    /// Blank uses the default from Settings.
    pub autonomy: String,
    /// Fields from newer builds, written back unchanged.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowTrigger {
    pub kind: String,
    pub enabled: bool,
    pub minutes: u32,
    pub time: String,
    pub days: Vec<u8>,
    pub pattern: String,
    pub slot: u8,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for FlowTrigger {
    fn default() -> Self {
        Self {
            kind: "daily".into(),
            enabled: true,
            minutes: 60,
            time: "09:00".into(),
            days: Vec::new(),
            pattern: String::new(),
            slot: 1,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Flow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub start_url: String,
    pub steps: Vec<FlowStep>,
    pub triggers: Vec<FlowTrigger>,
    pub variables: BTreeMap<String, String>,
    pub background: bool,
    pub notify: bool,
    pub monitor_prompt: String,
    /// Triggers don't fire while paused. Starting it by hand still works.
    pub paused: bool,
    pub last_run: u64,
    pub last_status: String,
    pub created: u64,
    pub updated: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowRunStep {
    pub index: usize,
    pub kind: String,
    pub label: String,
    /// `ok`, `healed`, `failed` or `skipped`.
    pub status: String,
    pub detail: String,
    /// A screenshot of the page was saved when this step failed. Read it with `run_shot`.
    pub shot: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowRun {
    pub id: u32,
    pub flow_id: String,
    pub flow_name: String,
    pub trigger: String,
    /// `running`, `done`, `failed` or `stopped`.
    pub status: String,
    pub started: u64,
    pub finished: u64,
    pub steps: Vec<FlowRunStep>,
    pub output: String,
    pub dataset_id: String,
    /// Index of the step that is running now.
    pub current: Option<usize>,
    pub current_label: String,
    /// First step the run started at. 0 is the whole flow.
    pub from_step: usize,
    /// Last step the run stops after. `None` runs to the end.
    pub to_step: Option<usize>,
}

/// Which part of a flow a run covers, and where.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunOptions {
    pub from_step: usize,
    pub to_step: Option<usize>,
    /// Stay on the page already open in the run's tab instead of loading the start address.
    pub here: bool,
}

/// Trigger bookkeeping per flow, so restarts neither repeat nor miss runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ScheduleState {
    /// Last time each trigger fired, keyed by `kind:index`.
    pub fired: BTreeMap<String, u64>,
    /// Hash of the watched page's text for `pageChange`.
    pub page_hash: String,
    pub page_text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Store {
    pub flows: Vec<Flow>,
    pub runs: Vec<FlowRun>,
    pub datasets: Vec<DatasetInfo>,
    pub schedule: BTreeMap<String, ScheduleState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationsSnapshot {
    pub flows: Vec<Flow>,
    pub runs: Vec<FlowRun>,
    pub recording: Option<FlowRecording>,
    pub datasets: Vec<DatasetInfo>,
    /// When each flow's triggers next fire, by flow id. Missing for paused and unscheduled flows.
    pub next_runs: BTreeMap<String, u64>,
    /// Why no trigger fires right now. Empty while the scheduler runs.
    pub scheduler_paused: String,
}

struct Loaded {
    path: PathBuf,
    store: Store,
}

fn cache() -> &'static Mutex<Option<Loaded>> {
    static CACHE: OnceLock<Mutex<Option<Loaded>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn load(app: &AppHandle) -> MutexGuard<'static, Option<Loaded>> {
    let path = paths::data_file(app, FILE);
    let mut guard = cache().lock().expect("flows lock");
    if guard.as_ref().is_none_or(|loaded| loaded.path != path) {
        let mut store: Store = files::read_json(&path);
        for run in &mut store.runs {
            if run.status == "running" {
                run.status = "stopped".into();
            }
            run.current = None;
            run.current_label.clear();
        }
        *guard = Some(Loaded { path, store });
    }
    guard
}

/// Reads or changes the store. Call `persist` after a change that should survive a restart.
pub fn with_store<T>(app: &AppHandle, change: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = load(app);
    change(&mut guard.as_mut().expect("loaded store").store)
}

pub fn persist(app: &AppHandle) {
    let guard = load(app);
    if let Some(loaded) = guard.as_ref()
        && let Err(error) = files::write_json(&loaded.path, &loaded.store)
    {
        eprintln!("automations: {error}");
    }
}

pub fn snapshot(app: &AppHandle) -> AutomationsSnapshot {
    let (flows, mut runs, mut datasets) = with_store(app, |store| {
        (
            store.flows.clone(),
            store.runs.clone(),
            store.datasets.clone(),
        )
    });
    runs.reverse();
    datasets.sort_by(|a, b| b.updated.cmp(&a.updated));
    AutomationsSnapshot {
        next_runs: scheduler::next_runs(app, &flows),
        scheduler_paused: scheduler::pause_reason(app).unwrap_or_default().to_string(),
        flows,
        runs,
        recording: recorder::current(),
        datasets,
    }
}

pub fn emit(app: &AppHandle) {
    let _ = app.emit("automations-changed", snapshot(app));
}

pub fn new_id() -> String {
    format!("{:x}{:04x}", now_ms(), rand::random::<u16>())
}

pub fn new_step() -> FlowStep {
    FlowStep {
        id: new_id(),
        ..FlowStep::default()
    }
}

pub fn flows(app: &AppHandle) -> Vec<Flow> {
    with_store(app, |store| store.flows.clone())
}

pub fn flow(app: &AppHandle, id: &str) -> Option<Flow> {
    with_store(app, |store| {
        store.flows.iter().find(|flow| flow.id == id).cloned()
    })
}

fn clean_time(raw: &str) -> String {
    let mut parts = raw.trim().split(':');
    let hour = parts
        .next()
        .and_then(|part| part.trim().parse::<u32>().ok());
    let minute = parts
        .next()
        .and_then(|part| part.trim().parse::<u32>().ok())
        .unwrap_or(0);
    match hour {
        Some(hour) if hour < 24 && minute < 60 => format!("{hour:02}:{minute:02}"),
        _ => "09:00".into(),
    }
}

fn normalize(mut flow: Flow) -> Flow {
    let now = now_ms();
    if flow.id.trim().is_empty() {
        flow.id = new_id();
    }
    if flow.created == 0 {
        flow.created = now;
    }
    flow.updated = now;
    flow.name = clip(flow.name.trim(), 120);
    if flow.name.is_empty() {
        flow.name = "Untitled flow".into();
    }
    flow.description = clip(&flow.description, 2000);
    flow.start_url = flow.start_url.trim().to_string();
    flow.steps.truncate(MAX_STEPS);
    for step in &mut flow.steps {
        if step.id.trim().is_empty() {
            step.id = new_id();
        }
        if !step.seconds.is_finite() || step.seconds < 0.0 {
            step.seconds = 0.0;
        }
        step.pages = step.pages.min(100);
    }
    for trigger in &mut flow.triggers {
        trigger.minutes = trigger.minutes.clamp(1, 10_080);
        trigger.time = clean_time(&trigger.time);
        trigger.days.retain(|day| *day < 7);
        trigger.days.sort_unstable();
        trigger.days.dedup();
        trigger.slot = trigger.slot.clamp(1, 9);
        trigger.pattern = trigger.pattern.trim().to_string();
    }
    flow
}

pub fn save(app: &AppHandle, flow: Flow) -> Result<Flow, String> {
    let flow = normalize(flow);
    let saved = with_store(app, |store| {
        if let Some(existing) = store.flows.iter_mut().find(|item| item.id == flow.id) {
            let mut next = flow.clone();
            next.created = existing.created.max(1);
            next.last_run = existing.last_run;
            next.last_status = existing.last_status.clone();
            *existing = next.clone();
            return Ok(next);
        }
        if store.flows.len() >= MAX_FLOWS {
            return Err("You have too many flows. Delete some first.".to_string());
        }
        store.flows.push(flow.clone());
        Ok(flow)
    })?;
    persist(app);
    emit(app);
    Ok(saved)
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    let shots = with_store(app, |store| {
        store.flows.retain(|flow| flow.id != id);
        store.schedule.remove(id);
        let mut shots = Vec::new();
        for run in store.runs.iter_mut().filter(|run| run.flow_id == id) {
            shots.push(run.clone());
            for step in &mut run.steps {
                step.shot = false;
            }
        }
        shots
    });
    remove_shots(app, &shots);
    persist(app);
    emit(app);
    Ok(())
}

pub fn set_paused(app: &AppHandle, id: &str, paused: bool) -> Result<(), String> {
    with_store(app, |store| {
        let flow = store
            .flows
            .iter_mut()
            .find(|flow| flow.id == id)
            .ok_or("That flow no longer exists.")?;
        flow.paused = paused;
        Ok::<(), String>(())
    })?;
    persist(app);
    emit(app);
    Ok(())
}

/// A flow with fresh ids and no run state, ready to save as a new flow. A copy with enabled
/// triggers starts paused, so it can't run alongside its original or start by itself after an import.
fn as_new(mut flow: Flow) -> Flow {
    flow.id = String::new();
    flow.created = 0;
    flow.last_run = 0;
    flow.last_status = String::new();
    for step in &mut flow.steps {
        step.id = String::new();
    }
    if flow.triggers.iter().any(|trigger| trigger.enabled) {
        flow.paused = true;
    }
    flow
}

pub fn duplicate(app: &AppHandle, id: &str) -> Result<Flow, String> {
    let source = flow(app, id).ok_or("That flow no longer exists.")?;
    let mut copy = as_new(source);
    copy.name = clip(&format!("Copy of {}", copy.name), 120);
    save(app, copy)
}

const EXPORT_VERSION: u64 = 1;

fn export_value(flow: &Flow) -> Value {
    let mut flow = flow.clone();
    flow.last_run = 0;
    flow.last_status = String::new();
    json!({ "ceFlow": EXPORT_VERSION, "flow": flow })
}

pub fn export(app: &AppHandle, id: &str, path: &str) -> Result<(), String> {
    let flow = flow(app, id).ok_or("That flow no longer exists.")?;
    let text =
        serde_json::to_string_pretty(&export_value(&flow)).map_err(|error| error.to_string())?;
    files::write_text_file(std::path::Path::new(path), &text)
}

/// Reads an exported flow, or a bare flow object, as a new flow that is not saved yet.
fn parse_export(text: &str) -> Result<Flow, String> {
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "That file isn't a ce-browser flow.".to_string())?;
    let raw = match value.get("flow") {
        Some(inner) if value.get("ceFlow").is_some() => inner.clone(),
        _ => value,
    };
    if !raw.is_object() || raw.get("steps").is_none_or(|steps| !steps.is_array()) {
        return Err("That file isn't a ce-browser flow.".into());
    }
    let flow: Flow =
        serde_json::from_value(raw).map_err(|_| "That file isn't a ce-browser flow.".to_string())?;
    Ok(as_new(flow))
}

pub fn import(app: &AppHandle, path: &str) -> Result<Flow, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("Couldn't read {path}: {error}"))?;
    save(app, parse_export(&text)?)
}

/// What `test_target` found on the page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetTest {
    pub found: bool,
    /// The locator that matched: `testId`, `role`, `css`, `text` or `xpath`.
    pub by: String,
}

/// Lets the user click the element a step should use, in `tab`. Shows the tab first.
pub fn pick_target(app: &AppHandle, tab: u32) -> Result<Option<FlowTarget>, String> {
    tabs::activate(app, tab)?;
    let Some(value) = driver::pick_element(app, tab)? else {
        return Ok(None);
    };
    let Some(target) = value.get("target") else {
        return Ok(None);
    };
    serde_json::from_value(target.clone())
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Looks for a step's element in `tab` with its saved locators and outlines it when found.
pub fn test_target(app: &AppHandle, tab: u32, target: &FlowTarget) -> Result<TargetTest, String> {
    tabs::activate(app, tab)?;
    let value = serde_json::to_value(target).map_err(|error| error.to_string())?;
    match driver::locate(app, tab, &value)? {
        Some((element, by)) => {
            let _ = driver::flash(app, tab, element);
            Ok(TargetTest { found: true, by })
        }
        None => Ok(TargetTest {
            found: false,
            by: String::new(),
        }),
    }
}

/// A short list for MCP clients.
pub fn summaries(app: &AppHandle) -> Vec<Value> {
    flows(app)
        .into_iter()
        .map(|flow| {
            json!({
                "id": flow.id,
                "name": flow.name,
                "description": flow.description,
                "startUrl": flow.start_url,
                "steps": flow.steps.len(),
                "triggers": flow.triggers.iter().filter(|trigger| trigger.enabled).map(|trigger| trigger.kind.clone()).collect::<Vec<_>>(),
                "lastRun": flow.last_run,
                "lastStatus": flow.last_status,
            })
        })
        .collect()
}

/// Opens `ce://automations`, or moves an open one to `section`.
pub fn open_page(app: &AppHandle, window: &str, section: Option<&str>) -> Result<(), String> {
    let address = match section.map(str::trim).filter(|section| !section.is_empty()) {
        Some(section) => format!("{}automations?{section}", paths::INTERNAL_SCHEME),
        None => format!("{}automations", paths::INTERNAL_SCHEME),
    };
    let existing = tabs::snapshot(app).tabs.into_iter().find(|tab| {
        tab.window == window
            && tab
                .url
                .starts_with(&format!("{}automations", paths::INTERNAL_SCHEME))
    });
    match existing {
        Some(tab) => {
            if section.is_some() {
                tabs::navigate(app, tab.id, &address)?;
            }
            tabs::activate(app, tab.id)
        }
        None => tabs::create_tab_on(app, window, Some(address), false, None).map(|_| ()),
    }
}

// Runs.

#[derive(Default)]
struct Runtime {
    stops: HashSet<u32>,
    /// Tab each running flow drives.
    tabs: BTreeMap<u32, u32>,
    next: u32,
}

fn runtime() -> &'static (Mutex<Runtime>, Condvar) {
    static RUNTIME: OnceLock<(Mutex<Runtime>, Condvar)> = OnceLock::new();
    RUNTIME.get_or_init(|| (Mutex::new(Runtime::default()), Condvar::new()))
}

fn runtime_lock() -> MutexGuard<'static, Runtime> {
    runtime().0.lock().expect("runtime lock")
}

fn stopped(run: u32) -> bool {
    runtime_lock().stops.contains(&run)
}

pub fn running_flow(app: &AppHandle, flow_id: &str) -> bool {
    with_store(app, |store| {
        store
            .runs
            .iter()
            .any(|run| run.flow_id == flow_id && run.status == "running")
    })
}

/// Starts a run of a saved flow. `tab` runs it in that tab instead of the flow's own.
pub fn run(app: &AppHandle, flow_id: &str, tab: Option<u32>, trigger: &str) -> Result<u32, String> {
    run_with(app, flow_id, tab, trigger, BTreeMap::new())
}

/// Like `run`, covering only the steps `options` names.
pub fn run_part(
    app: &AppHandle,
    flow_id: &str,
    tab: Option<u32>,
    trigger: &str,
    options: RunOptions,
) -> Result<u32, String> {
    launch(app, flow_id, tab, trigger, BTreeMap::new(), options)
}

/// Like `run`, with extra `{{name}}` values from the trigger, such as a downloaded file's path.
pub fn run_with(
    app: &AppHandle,
    flow_id: &str,
    tab: Option<u32>,
    trigger: &str,
    vars: BTreeMap<String, String>,
) -> Result<u32, String> {
    launch(app, flow_id, tab, trigger, vars, RunOptions::default())
}

/// Runs a flow on the page already open in `tab`, without loading its start address.
pub fn run_here(
    app: &AppHandle,
    flow_id: &str,
    tab: u32,
    trigger: &str,
    vars: BTreeMap<String, String>,
) -> Result<u32, String> {
    let options = RunOptions {
        here: true,
        ..RunOptions::default()
    };
    launch(app, flow_id, Some(tab), trigger, vars, options)
}

fn launch(
    app: &AppHandle,
    flow_id: &str,
    tab: Option<u32>,
    trigger: &str,
    vars: BTreeMap<String, String>,
    mut options: RunOptions,
) -> Result<u32, String> {
    if !settings::get(app).automation.enabled {
        return Err("Automation is turned off in Settings.".into());
    }
    let flow = flow(app, flow_id).ok_or("That flow no longer exists.")?;
    if running_flow(app, flow_id) {
        return Err("This flow is already running.".into());
    }
    if flow.steps.is_empty() && flow.start_url.is_empty() {
        return Err("Add a step to this flow first.".into());
    }
    if options.from_step > 0 && options.from_step >= flow.steps.len() {
        return Err("That step is no longer in the flow.".into());
    }
    options.to_step = options
        .to_step
        .map(|to| to.max(options.from_step).min(flow.steps.len().saturating_sub(1)));
    if tab.is_none() {
        options.here = false;
    }
    let id = push_run(app, &flow, trigger, "running", "", &options);
    let handle = app.clone();
    let trigger = trigger.to_string();
    thread::spawn(move || execute(&handle, id, flow, tab, options, &trigger, vars));
    Ok(id)
}

fn next_run_id(app: &AppHandle) -> u32 {
    let mut runtime = runtime_lock();
    if runtime.next == 0 {
        runtime.next = with_store(app, |store| {
            store.runs.iter().map(|run| run.id).max().unwrap_or(0)
        });
    }
    runtime.next += 1;
    runtime.next
}

fn push_run(
    app: &AppHandle,
    flow: &Flow,
    trigger: &str,
    status: &str,
    output: &str,
    options: &RunOptions,
) -> u32 {
    let id = next_run_id(app);
    let now = now_ms();
    let removed = with_store(app, |store| {
        store.runs.push(FlowRun {
            id,
            flow_id: flow.id.clone(),
            flow_name: flow.name.clone(),
            trigger: trigger.to_string(),
            status: status.into(),
            started: now,
            finished: if status == "running" { 0 } else { now },
            output: clip(output, 8000),
            from_step: options.from_step,
            to_step: options.to_step,
            ..FlowRun::default()
        });
        let removed = trim_runs(&mut store.runs);
        if status != "running"
            && let Some(saved) = store.flows.iter_mut().find(|item| item.id == flow.id)
        {
            saved.last_run = now;
            saved.last_status = status.into();
        }
        removed
    });
    remove_shots(app, &removed);
    if status != "running" {
        persist(app);
    }
    emit(app);
    id
}

/// Drops the oldest finished runs past the per-flow and overall limits. Returns what it dropped.
fn trim_runs(runs: &mut Vec<FlowRun>) -> Vec<FlowRun> {
    let mut removed = Vec::new();
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut keep = vec![true; runs.len()];
    for (index, run) in runs.iter().enumerate().rev() {
        if run.status == "running" {
            continue;
        }
        let count = kept.entry(run.flow_id.clone()).or_default();
        *count += 1;
        if *count > MAX_FLOW_RUNS {
            keep[index] = false;
        }
    }
    let mut index = 0;
    runs.retain(|run| {
        let stay = keep[index];
        index += 1;
        if !stay {
            removed.push(run.clone());
        }
        stay
    });
    while runs.len() > MAX_RUNS {
        match runs.iter().position(|run| run.status != "running") {
            Some(index) => removed.push(runs.remove(index)),
            None => break,
        }
    }
    removed
}

fn shots_dir(app: &AppHandle) -> PathBuf {
    paths::root_local_dir(app)
        .join("automation-shots")
        .join(files::sanitize_file_name(&crate::profiles::active_id()))
}

fn shot_path(app: &AppHandle, run: u32, index: usize) -> PathBuf {
    shots_dir(app).join(format!("{run}-{index}.jpg"))
}

/// Saves what the run's tab shows, for a failed step. Returns whether it worked.
fn save_shot(app: &AppHandle, run: u32, index: usize, tab: u32) -> bool {
    let Ok(shot) = driver::screenshot(app, tab, false, 1280) else {
        return false;
    };
    let path = shot_path(app, run, index);
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    std::fs::write(&path, &shot.bytes).is_ok()
}

fn remove_shots(app: &AppHandle, runs: &[FlowRun]) {
    for run in runs {
        for step in run.steps.iter().filter(|step| step.shot) {
            let _ = std::fs::remove_file(shot_path(app, run.id, step.index));
        }
    }
}

/// The screenshot saved when step `index` of run `run` failed, as a `data:` URL.
pub fn run_shot(app: &AppHandle, run: u32, index: usize) -> Option<String> {
    let bytes = std::fs::read(shot_path(app, run, index)).ok()?;
    Some(super::jpeg_data_url(&bytes))
}

/// Adds a finished entry to the run history, for monitor checks that don't run steps.
pub fn log_run(app: &AppHandle, flow: &Flow, trigger: &str, status: &str, output: &str) {
    push_run(app, flow, trigger, status, output, &RunOptions::default());
}

pub fn stop_run(app: &AppHandle, run: u32) {
    let tab = {
        let mut runtime = runtime_lock();
        runtime.stops.insert(run);
        runtime.tabs.get(&run).copied()
    };
    if let Some(tab) = tab
        && let Some(agent_run) = agent::run_for_tab(tab)
    {
        agent::stop(app, agent_run);
    }
    runtime().1.notify_all();
}

pub fn clear_runs(app: &AppHandle) {
    let removed = with_store(app, |store| {
        let (running, finished) = std::mem::take(&mut store.runs)
            .into_iter()
            .partition(|run| run.status == "running");
        store.runs = running;
        finished
    });
    remove_shots(app, &removed);
    persist(app);
    emit(app);
}

/// Stops runs whose tab closed.
pub fn tab_closed(app: &AppHandle, tab: u32) {
    let runs: Vec<u32> = runtime_lock()
        .tabs
        .iter()
        .filter(|(_, used)| **used == tab)
        .map(|(run, _)| *run)
        .collect();
    for run in runs {
        stop_run(app, run);
    }
}

/// Waits until a run finishes. `None` when it is still running after `limit`.
pub fn wait_run(run: u32, limit: Duration) -> Option<FlowRun> {
    let start = Instant::now();
    let (mutex, condvar) = runtime();
    let mut runtime = mutex.lock().expect("runtime lock");
    loop {
        let finished = cache()
            .lock()
            .expect("flows lock")
            .as_ref()
            .and_then(|loaded| {
                loaded
                    .store
                    .runs
                    .iter()
                    .find(|item| item.id == run)
                    .cloned()
            })
            .filter(|item| item.status != "running");
        if finished.is_some() {
            return finished;
        }
        if start.elapsed() >= limit {
            return None;
        }
        runtime = condvar
            .wait_timeout(runtime, Duration::from_millis(500))
            .expect("runtime lock")
            .0;
    }
}

fn record(app: &AppHandle, run: u32, step: FlowRunStep) {
    with_store(app, |store| {
        if let Some(item) = store.runs.iter_mut().find(|item| item.id == run) {
            item.steps.push(step);
        }
    });
    emit(app);
}

struct Context<'a> {
    app: &'a AppHandle,
    run: u32,
    flow: &'a Flow,
    tab: u32,
    from: usize,
    to: Option<usize>,
    auto: AutomationSettings,
    vars: BTreeMap<String, String>,
    rows: Vec<Map<String, Value>>,
    output: Vec<String>,
    /// Steps whose element was found another way, with fresh locators to save.
    healed: Vec<(String, FlowTarget)>,
}

/// `{{name}}` from the flow's variables. Secrets are filled in later, right before typing.
pub fn fill_vars(text: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let key = after[..end].trim();
        match vars.get(key) {
            Some(value) if !key.starts_with("secret:") => out.push_str(value),
            _ => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn sleep_checked(run: u32, duration: Duration) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < duration {
        if stopped(run) {
            return Err("Stopped.".into());
        }
        thread::sleep(Duration::from_millis(200).min(duration.saturating_sub(start.elapsed())));
    }
    Ok(())
}

/// Picks or opens the tab a run drives. The flag is true for a hidden tab the run should close.
fn run_tab(
    app: &AppHandle,
    flow: &Flow,
    tab: Option<u32>,
    here: bool,
) -> Result<(u32, bool), String> {
    if let Some(tab) = tab.filter(|tab| tabs::tab_info(app, *tab).is_some()) {
        if !here && !flow.start_url.is_empty() {
            let url = tabs::resolve(app, &flow.start_url);
            let current = tabs::tab_info(app, tab)
                .map(|info| info.url)
                .unwrap_or_default();
            if current != url.as_str() {
                driver::navigate(app, tab, url.as_str())?;
            }
        }
        return Ok((tab, false));
    }
    let address = if flow.start_url.is_empty() {
        "about:blank".to_string()
    } else {
        flow.start_url.clone()
    };
    let url = tabs::resolve(app, &address);
    if flow.background {
        let opened = tabs::open(
            app,
            tabs::OpenTab {
                shelf: true,
                background: true,
                window: tabs::focused_window(app),
                ..tabs::OpenTab::new(url)
            },
        )?;
        driver::view(app, opened)?;
        driver::wait_load(app, opened, Duration::from_secs(30));
        return Ok((opened, true));
    }
    if flow.start_url.is_empty()
        && let Some(active) = tabs::active_id(app)
    {
        return Ok((active, false));
    }
    let opened = tabs::open(
        app,
        tabs::OpenTab {
            window: tabs::focused_window(app),
            ..tabs::OpenTab::new(url)
        },
    )?;
    driver::wait_load(app, opened, Duration::from_secs(30));
    Ok((opened, false))
}

fn execute(
    app: &AppHandle,
    run: u32,
    flow: Flow,
    tab: Option<u32>,
    options: RunOptions,
    trigger: &str,
    extra: BTreeMap<String, String>,
) {
    let (status, output, dataset) = match run_tab(app, &flow, tab, options.here) {
        Ok((tab, hidden)) => {
            runtime_lock().tabs.insert(run, tab);
            tabs::set_automated(app, tab, true);
            let mut context = Context {
                app,
                run,
                flow: &flow,
                tab,
                from: options.from_step,
                to: options.to_step,
                auto: settings::get(app).automation_with_blocks(),
                vars: flow.variables.clone(),
                rows: Vec::new(),
                output: Vec::new(),
                healed: Vec::new(),
            };
            context.vars.insert("date".into(), today());
            context.vars.extend(extra);
            let status = drive(&mut context);
            let dataset = if context.rows.is_empty() {
                String::new()
            } else {
                let source = tabs::tab_info(app, tab)
                    .map(|info| info.url)
                    .unwrap_or_default();
                match extract::save_rows(
                    app,
                    &format!("{} {}", flow.name, today()),
                    &source,
                    std::mem::take(&mut context.rows),
                ) {
                    Ok(info) => info.id,
                    Err(error) => {
                        context
                            .output
                            .push(format!("Couldn't save the rows: {error}"));
                        String::new()
                    }
                }
            };
            let healed = std::mem::take(&mut context.healed);
            let output = context.output.join("\n");
            tabs::set_automated(app, tab, false);
            if hidden {
                let _ = tabs::close_tab(app, tab);
            }
            apply_healed(app, &flow.id, healed);
            (status, output, dataset)
        }
        Err(error) => ("failed", error, String::new()),
    };
    let finished = now_ms();
    with_store(app, |store| {
        if let Some(item) = store.runs.iter_mut().find(|item| item.id == run) {
            item.status = status.into();
            item.finished = finished;
            item.output = clip(&output, 8000);
            item.dataset_id = dataset.clone();
            item.current = None;
            item.current_label.clear();
        }
        if let Some(saved) = store.flows.iter_mut().find(|item| item.id == flow.id) {
            saved.last_run = finished;
            saved.last_status = status.into();
        }
    });
    persist(app);
    {
        let mut runtime = runtime_lock();
        runtime.stops.remove(&run);
        runtime.tabs.remove(&run);
    }
    runtime().1.notify_all();
    emit(app);
    audit(
        app,
        "flow",
        0,
        status,
        &format!("{} ({trigger})", flow.name),
    );
    if flow.notify && status != "stopped" {
        let first = output
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        let title = match status {
            "done" => format!("{} finished", flow.name),
            _ => format!("{} failed", flow.name),
        };
        system::toast(&title, &clip(first, 200), "");
    }
}

fn apply_healed(app: &AppHandle, flow_id: &str, healed: Vec<(String, FlowTarget)>) {
    if healed.is_empty() {
        return;
    }
    with_store(app, |store| {
        if let Some(flow) = store.flows.iter_mut().find(|flow| flow.id == flow_id) {
            for (step_id, target) in healed {
                if let Some(step) = flow.steps.iter_mut().find(|step| step.id == step_id) {
                    step.target = Some(target);
                }
            }
        }
    });
    persist(app);
}

/// Marks the step a run is on, so the editor and history can follow it live.
fn set_current(app: &AppHandle, run: u32, current: Option<(usize, String)>) {
    with_store(app, |store| {
        if let Some(item) = store.runs.iter_mut().find(|item| item.id == run) {
            match current {
                Some((index, label)) => {
                    item.current = Some(index);
                    item.current_label = clip(&label, 200);
                }
                None => {
                    item.current = None;
                    item.current_label.clear();
                }
            }
        }
    });
    emit(app);
}

fn drive(context: &mut Context) -> &'static str {
    let steps = context.flow.steps.clone();
    let last = context.to.unwrap_or(usize::MAX);
    for (index, step) in steps.iter().enumerate() {
        if index < context.from || index > last {
            continue;
        }
        if stopped(context.run) {
            return "stopped";
        }
        if tabs::tab_info(context.app, context.tab).is_none() {
            context.output.push("The tab was closed.".into());
            return "stopped";
        }
        if step.disabled {
            record(
                context.app,
                context.run,
                FlowRunStep {
                    index,
                    kind: step.kind.clone(),
                    label: clip(&step_label(step, &context.vars), 200),
                    status: "skipped".into(),
                    detail: "Turned off in the editor.".into(),
                    shot: false,
                },
            );
            continue;
        }
        set_current(
            context.app,
            context.run,
            Some((index, step_label(step, &context.vars))),
        );
        let result = run_step(context, step);
        let (status, label, detail) = match &result {
            Ok(outcome) => (
                outcome.status,
                outcome.label.clone(),
                outcome.detail.clone(),
            ),
            Err(error) => ("failed", step_label(step, &context.vars), error.clone()),
        };
        let shot = result.is_err()
            && !stopped(context.run)
            && result.as_ref().err().is_none_or(|error| error != "Stopped.")
            && save_shot(context.app, context.run, index, context.tab);
        record(
            context.app,
            context.run,
            FlowRunStep {
                index,
                kind: step.kind.clone(),
                label: clip(&label, 200),
                status: status.into(),
                detail: clip(&detail, 600),
                shot,
            },
        );
        if let Err(error) = result {
            if stopped(context.run) || error == "Stopped." {
                return "stopped";
            }
            if !step.optional {
                context
                    .output
                    .push(format!("Step {} failed: {error}", index + 1));
                return "failed";
            }
        }
        if !matches!(step.kind.as_str(), "wait" | "notify" | "assert") {
            driver::wait_load(context.app, context.tab, Duration::from_secs(10));
            thread::sleep(Duration::from_millis(250));
        }
    }
    "done"
}

struct StepOutcome {
    status: &'static str,
    label: String,
    detail: String,
}

fn ok(label: String, detail: impl Into<String>) -> Result<StepOutcome, String> {
    Ok(StepOutcome {
        status: "ok",
        label,
        detail: detail.into(),
    })
}

/// What a step does, in words, for the run history and the editor.
pub fn step_label(step: &FlowStep, vars: &BTreeMap<String, String>) -> String {
    let target = step
        .target
        .as_ref()
        .map(FlowTarget::describe)
        .unwrap_or_else(|| "the focused element".into());
    match step.kind.as_str() {
        "navigate" => format!("Go to {}", clip(&fill_vars(&step.url, vars), 100)),
        "click" => format!("Click {target}"),
        "type" => format!(
            "Type \"{}\" into {target}",
            clip(&mask_secrets(&fill_vars(&step.text, vars)), 60)
        ),
        "press" => format!("Press {}", step.keys),
        "select" => format!("Choose {} in {target}", step.values.join(", ")),
        "scroll" => format!(
            "Scroll {}",
            if step.text.is_empty() {
                "down"
            } else {
                &step.text
            }
        ),
        "hover" => format!("Hover over {target}"),
        "wait" => format!("Wait {}s", step.seconds.max(0.1)),
        "waitFor" => format!("Wait for \"{}\"", clip(&step.text, 60)),
        "back" => "Go back".into(),
        "extract" => "Extract data".into(),
        "ai" => format!("AI: {}", clip(&step.prompt, 80)),
        "assert" => format!("Check the page shows \"{}\"", clip(&step.text, 60)),
        "notify" => format!("Notify: {}", clip(&step.text, 60)),
        "upload" => format!("Upload {}", step.values.join(", ")),
        other => format!("Unknown step \"{other}\""),
    }
}

fn run_step(context: &mut Context, step: &FlowStep) -> Result<StepOutcome, String> {
    let app = context.app;
    let tab = context.tab;
    let label = step_label(step, &context.vars);
    let page_free = matches!(step.kind.as_str(), "navigate" | "wait" | "notify" | "back");
    if !page_free {
        let url = tabs::tab_info(app, tab)
            .map(|info| info.url)
            .unwrap_or_default();
        if let Verdict::Block(reason) = policy::page(&context.auto, &url) {
            return Err(reason);
        }
    }
    match step.kind.as_str() {
        "navigate" => {
            let address = fill_vars(&step.url, &context.vars);
            let url = tabs::resolve(app, &address);
            if let Verdict::Block(reason) = policy::open(&context.auto, url.as_str()) {
                return Err(reason);
            }
            driver::navigate(app, tab, url.as_str())?;
            ok(label, "Loaded.")
        }
        "click" => {
            let (element, healed) = find(context, step)?;
            let button = if step.keys.is_empty() {
                "left"
            } else {
                step.keys.as_str()
            };
            driver::click(app, tab, element, button, false)?;
            outcome(healed, label, "Clicked.")
        }
        "type" => {
            let raw = fill_vars(&step.text, &context.vars);
            let (text, _) = fill_secrets(app, &raw)?;
            let element = match &step.target {
                Some(_) => Some(find(context, step)?),
                None => None,
            };
            driver::type_text(
                app,
                tab,
                element.map(|(element, _)| element),
                &text,
                true,
                false,
            )?;
            outcome(element.is_some_and(|(_, healed)| healed), label, "Typed.")
        }
        "press" => {
            if step.keys.trim().is_empty() {
                return Err("Choose a key to press.".into());
            }
            driver::press(app, tab, step.keys.trim())?;
            ok(label, "Pressed.")
        }
        "select" => {
            let (element, healed) = find(context, step)?;
            let values: Vec<String> = step
                .values
                .iter()
                .map(|value| fill_vars(value, &context.vars))
                .collect();
            let picked = driver::select(app, tab, element, &values)?;
            outcome(healed, label, format!("Selected {}.", picked.join(", ")))
        }
        "scroll" => {
            let screens = if step.seconds > 0.0 {
                step.seconds.min(50.0)
            } else {
                1.0
            };
            let (amount, horizontal) = match step.text.trim() {
                "up" => (-screens, false),
                "left" => (-screens, true),
                "right" => (screens, true),
                _ => (screens, false),
            };
            let element = match &step.target {
                Some(_) => Some(find(context, step)?.0),
                None => None,
            };
            let detail = driver::scroll(app, tab, element, amount, horizontal)?;
            ok(label, detail)
        }
        "hover" => {
            let (element, healed) = find(context, step)?;
            driver::hover(app, tab, element)?;
            outcome(healed, label, "Done.")
        }
        "wait" => {
            let seconds = if step.seconds > 0.0 {
                step.seconds.min(3600.0)
            } else {
                1.0
            };
            sleep_checked(context.run, Duration::from_secs_f64(seconds))?;
            ok(label, "Waited.")
        }
        "waitFor" => {
            let text = fill_vars(&step.text, &context.vars);
            let selector = step
                .target
                .as_ref()
                .map(|target| target.css.clone())
                .filter(|_| step.text.is_empty())
                .unwrap_or_default();
            let query = if step.keys == "gone" {
                WaitQuery {
                    gone: text,
                    ..WaitQuery::default()
                }
            } else {
                WaitQuery {
                    text,
                    selector,
                    ..WaitQuery::default()
                }
            };
            let limit = if step.seconds > 0.0 {
                step.seconds.min(600.0)
            } else {
                15.0
            };
            if driver::wait_for(app, tab, &query, Duration::from_secs_f64(limit))? {
                ok(label, "Done.")
            } else {
                Err("Timed out.".into())
            }
        }
        "back" => {
            driver::back(app, tab)?;
            ok(label, "Went back.")
        }
        "extract" => {
            let rows = match &step.spec {
                Some(spec) if !spec.item.trim().is_empty() => {
                    extract::collect(app, tab, spec, step.pages.max(1), &step.text)?
                }
                _ if !step.prompt.trim().is_empty() => {
                    extract::ai_rows(app, tab, &fill_vars(&step.prompt, &context.vars))?
                }
                _ => return Err("Pick what to extract first.".into()),
            };
            let count = rows.len();
            context.rows.extend(rows);
            context
                .vars
                .insert("rows".into(), context.rows.len().to_string());
            context.output.push(format!("Extracted {count} rows."));
            ok(format!("Extract {count} rows"), format!("{count} rows."))
        }
        "ai" => {
            let prompt = fill_vars(&step.prompt, &context.vars);
            if prompt.trim().is_empty() {
                return Err("Describe what the AI should do.".into());
            }
            let result = run_agent(context, &prompt, &step.autonomy)?;
            context.vars.insert("output".into(), result.clone());
            context.output.push(result.clone());
            ok(label, clip(&result, 600))
        }
        "assert" => {
            let text = fill_vars(&step.text, &context.vars);
            let limit = if step.seconds > 0.0 {
                step.seconds.min(120.0)
            } else {
                5.0
            };
            let query = WaitQuery {
                text: text.clone(),
                ..WaitQuery::default()
            };
            if driver::wait_for(app, tab, &query, Duration::from_secs_f64(limit))? {
                ok(label, "Found.")
            } else {
                Err(format!("The page doesn't show \"{}\".", clip(&text, 80)))
            }
        }
        "notify" => {
            let body = fill_vars(&step.text, &context.vars);
            system::toast(&context.flow.name, &clip(&body, 300), "");
            context.output.push(body.clone());
            ok(label, "Sent.")
        }
        "upload" => {
            let (element, healed) = find(context, step)?;
            let paths: Vec<String> = step
                .values
                .iter()
                .map(|value| fill_vars(value, &context.vars))
                .collect();
            driver::upload(app, tab, element, &paths)?;
            outcome(healed, label, "Files attached.")
        }
        _ => Ok(StepOutcome {
            status: "skipped",
            label,
            detail: "This step needs a newer version of ce-browser.".into(),
        }),
    }
}

fn outcome(healed: bool, label: String, detail: impl Into<String>) -> Result<StepOutcome, String> {
    let detail = detail.into();
    if healed {
        Ok(StepOutcome {
            status: "healed",
            label,
            detail: format!("{detail} Found the element another way and updated the step."),
        })
    } else {
        ok(label, detail)
    }
}

/// Finds a step's element: its saved locators first, waiting for the page, then the AI.
fn find(context: &mut Context, step: &FlowStep) -> Result<(u32, bool), String> {
    let target = step.target.clone().ok_or("This step has no element.")?;
    let value = serde_json::to_value(&target).map_err(|error| error.to_string())?;
    let start = Instant::now();
    loop {
        if stopped(context.run) {
            return Err("Stopped.".into());
        }
        if let Some((element, by)) = driver::locate(context.app, context.tab, &value)? {
            let healed = matches!(by.as_str(), "text" | "xpath");
            if healed {
                remember(context, step, &target, element);
            }
            return Ok((element, healed));
        }
        if start.elapsed() >= FIND_TIMEOUT {
            break;
        }
        thread::sleep(Duration::from_millis(400));
    }
    let element = heal_with_ai(context, &target)?;
    remember(context, step, &target, element);
    Ok((element, true))
}

fn remember(context: &mut Context, step: &FlowStep, old: &FlowTarget, element: u32) {
    let Ok(fresh) = driver::locators_of(context.app, context.tab, element) else {
        return;
    };
    let Ok(mut target) = serde_json::from_value::<FlowTarget>(fresh) else {
        return;
    };
    target.description = old.description.clone();
    context.healed.retain(|(id, _)| *id != step.id);
    context.healed.push((step.id.clone(), target));
}

fn heal_with_ai(context: &Context, target: &FlowTarget) -> Result<u32, String> {
    let missing = format!("Couldn't find {} on the page.", target.describe());
    let current = settings::get(context.app);
    if !ai::available(context.app, &current) {
        return Err(missing);
    }
    let snap = driver::snapshot(context.app, context.tab)?;
    let prompt = format!(
        "A saved browser automation step needs this element:\n{}\nIt was recorded as role \"{}\", name \"{}\", text \"{}\", tag <{}>.\n\nThe page now has these elements:\n{}\n\nReply with JSON {{\"ref\": <number>}} for the element that does the same job, or {{\"ref\": null}} if none does.",
        target.describe(),
        target.role,
        target.name,
        clip(&target.text, 80),
        target.tag,
        snap.render(),
    );
    let reply = ai::complete(
        context.app,
        &current,
        &context.auto.agent_model,
        "You match elements between two versions of a web page. Answer with JSON only.",
        &prompt,
        true,
        Duration::from_secs(60),
    )
    .map_err(|_| missing.clone())?;
    let parsed: Value =
        serde_json::from_str(extract::json_block(&reply)).map_err(|_| missing.clone())?;
    let element = parsed
        .get("ref")
        .and_then(Value::as_u64)
        .ok_or(missing.clone())? as u32;
    driver::element_info(context.app, context.tab, element).map_err(|_| missing)?;
    Ok(element)
}

/// Hands one step to the agent in the run's tab and waits for its answer.
fn run_agent(context: &Context, prompt: &str, autonomy: &str) -> Result<String, String> {
    let id = agent::start_with(
        context.app,
        context.tab,
        prompt,
        agent::StartOptions {
            autonomy: autonomy.to_string(),
            background: context.flow.background,
            ..agent::StartOptions::default()
        },
    )?;
    let start = Instant::now();
    loop {
        if stopped(context.run) {
            agent::stop(context.app, id);
            return Err("Stopped.".into());
        }
        let run = agent::runs()
            .into_iter()
            .find(|run| run.id == id)
            .ok_or("The agent run was removed.")?;
        match run.status.as_str() {
            "done" => return Ok(run.result),
            "failed" | "stopped" => {
                return Err(if run.result.is_empty() {
                    "The agent stopped.".into()
                } else {
                    run.result
                });
            }
            _ => {}
        }
        if start.elapsed() > AI_STEP_TIMEOUT {
            agent::stop(context.app, id);
            return Err("The agent took too long.".into());
        }
        thread::sleep(Duration::from_millis(500));
    }
}

// Playwright export.

const ARIA_ROLES: &[&str] = &[
    "button",
    "link",
    "checkbox",
    "radio",
    "tab",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "option",
    "switch",
    "combobox",
    "textbox",
    "searchbox",
    "slider",
    "spinbutton",
    "treeitem",
    "gridcell",
    "listbox",
    "heading",
    "img",
];

fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

fn playwright_locator(target: &FlowTarget) -> String {
    if let Some((attribute, value)) = target.test_id.split_once('=') {
        if attribute == "data-testid" {
            return format!("page.getByTestId({})", js_string(value));
        }
        return format!(
            "page.locator({})",
            js_string(&format!("[{attribute}={}]", js_string(value)))
        );
    }
    if target.css.starts_with('#') {
        return format!("page.locator({})", js_string(&target.css));
    }
    if ARIA_ROLES.contains(&target.role.as_str()) && !target.name.trim().is_empty() {
        return format!(
            "page.getByRole({}, {{ name: {}, exact: true }})",
            js_string(&target.role),
            js_string(target.name.trim())
        );
    }
    if !target.css.is_empty() {
        return format!("page.locator({})", js_string(&target.css));
    }
    if !target.text.trim().is_empty() {
        return format!(
            "page.getByText({}, {{ exact: true }})",
            js_string(target.text.trim())
        );
    }
    format!(
        "page.locator({})",
        js_string(&format!("xpath={}", target.xpath))
    )
}

/// `{{secret:name}}` becomes `process.env.NAME` so the export holds no secrets.
fn playwright_text(text: &str, vars: &BTreeMap<String, String>) -> String {
    let filled = fill_vars(text, vars);
    let mut parts: Vec<String> = Vec::new();
    let mut rest = filled.as_str();
    while let Some(start) = rest.find("{{secret:") {
        if start > 0 {
            parts.push(js_string(&rest[..start]));
        }
        let after = &rest[start + 9..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let env: String = after[..end]
            .trim()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        parts.push(format!("(process.env.{env} ?? \"\")"));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() || parts.is_empty() {
        parts.push(js_string(rest));
    }
    parts.join(" + ")
}

pub fn playwright(app: &AppHandle, id: &str) -> Result<String, String> {
    let flow = flow(app, id).ok_or("That flow no longer exists.")?;
    Ok(playwright_script(&flow))
}

fn playwright_script(flow: &Flow) -> String {
    let vars = &flow.variables;
    let mut lines = vec![
        "import { test, expect } from \"@playwright/test\";".to_string(),
        String::new(),
        format!("test({}, async ({{ page }}) => {{", js_string(&flow.name)),
    ];
    if !flow.start_url.is_empty() {
        lines.push(format!(
            "  await page.goto({});",
            js_string(&fill_vars(&flow.start_url, vars))
        ));
    }
    for step in flow.steps.iter().filter(|step| !step.disabled) {
        let locator = step
            .target
            .as_ref()
            .map(playwright_locator)
            .unwrap_or_else(|| "page.locator(\":focus\")".into());
        let line = match step.kind.as_str() {
            "navigate" => format!(
                "await page.goto({});",
                js_string(&fill_vars(&step.url, vars))
            ),
            "click" => format!("await {locator}.click();"),
            "type" => format!(
                "await {locator}.fill({});",
                playwright_text(&step.text, vars)
            ),
            "press" => format!("await page.keyboard.press({});", js_string(&step.keys)),
            "select" => format!(
                "await {locator}.selectOption([{}]);",
                step.values
                    .iter()
                    .map(|value| format!("{{ label: {} }}", js_string(&fill_vars(value, vars))))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "scroll" => {
                let screens = if step.seconds > 0.0 {
                    step.seconds
                } else {
                    1.0
                };
                let sign = if matches!(step.text.as_str(), "up" | "left") {
                    -1.0
                } else {
                    1.0
                };
                if matches!(step.text.as_str(), "left" | "right") {
                    format!(
                        "await page.mouse.wheel({}, 0);",
                        (sign * screens * 800.0).round()
                    )
                } else {
                    format!(
                        "await page.mouse.wheel(0, {});",
                        (sign * screens * 800.0).round()
                    )
                }
            }
            "hover" => format!("await {locator}.hover();"),
            "wait" => format!(
                "await page.waitForTimeout({});",
                ((if step.seconds > 0.0 {
                    step.seconds
                } else {
                    1.0
                }) * 1000.0)
                    .round()
            ),
            "waitFor" => format!(
                "await page.getByText({}).first().waitFor();",
                js_string(&step.text)
            ),
            "back" => "await page.goBack();".into(),
            "assert" => format!(
                "await expect(page.getByText({}).first()).toBeVisible();",
                js_string(&fill_vars(&step.text, vars))
            ),
            "upload" => format!(
                "await {locator}.setInputFiles([{}]);",
                step.values
                    .iter()
                    .map(|value| js_string(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "extract" | "ai" | "notify" => {
                format!("// {} (runs only in ce-browser)", step_label(step, vars))
            }
            other => format!("// Unknown step \"{other}\""),
        };
        lines.push(format!("  {line}"));
    }
    lines.push("});".into());
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_new_fields_survive() {
        let raw = r#"{"flows":[{"id":"a","name":"A","steps":[{"id":"s","kind":"teleport","future":{"x":1}}],"newField":true}]}"#;
        let store: Store = serde_json::from_str(raw).unwrap();
        let flow = &store.flows[0];
        assert_eq!(flow.steps[0].kind, "teleport");
        assert_eq!(flow.steps[0].extra.get("future"), Some(&json!({ "x": 1 })));
        assert_eq!(flow.extra.get("newField"), Some(&json!(true)));
        let written = serde_json::to_value(&store).unwrap();
        assert_eq!(written["flows"][0]["steps"][0]["future"]["x"], 1);
        let empty: Store = serde_json::from_str("{}").unwrap();
        assert!(empty.flows.is_empty());
    }

    #[test]
    fn older_files_get_the_new_defaults() {
        let raw = r#"{"flows":[{"id":"a","steps":[{"id":"s","kind":"click"}]}],"runs":[{"id":3,"flowId":"a","status":"running","steps":[{"index":0,"status":"ok"}]}]}"#;
        let store: Store = serde_json::from_str(raw).unwrap();
        assert!(!store.flows[0].paused);
        assert!(!store.flows[0].steps[0].disabled);
        let run = &store.runs[0];
        assert_eq!((run.current, run.from_step, run.to_step), (None, 0, None));
        assert!(!run.steps[0].shot);
        let written = serde_json::to_value(&store).unwrap();
        let again: Store = serde_json::from_value(written.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), written);
    }

    #[test]
    fn runs_are_capped_per_flow_and_overall() {
        let run = |id: u32, flow: &str, status: &str| FlowRun {
            id,
            flow_id: flow.into(),
            status: status.into(),
            ..FlowRun::default()
        };
        let mut runs: Vec<FlowRun> = (1..=30).map(|id| run(id, "a", "done")).collect();
        runs.push(run(31, "b", "done"));
        runs.insert(0, run(100, "a", "running"));
        let removed = trim_runs(&mut runs);
        assert_eq!(removed.len(), 5);
        assert!(removed.iter().all(|run| run.flow_id == "a" && run.id <= 5));
        assert!(runs.iter().any(|run| run.id == 100));
        assert_eq!(runs.iter().filter(|run| run.flow_id == "a").count(), MAX_FLOW_RUNS + 1);

        let mut many: Vec<FlowRun> = (1..=400)
            .map(|id| run(id, &format!("f{id}"), "failed"))
            .collect();
        trim_runs(&mut many);
        assert_eq!(many.len(), MAX_RUNS);
        assert_eq!(many[0].id, 101);
    }

    #[test]
    fn exports_import_as_new_paused_flows() {
        let flow = Flow {
            id: "keep-me-out".into(),
            name: "Prices".into(),
            steps: vec![FlowStep {
                id: "s1".into(),
                kind: "click".into(),
                ..FlowStep::default()
            }],
            triggers: vec![FlowTrigger::default()],
            last_run: 5,
            last_status: "done".into(),
            ..Flow::default()
        };
        let text = export_value(&flow).to_string();
        let imported = parse_export(&text).unwrap();
        assert_eq!(imported.name, "Prices");
        assert!(imported.id.is_empty());
        assert!(imported.steps[0].id.is_empty());
        assert_eq!((imported.last_run, imported.paused), (0, true));

        let bare = serde_json::to_string(&Flow {
            triggers: Vec::new(),
            ..flow.clone()
        })
        .unwrap();
        assert!(!parse_export(&bare).unwrap().paused);
        assert!(parse_export("{\"name\":\"no steps\"}").is_err());
        assert!(parse_export("not json").is_err());
    }

    #[test]
    fn variables_fill_but_secrets_wait() {
        let mut vars = BTreeMap::new();
        vars.insert("city".to_string(), "Oslo".to_string());
        assert_eq!(
            fill_vars("Weather in {{city}} {{secret:pw}} {{missing}}", &vars),
            "Weather in Oslo {{secret:pw}} {{missing}}"
        );
    }

    #[test]
    fn exports_playwright() {
        let flow = Flow {
            name: "Search".into(),
            start_url: "https://example.com".into(),
            steps: vec![
                FlowStep {
                    kind: "type".into(),
                    target: Some(FlowTarget {
                        css: "#q".into(),
                        ..FlowTarget::default()
                    }),
                    text: "hi {{secret:site-password}}".into(),
                    ..FlowStep::default()
                },
                FlowStep {
                    kind: "click".into(),
                    target: Some(FlowTarget {
                        role: "button".into(),
                        name: "Go".into(),
                        css: "form > button".into(),
                        ..FlowTarget::default()
                    }),
                    ..FlowStep::default()
                },
            ],
            ..Flow::default()
        };
        let script = playwright_script(&flow);
        assert!(script.contains("await page.goto(\"https://example.com\");"));
        assert!(
            script.contains(
                "page.locator(\"#q\").fill(\"hi \" + (process.env.SITE_PASSWORD ?? \"\"))"
            )
        );
        assert!(
            script.contains("page.getByRole(\"button\", { name: \"Go\", exact: true }).click()")
        );
    }

    #[test]
    fn times_are_cleaned() {
        assert_eq!(clean_time("7:5"), "07:05");
        assert_eq!(clean_time("25:00"), "09:00");
        assert_eq!(clean_time(""), "09:00");
    }
}
