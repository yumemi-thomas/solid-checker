//! The executor's type checks (scripts/fuzz/typecheck.mjs: `typecheck` and `typecheckBatch`, the same answers). A file of its own
//! because the type-check cache keys on the sources that produce a type check's answer (cache.mjs `TYPECHECK_SOURCES`): an edit
//! elsewhere in the executor keeps the cached answers.

use crate::run_command;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// `path.resolve(base, relative)` without touching the disk.
pub(crate) fn resolve_lexical(base: &Path, relative: &str) -> PathBuf {
    let joined = base.join(relative);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub(crate) fn has_ts_error(line: &str) -> bool {
    line.find("error TS").is_some_and(|at| line[at + 8..].starts_with(|c: char| c.is_ascii_digit()))
}

/// `typecheck(dir)`: one `tsrs -p`.
pub(crate) fn typecheck_one(tsc: &str, dir: &str) -> Value {
    if !Path::new(tsc).exists() {
        return json!({ "ok": false, "errors": [format!("type checker not found: {tsc}")], "missing": true });
    }
    let mut command = Command::new(tsc);
    command.args(["-p", &format!("{dir}/tsconfig.json"), "--noEmit"]).current_dir(dir);
    let ran = run_command(command, Duration::from_secs(60));
    if let Some(error) = ran.spawn_error {
        return json!({ "ok": false, "errors": [error], "missing": true });
    }
    let out = format!("{}{}", ran.stdout, ran.stderr);
    let errors: Vec<&str> = out.split('\n').filter(|line| has_ts_error(line)).collect();
    json!({ "ok": ran.status == Some(0) && errors.is_empty(), "errors": errors })
}

/// `typecheckBatch(dirs)`: one `tsrs -b --force`; `None` where the batch gives no answer (the caller checks each alone).
pub(crate) fn typecheck_batch(tsc: &str, dirs: &[String]) -> Option<Vec<Value>> {
    // A directory twice in one batch could not tell its two answers apart: each is checked alone.
    let distinct: std::collections::HashSet<&String> = dirs.iter().collect();
    if !Path::new(tsc).exists() || dirs.is_empty() || distinct.len() != dirs.len() {
        return None;
    }
    let mut command = Command::new(tsc);
    command.arg("-b").arg("--force");
    for dir in dirs {
        command.arg(format!("{dir}/tsconfig.json"));
    }
    command.arg("--noEmit").current_dir(&dirs[0]);
    let ran = run_command(command, Duration::from_secs(120));
    for dir in dirs {
        let _ = std::fs::remove_file(format!("{dir}/tsconfig.tsbuildinfo"));
    }
    if ran.spawn_error.is_some() {
        return None;
    }
    batch_answers(dirs, &format!("{}{}", ran.stdout, ran.stderr), ran.status, ran.killed)
}

/// A batch's output as one answer per project, or `None` (typecheck.mjs `typecheckBatch`'s rules).
pub(crate) fn batch_answers(dirs: &[String], out: &str, status: Option<i32>, killed: bool) -> Option<Vec<Value>> {
    let lines: Vec<&str> = out.split('\n').filter(|line| has_ts_error(line)).collect();
    if killed || (status != Some(0) && lines.is_empty()) {
        return None;
    }
    let mut answers: Vec<(bool, Vec<String>)> = dirs.iter().map(|_| (true, Vec::new())).collect();
    let base = Path::new(&dirs[0]);
    for line in lines {
        // `^(.*?)\(\d+,\d+\): error TS\d+`
        let at = line.find("): error TS")?;
        let open = line[..at].rfind('(')?;
        let numbers = &line[open + 1..at];
        let mut parts = numbers.split(',');
        let valid = parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && parts.next().is_none();
        if !valid {
            return None;
        }
        let file_text = &line[..open];
        let file = resolve_lexical(base, file_text);
        let index = dirs.iter().position(|dir| file.starts_with(dir) && file != Path::new(dir))?;
        let relative = file.strip_prefix(&dirs[index]).ok()?.to_string_lossy().into_owned();
        answers[index].0 = false;
        answers[index].1.push(format!("{relative}{}", &line[file_text.len()..]));
    }
    Some(answers.into_iter().map(|(ok, errors)| json!({ "ok": ok, "errors": errors })).collect())
}
