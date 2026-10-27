//! The S4e differential gate: every fact family of `analyze` from the Rust
//! producer (tsrs) must equal the Go producer's on the repository's fixture
//! projects, over a dense set of spans (every word, every dotted chain, and
//! string and number literals), each with every family demanded.
//!
//! Both producers are driven by the client crate's own `Session` over the same
//! sources and demands. Each project goes through a cold analysis, an `update`
//! that shifts every span of one file, and an `update` that restores it. Entity
//! rows are compared field for field after one normalization: symbol ids
//! (hashed or `symbol:<generation>:<n>`, wherever they appear in a row) are renamed by
//! first appearance in location order. Rows must then be equal with every id
//! redacted; how many rows partition their symbols into ids differently from Go
//! (Go's ids follow its claim order, ADR 0366) is counted and not failed, and
//! so is how many rows carry a different type descriptor of their own (Go
//! shared a symbol's first descriptor, ADR 0367).
//!
//! * Default: a deterministic subset (every `TSRS_DIFF_STRIDE`-th project, 12
//!   by default, plus every project that uses `references` or `paths`).
//! * `TSRS_DIFF_ALL=1`: every `fixtures/**/tsconfig.json`.
//! * The reference producer is `SOLID_TYPEFACTS_REFERENCE_BIN` (default: the archived Go producer, see common::reference_binary);
//!   without one the test prints a notice and passes vacuously.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use typefacts::v3::{EntityDemand, FileChange, SourceFile};
use typefacts::{AnalysisDemand, FactTable, Location, Producer, Session};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn go_binary() -> Option<PathBuf> {
    common::reference_binary("families differential")
}

fn rust_binary() -> PathBuf {
    // `TSRS_DIFF_RUST_BIN` points the "rust" side elsewhere, e.g. at the Go
    // binary, to measure the Go producer's own run-to-run variation.
    if let Some(path) = std::env::var_os("TSRS_DIFF_RUST_BIN") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts"))
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        if name == "node_modules" || name == ".git" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            collect(&path, found);
        } else if name == "tsconfig.json" {
            found.push(path);
        }
    }
}

fn projects() -> Vec<PathBuf> {
    // `TSRS_DIFF_PROJECT=<tsconfig.json>[,<tsconfig.json>...]` runs exactly
    // those projects (a probe for one project outside `fixtures/`).
    if let Some(list) = std::env::var_os("TSRS_DIFF_PROJECT") {
        return list
            .to_string_lossy()
            .split(',')
            .map(PathBuf::from)
            .collect();
    }
    let mut all = Vec::new();
    collect(&repo_root().join("fixtures"), &mut all);
    all.sort();
    if let Ok(only) = std::env::var("TSRS_DIFF_ONLY") {
        return all
            .into_iter()
            .filter(|path| path.to_string_lossy().contains(&only))
            .collect();
    }
    if std::env::var_os("TSRS_DIFF_ALL").is_some_and(|value| value == "1") {
        return all;
    }
    let stride: usize = std::env::var("TSRS_DIFF_STRIDE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(12)
        .max(1);
    all.into_iter()
        .enumerate()
        .filter(|(index, path)| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            index % stride == 0 || text.contains("\"references\"") || text.contains("\"paths\"")
        })
        .map(|(_, path)| path)
        .collect()
}

fn is_word(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

/// Every word and every dotted chain of words in `source`, as byte spans. A
/// chain is demanded as a whole so the covering claim sees `a.b` (a span whose
/// first token is not the whole span) as well as bare identifiers.
fn spans(source: &str) -> Vec<(usize, usize)> {
    const LIMIT: usize = 1500;
    let bytes = source.as_bytes();
    let mut found: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' || bytes[index] == b'$') {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = start;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        found.insert((start, end));
        let mut chain_end = end;
        while bytes.get(chain_end) == Some(&b'.')
            && bytes.get(chain_end + 1).is_some_and(|byte| is_word(*byte))
        {
            chain_end += 1;
            while chain_end < bytes.len() && is_word(bytes[chain_end]) {
                chain_end += 1;
            }
        }
        if chain_end != end {
            found.insert((start, chain_end));
        }
        index = end;
    }
    let all: Vec<(usize, usize)> = found.into_iter().collect();
    if all.len() <= LIMIT {
        return all;
    }
    let stride = all.len().div_ceil(LIMIT);
    all.into_iter()
        .enumerate()
        .filter(|(index, _)| index % stride == 0)
        .map(|(_, span)| span)
        .collect()
}

/// String and number literal spans found by a crude scan (quotes without
/// escapes or newlines; digit runs outside words).
fn literal_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() && found.len() < 300 {
        let byte = bytes[index];
        if byte == b'"' || byte == b'\'' {
            let mut end = index + 1;
            while end < bytes.len() && !matches!(bytes[end], b'\n' | b'\\') && bytes[end] != byte {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == byte {
                found.push((index, end + 1));
                index = end + 1;
                continue;
            }
        } else if byte.is_ascii_digit()
            && (index == 0
                || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_'))
        {
            let mut end = index;
            while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
                end += 1;
            }
            found.push((index, end));
            index = end;
            continue;
        }
        index += 1;
    }
    found
}

fn demands_for(files: &[SourceFile]) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    // `TSRS_DIFF_FILES=a,b` demands only files whose path contains one of them.
    let only = std::env::var("TSRS_DIFF_FILES").ok();
    for file in files {
        if file.source.len() > 60_000 {
            continue;
        }
        if let Some(only) = &only
            && !only.split(',').any(|part| file.path.contains(part))
        {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&file.source) else {
            continue;
        };
        let mut all: BTreeSet<(usize, usize)> = spans(text).into_iter().collect();
        all.extend(literal_spans(text));
        for (index, (start, end)) in all.into_iter().enumerate() {
            demands.push(EntityDemand {
                location: Location {
                    path: file.path.as_str().into(),
                    start_byte: start as u64,
                    end_byte: end as u64,
                },
                r#async: index % 5 == 0,
                symbol: true,
                references: index % 4 == 0,
                resolved_call: true,
                type_descriptor: true,
                structural_accessor: index % 9 == 0,
                callability: true,
                constructability: true,
                runtime_value_domain: true,
                primitive_value_domain: true,
                primitive_literal_candidates: true,
                parameter_object_shape: true,
                call_result_domain: true,
                constant_value: true,
                array_shape: true,
                tuple_shape: true,
                library_types: true,
                reference_space: true,
                runtime_identity: true,
                declared_keys: false,
                writable_optional_property: false,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

/// Renames every symbol id in `text` by first appearance: a counter id
/// (`symbol:<digits>:<digits>`) and a hashed one (`symbol:h:<hex>`). The
/// spelling of an id is not a fact (ADR 0366: the Go producer's hashed ids
/// follow its claim order and its lib text); which symbols share an id is.
fn rename_non_durable(text: &str, names: &mut BTreeMap<String, String>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("symbol:") {
        result.push_str(&rest[..at]);
        let tail = &rest[at..];
        let body = &tail["symbol:".len()..];
        if let Some(hex) = body.strip_prefix("h:") {
            let digits = hex.bytes().take_while(u8::is_ascii_hexdigit).count();
            if digits > 0 {
                let length = "symbol:h:".len() + digits;
                let id = &tail[..length];
                let next = format!("id#{}", names.len() + 1);
                result.push_str(names.entry(id.to_owned()).or_insert(next));
                rest = &tail[length..];
                continue;
            }
        }
        let generation = body.bytes().take_while(u8::is_ascii_digit).count();
        let after = &body[generation..];
        let number = after.strip_prefix(':').map_or(0, |after| {
            after.bytes().take_while(u8::is_ascii_digit).count()
        });
        if generation > 0 && number > 0 {
            let length = "symbol:".len() + generation + 1 + number;
            let id = &tail[..length];
            let next = format!("id#{}", names.len() + 1);
            result.push_str(names.entry(id.to_owned()).or_insert(next));
            rest = &tail[length..];
        } else {
            result.push_str("symbol:");
            rest = body;
        }
    }
    result.push_str(rest);
    result
}

/// A well-known symbol property name embeds the checker's unique-symbol type
/// counter (`__@iterator@88` in one producer, `...@86` in the other; library
/// type names spell the marker `\u{fffd}@`): it follows type creation order,
/// which the two checkers do not share. Drops the counter, as the differential
/// harness does; the Rust producer no longer writes it (ADR 0367).
fn drop_unique_symbol_counters(text: &str) -> String {
    drop_counters_after(&drop_counters_after(text, "__@"), "\u{fffd}@")
}

fn drop_counters_after(text: &str, marker: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(marker) {
        result.push_str(&rest[..at + marker.len()]);
        rest = &rest[at + marker.len()..];
        let name = rest
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'$'))
            .count();
        result.push_str(&rest[..name]);
        rest = &rest[name..];
        if let Some(after) = rest.strip_prefix('@') {
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 {
                rest = &after[digits..];
            }
        }
    }
    result.push_str(rest);
    result
}

/// `text` with every renamed id (`id#<n>`) replaced by `id`.
fn redact_ids(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("id#") {
        result.push_str(&rest[..at + 2]);
        let tail = &rest[at + 3..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        rest = if digits > 0 { &tail[digits..] } else { tail };
    }
    result.push_str(rest);
    result
}

/// One entity row: its text with ids redacted (what must equal Go's), and
/// with ids renamed by first appearance (which symbols share an id). The
/// row's own type descriptor is held apart (`descriptor`): Go gave every
/// entity of a symbol the first descriptor any entity of it was answered with,
/// the Rust producer answers each location's own type (ADR 0367), so the two
/// are counted, not required to be equal.
struct Row {
    redacted: String,
    renamed: String,
    descriptor: String,
}

/// Every entity row of `table` as pretty Debug lines, keyed by span.
fn rows(table: &FactTable) -> BTreeMap<(String, u64, u64), Row> {
    let mut entities: Vec<_> = table.entities().collect();
    entities.sort_by(|left, right| {
        (
            &*left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &*right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });
    let mut names = BTreeMap::new();
    entities
        .into_iter()
        .map(|entity| {
            (
                (
                    entity.location.path.to_string(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ),
                {
                    let mut bare = entity.clone();
                    let descriptor = bare.type_descriptor.take();
                    let renamed = rename_non_durable(
                        &drop_unique_symbol_counters(&format!("{bare:#?}")),
                        &mut names,
                    );
                    Row {
                        redacted: redact_ids(&renamed),
                        renamed,
                        descriptor: format!("{descriptor:?}"),
                    }
                },
            )
        })
        .collect()
}

/// One producer's analyses: cold, shifted, restored.
type Phases = Vec<Result<FactTable, String>>;

fn drive(
    binary: &Path,
    project: &str,
    shared_sources: Option<&[SourceFile]>,
) -> Result<(Phases, Vec<SourceFile>), String> {
    let mut session = Session::open(Producer::at(binary), project, Vec::<FileChange>::new())
        .map_err(|error| format!("open: {error}"))?;
    let sources = match shared_sources {
        Some(sources) => sources.to_vec(),
        None => session
            .configured_sources()
            .map_err(|error| format!("sources: {error}"))?,
    };
    let mut tables = Vec::new();
    let analyze = |session: &mut Session, sources: &[SourceFile]| {
        session
            .analyze(&AnalysisDemand {
                entities: demands_for(sources),
            })
            .map_err(|error| error.to_string())
    };
    tables.push(analyze(&mut session, &sources));
    let target = sources
        .iter()
        .find(|file| !file.path.contains("/node_modules/") && file.source.len() < 60_000);
    if let Some(target) = target {
        let mut shifted = sources.clone();
        let mut source = b"\n".to_vec();
        source.extend_from_slice(&target.source);
        for file in &mut shifted {
            if file.path == target.path {
                file.source.clone_from(&source);
            }
        }
        let edit = FileChange {
            path: target.path.clone(),
            source,
            deleted: false,
            version: 1,
        };
        match session.update([edit]) {
            Ok(()) => tables.push(analyze(&mut session, &shifted)),
            Err(error) => tables.push(Err(format!("update: {error}"))),
        }
        let restore = FileChange {
            path: target.path.clone(),
            source: target.source.clone(),
            deleted: false,
            version: 2,
        };
        match session.update([restore]) {
            Ok(()) => tables.push(analyze(&mut session, &sources)),
            Err(error) => tables.push(Err(format!("update: {error}"))),
        }
    }
    let _ = session.close();
    Ok((tables, sources))
}

/// The pretty-printed lines that differ between two rows (the rows are the
/// same shape, so a line-by-line comparison names the fields).
fn changed_lines(go: &str, rust: &str) -> String {
    let (go_lines, rust_lines): (Vec<&str>, Vec<&str>) =
        (go.lines().collect(), rust.lines().collect());
    let only_go: Vec<&&str> = go_lines
        .iter()
        .filter(|line| !rust_lines.contains(line))
        .take(4)
        .collect();
    let only_rust: Vec<&&str> = rust_lines
        .iter()
        .filter(|line| !go_lines.contains(line))
        .take(4)
        .collect();
    format!("go-only {only_go:?}; rust-only {only_rust:?}")
}

#[test]
fn every_family_equals_go_on_the_corpus() {
    let Some(go) = go_binary() else {
        return;
    };
    let rust = rust_binary();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let projects = projects();
    for config in &projects {
        let project = config
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let label = project
            .strip_prefix(&format!(
                "{}/",
                repo_root().canonicalize().unwrap().display()
            ))
            .unwrap_or(&project)
            .to_owned();
        let go_run = drive(&go, &project, None);
        let rust_run = match &go_run {
            Ok((_, sources)) => drive(&rust, &project, Some(sources)),
            Err(_) => drive(&rust, &project, None),
        };
        let (go_tables, rust_tables) = match (go_run, rust_run) {
            (Ok((go_tables, _)), Ok((rust_tables, _))) => (go_tables, rust_tables),
            (Err(go_error), Err(rust_error)) if go_error == rust_error => {
                *counts
                    .entry("project not opened by either (identical error)".into())
                    .or_default() += 1;
                continue;
            }
            (go_result, rust_result) => {
                failures.push(format!(
                    "{label}: open differs: go {:?}; rust {:?}",
                    go_result.err(),
                    rust_result.err()
                ));
                continue;
            }
        };
        *counts.entry("projects compared".into()).or_default() += 1;
        for (phase, (go_table, rust_table)) in go_tables.iter().zip(&rust_tables).enumerate() {
            let name = ["cold", "shifted", "restored"][phase];
            match (go_table, rust_table) {
                (Ok(go_table), Ok(rust_table)) => {
                    let (go_rows, rust_rows) = (rows(go_table), rows(rust_table));
                    *counts.entry("entity rows compared".into()).or_default() += go_rows.len();
                    for (key, go_row) in &go_rows {
                        match rust_rows.get(key) {
                            None => failures.push(format!("{label} [{name}] {key:?}: only in go")),
                            Some(rust_row) if rust_row.redacted != go_row.redacted => {
                                failures.push(format!(
                                    "{label} [{name}] {}:{}-{}: {}",
                                    key.0.rsplit('/').next().unwrap_or(&key.0),
                                    key.1,
                                    key.2,
                                    changed_lines(&go_row.redacted, &rust_row.redacted)
                                ));
                            }
                            Some(rust_row) => {
                                if rust_row.descriptor != go_row.descriptor {
                                    *counts
                                        .entry(
                                            "rows whose own type descriptor differs (not a failure, ADR 0367)"
                                                .into(),
                                        )
                                        .or_default() += 1;
                                }
                                if rust_row.renamed != go_row.renamed {
                                    *counts
                                        .entry(
                                            "rows whose symbols partition into ids differently (not a failure, ADR 0366)"
                                                .into(),
                                        )
                                        .or_default() += 1;
                                }
                            }
                        }
                    }
                    for key in rust_rows.keys().filter(|key| !go_rows.contains_key(*key)) {
                        failures.push(format!("{label} [{name}] {key:?}: only in rust"));
                    }
                }
                (Err(go_error), Err(rust_error)) if go_error == rust_error => {
                    *counts
                        .entry("analysis refused identically".into())
                        .or_default() += 1;
                }
                (go_result, rust_result) => failures.push(format!(
                    "{label} [{name}]: outcome differs: go {:?}; rust {:?}",
                    go_result.as_ref().err(),
                    rust_result.as_ref().err()
                )),
            }
        }
    }
    eprintln!("families differential over {} projects", projects.len());
    for (category, count) in &counts {
        eprintln!("  {count:>8}  {category}");
    }
    eprintln!("  {:>8}  differences", failures.len());
    // Group by what differs (the text after the span), so one cause is one line.
    let mut groups: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for failure in &failures {
        let cause = failure
            .split_once(": ")
            .map_or(failure.as_str(), |(_, cause)| cause);
        let entry = groups
            .entry(cause.chars().take(300).collect())
            .or_insert((0, failure.clone()));
        entry.0 += 1;
    }
    for (count, example) in groups.values().take(40) {
        eprintln!("    x{count}: {example}");
    }
    assert!(failures.is_empty(), "{} differences", failures.len());
}
