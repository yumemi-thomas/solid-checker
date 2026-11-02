//! The role log (ADR 0533): `SOLID_CHECKER_IR_ROLE_LOG=<path>` records which
//! arm of the role and owner classifiers answered each decision, so a fuzz
//! grid can say which arms its programs reach (`scripts/fuzz/roles.mjs`).
//!
//! An arm is a role-deciding return tagged with a stable id: `role_arm!("id",
//! role)` in [`crate::execution_role`], `arm_id!("id")` at the owner answers
//! of `owners::owner_emission`. `arm_id!(default "id")` marks an arm that
//! answers a non-reporting role for want of a proof (the saturation report's
//! class 4: a deferred role for a function no run places, an owner context
//! zero) so a run can count the decisions that are silent by default.
//!
//! One JSON line per outermost decision ([`decide`]): `{"k": kind, "f": path,
//! "s": start, "e": end, "role", "arm", "arms", "default", "silent"}`, where
//! `arm` is the last tagged arm that returned the final role (null when an
//! untagged path did), `arms` every tagged arm the decision passed, in order,
//! once each. Owner lines carry `rule` (the operation) and `ctx` (the owner
//! context) instead of `role`. An arm hit outside any decision is a `k:
//! "unframed"` line with no site.
//!
//! Write-only: nothing reads the log back, and every hook returns the value it
//! was given, so the log changes no answer. Unset, each hook is one atomic
//! load and a branch: no allocation, no lock. A file is appended to in
//! whole-line writes (a fuzz grid's concurrent checkers share one); a
//! directory gets one `roles-<pid>.jsonl` per process.

use std::cell::RefCell;
use std::io::Write as _;
use std::sync::{Mutex, OnceLock};

use solid_facts::core::Span;

use crate::ExecutionRole;

/// A tagged arm's id and whether it is a default (non-reporting for want of a
/// proof): `arm_id!("file.name")`, `arm_id!(default "file.name")`. Ids are
/// `<module file stem>.<name>`, unique, and literal: `scripts/fuzz/roles.mjs`
/// reads the inventory of arms from the source.
macro_rules! arm_id {
    (default $id:literal) => {
        ($id, true)
    };
    ($id:literal) => {
        ($id, false)
    };
}
pub(crate) use arm_id;

/// `role` answered by the tagged arm `id` (see [`arm_id`]); returns `role`.
macro_rules! role_arm {
    (default $id:literal, $role:expr) => {
        $crate::role_log::arm($crate::role_log::arm_id!(default $id), $role)
    };
    ($id:literal, $role:expr) => {
        $crate::role_log::arm($crate::role_log::arm_id!($id), $role)
    };
}
pub(crate) use role_arm;

/// Lines are flushed when this much is buffered, and at the end of a build.
const FLUSH_AT: usize = 1 << 16;

struct Sink {
    file: std::fs::File,
    buffer: Vec<u8>,
}

fn sink() -> Option<&'static Mutex<Sink>> {
    static SINK: OnceLock<Option<Mutex<Sink>>> = OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::path::PathBuf::from(std::env::var_os("SOLID_CHECKER_IR_ROLE_LOG")?);
        let path = if path.is_dir() {
            path.join(format!("roles-{}.jsonl", process_id()?))
        } else {
            path
        };
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Mutex::new(Sink {
            file,
            buffer: Vec::new(),
        }))
    })
    .as_ref()
}

/// The process id a log directory's file is named by; none where the
/// platform has no process ids (WASI), which leaves the log off.
fn process_id() -> Option<u32> {
    #[cfg(any(unix, windows))]
    {
        Some(std::process::id())
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Whether the role log is on for this process.
#[inline]
pub(crate) fn enabled() -> bool {
    sink().is_some()
}

struct Decision {
    kind: &'static str,
    path: String,
    span: Span,
    arms: Vec<((&'static str, bool), ExecutionRole)>,
}

thread_local! {
    static CURRENT: RefCell<Option<Decision>> = const { RefCell::new(None) };
    /// The lines this thread appends, for each [`record`] running on it.
    static RECORDING: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// What the log recorded on this thread while a computation ran
/// ([`record`]): the arms it noted into the open decision and the lines it
/// appended. [`replay`] makes the log as the computation run again would.
#[derive(Clone)]
pub(crate) struct Recorded {
    arms: Vec<((&'static str, bool), ExecutionRole)>,
    lines: Vec<u8>,
}

/// Whether a decision is open on this thread: what an arm hit is logged as.
pub(crate) fn frame_open() -> bool {
    enabled() && CURRENT.with(|current| current.borrow().is_some())
}

/// Runs `f`; with the log on, also returns what the log recorded while it
/// ran on this thread. A decision open at the start is still open at the
/// end (decisions nest by thread, and `f` returns normally).
pub(crate) fn record<R>(f: impl FnOnce() -> R) -> (R, Option<Recorded>) {
    if !enabled() {
        return (f(), None);
    }
    let arms_before = CURRENT.with(|current| current.borrow().as_ref().map(|d| d.arms.len()));
    RECORDING.with(|recording| recording.borrow_mut().push(Vec::new()));
    let result = f();
    let lines = RECORDING
        .with(|recording| recording.borrow_mut().pop())
        .unwrap_or_default();
    let arms = CURRENT.with(|current| match (current.borrow().as_ref(), arms_before) {
        (Some(decision), Some(before)) => decision.arms[before..].to_vec(),
        _ => Vec::new(),
    });
    (result, Some(Recorded { arms, lines }))
}

/// Logs again what `recorded` holds: its arms into the open decision, its
/// lines appended.
pub(crate) fn replay(recorded: &Recorded) {
    if !enabled() {
        return;
    }
    if !recorded.arms.is_empty() {
        CURRENT.with(|current| {
            if let Some(decision) = current.borrow_mut().as_mut() {
                decision.arms.extend_from_slice(&recorded.arms);
            }
        });
    }
    if !recorded.lines.is_empty() {
        append(&recorded.lines);
    }
}

/// A tagged arm answered `role`; returns it unchanged.
#[inline]
pub(crate) fn arm(id: (&'static str, bool), role: ExecutionRole) -> ExecutionRole {
    if enabled() {
        note(id, role);
    }
    role
}

#[cold]
#[inline(never)]
fn note(id: (&'static str, bool), role: ExecutionRole) {
    let framed = CURRENT.with(|current| {
        current.borrow_mut().as_mut().map(|decision| {
            decision.arms.push((id, role));
        })
    });
    if framed.is_none() {
        let mut line = Vec::with_capacity(96);
        let _ = write!(line, "{{\"k\":\"unframed\",\"role\":\"{role:?}\",\"arm\":");
        push_json_str(&mut line, id.0);
        let _ = write!(line, ",\"arms\":[");
        push_json_str(&mut line, id.0);
        let _ = writeln!(line, "],\"default\":{}}}", id.1);
        append(&line);
    }
}

/// Runs the role decision `classify` for the site `span` of `path`; when the
/// log is on and no decision is open on this thread, records it as one line of
/// kind `kind` (`read`, `write`, `action`, `role`). Returns what `classify`
/// returned.
#[inline]
pub(crate) fn decide(
    kind: &'static str,
    path: &str,
    span: Span,
    classify: impl FnOnce() -> ExecutionRole,
) -> ExecutionRole {
    if !enabled() {
        return classify();
    }
    decide_logged(kind, path, span, classify)
}

#[inline(never)]
fn decide_logged(
    kind: &'static str,
    path: &str,
    span: Span,
    classify: impl FnOnce() -> ExecutionRole,
) -> ExecutionRole {
    let outermost = CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        if current.is_some() {
            return false;
        }
        *current = Some(Decision {
            kind,
            path: path.to_owned(),
            span,
            arms: Vec::new(),
        });
        true
    });
    if !outermost {
        return classify();
    }
    // Closes the decision on unwind too, so a caught panic leaves no frame
    // open to swallow the thread's later decisions.
    struct Close;
    impl Drop for Close {
        fn drop(&mut self) {
            CURRENT.with(|current| current.borrow_mut().take());
        }
    }
    let close = Close;
    let role = classify();
    let decision = CURRENT.with(|current| current.borrow_mut().take());
    drop(close);
    if let Some(decision) = decision {
        write_decision(&decision, role);
    }
    role
}

/// Whether a final `role` of a `kind` decision is reported by no rule: a
/// proven non-reporting role (not [`ExecutionRole::Unknown`], which is an
/// obligation). `role` decisions feed both rule families, so they say nothing.
fn silent(kind: &str, role: ExecutionRole) -> Option<bool> {
    match kind {
        "read" => Some(!role.reports_untracked_read() && !role.reports_unplaced()),
        "write" | "action" => Some(!role.reports_disallowed_write() && !role.reports_unplaced()),
        _ => None,
    }
}

fn write_decision(decision: &Decision, role: ExecutionRole) {
    append(&decision_line(decision, role));
}

fn decision_line(decision: &Decision, role: ExecutionRole) -> Vec<u8> {
    let answering = decision
        .arms
        .iter()
        .rev()
        .find(|(_, answered)| *answered == role)
        .map(|(id, _)| *id);
    let mut line = Vec::with_capacity(160 + decision.arms.len() * 32);
    let _ = write!(line, "{{\"k\":\"{}\",\"f\":", decision.kind);
    push_json_str(&mut line, &decision.path);
    let _ = write!(
        line,
        ",\"s\":{},\"e\":{},\"role\":\"{role:?}\",\"arm\":",
        decision.span.start, decision.span.end
    );
    match answering {
        Some((id, _)) => push_json_str(&mut line, id),
        None => line.extend_from_slice(b"null"),
    }
    line.extend_from_slice(b",\"arms\":[");
    let mut written: Vec<&str> = Vec::with_capacity(decision.arms.len());
    for ((id, _), _) in &decision.arms {
        if written.contains(id) {
            continue;
        }
        if !written.is_empty() {
            line.push(b',');
        }
        push_json_str(&mut line, id);
        written.push(id);
    }
    let _ = write!(
        line,
        "],\"default\":{}",
        answering.is_some_and(|(_, default)| default)
    );
    match silent(decision.kind, role) {
        Some(silent) => {
            let _ = writeln!(line, ",\"silent\":{silent}}}");
        }
        None => {
            let _ = writeln!(line, "}}");
        }
    }
    line
}

/// One owner answer of `owners::owner_emission`: the operation `rule` at
/// `span` of `path` took the arm `arm` with the owner context `context`;
/// `silent` when the requirement is neither reported nor uncertain.
#[inline]
pub(crate) fn owner(
    rule: &str,
    path: &str,
    span: Span,
    arm: (&'static str, bool),
    context: u8,
    silent: bool,
) {
    if enabled() {
        owner_logged(rule, path, span, arm, context, silent);
    }
}

#[cold]
#[inline(never)]
fn owner_logged(
    rule: &str,
    path: &str,
    span: Span,
    arm: (&'static str, bool),
    context: u8,
    silent: bool,
) {
    let mut line = Vec::with_capacity(192);
    line.extend_from_slice(b"{\"k\":\"owner\",\"rule\":");
    push_json_str(&mut line, rule);
    line.extend_from_slice(b",\"f\":");
    push_json_str(&mut line, path);
    let _ = write!(
        line,
        ",\"s\":{},\"e\":{},\"ctx\":{context},\"arm\":",
        span.start, span.end
    );
    push_json_str(&mut line, arm.0);
    line.extend_from_slice(b",\"arms\":[");
    push_json_str(&mut line, arm.0);
    let _ = writeln!(line, "],\"default\":{},\"silent\":{silent}}}", arm.1);
    append(&line);
}

fn push_json_str(line: &mut Vec<u8>, text: &str) {
    let _ = serde_json::to_writer(&mut *line, text);
}

fn append(line: &[u8]) {
    let Some(sink) = sink() else {
        return;
    };
    RECORDING.with(|recording| {
        for lines in recording.borrow_mut().iter_mut() {
            lines.extend_from_slice(line);
        }
    });
    let Ok(mut sink) = sink.lock() else {
        return;
    };
    sink.buffer.extend_from_slice(line);
    if sink.buffer.len() >= FLUSH_AT {
        flush_locked(&mut sink);
    }
}

fn flush_locked(sink: &mut Sink) {
    // Whole lines only: the buffer never holds a partial one.
    let Sink { file, buffer } = sink;
    let _ = file.write_all(buffer);
    buffer.clear();
}

/// Writes out what is buffered (the end of each build).
pub(crate) fn flush() {
    if let Some(sink) = sink()
        && let Ok(mut sink) = sink.lock()
    {
        flush_locked(&mut sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_line_names_the_answering_arm_once_each() {
        let decision = Decision {
            kind: "read",
            path: "/p/\"App\".tsx".to_owned(),
            span: Span::new(3, 9),
            arms: vec![
                (arm_id!("a.first"), ExecutionRole::TrackedJsx),
                (
                    arm_id!(default "a.default"),
                    ExecutionRole::DeferredCallback,
                ),
                (arm_id!("a.first"), ExecutionRole::TrackedJsx),
            ],
        };
        let line = decision_line(&decision, ExecutionRole::DeferredCallback);
        assert_eq!(
            std::str::from_utf8(&line).unwrap(),
            "{\"k\":\"read\",\"f\":\"/p/\\\"App\\\".tsx\",\"s\":3,\"e\":9,\
             \"role\":\"DeferredCallback\",\"arm\":\"a.default\",\
             \"arms\":[\"a.first\",\"a.default\"],\"default\":true,\"silent\":true}\n"
        );
        let untagged = decision_line(&decision, ExecutionRole::Unknown);
        let untagged: serde_json::Value = serde_json::from_slice(&untagged).unwrap();
        assert_eq!(untagged["arm"], serde_json::Value::Null);
        assert_eq!(untagged["default"], false);
        assert_eq!(untagged["silent"], false);
        assert_eq!(silent("write", ExecutionRole::TrackedJsx), Some(false));
        assert_eq!(silent("role", ExecutionRole::TrackedJsx), None);
    }

    #[test]
    fn hooks_return_what_they_were_given() {
        assert_eq!(
            arm(arm_id!("t.x"), ExecutionRole::EffectApply),
            ExecutionRole::EffectApply
        );
        assert_eq!(
            decide("read", "/p", Span::new(0, 1), || ExecutionRole::Unreported),
            ExecutionRole::Unreported
        );
    }
}
