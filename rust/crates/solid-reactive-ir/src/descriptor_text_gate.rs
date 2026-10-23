//! Type text is display only (ADR 0367). `TypeDescriptor.text` and
//! `ResolvedCall.return_type_text` follow the checker's type creation order
//! (union constituents, sibling properties), which no history-free contract
//! can promise, so no decision may read them: they are quoted in messages and
//! internal uncertifiable details only. This gate scans the analysis and
//! dialect sources and fails when a read appears outside the listed message
//! sites. A new message site is added here with the message it builds; a
//! decision that needs type structure asks the producer for a fact instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `(path from rust/, reads)`: the message sites, each counted.
const MESSAGE_SITES: &[(&str, usize)] = &[
    // The JSON-safety message quotes the argument's type.
    ("crates/solid-reactive-ir/src/server_rules.rs", 1),
    // Unknown-callback obligation messages quote the parameter's type.
    ("crates/solid-reactive-ir/src/interproc/obligations.rs", 2),
];

/// The crates whose code decides findings.
const SCANNED: &[&str] = &[
    "crates/solid-reactive-ir/src",
    "crates/solid-facts/src",
    "crates/solid-facts-backend/src",
    "dialects",
];

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if name != "tests" && name != "target" && name != "node_modules" {
                rust_files(&path, files);
            }
        } else if name.ends_with(".rs")
            && name != "tests.rs"
            && name != "descriptor_text_gate.rs"
            && !name.ends_with("_tests.rs")
        {
            files.push(path);
        }
    }
}

fn is_ident(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

/// Reads of a descriptor's `.text` (a `.text` field access whose statement
/// names a descriptor) and of `return_type_text`, outside `#[cfg(test)]`
/// modules.
fn reads(source: &str) -> usize {
    let code = source.split("#[cfg(test)]").next().unwrap_or(source);
    let bytes = code.as_bytes();
    let mut count = 0;
    for (at, _) in code.match_indices("return_type_text") {
        let before = at.checked_sub(1).map(|index| bytes[index]);
        let after = bytes.get(at + "return_type_text".len()).copied();
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            count += 1;
        }
    }
    for (at, _) in code.match_indices(".text") {
        if bytes.get(at + ".text".len()).copied().is_some_and(is_ident) {
            continue;
        }
        let statement_start = code[..at]
            .rfind([';', '{', '}'])
            .map_or(0, |index| index + 1);
        if code[statement_start..at].contains("descriptor") {
            count += 1;
        }
    }
    count
}

#[test]
fn no_decision_reads_type_text() {
    let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for directory in SCANNED {
        rust_files(&rust.join(directory), &mut files);
    }
    assert!(files.len() > 50, "the gate scanned {} files", files.len());
    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read source");
        let count = reads(&source);
        if count > 0 {
            let relative = file
                .strip_prefix(&rust)
                .expect("under rust/")
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative, count);
        }
    }
    let allowed: BTreeMap<String, usize> = MESSAGE_SITES
        .iter()
        .map(|(path, count)| ((*path).to_owned(), *count))
        .collect();
    assert_eq!(
        found, allowed,
        "a type text read outside the message sites (ADR 0367): decisions must not read \
         TypeDescriptor.text or ResolvedCall.return_type_text"
    );
}

#[test]
fn the_gate_sees_the_read_shapes_it_forbids() {
    assert_eq!(
        reads("let t = entity.type_descriptor.as_ref().map(|d| d.text.len());"),
        1
    );
    assert_eq!(
        reads("if descriptor.text.contains(\"never\") { return; }"),
        1
    );
    assert_eq!(reads("let r = &call.return_type_text;"), 1);
    assert_eq!(reads("let t = import.text.as_ref();"), 0);
    assert_eq!(reads("let t = descriptor.text_len;"), 0);
    assert_eq!(
        reads("fn f() {}\n#[cfg(test)]\nmod tests { fn g() { descriptor.text; } }"),
        0
    );
}
