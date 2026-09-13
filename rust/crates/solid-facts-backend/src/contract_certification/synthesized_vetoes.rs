//! Synthesized vetoes (ADR 0036 § 3): a recipe module the checker derives from
//! an export's Type Facts call signature — or its complete overload set — for a
//! proposable closure candidate no hand recipe addresses.
//!
//! A synthesized module is an **input to the veto, never evidence for the
//! closure** — exactly the standing of a hand recipe. It imports the export
//! from the package under test, marks the call window, calls the export on a
//! finite sample drawn from the signature facts, and emits the falsification
//! marker when it observes the domain's contradiction: a non-`undefined`
//! result for `returns: []` (exact), an own-property addition to `globalThis`
//! during the window for `creates: []` (the checked hand corpora's convention,
//! and not an exact observation — the module's coverage limitation says so).
//! ADR 0096 also observes a whole-parameter return: each normally completed
//! call must return the original argument under `Object.is`. This observation
//! says nothing about mutations to the argument's contents.
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
use solid_reactive_ir::contract_semantics::{
    ClaimDomain, ExportSemantics, OperationKind, ValueShape,
};

use super::ProbeHarnessConfiguration;
use super::type_facts::VerifiedTypeFactsEvidence;
use super::{CertificationPlan, WITHHELD_CLOSURE_NO_RECIPE, WithheldClosure};

static SYNTHESIZED_CORPUS_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

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
/// want of a recipe and whose export stated a call signature or a complete
/// overload set. `None` when nothing could be synthesized, so the caller keeps
/// the hand corpus.
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
            let case = plan
                .selected_candidate
                .artifact_cases()
                .iter()
                .find(|case| case.id.as_str() == record.artifact_case)?;
            let export = case.exports.get(&record.export)?;
            let observation = candidate_observation(&record.domain, export)?;
            let signatures = evidence.call_signatures(&record.export)?;
            if let Observation::ParameterReturn(index) = observation
                && !identity_signatures_supported(signatures, index)
            {
                return None;
            }
            Some((record, signatures, observation))
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
    // The name carries a process-unique counter beside the identity: two graph
    // nodes that are importer variants of one artifact share a demand-graph
    // root, and a name keyed on the root alone made the second synthesis wipe
    // the first node's directory and the first drop delete the second node's
    // corpus from under its gate.
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-synthesized-corpus-{}-{}-{:.16x}",
        std::process::id(),
        SYNTHESIZED_CORPUS_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        identity.finalize()
    ));
    create_private_directory(&directory)?;

    // Every hand module and entry, verbatim.
    for entry in &hand_entries {
        if let Some(module) = entry.get("module").and_then(serde_json::Value::as_str) {
            std::fs::copy(base_dir.join(module), directory.join(module))?;
        }
    }
    let mut entries = hand_entries;
    let specifier = plan.resolved_import.specifier.as_str();
    for (record, signatures, observation) in candidates {
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
        let source = module_source(specifier, &record.export, observation, signatures);
        std::fs::write(directory.join(&module), source)?;
        let reviewed = observation.reviewed();
        entries.push(serde_json::json!({
            "claimId": record.semantic_claim_id,
            "module": module,
            "importKind": import_kind,
            "dependencySpecifiers": [],
            "scenario": "operation",
            "expectedEvent": { "marker": reviewed.marker, "class": "call" },
            "drain": [{ "kind": "microtasks", "maxTurns": 1 }],
            "coverageLimitations": [
                format!(
                    "synthesized veto (ADR 0036) for {} `{}`: a finite sample of {} call(s) derived from the export's Type Facts call signature{}; a throwing sample call is recorded as a sample-threw event and observes nothing",
                    record.domain,
                    record.export,
                    observation.sample_tuples(signatures).len(),
                    if signatures.len() > 1 {
                        format!(" ({} overloads, every one sampled)", signatures.len())
                    } else {
                        String::new()
                    }
                ),
                format!(
                    "{} contradiction observed as: {}",
                    record.domain, reviewed.observation
                ),
                observation.sampling_limitations(),
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

/// The argument tuples for an export: every overload's tuples in declaration
/// order, each distinct tuple once. An overloaded export is sampled under every
/// overload — a contradiction one call shape provokes is a contradiction — and
/// the cap applies per overload, so no overload is starved by another's width.
fn sample_tuples(signatures: &[typefacts::SelectedSignature]) -> Vec<Vec<String>> {
    let mut tuples = Vec::new();
    for signature in signatures {
        for tuple in signature_sample_tuples(signature) {
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
    }
    tuples
}

/// One signature's argument tuples: tuple `i` takes each slot's `i`-th
/// candidate, cycling, so every candidate of every slot is exercised at least
/// once within the cap.
fn signature_sample_tuples(signature: &typefacts::SelectedSignature) -> Vec<Vec<String>> {
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

/// Observations are selected from the exact normalized claim, not just its
/// domain. An empty-return observation would contradict a permitted value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Observation {
    Creates,
    EmptyReturns,
    ParameterReturn(u16),
    /// The `callbacks: []` claim. Observed from inside the sampled callback
    /// rather than at a checkpoint after the sample loop: a synthesized entry
    /// drains microtasks *after* `runProbeSession` returns, so a queued
    /// invocation lands after any post-loop check would have run, and a missed
    /// invocation is a clean non-observation that certifies the very claim it
    /// should have contradicted.
    EmptyCallbacks,
}

fn candidate_observation(domain: &str, export: &ExportSemantics) -> Option<Observation> {
    match domain {
        "creates" => Some(Observation::Creates),
        // `Callbacks` carries `KnowledgeSet<CallbackInvocation>` of its own and
        // is not an operation claim, so it is read through its own accessor.
        "callbacks" => export
            .callbacks()
            .items()
            .is_empty()
            .then_some(Observation::EmptyCallbacks),
        "returns" => {
            let claim = export.operation_claim(ClaimDomain::Returns)?;
            if claim.items().is_empty() {
                return Some(Observation::EmptyReturns);
            }
            let [id] = claim.items() else { return None };
            let operation = export.operation(&id.0)?;
            match &operation.output {
                Some(ValueShape::Parameter { index, path })
                    if operation.kind == OperationKind::Return && path.is_empty() =>
                {
                    Some(Observation::ParameterReturn(*index))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A rest parameter denotes a newly collected array, not one argument slot.
/// Require each overload to describe the same ordinary positional input.
fn identity_signatures_supported(signatures: &[typefacts::SelectedSignature], index: u16) -> bool {
    !signatures.is_empty()
        && signatures.iter().all(|signature| {
            signature.parameters.get(usize::from(index)).is_some()
                && signature
                    .parameters
                    .iter()
                    .enumerate()
                    .all(|(position, parameter)| {
                        parameter.index == position
                            && (position > usize::from(index) || !parameter.rest)
                            && (parameter.rest || !identity_slot_candidates(parameter).is_empty())
                    })
        })
}

impl Observation {
    fn reviewed(self) -> ReviewedObservation {
        match self {
            Self::Creates => reviewed_observation("creates").unwrap(),
            Self::EmptyReturns => reviewed_observation("returns").unwrap(),
            Self::EmptyCallbacks => reviewed_observation("callbacks").unwrap(),
            Self::ParameterReturn(_) => ReviewedObservation {
                marker: "return-outside-identity",
                observation: "exact on a normal completion: Object.is(result, original argument at the claimed index) is false; object contents and throwing calls are not observed",
                emit: "",
            },
        }
    }

    fn sample_tuples(self, signatures: &[typefacts::SelectedSignature]) -> Vec<Vec<String>> {
        match self {
            Self::ParameterReturn(index) => identity_sample_tuples(signatures, index),
            _ => sample_tuples(signatures),
        }
    }

    fn sampling_limitations(self) -> &'static str {
        match self {
            Self::ParameterReturn(_) => {
                "identity samples use distinct object/callable identities and vary the returned slot separately; at most twelve tuples per overload, no variadic tail or structural object construction; throwing-only runs are incomplete and cannot satisfy the veto"
            }
            _ => {
                "at most six tuples per overload; no variadic tail or structural object construction"
            }
        }
    }
}

fn identity_slot_candidates(parameter: &typefacts::SelectedParameter) -> Vec<String> {
    let (mut candidates, literals) = slot_candidates(parameter);
    let primitive = &parameter.value.primitive;
    // The older empty-domain sampler has no symbol sample and therefore uses
    // its unknown-input fallback for a symbol-only slot. Here a real symbol
    // is available: do not probe off-domain undefined/object values instead.
    if primitive.may_be_symbol
        && !primitive.may_be_undefined
        && !primitive.may_be_object
        && !parameter.optional
        && !parameter.defaulted
        && candidates == [Sample::Undefined, Sample::Object]
    {
        candidates.clear();
    }
    let literal_only = parameter.value.partitions.iter().any(|partition| {
        partition.axis == typefacts::FinitePartitionAxis::Literal && partition.complete
    });
    let mut values = candidates
        .into_iter()
        .filter(|sample| {
            !literal_only
                || matches!(sample, Sample::Literal(_))
                || (matches!(sample, Sample::Undefined)
                    && (parameter.optional || parameter.defaulted))
        })
        .map(|sample| match sample {
            Sample::Str => format!("\"identity-slot-{}\"", parameter.index),
            Sample::Number => (parameter.index + 1).to_string(),
            Sample::BigInt => format!("{}n", parameter.index + 1),
            Sample::Callable => "(() => undefined)".into(),
            _ => render(sample, &literals),
        })
        .collect::<Vec<_>>();
    if !literal_only {
        if primitive.may_be_number {
            values.push("-0".into());
            if !primitive.numbers_finite {
                values.push("NaN".into());
            }
        }
        if primitive.may_be_boolean {
            values.push("false".into());
        }
        if primitive.may_be_symbol {
            values.push(format!("Symbol(\"identity-slot-{}\")", parameter.index));
        }
    }
    values
}

fn identity_sample_tuples(
    signatures: &[typefacts::SelectedSignature],
    index: u16,
) -> Vec<Vec<String>> {
    let mut tuples = Vec::new();
    for signature in signatures {
        let slots = signature
            .parameters
            .iter()
            .filter(|parameter| !parameter.rest)
            .map(identity_slot_candidates)
            .collect::<Vec<_>>();
        if slots.iter().any(Vec::is_empty) || usize::from(index) >= slots.len() {
            continue;
        }
        let widest = slots
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(1)
            .min(MAX_SAMPLE_CALLS);
        let baseline = slots
            .iter()
            .map(|values| values[0].clone())
            .collect::<Vec<_>>();
        for round in 0..widest {
            let tuple = slots
                .iter()
                .map(|values| values[round % values.len()].clone())
                .collect();
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
        // Cycling all slots together misses, for example, returning the wrong
        // boolean parameter when both slots receive true and then false.
        for value in slots[usize::from(index)].iter().take(MAX_SAMPLE_CALLS) {
            let mut tuple = baseline.clone();
            tuple[usize::from(index)] = value.clone();
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
    }
    tuples
}

/// What a synthesized module emits as the domain's contradiction, and how the
/// coverage limitation describes it — for the domains whose observation has
/// been **reviewed**, and for no others.
///
/// There is deliberately no fallback arm. Until this returned `None` for an
/// unknown domain, a domain added to
/// [`solid_reactive_ir::contract_semantics::ClaimDomain::PROPOSABLE`] would
/// have inherited the `creates` observation silently: its candidates would
/// have been gated by a veto watching `globalThis` for a claim about cleanup
/// registration or invalidation, and a clean run would have read as "nothing
/// contradicted it". A domain whose contradiction nobody has reviewed
/// synthesizes nothing, so its candidate stays withheld for want of a recipe
/// and a hand recipe remains the only way to gate it.
///
/// Adding a domain here is an ADR: the observation has to be stated, and its
/// exactness has to be stated with it — `returns` is exact, `creates` is
/// explicitly not.
struct ReviewedObservation {
    marker: &'static str,
    observation: &'static str,
    emit: &'static str,
}

fn reviewed_observation(domain: &str) -> Option<ReviewedObservation> {
    match domain {
        "returns" => Some(ReviewedObservation {
            marker: "return-value",
            observation: "exact: any call whose result is not undefined",
            emit: "  if (yielded) harness.emit({ marker: \"return-value\", kind: \"call\", phase: \"enter\" });",
        }),
        "callbacks" => Some(ReviewedObservation {
            marker: "callback-invocation",
            observation: "exact: a callable argument the sample supplied was invoked, at any time up to the end of the session's drain",
            emit: "",
        }),
        "creates" => Some(ReviewedObservation {
            marker: "create-operation",
            observation: "own-property additions to globalThis during the call window; not an exact observation of a create operation",
            emit: "  if (Reflect.ownKeys(globalThis).some((key) => !before.includes(key))) {\n    harness.emit({ marker: \"create-operation\", kind: \"call\", phase: \"enter\" });\n  }",
        }),
        _ => None,
    }
}

fn module_source(
    specifier: &str,
    export: &str,
    observation: Observation,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    if let Observation::ParameterReturn(index) = observation {
        return identity_module_source(specifier, export, index, signatures);
    }
    let domain = match observation {
        Observation::Creates => "creates",
        Observation::EmptyReturns => "returns",
        Observation::EmptyCallbacks => "callbacks",
        Observation::ParameterReturn(_) => unreachable!(),
    };
    // Only this observation emits from inside the callback. Giving every
    // synthesized module that emitter would spend the session's event budget on
    // a marker no other gate matches.
    let emits_on_invocation = observation == Observation::EmptyCallbacks;
    let tuples = sample_tuples(signatures)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    let observation = reviewed_observation(domain)
        .expect("a module is only synthesized for a domain with a reviewed observation")
        .emit;
    format!(
        "// Synthesized veto (ADR 0036) for the `{domain}: []` claim of `{export}`.\n\
         // Derived from the export's Type Facts call signature; deterministic in it.\n\
         // It observes, it never proves: the implementation census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         {emitter_binding}\
         const invoked = {{ count: 0 }};\n\
         const callback = () => {{\n  invoked.count += 1;\n{callback_emit}}};\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         {emitter_arm}\
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
        emitter_binding = if emits_on_invocation {
            "let emitter = null;\n"
        } else {
            ""
        },
        callback_emit = if emits_on_invocation {
            "  if (invoked.count === 1 && emitter) {\n    emitter.emit({ marker: \"callback-invocation\", kind: \"call\", phase: \"enter\" });\n  }\n"
        } else {
            ""
        },
        emitter_arm = if emits_on_invocation {
            "  emitter = harness;\n"
        } else {
            ""
        },
    )
}

fn identity_module_source(
    specifier: &str,
    export: &str,
    index: u16,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let tuples = identity_sample_tuples(signatures, index)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"// Synthesized whole-parameter return veto (ADR 0096).
// Finite observations only falsify; the authenticated census proves closure.
import * as subjectModule from {specifier};
const subject = subjectModule[{export}];
// SameValue without reading a mutable intrinsic after the subject's import.
const sameValue = (left, right) => left === right
  ? left !== 0 || 1 / left === 1 / right
  : left !== left && right !== right;
const samples = [
{tuples}
];
export async function runProbeSession(_session, harness) {{
  harness.emit({{ marker: "call", kind: "call", phase: "enter" }});
  if (typeof subject !== "function") throw new Error("synthesized veto: export is not callable");
  let completed = 0;
  let threw = 0;
  for (const args of samples) {{
    const expected = args[{index}];
    let result;
    try {{ result = subject(...args); }} catch {{ threw += 1; continue; }}
    completed += 1;
    if (!sameValue(result, expected)) {{
      harness.emit({{ marker: "return-outside-identity", kind: "call", phase: "enter" }});
    }}
  }}
  if (threw > 0) harness.emit({{ marker: "sample-threw", kind: "call", phase: "enter" }});
  if (completed === 0) throw new Error("synthesized identity veto: no sample completed normally");
  harness.emit({{ marker: "call", kind: "call", phase: "exit" }});
}}
"#,
        specifier = serde_json::to_string(specifier).unwrap(),
        export = serde_json::to_string(export).unwrap(),
    )
}

#[cfg(test)]
#[path = "synthesized_vetoes_tests.rs"]
mod adversarial_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a domain whose contradiction has been reviewed synthesizes a veto,
    /// and each states its own observation. This is the gate that keeps a
    /// domain added to `ClaimDomain::PROPOSABLE` from silently inheriting the
    /// `creates` convention: a `cleanups` candidate gated by a veto watching
    /// `globalThis` would report "nothing contradicted it" for a claim that
    /// observation says nothing about.
    #[test]
    fn only_a_reviewed_domain_synthesizes_a_veto() {
        let returns = reviewed_observation("returns").expect("returns is reviewed");
        assert_eq!(returns.marker, "return-value");
        assert!(returns.observation.starts_with("exact:"));

        let creates = reviewed_observation("creates").expect("creates is reviewed");
        assert_eq!(creates.marker, "create-operation");
        assert!(
            creates.observation.contains("not an exact observation"),
            "the creates convention states its own inexactness: {}",
            creates.observation
        );
        assert!(creates.emit.contains("globalThis"));

        let callbacks = reviewed_observation("callbacks").expect("callbacks is reviewed");
        assert_eq!(callbacks.marker, "callback-invocation");
        assert!(callbacks.observation.starts_with("exact:"));
        assert!(
            callbacks.emit.is_empty(),
            "the callbacks observation emits from inside the callback, not at a checkpoint"
        );

        for domain in [
            "cleanups",
            "disposals",
            "invalidates",
            "reads",
            "writes",
            "throws",
            "",
        ] {
            assert!(
                reviewed_observation(domain).is_none(),
                "{domain} has no reviewed observation and must synthesize nothing"
            );
        }
    }
}
