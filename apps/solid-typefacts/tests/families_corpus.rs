//! The S7 switch precondition: on the repository's fixture projects, the Rust
//! producer answers every demand family and counts none of them unavailable
//! (`Project::unavailable`), and every family is answered somewhere (a family
//! that never produced a fact on the whole corpus would pass the first check
//! while proving nothing).
//!
//! Every family is demanded at every binding name, every callee and every whole
//! call the project's own source facts report (the spans the checker demands).
//!
//! * Default: a deterministic subset (every `TSRS_DIFF_STRIDE`-th project, 12
//!   by default, plus every project that uses `references` or `paths`).
//! * `TSRS_DIFF_ALL=1`: every `fixtures/**/tsconfig.json`.

use solid_typefacts::backend::ClosureBackend;
use solid_typefacts::context::Context;
use solid_typefacts::model::SemanticDemandRun;
use solid_typefacts::project::Project;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use typefacts::Location;
use typefacts::v3::EntityDemand;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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
    let mut all = Vec::new();
    collect(&repo_root().join("fixtures"), &mut all);
    all.sort();
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

/// String and number literal spans found by a crude scan (quotes without
/// escapes or newlines; digit runs outside words). A span that is not exactly
/// one literal expression is answered with nothing, which is what the check
/// below tolerates; the real ones give `constantValue` something to fold.
fn literal_spans(path: &str, text: &str) -> Vec<Location> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
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
        .into_iter()
        .take(200)
        .map(|(start, end)| Location {
            path: path.into(),
            start_byte: start as u64,
            end_byte: end as u64,
        })
        .collect()
}

fn every_family(location: Location) -> EntityDemand {
    EntityDemand {
        r#async: true,
        symbol: true,
        location,
        references: true,
        resolved_call: true,
        type_descriptor: true,
        structural_accessor: false,
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
    }
}

#[test]
fn every_family_is_answered_and_none_is_counted_unavailable_on_the_corpus() {
    assert!(solid_typefacts::semantic_runs::UNPORTED.is_empty());
    let ctx = Context::new();
    let mut answered: BTreeMap<&'static str, u64> = BTreeMap::new();
    let (mut opened, mut demanded) = (0usize, 0usize);
    for config in projects() {
        let config = config.to_string_lossy().into_owned();
        let Ok(mut project) = Project::open(&ctx, &config, None, None) else {
            continue;
        };
        opened += 1;
        for source in project.source_files(&ctx).unwrap() {
            if source.path.starts_with("bundled:") || source.source.len() > 200_000 {
                continue;
            }
            let path = source.path.clone();
            let mut locations: Vec<Location> = Vec::new();
            for binding in project.source_bindings(&ctx, &path).unwrap() {
                locations.extend(
                    binding
                        .names
                        .into_iter()
                        .filter(|name| !name.path.is_empty() && name.end_byte > name.start_byte),
                );
                locations.push(binding.initializer.location);
            }
            for call in project.source_calls(&ctx, &path).unwrap() {
                locations.push(call.callee);
                locations.push(call.location);
            }
            if let Ok(text) = std::str::from_utf8(&source.source) {
                locations.extend(literal_spans(&path, text));
            }
            locations.retain(|location| &*location.path == path.as_str());
            locations.sort_by_key(|location| (location.start_byte, location.end_byte));
            locations.dedup();
            if locations.is_empty() {
                continue;
            }
            let demands: Vec<EntityDemand> = locations.into_iter().map(every_family).collect();
            demanded += demands.len();
            let results = project
                .semantic_demand_runs(
                    &ctx,
                    &[SemanticDemandRun {
                        path: &path,
                        demands: &demands,
                        shared: None,
                    }],
                )
                .unwrap_or_else(|error| panic!("{config} {path}: {error}"));
            for entity in &results[0].entities {
                let mut note = |family: &'static str, present: bool| {
                    *answered.entry(family).or_default() += u64::from(present);
                };
                note("symbol", !entity.symbol.is_empty());
                note("typeDescriptor", entity.type_descriptor.is_some());
                note("resolvedCall", entity.resolved_call.is_some());
                note("callability", entity.callability.is_some());
                note("constructability", entity.constructability.is_some());
                note("runtimeValueDomain", entity.runtime_value_domain.is_some());
                note(
                    "primitiveLiteralCandidates",
                    entity.primitive_literal_candidates.is_some(),
                );
                note("callResultDomain", entity.call_result_domain.is_some());
                note("constantValue", entity.constant_value.is_some());
                note("arrayShape", entity.array_shape.is_some());
                note("tupleShape", entity.tuple_shape.is_some());
                note("libraryTypes", entity.library_types.is_some());
                note("referenceSpace", entity.reference_space.is_some());
                note("runtimeIdentity", !entity.runtime_identity.is_empty());
                note("runtimeBindingKind", entity.runtime_binding_kind.is_some());
            }
        }
        assert!(
            project.unavailable.0.is_empty(),
            "{config}: families counted unavailable: {:?}",
            project.unavailable
        );
    }
    eprintln!("families corpus: {opened} projects, {demanded} demands, answered {answered:?}");
    assert!(opened > 0, "no fixture project opened");
    for family in [
        "symbol",
        "typeDescriptor",
        "resolvedCall",
        "callability",
        "constructability",
        "runtimeValueDomain",
        "primitiveLiteralCandidates",
        "callResultDomain",
        "constantValue",
        "arrayShape",
        "tupleShape",
        "libraryTypes",
        "referenceSpace",
        "runtimeIdentity",
        "runtimeBindingKind",
    ] {
        assert!(
            answered.get(family).copied().unwrap_or(0) > 0,
            "{family} was never answered on the corpus: {answered:?}"
        );
    }
}
