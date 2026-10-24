//! The retained daemon's idle-time check (ADR 0394).
//!
//! The daemon answers an edit from its retained, incremental build. A bug in
//! incremental reuse could serve a stale answer that no test caught, and in an
//! IDE nobody would compare it with a fresh run. So, once the daemon has been
//! idle for a short debounce, a low-priority thread opens a *fresh* state (a
//! new Type Facts program, new native session and empty analysis caches) from
//! the same request, answers the same check, and compares the two:
//!
//! - the emitted findings JSON, the status and the notice, byte for byte;
//! - the reactive IR `Program`, table by table (`shadow::first_difference`).
//!
//! The comparison is made only when both builds saw the same inputs: the
//! tracked sources, the configuration and every package input the answer
//! depends on are compared by content hash first, and a difference there
//! (a file saved between the two) makes the check inconclusive, never a
//! mismatch. A fresh build that fails or is cancelled is likewise
//! inconclusive. Only equal inputs with a different answer are a mismatch.
//!
//! A request cancels a running check (a flag read at each phase of the fresh
//! answer, and the check is abandoned, not paused: it restarts from scratch
//! after the next debounce). The client protocol is one connection per
//! request with no push channel, so a mismatch is delivered by the *next*
//! request: the retained state is dropped (every memo, index and per-file
//! cache with it), the request opens a state from scratch, and its response
//! header carries `recovered`.
//!
//! Environment:
//! - `SOLID_CHECKER_IDLE_SHADOW`: unset, `1`, `on`: the check runs (default);
//!   `0`, `off`, `false`: it does not; `panic`: a mismatch ends the daemon
//!   with a failure status (tests and CI). It is also off whenever the inline
//!   IR shadow (`SOLID_CHECKER_IR_SHADOW`) is on: that already compares every
//!   build.
//! - `SOLID_CHECKER_IDLE_SHADOW_DEBOUNCE_MS`: idle time before a check (300).
//! - `SOLID_CHECKER_IDLE_SHADOW_LOG`: the log path (default: beside the
//!   daemon's socket, `<socket>.shadow.log`).
//! - `SOLID_CHECKER_IDLE_SHADOW_TRACE`: also log each check's outcome.
//! - debug builds only (test hooks): `SOLID_CHECKER_IDLE_SHADOW_FAULT` adds a
//!   spurious finding to the first materialized answer, and
//!   `SOLID_CHECKER_IDLE_SHADOW_DELAY_MS` delays each check's fresh build.

use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use solid_reactive_ir::{Program, shadow};

use super::{CheckRequest, Request, State};
use crate::{daemon_cache::InputFile, idle_memory, unsafe_boundary};

const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mode {
    Off,
    /// A mismatch is logged and the retained state dropped.
    Serve,
    /// A mismatch also ends the daemon.
    Panic,
}

impl Mode {
    pub(super) fn from_setting(setting: Option<&str>, inline: shadow::ShadowMode) -> Self {
        if inline != shadow::ShadowMode::Off {
            return Self::Off;
        }
        match setting.map(str::trim) {
            Some("0" | "off" | "false") => Self::Off,
            Some("panic") => Self::Panic,
            _ => Self::Serve,
        }
    }
}

/// What the retained build saw, which a fresh build must have seen too for
/// their answers to be comparable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Witness {
    sources: BTreeMap<String, [u8; 32]>,
    config: [u8; 32],
    input_files: Vec<InputFile>,
    inference_inputs: Vec<InputFile>,
}

impl Witness {
    /// `None` when the answer was not cached, which is when its package
    /// inputs were not stable and so cannot be compared.
    pub(super) fn of(state: &State) -> Option<Self> {
        let last = state.last.as_ref()?;
        Some(Self {
            sources: state
                .tracking
                .fingerprints
                .iter()
                .map(|(path, fingerprint)| (path.clone(), fingerprint.hash))
                .collect(),
            config: state.tracking.tsconfig.hash,
            input_files: last.input_files.clone(),
            inference_inputs: last.inference_inputs.clone(),
        })
    }
}

/// One answer the retained build materialized.
pub(super) struct Served {
    pub(super) check: CheckRequest,
    pub(super) status: Arc<str>,
    pub(super) body: Arc<[u8]>,
    pub(super) notice: Arc<str>,
    pub(super) program: Option<Arc<Program>>,
    pub(super) witness: Witness,
}

struct Inner {
    /// Counts materialized answers.
    epoch: u64,
    busy: bool,
    last_activity: Instant,
    served: Option<Arc<Served>>,
    /// The epoch of the last check that ended with a verdict.
    decided: u64,
    /// A mismatch was found: the next request drops the retained state.
    stale: bool,
    /// The running check's cancel flag.
    running: Option<Arc<AtomicBool>>,
    /// The running check was abandoned for memory, not for a request.
    relieved: bool,
}

pub(super) struct IdleShadow {
    mode: Mode,
    debounce: Duration,
    trace: bool,
    log: PathBuf,
    socket: PathBuf,
    inner: Mutex<Inner>,
    wake: Condvar,
}

enum Outcome {
    Cancelled,
    Inconclusive(String),
    Verified,
    Mismatch(String),
}

impl IdleShadow {
    pub(super) fn from_env(socket: &Path) -> Arc<Self> {
        let mode = Mode::from_setting(
            std::env::var("SOLID_CHECKER_IDLE_SHADOW").ok().as_deref(),
            shadow::ShadowMode::from_env(),
        );
        let debounce = std::env::var("SOLID_CHECKER_IDLE_SHADOW_DEBOUNCE_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map_or(DEFAULT_DEBOUNCE, Duration::from_millis);
        let log = std::env::var_os("SOLID_CHECKER_IDLE_SHADOW_LOG").map_or_else(
            || {
                let mut path = socket.as_os_str().to_owned();
                path.push(".shadow.log");
                PathBuf::from(path)
            },
            PathBuf::from,
        );
        Arc::new(Self {
            mode,
            debounce,
            trace: std::env::var_os("SOLID_CHECKER_IDLE_SHADOW_TRACE").is_some(),
            log,
            socket: socket.to_path_buf(),
            inner: Mutex::new(Inner {
                epoch: 0,
                busy: false,
                last_activity: Instant::now(),
                served: None,
                decided: 0,
                stale: false,
                running: None,
                relieved: false,
            }),
            wake: Condvar::new(),
        })
    }

    pub(super) fn enabled(&self) -> bool {
        self.mode != Mode::Off
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A request begins: any running check is cancelled. Returns whether a
    /// mismatch was found since the last request, in which case the caller
    /// must drop the retained state before answering.
    pub(super) fn begin_request(&self) -> bool {
        let mut inner = self.lock();
        inner.busy = true;
        if let Some(cancel) = &inner.running {
            cancel.store(true, Ordering::Relaxed);
        }
        let stale = std::mem::take(&mut inner.stale);
        if stale {
            inner.served = None;
        }
        stale
    }

    pub(super) fn end_request(&self) {
        let mut inner = self.lock();
        inner.busy = false;
        inner.last_activity = Instant::now();
        drop(inner);
        self.wake.notify_all();
    }

    /// The retained build materialized an answer for the check to verify.
    pub(super) fn record(&self, served: Served) {
        if !self.enabled() {
            return;
        }
        let mut inner = self.lock();
        inner.epoch += 1;
        inner.served = Some(Arc::new(served));
        drop(inner);
        self.wake.notify_all();
    }

    /// The retained state was dropped; nothing served is left to verify.
    pub(super) fn forget(&self) {
        self.lock().served = None;
    }

    /// Whether a check is running now.
    pub(super) fn running(&self) -> bool {
        self.lock().running.is_some()
    }

    /// The daemon is over its memory limit: abandon a running check, whose
    /// second build is the transient part. Returns whether one was running.
    pub(super) fn relieve_memory(&self) -> bool {
        let mut inner = self.lock();
        match &inner.running {
            Some(cancel) => {
                cancel.store(true, Ordering::Relaxed);
                inner.relieved = true;
                true
            }
            None => false,
        }
    }

    fn append_log(&self, message: &str) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
        {
            let _ = writeln!(file, "{message}");
        }
    }

    fn trace(&self, message: &str) {
        if self.trace {
            self.append_log(message);
        }
    }

    /// The check thread: waits for the daemon to be idle, verifies the latest
    /// served answer, and delivers a mismatch to the next request.
    pub(super) fn spawn(
        self: &Arc<Self>,
        request: Request,
        room: impl Fn() -> Option<String> + Send + 'static,
    ) {
        if !self.enabled() {
            return;
        }
        let this = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("idle-shadow".into())
            .spawn(move || {
                unsafe_boundary::lower_thread_priority();
                this.run(&request, &room);
            });
        if let Err(error) = spawned {
            self.append_log(&format!(
                "solid-checker: idle shadow could not start: {error}"
            ));
        }
    }

    fn run(&self, request: &Request, room: &dyn Fn() -> Option<String>) {
        loop {
            let (served, epoch, cancel) = {
                let mut inner = self.lock();
                loop {
                    let due = !inner.busy
                        && inner.epoch != inner.decided
                        && inner.served.is_some()
                        && !inner.stale;
                    if due {
                        let waited = inner.last_activity.elapsed();
                        if waited >= self.debounce {
                            break;
                        }
                        let remaining = self.debounce - waited;
                        inner = self
                            .wake
                            .wait_timeout(inner, remaining)
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .0;
                    } else {
                        inner = self
                            .wake
                            .wait(inner)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                }
                let cancel = Arc::new(AtomicBool::new(false));
                inner.running = Some(Arc::clone(&cancel));
                (
                    inner.served.clone().expect("checked above"),
                    inner.epoch,
                    cancel,
                )
            };
            // A second build roughly doubles the daemon's resident memory:
            // it runs only while the machine has room for it (`room` names
            // the reason it does not), whatever the daemon's own limit.
            let skipped = room();
            let outcome = if let Some(reason) = skipped {
                Outcome::Inconclusive(format!("skipped: {reason}"))
            } else {
                verify(request, &served, &cancel)
            };
            idle_memory::reclaim_idle_pages();
            let mut inner = self.lock();
            inner.running = None;
            match outcome {
                Outcome::Cancelled if std::mem::take(&mut inner.relieved) => {
                    // Retrying would only meet the same limit.
                    inner.decided = epoch;
                    self.trace(&format!(
                        "idle shadow epoch {epoch}: inconclusive: abandoned for memory"
                    ));
                }
                Outcome::Cancelled => self.trace(&format!("idle shadow epoch {epoch}: cancelled")),
                Outcome::Inconclusive(reason) => {
                    inner.decided = epoch;
                    self.trace(&format!(
                        "idle shadow epoch {epoch}: inconclusive: {reason}"
                    ));
                }
                Outcome::Verified => {
                    inner.decided = epoch;
                    self.trace(&format!("idle shadow epoch {epoch}: verified"));
                }
                Outcome::Mismatch(report) => {
                    inner.decided = epoch;
                    // Even when a later answer was materialized meanwhile: it
                    // descends from the state that was just proven wrong.
                    inner.stale = true;
                    inner.served = None;
                    let message = format!(
                        "solid-checker: the retained answer differs from a fresh build; retained \
                         caches are dropped and the next answer is fresh (idle shadow epoch \
                         {epoch})\n{report}"
                    );
                    eprintln!("{message}");
                    self.append_log(&message);
                    if self.mode == Mode::Panic {
                        let _ = std::fs::remove_file(&self.socket);
                        std::process::exit(70);
                    }
                }
            }
        }
    }
}

/// Builds the same check from nothing and compares it with what was served.
fn verify(request: &Request, served: &Served, cancel: &AtomicBool) -> Outcome {
    let cancelled = || cancel.load(Ordering::Relaxed);
    if cancelled() {
        return Outcome::Cancelled;
    }
    #[cfg(debug_assertions)]
    if let Some(delay) = std::env::var("SOLID_CHECKER_IDLE_SHADOW_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        // The test delay hook: a fresh build that takes long enough for a
        // request to arrive while it runs.
        let until = Instant::now() + Duration::from_millis(delay);
        while Instant::now() < until && !cancelled() {
            std::thread::sleep(Duration::from_millis(10));
        }
        if cancelled() {
            return Outcome::Cancelled;
        }
    }
    let mut fresh = match State::open(request) {
        Ok(fresh) => fresh,
        Err(error) => {
            return Outcome::Inconclusive(format!("the fresh build did not open: {error}"));
        }
    };
    let answer = match super::answer_with(&mut fresh, request, &served.check, false, Some(cancel)) {
        Ok(answer) => answer,
        Err(_) if cancelled() => return Outcome::Cancelled,
        Err(error) => {
            return Outcome::Inconclusive(format!("the fresh build did not answer: {error}"));
        }
    };
    if cancelled() {
        return Outcome::Cancelled;
    }
    let Some(witness) = Witness::of(&fresh) else {
        return Outcome::Inconclusive("the fresh build's package inputs were not stable".into());
    };
    if witness != served.witness {
        return Outcome::Inconclusive("the inputs changed since the retained answer".into());
    }
    if *answer.status != *served.status {
        return Outcome::Mismatch(format!(
            "status: retained `{}`, fresh `{}`",
            served.status, answer.status
        ));
    }
    if answer.body[..] != served.body[..] {
        return Outcome::Mismatch(body_difference(&served.body, &answer.body));
    }
    if *answer.notice != *served.notice {
        return Outcome::Mismatch(format!(
            "notice: retained `{}`, fresh `{}`",
            served.notice, answer.notice
        ));
    }
    if let (Some(retained), Some(fresh)) = (&served.program, &answer.program)
        && let Some(difference) = shadow::first_difference(retained, fresh)
    {
        return Outcome::Mismatch(difference.to_string());
    }
    Outcome::Verified
}

/// Names the first finding that differs between two findings documents, or
/// the first differing line when they are not both findings documents.
fn body_difference(retained: &[u8], fresh: &[u8]) -> String {
    let findings = |body: &[u8]| {
        serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|document| document.get("findings").and_then(|f| f.as_array().cloned()))
    };
    let clip = |text: String| {
        if text.len() > 600 {
            format!("{}...", text.chars().take(600).collect::<String>())
        } else {
            text
        }
    };
    if let (Some(left), Some(right)) = (findings(retained), findings(fresh)) {
        let index = left
            .iter()
            .zip(&right)
            .position(|(left, right)| left != right)
            .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())));
        if let Some(index) = index {
            let row = |rows: &[serde_json::Value]| {
                rows.get(index)
                    .map_or_else(String::new, ToString::to_string)
            };
            return format!(
                "table `findings` row {index} (retained {} rows, fresh {} rows)\n  retained: {}\n  \
                 fresh:    {}",
                left.len(),
                right.len(),
                clip(row(&left)),
                clip(row(&right)),
            );
        }
    }
    let retained = String::from_utf8_lossy(retained);
    let fresh = String::from_utf8_lossy(fresh);
    let line = retained
        .lines()
        .zip(fresh.lines())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| retained.lines().count().min(fresh.lines().count()));
    format!(
        "response body line {line}\n  retained: {}\n  fresh:    {}",
        clip(retained.lines().nth(line).unwrap_or_default().to_owned()),
        clip(fresh.lines().nth(line).unwrap_or_default().to_owned()),
    )
}

/// The test fault hook: perturbs the first materialized answer of a debug
/// build whose environment names `SOLID_CHECKER_IDLE_SHADOW_FAULT`.
#[cfg(debug_assertions)]
pub(super) fn fault(body: &Arc<[u8]>) -> Option<Arc<[u8]>> {
    static ARMED: AtomicBool = AtomicBool::new(true);
    std::env::var_os("SOLID_CHECKER_IDLE_SHADOW_FAULT")?;
    if !ARMED.swap(false, Ordering::Relaxed) {
        return None;
    }
    // A spurious finding: the retained answer claims a defect no fresh build
    // finds.
    let mut document = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    document
        .get_mut("findings")?
        .as_array_mut()?
        .push(serde_json::json!({"id": "SC0000", "injected": true}));
    Some(serde_json::to_vec(&document).ok()?.into())
}

#[cfg(not(debug_assertions))]
pub(super) fn fault(_body: &Arc<[u8]>) -> Option<Arc<[u8]>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_selects_the_mode_and_inline_shadow_turns_it_off() {
        let off = shadow::ShadowMode::Off;
        assert_eq!(Mode::from_setting(None, off), Mode::Serve);
        assert_eq!(Mode::from_setting(Some("1"), off), Mode::Serve);
        assert_eq!(Mode::from_setting(Some("0"), off), Mode::Off);
        assert_eq!(Mode::from_setting(Some("off"), off), Mode::Off);
        assert_eq!(Mode::from_setting(Some("panic"), off), Mode::Panic);
        for inline in [shadow::ShadowMode::Serve, shadow::ShadowMode::Panic] {
            assert_eq!(Mode::from_setting(None, inline), Mode::Off);
            assert_eq!(Mode::from_setting(Some("panic"), inline), Mode::Off);
        }
    }

    #[test]
    fn the_first_differing_finding_is_named() {
        let left = br#"{"findings":[{"a":1},{"a":2}]}"#;
        let right = br#"{"findings":[{"a":1},{"a":3}]}"#;
        let report = body_difference(left, right);
        assert!(report.contains("row 1"), "{report}");
        let report = body_difference(b"x\ny", b"x\nz");
        assert!(report.contains("line 1"), "{report}");
    }
}
