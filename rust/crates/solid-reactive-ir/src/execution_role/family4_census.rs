//! MEASUREMENT ONLY (ADR 1114 family 4, step 0). Not for merge; no verdict reads it.
//!
//! `SOLID_CHECKER_F4_CENSUS=<file>` appends one JSON line per read or write decision made at a site inside a *named callback* (a function some
//! position names by its binding, `NamedCallbackRoles::admitted`): the function's positions, its direct calls through every binding
//! ([`function_called_by_name`]), the role each direct call runs with, whether the entry census is complete ([`role_entries_complete`]),
//! and the role the site's own decision ended with. A fold (`f4-fold.mjs`) says how many sites a position's role alone decides while a
//! direct call's role would report or is unknown.
//!
//! Unset, each hook is one atomic load.

use std::cell::Cell;
use std::io::Write as _;
use std::sync::{Mutex, OnceLock};

use super::*;

static SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

fn sink() -> Option<&'static Mutex<std::fs::File>> {
    SINK.get_or_init(|| {
        let path = std::env::var_os("SOLID_CHECKER_F4_CENSUS").filter(|path| !path.is_empty())?;
        // A directory gets one file per process (a corpus run's checkers never share a file).
        let path = std::path::PathBuf::from(path);
        let path = if path.is_dir() {
            path.join(format!("f4-{}.jsonl", std::process::id()))
        } else {
            path
        };
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    })
    .as_ref()
}

/// What a role says to the rule family `kind` (`read` or `write`): `report` (a violation if it runs), `unplaced` (an obligation), `silent`.
fn class(kind: &str, role: ExecutionRole) -> &'static str {
    let reports = if kind == "read" {
        role.reports_untracked_read()
    } else {
        role.reports_disallowed_write()
    };
    if reports {
        "report"
    } else if role.reports_unplaced() || role == ExecutionRole::Unknown {
        "unplaced"
    } else {
        "silent"
    }
}

/// Notes the decision `role` of kind `kind` at `span`, when `span` is inside a named callback. `call_role` classifies a direct call of the
/// callback the way `kind` classifies a site.
pub(super) fn note(
    kind: &'static str,
    file: &solid_facts::FileFacts,
    span: Span,
    role: ExecutionRole,
    lookup: &SemanticLookup<'_>,
    call_role: impl Fn(Span) -> ExecutionRole,
) {
    let Some(sink) = sink() else {
        return;
    };
    if BUSY.with(Cell::get) {
        return;
    }
    BUSY.with(|busy| busy.set(true));
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            BUSY.with(|busy| busy.set(false));
        }
    }
    let _reset = Reset;
    let Some(callback) = super::named_callbacks::named_callback_containing(file, span, lookup)
    else {
        return;
    };
    let roles = lookup.named_callback_roles(file);
    let positions = roles.positions(callback.span).len();
    let calls = function_called_by_name(file, callback, lookup);
    let complete = role_entries_complete(file, callback, lookup);
    let at_calls: Vec<serde_json::Value> = calls
        .iter()
        .map(|call| {
            let at = call_role(*call);
            serde_json::json!({
                "at": [call.start, call.end],
                "role": format!("{at:?}"),
                "class": class(kind, at),
            })
        })
        .collect();
    let line = serde_json::json!({
        "k": "f4",
        "kind": kind,
        "path": file.path.as_str(),
        "span": [span.start, span.end],
        "role": format!("{role:?}"),
        "class": class(kind, role),
        "fn": [callback.span.start, callback.span.end],
        "pos": positions,
        "complete": complete,
        "calls": at_calls,
    });
    let mut text = line.to_string();
    text.push('\n');
    if let Ok(mut file) = sink.lock() {
        let _ = file.write_all(text.as_bytes());
    }
}

/// Whether the census is on (the hooks cost one load when it is not).
pub(super) fn enabled() -> bool {
    sink().is_some()
}
