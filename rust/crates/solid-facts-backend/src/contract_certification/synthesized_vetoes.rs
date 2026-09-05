//! Synthesized vetoes (ADR 0036 § 3): a recipe module the checker derives from
//! an export's Type Facts call signature for a proposable closure candidate no
//! hand recipe addresses.
//!
//! A synthesized module is an **input to the veto, never evidence for the
//! closure** — exactly the standing of a hand recipe. It imports the export
//! from the package under test, marks the call window, calls the export on a
//! finite sample drawn from the signature facts, and emits the falsification
//! marker when it observes the domain's contradiction: a non-`undefined`
//! result for `returns: []` (exact), an own-property addition to `globalThis`
//! during the window for `creates: []` (the checked hand corpora's convention,
//! and not an exact observation — the module's coverage limitation says so).
//!
//! The merged corpus — every hand module and manifest entry copied verbatim,
//! plus the synthesized entries marked `provenance: "synthesized"` — is
//! written to a private directory of the transaction and loaded through the
//! same [`super::probe_harness::RecipeCorpus`] path as any corpus, so Rust
//! derives every construction digest from the bytes it copied. A hand recipe
//! always wins for a claim id it addresses. Nothing here hands `session` or
//! `harness` to the package: the recording callable closes over a counter.

use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use super::ProbeHarnessConfiguration;
use super::type_facts::VerifiedTypeFactsEvidence;
use super::{CertificationPlan, WITHHELD_CLOSURE_NO_RECIPE, WithheldClosure};

/// The transaction's merged corpus. Removed with the value.
pub(crate) struct SynthesizedCorpus {
    directory: PathBuf,
    configuration: ProbeHarnessConfiguration,
}

impl SynthesizedCorpus {
    pub(crate) fn configuration(&self) -> &ProbeHarnessConfiguration {
        &self.configuration
    }
}

impl Drop for SynthesizedCorpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VetoSynthesisError {
    #[error("synthesized veto corpus could not be written: {0}")]
    Io(#[from] std::io::Error),
    #[error("the hand recipe corpus manifest could not be read for merging: {0}")]
    Manifest(String),
}

/// Synthesizes a veto for every candidate in `withheld` that was withheld for
/// want of a recipe and whose export stated a unique call signature. `None`
/// when nothing could be synthesized, so the caller keeps the hand corpus.
pub(crate) fn synthesize(
    plan: &CertificationPlan,
    evidence: &VerifiedTypeFactsEvidence,
    base: &ProbeHarnessConfiguration,
    withheld: &[WithheldClosure],
) -> Result<Option<SynthesizedCorpus>, VetoSynthesisError> {
    let candidates = withheld
        .iter()
        .filter(|record| record.reason == WITHHELD_CLOSURE_NO_RECIPE)
        .filter_map(|record| {
            evidence
                .call_signature(&record.export)
                .map(|signature| (record, signature))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    let base_dir = base.recipe_corpus();
    let manifest_bytes = std::fs::read(base_dir.join("recipes.json"))
        .map_err(|error| VetoSynthesisError::Manifest(error.to_string()))?;
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| VetoSynthesisError::Manifest(error.to_string()))?;
    let hand_entries = manifest
        .get("recipes")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| VetoSynthesisError::Manifest("manifest has no recipes array".into()))?;
    let import_kind = hand_entries
        .first()
        .and_then(|entry| entry.get("importKind"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("esm")
        .to_owned();

    let mut identity = Sha256::new();
    identity.update(plan.demand_graph().root().as_str().as_bytes());
    identity.update(base_dir.as_os_str().as_encoded_bytes());
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-synthesized-corpus-{}-{:.16x}",
        std::process::id(),
        identity.finalize()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    create_private_directory(&directory)?;

    // Every hand module and entry, verbatim.
    for entry in &hand_entries {
        if let Some(module) = entry.get("module").and_then(serde_json::Value::as_str) {
            std::fs::copy(base_dir.join(module), directory.join(module))?;
        }
    }
    let mut entries = hand_entries;
    let specifier = plan.resolved_import.specifier.as_str();
    for (record, signature) in candidates {
        let module = format!(
            "synthesized-{}.mjs",
            record
                .semantic_claim_id
                .rsplit(':')
                .next()
                .unwrap_or("claim")
                .chars()
                .take(16)
                .collect::<String>()
        );
        let source = module_source(specifier, &record.export, &record.domain, signature);
        std::fs::write(directory.join(&module), source)?;
        let (marker, observation) = match record.domain.as_str() {
            "returns" => (
                "return-value",
                "exact: any call whose result is not undefined",
            ),
            _ => (
                "create-operation",
                "own-property additions to globalThis during the call window; not an exact observation of a create operation",
            ),
        };
        entries.push(serde_json::json!({
            "claimId": record.semantic_claim_id,
            "module": module,
            "importKind": import_kind,
            "dependencySpecifiers": [],
            "scenario": "operation",
            "expectedEvent": { "marker": marker, "class": "call" },
            "drain": [{ "kind": "microtasks", "maxTurns": 1 }],
            "coverageLimitations": [
                format!(
                    "synthesized veto (ADR 0036) for {} `{}`: a finite sample of {} call(s) derived from the export's Type Facts call signature; a throwing sample call is recorded as a sample-threw event and observes nothing",
                    record.domain, record.export, sample_count(signature)
                ),
                format!("{} contradiction observed as: {}", record.domain, observation),
            ],
            "provenance": "synthesized",
        }));
    }
    manifest["recipes"] = serde_json::Value::Array(entries);
    std::fs::write(
        directory.join("recipes.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).expect("manifest serializes")
        ),
    )?;
    Ok(Some(SynthesizedCorpus {
        configuration: base.with_recipe_corpus(&directory),
        directory,
    }))
}

fn create_private_directory(directory: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(directory)
    }
}

/// One JavaScript value the sample can hand to a parameter slot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sample {
    Undefined,
    Null,
    Str,
    Number,
    Boolean,
    BigInt,
    Object,
    Array,
    Callable,
    Literal(usize),
}

/// The candidate values for one parameter slot, from its value facts. Every
/// list is nonempty: a slot the facts say nothing about is sampled with the
/// two values most callables accept without throwing on arrival.
fn slot_candidates(parameter: &typefacts::SelectedParameter) -> (Vec<Sample>, Vec<String>) {
    let value = &parameter.value;
    let mut candidates = Vec::new();
    let mut literals = Vec::new();
    for partition in &value.partitions {
        if partition.axis != typefacts::FinitePartitionAxis::Literal {
            continue;
        }
        for case in &partition.cases {
            if let Some(literal) = &case.literal {
                let rendered = match literal.kind {
                    typefacts::PrimitiveLiteralKind::String => {
                        serde_json::to_string(literal.string.as_ref()).unwrap_or_default()
                    }
                    typefacts::PrimitiveLiteralKind::Number => {
                        if literal.number.is_finite() {
                            format!("{}", literal.number)
                        } else {
                            continue;
                        }
                    }
                    typefacts::PrimitiveLiteralKind::Boolean => literal.boolean.to_string(),
                };
                candidates.push(Sample::Literal(literals.len()));
                literals.push(rendered);
            }
        }
    }
    if matches!(
        value.callability,
        typefacts::Callability::Callable
            | typefacts::Callability::UntypedCallable
            | typefacts::Callability::Mixed
    ) {
        candidates.push(Sample::Callable);
    }
    let primitive = &value.primitive;
    if primitive.may_be_string {
        candidates.push(Sample::Str);
    }
    if primitive.may_be_number {
        candidates.push(Sample::Number);
    }
    if primitive.may_be_boolean {
        candidates.push(Sample::Boolean);
    }
    if primitive.may_be_big_int {
        candidates.push(Sample::BigInt);
    }
    if primitive.may_be_object && !candidates.contains(&Sample::Callable) {
        candidates.push(Sample::Object);
        // A tuple partition is the one fact that says "an array goes here".
        if value
            .partitions
            .iter()
            .any(|partition| partition.axis == typefacts::FinitePartitionAxis::Tuple)
        {
            candidates.push(Sample::Array);
        }
    }
    if primitive.may_be_null {
        candidates.push(Sample::Null);
    }
    if primitive.may_be_undefined || parameter.optional || parameter.defaulted {
        candidates.push(Sample::Undefined);
    }
    if candidates.is_empty() {
        candidates.push(Sample::Undefined);
        candidates.push(Sample::Object);
    }
    (candidates, literals)
}

fn render(sample: Sample, literals: &[String]) -> String {
    match sample {
        Sample::Undefined => "undefined".into(),
        Sample::Null => "null".into(),
        Sample::Str => "\"x\"".into(),
        Sample::Number => "1".into(),
        Sample::Boolean => "true".into(),
        Sample::BigInt => "1n".into(),
        Sample::Object => "{}".into(),
        Sample::Array => "[]".into(),
        Sample::Callable => "callback".into(),
        Sample::Literal(index) => literals[index].clone(),
    }
}

const MAX_SAMPLE_CALLS: usize = 6;

/// The argument tuples: tuple `i` takes each slot's `i`-th candidate, cycling,
/// so every candidate of every slot is exercised at least once within the cap.
fn sample_tuples(signature: &typefacts::SelectedSignature) -> Vec<Vec<String>> {
    let slots = signature
        .parameters
        .iter()
        .filter(|parameter| !parameter.rest)
        .map(slot_candidates)
        .collect::<Vec<_>>();
    let widest = slots
        .iter()
        .map(|(candidates, _)| candidates.len())
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_SAMPLE_CALLS);
    (0..widest)
        .map(|index| {
            slots
                .iter()
                .map(|(candidates, literals)| {
                    render(candidates[index % candidates.len()], literals)
                })
                .collect()
        })
        .collect()
}

fn sample_count(signature: &typefacts::SelectedSignature) -> usize {
    sample_tuples(signature).len()
}

fn module_source(
    specifier: &str,
    export: &str,
    domain: &str,
    signature: &typefacts::SelectedSignature,
) -> String {
    let tuples = sample_tuples(signature)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    let observation = match domain {
        "returns" => {
            "  if (yielded) harness.emit({ marker: \"return-value\", kind: \"call\", phase: \"enter\" });"
        }
        _ => {
            "  if (Reflect.ownKeys(globalThis).some((key) => !before.includes(key))) {\n    harness.emit({ marker: \"create-operation\", kind: \"call\", phase: \"enter\" });\n  }"
        }
    };
    format!(
        "// Synthesized veto (ADR 0036) for the `{domain}: []` claim of `{export}`.\n\
         // Derived from the export's Type Facts call signature; deterministic in it.\n\
         // It observes, it never proves: the implementation census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         const invoked = {{ count: 0 }};\n\
         const callback = () => {{\n  invoked.count += 1;\n}};\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject !== \"function\") {{\n\
         \x20   throw new Error(\"synthesized veto: the export is not callable in this realm\");\n\
         \x20 }}\n\
         \x20 const before = Reflect.ownKeys(globalThis);\n\
         \x20 let yielded = false;\n\
         \x20 let threw = 0;\n\
         \x20 for (const args of samples) {{\n\
         \x20   try {{\n\
         \x20     const result = subject(...args);\n\
         \x20     if (result !== undefined) yielded = true;\n\
         \x20   }} catch {{\n\
         \x20     threw += 1;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 if (threw > 0) harness.emit({{ marker: \"sample-threw\", kind: \"call\", phase: \"enter\" }});\n\
         {observation}\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}
