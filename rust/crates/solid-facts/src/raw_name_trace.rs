//! The raw-name trace (ADR 1152, feature `raw-name-trace`): a measurement, never a verdict.
//!
//! A name is its cooked value (`ev\u0061l` is `eval`), so a check that reads a name's source text and compares or classifies it reads the spelling, not the name. With the feature
//! on and `SOLID_CHECKER_RAW_NAME_TRACE=<file>` set, every `FileFacts::source_text` of a text that looks like a name (identifier characters, quotes, escapes and dots) and holds
//! an escape is appended to the file, once per process and call site and text, as `<rust file>:<line>\t<source path>\t<text>`. The sweep
//! (scripts/fuzz/escape-sweep.mjs) runs the corpus with every name escaped and counts the sites that read one: a site that reads an escaped name as written is a site to fix, whether
//! or not any verdict moved.

use std::collections::HashSet;
use std::io::Write as _;
use std::panic::Location;
use std::sync::{Mutex, OnceLock};

struct Trace {
    out: std::fs::File,
    seen: HashSet<(&'static str, u32, String)>,
}

fn trace() -> Option<&'static Mutex<Trace>> {
    static TRACE: OnceLock<Option<Mutex<Trace>>> = OnceLock::new();
    TRACE
        .get_or_init(|| {
            let path =
                std::env::var_os("SOLID_CHECKER_RAW_NAME_TRACE").filter(|path| !path.is_empty())?;
            let out = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()?;
            Some(Mutex::new(Trace {
                out,
                seen: HashSet::new(),
            }))
        })
        .as_ref()
}

/// Whether `text` looks like one name (or a chain of them) written with an escape.
fn escaped_name(text: &str) -> bool {
    text.contains('\\')
        && text.len() <= 120
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '_' | '$' | '\\' | '{' | '}' | '.' | '[' | ']' | '"' | '\'' | '`'
                )
        })
}

/// Records one read of `text` made at `at` of the file `path`.
pub(crate) fn note(path: &str, text: &str, at: &'static Location<'static>) {
    let Some(trace) = trace() else {
        return;
    };
    if !escaped_name(text) {
        return;
    }
    let mut trace = trace
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if trace.seen.insert((at.file(), at.line(), text.to_owned())) {
        let _ = writeln!(trace.out, "{}:{}\t{}\t{}", at.file(), at.line(), path, text);
    }
}
