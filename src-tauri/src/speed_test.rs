//! A short internet speed test against Cloudflare's public endpoints.
//!
//! Download and upload each run for about eight seconds on four connections.
//! A warmup request is excluded from the rate. The last finished result stays
//! in memory so the popup can show it again.
//!
//! A socket read can outlive the window, and DNS cannot be interrupted, so the
//! test never waits on a worker. Cancel bumps an epoch the workers and the
//! parent both watch.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const DOWN: &str = "https://speed.cloudflare.com/__down";
const UP: &str = "https://speed.cloudflare.com/__up";
const USER_AGENT: &str = "ce-browser";
const WINDOW: Duration = Duration::from_secs(8);
const CONNECTIONS: usize = 4;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
const READ_CHUNK: usize = 64 * 1024;
const DOWNLOAD_REQUEST: u64 = 25_000_000;
const UPLOAD_BYTES: usize = 8 * 1024 * 1024;
const WARMUP_DOWN: u64 = 100_000;
const WARMUP_UP: usize = 64 * 1024;
const CALL_LIMIT: Duration = Duration::from_secs(8);
const STOPPED: &str = "Stopped.";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeedTestPhase {
    Latency,
    Download,
    Upload,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestProgress {
    pub phase: SpeedTestPhase,
    pub bytes: u64,
    /// Bytes per second for the phase in progress.
    pub bps: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestResult {
    /// Download bytes per second, warmup excluded.
    pub down_bps: f64,
    /// Upload bytes per second, warmup excluded.
    pub up_bps: f64,
    /// Median round trip to the speed test server, milliseconds.
    pub latency_ms: Option<u32>,
    /// When the test finished, unix milliseconds.
    pub at: u64,
}

struct State {
    running: bool,
    last: Option<SpeedTestResult>,
}

static STATE: Mutex<State> = Mutex::new(State {
    running: false,
    last: None,
});

/// Bumped by cancel. A test keeps the value it saw at the start.
static EPOCH: AtomicU64 = AtomicU64::new(1);

struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        if let Ok(mut state) = STATE.lock() {
            state.running = false;
        }
    }
}

pub fn run(app: &AppHandle) -> Result<SpeedTestResult, String> {
    {
        let mut state = STATE.lock().expect("speed test");
        if state.running {
            return Err("A speed test is already running.".into());
        }
        state.running = true;
    }
    let _running = Running;
    let epoch = EPOCH.load(Ordering::SeqCst);
    let outcome = measure(app, epoch);
    if let Ok(result) = &outcome {
        STATE.lock().expect("speed test").last = Some(result.clone());
    }
    outcome
}

pub fn cancel() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
}

pub fn last() -> Option<SpeedTestResult> {
    STATE.lock().expect("speed test").last.clone()
}

fn measure(app: &AppHandle, epoch: u64) -> Result<SpeedTestResult, String> {
    let agent = http_agent();
    emit(app, SpeedTestPhase::Latency, 0, 0.0);
    let latency_ms = match probe_latency(&agent, epoch) {
        Ok(latency) => latency,
        Err(()) if stopped(epoch) => return Err(STOPPED.into()),
        Err(()) => return Err("Could not reach the speed test server.".into()),
    };
    if stopped(epoch) {
        return Err(STOPPED.into());
    }

    if !warmup_down(&agent, epoch) {
        return stop_or(epoch, "Could not reach the speed test server.");
    }
    let down_bps = timed_download(app, epoch)?;

    let payload = Arc::new(payload(UPLOAD_BYTES));
    if !warmup_up(&agent, &payload, epoch) {
        return stop_or(epoch, "Could not reach the speed test server.");
    }
    let up_bps = timed_upload(app, &payload, epoch)?;

    Ok(SpeedTestResult {
        down_bps,
        up_bps,
        latency_ms,
        at: unix_ms(),
    })
}

struct Job {
    agent: ureq::Agent,
    counter: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    deadline: Instant,
    epoch: u64,
}

fn timed_download(app: &AppHandle, epoch: u64) -> Result<f64, String> {
    let counter = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let deadline = started + WINDOW;
    let mut handles = Vec::with_capacity(CONNECTIONS);
    for _ in 0..CONNECTIONS {
        let job = job(&counter, &stop, deadline, epoch);
        handles.push(thread::spawn(move || download_worker(job)));
    }
    finish(
        app,
        epoch,
        SpeedTestPhase::Download,
        "The download produced no data.",
        &counter,
        &stop,
        &handles,
        started,
        deadline,
    )
}

fn timed_upload(app: &AppHandle, payload: &Arc<Vec<u8>>, epoch: u64) -> Result<f64, String> {
    let counter = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let deadline = started + WINDOW;
    let mut handles = Vec::with_capacity(CONNECTIONS);
    for _ in 0..CONNECTIONS {
        let job = job(&counter, &stop, deadline, epoch);
        let payload = Arc::clone(payload);
        handles.push(thread::spawn(move || upload_worker(job, payload)));
    }
    finish(
        app,
        epoch,
        SpeedTestPhase::Upload,
        "The upload produced no data.",
        &counter,
        &stop,
        &handles,
        started,
        deadline,
    )
}

fn job(counter: &Arc<AtomicU64>, stop: &Arc<AtomicBool>, deadline: Instant, epoch: u64) -> Job {
    Job {
        agent: http_agent(),
        counter: Arc::clone(counter),
        stop: Arc::clone(stop),
        deadline,
        epoch,
    }
}

/// Waits out the window, then detaches any worker still blocked on a socket.
fn finish(
    app: &AppHandle,
    epoch: u64,
    phase: SpeedTestPhase,
    empty: &str,
    counter: &AtomicU64,
    stop: &AtomicBool,
    handles: &[thread::JoinHandle<()>],
    started: Instant,
    deadline: Instant,
) -> Result<f64, String> {
    emit(app, phase, 0, 0.0);
    while Instant::now() < deadline && !stopped(epoch) && handles.iter().any(|handle| !handle.is_finished()) {
        thread::sleep(PROGRESS_EVERY);
        let bytes = counter.load(Ordering::Relaxed);
        emit(app, phase, bytes, rate(bytes, started));
    }
    stop.store(true, Ordering::Relaxed);
    let bytes = counter.load(Ordering::Relaxed);
    emit(app, phase, bytes, rate(bytes, started));
    if stopped(epoch) {
        return Err(STOPPED.into());
    }
    if bytes == 0 {
        return Err(empty.into());
    }
    Ok(bytes as f64 / started.elapsed().as_secs_f64().max(0.05))
}

fn download_worker(job: Job) {
    let mut buf = vec![0u8; READ_CHUNK];
    while job_running(&job) {
        let response = match job
            .agent
            .get(&format!("{DOWN}?bytes={DOWNLOAD_REQUEST}"))
            .set("User-Agent", USER_AGENT)
            .call()
        {
            Ok(response) => response,
            Err(_) => {
                if !pause(&job) {
                    return;
                }
                continue;
            }
        };
        let mut reader = response.into_reader().take(DOWNLOAD_REQUEST);
        loop {
            if !job_running(&job) {
                return;
            }
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => {
                    job.counter.fetch_add(read as u64, Ordering::Relaxed);
                }
                Err(_) => return,
            }
        }
    }
}

fn upload_worker(job: Job, payload: Arc<Vec<u8>>) {
    while job_running(&job) {
        if send_up(&job, &payload).is_err() && !pause(&job) {
            return;
        }
    }
}

fn send_up(job: &Job, payload: &[u8]) -> Result<(), ()> {
    let reader = SliceReader {
        data: payload,
        pos: 0,
        job,
    };
    job.agent
        .post(UP)
        .set("User-Agent", USER_AGENT)
        .set("Content-Type", "application/octet-stream")
        .set("Content-Length", &payload.len().to_string())
        .send(reader)
        .map(|_| ())
        .map_err(|_| ())
}

struct SliceReader<'a> {
    data: &'a [u8],
    pos: usize,
    job: &'a Job,
}

impl Read for SliceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !job_running(self.job) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let size = (self.data.len() - self.pos).min(buf.len());
        buf[..size].copy_from_slice(&self.data[self.pos..self.pos + size]);
        self.pos += size;
        self.job.counter.fetch_add(size as u64, Ordering::Relaxed);
        Ok(size)
    }
}

/// Median of three small downloads. The body is discarded.
fn probe_latency(agent: &ureq::Agent, epoch: u64) -> Result<Option<u32>, ()> {
    let mut samples = [0u32; 3];
    for sample in &mut samples {
        if stopped(epoch) {
            return Err(());
        }
        let agent = agent.clone();
        let started = Instant::now();
        let ok = bounded(CALL_LIMIT, move || discard_down(&agent, 1000).is_ok());
        if ok != Some(true) {
            return Err(());
        }
        *sample = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    }
    samples.sort_unstable();
    Ok(Some(samples[1]))
}

fn warmup_down(agent: &ureq::Agent, epoch: u64) -> bool {
    if stopped(epoch) {
        return false;
    }
    let agent = agent.clone();
    bounded(CALL_LIMIT, move || discard_down(&agent, WARMUP_DOWN).is_ok()) == Some(true)
}

fn warmup_up(agent: &ureq::Agent, payload: &Arc<Vec<u8>>, epoch: u64) -> bool {
    if stopped(epoch) {
        return false;
    }
    let agent = agent.clone();
    let payload = Arc::clone(payload);
    bounded(CALL_LIMIT, move || {
        let job = Job {
            agent,
            counter: Arc::new(AtomicU64::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + CALL_LIMIT,
            epoch,
        };
        send_up(&job, &payload[..WARMUP_UP]).is_ok()
    }) == Some(true)
}

fn discard_down(agent: &ureq::Agent, bytes: u64) -> Result<(), ()> {
    let response = agent
        .get(&format!("{DOWN}?bytes={bytes}"))
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|_| ())?;
    let mut reader = response.into_reader().take(bytes.max(1));
    let mut buf = [0u8; 1024];
    loop {
        if reader.read(&mut buf).map_err(|_| ())? == 0 {
            return Ok(());
        }
    }
}

/// Runs `work` on another thread and gives up when `limit` elapses.
/// A stuck DNS lookup is left behind instead of holding the test.
fn bounded<T: Send + 'static>(limit: Duration, work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    let spawned = thread::Builder::new().name("speed-test-call".into()).spawn(move || {
        let _ = tx.send(work());
    });
    if spawned.is_err() {
        return None;
    }
    rx.recv_timeout(limit).ok()
}

fn payload(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let mut state = 0xC0FFEE_u64;
    for byte in &mut out {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        *byte = (state >> 33) as u8;
    }
    out
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(3))
        .timeout_write(Duration::from_secs(3))
        .timeout(Duration::from_secs(12))
        .redirects(0)
        .build()
}

fn emit(app: &AppHandle, phase: SpeedTestPhase, bytes: u64, bps: f64) {
    let _ = app.emit("speed-test-progress", SpeedTestProgress { phase, bytes, bps });
}

fn rate(bytes: u64, started: Instant) -> f64 {
    let secs = started.elapsed().as_secs_f64();
    if secs < 0.05 {
        0.0
    } else {
        bytes as f64 / secs
    }
}

fn job_running(job: &Job) -> bool {
    !job.stop.load(Ordering::Relaxed) && Instant::now() < job.deadline && !stopped(job.epoch)
}

fn pause(job: &Job) -> bool {
    if !job_running(job) {
        return false;
    }
    thread::sleep(Duration::from_millis(200));
    job_running(job)
}

fn stopped(epoch: u64) -> bool {
    EPOCH.load(Ordering::SeqCst) != epoch
}

fn stop_or(epoch: u64, message: &str) -> Result<SpeedTestResult, String> {
    if stopped(epoch) {
        Err(STOPPED.into())
    } else {
        Err(message.into())
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}
