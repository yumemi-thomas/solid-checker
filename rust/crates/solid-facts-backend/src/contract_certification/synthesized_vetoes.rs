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
//! says nothing about mutations to the argument's contents. ADR 0100 observes
//! a *described* `callbacks` enumeration: each callable slot gets its own
//! recording callable, and the marker fires for an invocation of a slot the
//! enumeration does not describe, or of a described slot outside the sample
//! call's own stack.
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
    ClaimDomain, Event, ExportSemantics, InvokeProtocol, OperationKind, Schedule, ValueShape,
    ValueSource,
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
            // ADR 0103: an export that *is* a reviewed default-library member
            // is served by an identity witness, whether or not the checker
            // could select a signature -- `Object.keys` is overloaded and has
            // several, `Math.floor` has one and no body. The identity decides
            // the claim outright, so it is preferred over any sampled
            // observation for the domains it closes: the three empty ones, and
            // (second 2026-09-24 amendment) `returns` over the one return the
            // member's reviewed row states.
            if let Some(alias) = evidence.default_library_alias(&record.export)
                && let Some(index) =
                    super::type_facts::reviewed_default_library_alias_index(&alias.qualified_name())
                && match record.domain.as_str() {
                    "reads" | "creates" | "callbacks" => empty_enumeration(&record.domain, export),
                    "returns" => super::type_facts::reviewed_default_library_alias_return(
                        &alias.qualified_name(),
                    )
                    .is_some_and(|shape| {
                        export
                            .operation_claim(ClaimDomain::Returns)
                            .and_then(|claim| match claim.items() {
                                [id] => export.operation(&id.0),
                                _ => None,
                            })
                            .is_some_and(|operation| {
                                operation.kind == OperationKind::Return
                                    && operation.output.as_ref() == Some(&shape)
                            })
                    }),
                    _ => false,
                }
            {
                return Some((record, &[][..], Observation::DefaultLibraryAlias(index)));
            }
            let Some(signatures) = evidence.call_signatures(&record.export) else {
                // ADR 0099: no call signature, but the producer stated the
                // value cannot be invoked. The veto observes `typeof`; it
                // needs no sample and serves every empty proposable domain.
                evidence.not_callable_value(&record.export)?;
                if !empty_enumeration(&record.domain, export) {
                    return None;
                }
                return Some((record, &[][..], Observation::NotCallable));
            };
            let observation = candidate_observation(&record.domain, export)?;
            if let Observation::ParameterReturn(index) = observation
                && !identity_signatures_supported(signatures, index)
            {
                return None;
            }
            if let Observation::ArgumentContainers(set) = observation
                && !set
                    .indices()
                    .into_iter()
                    .all(|index| identity_signatures_supported(signatures, index))
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
                if observation == Observation::NotCallable {
                    format!(
                        "synthesized veto (ADR 0099) for {} `{}`: the export's value type states no call or construct signature, so no call is sampled; the module observes typeof of the runtime value",
                        record.domain, record.export
                    )
                } else {
                    format!(
                        "synthesized veto ({}) for {} `{}`: a finite sample of {} call(s) derived from the export's Type Facts call signature{}; a throwing sample call is recorded as a sample-threw event and observes nothing",
                        match observation {
                            Observation::DescribedCallbacks(_) => {
                                "ADR 0100, described callbacks enumeration"
                            }
                            Observation::DescribedProtocols(_) => {
                                "ADR 0100 per protocol, described callbacks enumeration with non-call items"
                            }
                            Observation::DescribedMembers(..) => {
                                "ADR 0100 per protocol and member, described callbacks enumeration with member calls"
                            }
                            Observation::DescribedReads(_) => {
                                "ADR 0101, described reads enumeration"
                            }
                            Observation::PrimitiveReturn => "ADR 0113, primitive return",
                            Observation::ArgumentContainers(_) => {
                                "ADR 0115, argument container returns"
                            }
                            _ => "ADR 0036",
                        },
                        record.domain,
                        record.export,
                        observation.sample_tuples(signatures).len(),
                        if signatures.len() > 1 {
                            format!(" ({} overloads, every one sampled)", signatures.len())
                        } else {
                            String::new()
                        }
                    )
                },
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
    render_at(sample, literals, None)
}

/// [`render`], with a callable slot rendered as `callbackAt(<slot>)` when the
/// module records per slot (ADR 0100) rather than through the one shared
/// `callback` (ADR 0036).
fn render_at(sample: Sample, literals: &[String], slot: Option<usize>) -> String {
    match sample {
        Sample::Undefined => "undefined".into(),
        Sample::Null => "null".into(),
        Sample::Str => "\"x\"".into(),
        Sample::Number => "1".into(),
        Sample::Boolean => "true".into(),
        Sample::BigInt => "1n".into(),
        Sample::Object => "{}".into(),
        Sample::Array => "[]".into(),
        Sample::Callable => {
            slot.map_or_else(|| "callback".into(), |slot| format!("callbackAt({slot})"))
        }
        Sample::Literal(index) => literals[index].clone(),
    }
}

const MAX_SAMPLE_CALLS: usize = 6;

/// The argument tuples for an export: every overload's tuples in declaration
/// order, each distinct tuple once. An overloaded export is sampled under every
/// overload — a contradiction one call shape provokes is a contradiction — and
/// the cap applies per overload, so no overload is starved by another's width.
fn sample_tuples(signatures: &[typefacts::SelectedSignature]) -> Vec<Vec<String>> {
    sample_tuples_with(signatures, false)
}

/// [`sample_tuples`], rendering each callable slot as its own recording
/// callable when `per_slot` is set (ADR 0100).
fn sample_tuples_with(
    signatures: &[typefacts::SelectedSignature],
    per_slot: bool,
) -> Vec<Vec<String>> {
    let mut tuples = Vec::new();
    for signature in signatures {
        for tuple in signature_sample_tuples(signature, per_slot) {
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
fn signature_sample_tuples(
    signature: &typefacts::SelectedSignature,
    per_slot: bool,
) -> Vec<Vec<String>> {
    let slots = signature
        .parameters
        .iter()
        .filter(|parameter| !parameter.rest)
        .map(|parameter| (parameter.index, slot_candidates(parameter)))
        .collect::<Vec<_>>();
    let widest = slots
        .iter()
        .map(|(_, (candidates, _))| candidates.len())
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_SAMPLE_CALLS);
    (0..widest)
        .map(|index| {
            slots
                .iter()
                .map(|(slot, (candidates, literals))| {
                    render_at(
                        candidates[index % candidates.len()],
                        literals,
                        per_slot.then_some(*slot),
                    )
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
    /// ADR 0113: one `return` whose output is `plain`, closed over a primitive
    /// completion. The contradiction is a normal completion handing back an
    /// object other than `null`, or a function -- the one thing a primitive
    /// completion rules out. An empty-return observation would contradict
    /// every value this claim permits, so it is never substituted.
    PrimitiveReturn,
    /// ADR 0115: returns that each hand back the caller's own argument or a
    /// fresh array of the caller's arguments. The contradiction is a normal
    /// completion whose result is none of them: not the argument at a claimed
    /// index by SameValue, and not an object whose `length` and elements are
    /// the claimed arguments by SameValue in order. ADR 0116 adds what an
    /// invocation of the argument at a claimed index returned: a token one of
    /// the recording functions sampled into that slot handed back during the
    /// same sample call.
    ArgumentContainers(ContainerSet),
    /// The `callbacks: []` claim. Observed from inside the sampled callback
    /// rather than at a checkpoint after the sample loop: a synthesized entry
    /// drains microtasks *after* `runProbeSession` returns, so a queued
    /// invocation lands after any post-loop check would have run, and a missed
    /// invocation is a clean non-observation that certifies the very claim it
    /// should have contradicted.
    EmptyCallbacks,
    /// ADR 0100: a described `callbacks` enumeration, every item `from` a bare
    /// parameter `at` the call event on the same stack. The bits are the
    /// described parameter indices — a set small enough to be `Copy`, and a
    /// proposal naming an index the mask cannot hold is not synthesized. The
    /// contradiction is an invocation of a callable at a slot outside the
    /// set, or of one inside it after the sample call has returned.
    DescribedCallbacks(u64),
    /// Item A of ways-to-improve § 3.3: a described `callbacks` enumeration
    /// with at least one non-call item (a property read, iteration, coercion or
    /// `hasInstance` of a bare parameter). Every described slot, and every
    /// slot whose sample is an object, array or callable, is a recording
    /// `Proxy` whose traps classify each use by protocol and return the
    /// target's own answer (`Reflect.*`), so `.length` reads 0 and `access`
    /// takes its call branch. The contradiction is a use of a slot by a
    /// protocol the enumeration does not describe for that slot, at any time up
    /// to the end of the drain, or a described use outside the sample call.
    /// An enumeration with no non-call item is `DescribedCallbacks`, byte for
    /// byte as before.
    DescribedProtocols(ProtocolMasks),
    /// Item B of ways-to-improve § 3.3: a described `callbacks` enumeration
    /// with at least one call item from a member of a parameter at an index
    /// (`handler[0](…)`). The `DescribedProtocols` module, with every slot a
    /// member item names carrying a recording callable at each index up to one
    /// past the highest described one, so the call of a described member
    /// inside the sample call is the item, and a call of any other of them, or
    /// of a described one outside the call, is the contradiction. An
    /// enumeration with no member item is one of the two variants above, byte
    /// for byte as before; a member item at a property key is not synthesized.
    DescribedMembers(ProtocolMasks, MemberCalls),
    /// ADR 0101: a described `reads` enumeration, every item a `read` of a
    /// caller parameter -- the generator's `parameter-member` row for
    /// `props.of.values()` -- `at` the call event on the same stack. The bits
    /// are the described parameter indices, as for `DescribedCallbacks`. The
    /// contradiction is an invocation of a *member* of a caller-supplied value
    /// at a slot outside the set, or of one at a described slot after the
    /// sample call has returned. Which member is not observed: the census
    /// confirms the paths, the module tells the slots apart. A bare call of
    /// the slot itself is the `callbacks` domain's item and is not observed
    /// here.
    DescribedReads(u64),
    /// ADR 0099: the export's value cannot be invoked, per the producer, so
    /// every empty proposable call domain closes vacuously. The runtime half
    /// observes `typeof` of the exported value and emits when it is a
    /// function: the one way the stated fact could be false at run time.
    NotCallable,
    /// ADR 0103: the export *is* a reviewed default-library member. The
    /// contradiction is an exact identity witness rather than a sample —
    /// `Object.is(subject, Container.member)` is false in the probe realm —
    /// which is stronger than any finite call could be: it decides the whole
    /// claim in one comparison instead of probing behaviour one tuple at a
    /// time. The index is into `REVIEWED_DEFAULT_LIBRARY_ALIASES`, so a
    /// variant can never name a member the certifier has not reviewed.
    DefaultLibraryAlias(u16),
}

/// Whether the proposal closes `domain` with an empty enumeration -- the only
/// shape a not-callable export's vacuous closure reaches (ADR 0099).
fn empty_enumeration(domain: &str, export: &ExportSemantics) -> bool {
    match domain {
        "callbacks" => export.callbacks().items().is_empty(),
        "creates" => export
            .operation_claim(ClaimDomain::Creates)
            .is_some_and(|claim| claim.items().is_empty()),
        "returns" => export
            .operation_claim(ClaimDomain::Returns)
            .is_some_and(|claim| claim.items().is_empty()),
        "reads" => export
            .operation_claim(ClaimDomain::Reads)
            .is_some_and(|claim| claim.items().is_empty()),
        _ => false,
    }
}

fn candidate_observation(domain: &str, export: &ExportSemantics) -> Option<Observation> {
    match domain {
        "creates" => Some(Observation::Creates),
        // `Callbacks` carries `KnowledgeSet<CallbackInvocation>` of its own and
        // is not an operation claim, so it is read through its own accessor.
        "callbacks" => {
            if export.callbacks().items().is_empty() {
                return Some(Observation::EmptyCallbacks);
            }
            let mut masks = ProtocolMasks::default();
            let mut members = MemberCalls::default();
            for item in export.callbacks().items() {
                let ValueSource::Parameter { index, path } = &item.from else {
                    return None;
                };
                if u32::from(*index) >= u64::BITS {
                    return None;
                }
                let operation = export.operation(&item.operation.0)?;
                if operation.kind != OperationKind::Invoke
                    || operation.at != Some(Event::Call)
                    || operation.schedule != Some(Schedule::SameStack)
                    || operation.guard.is_some()
                {
                    return None;
                }
                if !path.is_empty() {
                    // Item B: a call of the member at one index; any other
                    // member item is not synthesized.
                    if operation.is_protocol_invocation() {
                        return None;
                    }
                    let [key] = path.as_slice() else {
                        return None;
                    };
                    members.push(*index, member_index(key)?)?;
                    continue;
                }
                // A non-call item is never a call slot: it is recorded under
                // its own protocol, and its presence selects the Proxy module.
                *masks.of_mut(operation.invoke_protocol()) |= 1 << index;
            }
            Some(if !members.is_empty() {
                Observation::DescribedMembers(masks, members)
            } else if masks.has_protocol_items() {
                Observation::DescribedProtocols(masks)
            } else {
                Observation::DescribedCallbacks(masks.call)
            })
        }
        // The empty `reads` enumeration deliberately registers no observation
        // (`reviewed_observation`): its contradiction is a read of a source the
        // export owns, which no synthesized module can see, so it is served by
        // hand recipes. A described enumeration is different in kind -- its
        // items are member invocations of the caller's own values, which the
        // module can hand in and watch -- and is observed on the same footing
        // as a described `callbacks` enumeration (ADR 0101).
        "reads" => {
            let claim = export.operation_claim(ClaimDomain::Reads)?;
            if claim.items().is_empty() {
                return None;
            }
            let mut mask = 0u64;
            for id in claim.items() {
                let operation = export.operation(&id.0)?;
                let Some(ValueShape::Parameter { index, .. }) = operation.inputs.first() else {
                    return None;
                };
                if u32::from(*index) >= u64::BITS {
                    return None;
                }
                if operation.kind != OperationKind::Read
                    || operation.at != Some(Event::Call)
                    || operation.schedule != Some(Schedule::SameStack)
                    || operation.guard.is_some()
                    || operation.composed_from.is_some()
                {
                    return None;
                }
                mask |= 1 << index;
            }
            Some(Observation::DescribedReads(mask))
        }
        "returns" => {
            let claim = export.operation_claim(ClaimDomain::Returns)?;
            if claim.items().is_empty() {
                return Some(Observation::EmptyReturns);
            }
            let [id] = claim.items() else {
                // ADR 0115 is the one multi-return claim with a reviewed
                // observation.
                return ContainerSet::of(export, claim.items())
                    .map(Observation::ArgumentContainers);
            };
            let operation = export.operation(&id.0)?;
            match &operation.output {
                Some(ValueShape::Parameter { index, path })
                    if operation.kind == OperationKind::Return && path.is_empty() =>
                {
                    Some(Observation::ParameterReturn(*index))
                }
                Some(ValueShape::Plain) if operation.kind == OperationKind::Return => {
                    Some(Observation::PrimitiveReturn)
                }
                _ => ContainerSet::of(export, claim.items()).map(Observation::ArgumentContainers),
            }
        }
        _ => None,
    }
}

/// The member call items of a described `callbacks` enumeration (item B), as
/// `(slot, index)` pairs: at most [`MemberCalls::CAPACITY`], each a slot below
/// 64 and an index no larger than [`MemberCalls::MAX_INDEX`], small enough to
/// stay `Copy`. A claim past that is not synthesized.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MemberCalls {
    len: u8,
    items: [(u8, u8); MemberCalls::CAPACITY],
}

impl MemberCalls {
    const CAPACITY: usize = 4;
    /// The highest index a recording member is installed at is one past the
    /// highest described one, so this bounds the target's size.
    const MAX_INDEX: u8 = 15;

    fn push(&mut self, slot: u16, index: u8) -> Option<()> {
        let slot = u8::try_from(slot)
            .ok()
            .filter(|slot| u32::from(*slot) < u64::BITS)?;
        if index > Self::MAX_INDEX {
            return None;
        }
        if self.items().contains(&(slot, index)) {
            return Some(());
        }
        let position = usize::from(self.len);
        if position >= Self::CAPACITY {
            return None;
        }
        self.items[position] = (slot, index);
        self.len += 1;
        Some(())
    }

    fn items(&self) -> &[(u8, u8)] {
        &self.items[..usize::from(self.len)]
    }

    const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Every slot some member item names, as bits.
    fn slots(self) -> u64 {
        self.items()
            .iter()
            .fold(0, |mask, (slot, _)| mask | (1 << slot))
    }

    /// `new Set(["1:0"])`: the described `(slot, index)` pairs.
    fn javascript_described(self) -> String {
        format!(
            "new Set([{}])",
            self.items()
                .iter()
                .map(|(slot, index)| format!("\"{slot}:{index}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// `{ 1: 0 }`: per member slot, the highest described index.
    fn javascript_tops(self) -> String {
        let mut tops = std::collections::BTreeMap::<u8, u8>::new();
        for (slot, index) in self.items() {
            let top = tops.entry(*slot).or_insert(*index);
            *top = (*top).max(*index);
        }
        format!(
            "{{ {} }}",
            tops.iter()
                .map(|(slot, top)| format!("{slot}: {top}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The index a member item's one key names, when it is a canonical decimal
/// integer no larger than [`MemberCalls::MAX_INDEX`].
fn member_index(key: &str) -> Option<u8> {
    if key.is_empty() || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    key.parse::<u8>()
        .ok()
        .filter(|index| *index <= MemberCalls::MAX_INDEX)
}

/// The slots a described `callbacks` enumeration names, per protocol, as bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProtocolMasks {
    call: u64,
    get: u64,
    iterate: u64,
    coerce: u64,
    has_instance: u64,
}

impl ProtocolMasks {
    fn of_mut(&mut self, protocol: InvokeProtocol) -> &mut u64 {
        match protocol {
            InvokeProtocol::Call => &mut self.call,
            InvokeProtocol::Get => &mut self.get,
            InvokeProtocol::Iterate => &mut self.iterate,
            InvokeProtocol::Coerce => &mut self.coerce,
            InvokeProtocol::HasInstance => &mut self.has_instance,
        }
    }

    const fn has_protocol_items(self) -> bool {
        self.get | self.iterate | self.coerce | self.has_instance != 0
    }

    /// Every slot some item names, whatever its protocol.
    const fn any(self) -> u64 {
        self.call | self.get | self.iterate | self.coerce | self.has_instance
    }

    /// The JavaScript object literal naming each protocol's described slots,
    /// keyed by the protocol's wire name.
    fn javascript(self) -> String {
        let set = |mask: u64| {
            (0..u64::BITS)
                .filter(|bit| mask & (1 << bit) != 0)
                .map(|bit| bit.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "{{ call: new Set([{}]), get: new Set([{}]), iterate: new Set([{}]), coerce: new Set([{}]), \"has-instance\": new Set([{}]) }}",
            set(self.call),
            set(self.get),
            set(self.iterate),
            set(self.coerce),
            set(self.has_instance)
        )
    }
}

/// The containers an ADR 0115 claim enumerates, small enough to stay `Copy`:
/// at most four returns, each the argument at one index or an array of at most
/// four of them. A claim past that is not synthesized, and its candidate stays
/// withheld for want of a recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContainerSet {
    len: u8,
    slots: [ContainerSlot; 4],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ContainerSlot {
    array: bool,
    /// ADR 0116: what an invocation of the argument at `items[0]` returned.
    invocation: bool,
    len: u8,
    items: [u8; 4],
}

impl ContainerSet {
    fn of(
        export: &solid_reactive_ir::contract_semantics::ExportSemantics,
        items: &[solid_reactive_ir::contract_semantics::OperationId],
    ) -> Option<Self> {
        if items.is_empty() || items.len() > 4 {
            return None;
        }
        let mut set = Self {
            len: 0,
            slots: [ContainerSlot::default(); 4],
        };
        for id in items {
            let operation = export.operation(&id.0)?;
            if operation.kind != OperationKind::Return {
                return None;
            }
            let slot = match operation.output.as_ref()? {
                ValueShape::Parameter { index, path } if path.is_empty() => ContainerSlot {
                    array: false,
                    invocation: false,
                    len: 1,
                    items: [u8::try_from(*index).ok()?, 0, 0, 0],
                },
                ValueShape::InvocationResult { parameter } => ContainerSlot {
                    array: false,
                    invocation: true,
                    len: 1,
                    items: [u8::try_from(*parameter).ok()?, 0, 0, 0],
                },
                ValueShape::ArgumentArray { items } if items.len() <= 4 => {
                    let mut slot = ContainerSlot {
                        array: true,
                        invocation: false,
                        len: u8::try_from(items.len()).ok()?,
                        items: [0; 4],
                    };
                    for (position, index) in items.iter().enumerate() {
                        slot.items[position] = u8::try_from(*index).ok()?;
                    }
                    slot
                }
                _ => return None,
            };
            // The census refuses a claim that names one container twice, and
            // so does this.
            if set.slots().contains(&slot) {
                return None;
            }
            set.slots[usize::from(set.len)] = slot;
            set.len += 1;
        }
        // A lone whole parameter is ADR 0096's identity veto, never this one.
        if set.len == 1 && !set.slots[0].array && !set.slots[0].invocation {
            return None;
        }
        Some(set)
    }

    /// The argument indices a claimed invocation result is read from, once
    /// each, ascending (ADR 0116).
    fn invocation_indices(&self) -> Vec<u16> {
        let mut indices = self
            .slots()
            .iter()
            .filter(|slot| slot.invocation)
            .map(|slot| u16::from(slot.items[0]))
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    fn slots(&self) -> &[ContainerSlot] {
        &self.slots[..usize::from(self.len)]
    }

    /// Every argument index the containers name, once each, ascending.
    fn indices(&self) -> Vec<u16> {
        let mut indices = self
            .slots()
            .iter()
            .flat_map(|slot| slot.items[..usize::from(slot.len)].iter().copied())
            .map(u16::from)
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    fn javascript(&self) -> String {
        self.slots()
            .iter()
            .map(|slot| {
                format!(
                    "{{ array: {}, invocation: {}, items: [{}] }}",
                    slot.array,
                    slot.invocation,
                    slot.items[..usize::from(slot.len)]
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
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
            Self::DescribedCallbacks(_) => ReviewedObservation {
                marker: "callback-invocation",
                observation: "exact: a callable argument the sample supplied at a slot the enumeration does not describe was invoked at any time up to the end of the session's drain, or one at a described slot was invoked outside the sample call's own stack",
                emit: "",
            },
            Self::DescribedProtocols(_) => ReviewedObservation {
                marker: "callback-invocation",
                observation: "exact for the recorded slots: a Proxy argument the sample supplied was called or constructed, had a string-keyed member read, tested with `in`, enumerated or described (get), had Symbol.iterator or Symbol.asyncIterator read (iterate), had Symbol.toPrimitive, valueOf or toString read (coerce), or had Symbol.hasInstance read (has-instance), by a protocol the enumeration does not describe for that slot, at any time up to the end of the session's drain, or by a described one outside the sample call's own stack",
                emit: "",
            },
            Self::DescribedMembers(..) => ReviewedObservation {
                marker: "callback-invocation",
                observation: "exact for the recorded slots: as for the per-protocol observation, and in addition a recording function the sample installed at each index of a member slot up to one past the highest described index was called at an index the enumeration does not describe for that slot, at any time up to the end of the session's drain, or at a described one outside the sample call's own stack",
                emit: "",
            },
            Self::DescribedReads(_) => ReviewedObservation {
                marker: "read-operation",
                observation: "exact for the described slots: a member of an object argument the sample supplied at a slot the enumeration does not describe was invoked at any time up to the end of the session's drain, or a member of one at a described slot was invoked outside the sample call's own stack; which member, and any read of a source the export owns, are not observed",
                emit: "",
            },
            Self::NotCallable => ReviewedObservation {
                marker: "callable-value",
                observation: "exact: typeof of the exported runtime value is \"function\"",
                emit: "",
            },
            Self::DefaultLibraryAlias(_) => ReviewedObservation {
                marker: "alias-identity",
                observation: "exact: the exported runtime value is not the same function object as the named default-library member, by Object.is in the probe realm",
                emit: "",
            },
            Self::ParameterReturn(_) => ReviewedObservation {
                marker: "return-outside-identity",
                observation: "exact on a normal completion: Object.is(result, original argument at the claimed index) is false; object contents and throwing calls are not observed",
                emit: "",
            },
            Self::ArgumentContainers(_) => ReviewedObservation {
                marker: "return-outside-containers",
                observation: "exact on a normal completion for the argument returns: the result is not the argument at any claimed index by SameValue; for the array returns, not exact: an object whose length and elements equal the claimed arguments in order by SameValue satisfies it whether or not it is a fresh array; throwing calls are not observed",
                emit: "",
            },
            Self::PrimitiveReturn => ReviewedObservation {
                marker: "return-not-primitive",
                observation: "exact on a normal completion: typeof result is \"function\", or \"object\" and result is not null; throwing calls and unsampled inputs are not observed",
                emit: "",
            },
        }
    }

    fn sample_tuples(self, signatures: &[typefacts::SelectedSignature]) -> Vec<Vec<String>> {
        match self {
            Self::ParameterReturn(index) => identity_sample_tuples(signatures, index),
            Self::ArgumentContainers(set) => {
                let mut tuples = Vec::new();
                for index in set.indices() {
                    for tuple in identity_sample_tuples(signatures, index) {
                        if !tuples.contains(&tuple) {
                            tuples.push(tuple);
                        }
                    }
                }
                // ADR 0116: every callable the slot of a claimed invocation
                // receives is replaced by a recording function of its arity,
                // and the identity samples are all of arity zero; one sample of
                // arity one reaches the branch an arity test excludes
                // (`access`'s `!v.length`).
                for index in set.invocation_indices() {
                    let slot = usize::from(index);
                    let variants = tuples
                        .iter()
                        .filter(|tuple| {
                            tuple
                                .get(slot)
                                .is_some_and(|value| value == "(() => undefined)")
                        })
                        .map(|tuple| {
                            let mut variant = tuple.clone();
                            variant[slot] = "(function (_) { return undefined; })".into();
                            variant
                        })
                        .collect::<Vec<_>>();
                    for variant in variants {
                        if !tuples.contains(&variant) {
                            tuples.push(variant);
                        }
                    }
                }
                tuples
            }
            Self::NotCallable | Self::DefaultLibraryAlias(_) => Vec::new(),
            Self::DescribedCallbacks(_) => sample_tuples_with(signatures, true),
            Self::DescribedProtocols(masks) => protocol_sample_tuples(signatures, masks),
            Self::DescribedMembers(masks, members) => {
                member_sample_tuples(signatures, masks, members)
            }
            Self::DescribedReads(mask) => reads_sample_tuples(signatures, mask),
            _ => sample_tuples(signatures),
        }
    }

    fn sampling_limitations(self) -> &'static str {
        match self {
            Self::DescribedCallbacks(_) => {
                "at most six tuples per overload; no variadic tail or structural object construction; a described slot the signature does not type as callable is sampled with a non-callable value, so a call of it throws and observes nothing"
            }
            Self::DescribedProtocols(_) => {
                "at most six tuples per overload; no variadic tail or structural object construction; every described slot and every object-, array- or callable-typed slot is sampled with a recording Proxy over an empty object, an empty array or a zero-arity function, whose traps answer as the target does, and a described slot the signature types as a primitive is also sampled with the object Proxy; a symbol-keyed member other than the iteration, coercion and hasInstance symbols is not recorded, and a use on a value the export derived from an argument, rather than the argument itself, is not observed"
            }
            Self::DescribedMembers(..) => {
                "at most six tuples per overload; no variadic tail or structural object construction; every described slot and every object-, array- or callable-typed slot is sampled with a recording Proxy over an empty object, an empty array or a zero-arity function, whose traps answer as the target does, and a slot a member item names is also sampled with the array Proxy; a member slot's target carries a recording function at each index up to one past the highest described one and nothing past it, so a call of a member at a higher index or at a property key throws and observes nothing; a symbol-keyed member other than the iteration, coercion and hasInstance symbols is not recorded, and a use on a value the export derived from an argument, rather than the argument itself, is not observed"
            }
            Self::DescribedReads(_) => {
                "at most six tuples per overload; no variadic tail; every described slot and every object-typed slot is sampled with a recording tripwire whose members are all callable to a depth of eight, so an export that expects a real value there throws and observes nothing, and a walk along a member chain ends; engine-protocol members (then, valueOf, toString, toJSON, constructor, symbols) are not recorded, and iterating or coercing a tripwire throws"
            }
            Self::NotCallable => {
                "no call is sampled: the observation is typeof of the exported value in the probe realm, so a value that is callable only through a construct signature the checker did not see, or only in another realm, is not observed"
            }
            Self::DefaultLibraryAlias(_) => {
                "no call is sampled: the observation compares the exported value with the named default-library member by Object.is in the probe realm. It cannot see a realm whose built-in differs from the probe realm's, and it says nothing about what the member does -- that the member is safe to close these domains on is the certifier's reviewed decision, not this observation's"
            }
            Self::ParameterReturn(_) => {
                "identity samples use distinct object/callable identities and vary the returned slot separately; at most twelve tuples per overload, no variadic tail or structural object construction; throwing-only runs are incomplete and cannot satisfy the veto"
            }
            Self::PrimitiveReturn => {
                "at most six tuples per overload, no variadic tail or structural object construction; an input outside the sample that makes the export return an object is not observed; throwing-only runs are incomplete and cannot satisfy the veto"
            }
            Self::ArgumentContainers(_) => {
                "identity samples for each claimed index, distinct object/callable identities per slot; a claimed invocation slot's callables replaced by recording functions of the same arity (zero, and one sample of one) whose returned tokens are the only values the invocation result admits; at most twelve tuples per index and overload, no variadic tail or structural object construction; an input outside the sample that selects another completion is not observed; throwing-only runs are incomplete and cannot satisfy the veto"
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
    if observation == Observation::PrimitiveReturn {
        return primitive_return_module_source(specifier, export, signatures);
    }
    if let Observation::ArgumentContainers(set) = observation {
        return argument_container_module_source(specifier, export, set, signatures);
    }
    if observation == Observation::NotCallable {
        return not_callable_module_source(specifier, export);
    }
    if let Observation::DefaultLibraryAlias(index) = observation {
        return default_library_alias_module_source(specifier, export, index);
    }
    if let Observation::DescribedCallbacks(mask) = observation {
        return described_callbacks_module_source(specifier, export, mask, signatures);
    }
    if let Observation::DescribedProtocols(masks) = observation {
        return described_protocols_module_source(specifier, export, masks, signatures);
    }
    if let Observation::DescribedMembers(masks, members) = observation {
        return described_members_module_source(specifier, export, masks, members, signatures);
    }
    if let Observation::DescribedReads(mask) = observation {
        return described_reads_module_source(specifier, export, mask, signatures);
    }
    let domain = match observation {
        Observation::Creates => "creates",
        Observation::EmptyReturns => "returns",
        Observation::EmptyCallbacks => "callbacks",
        Observation::ParameterReturn(_)
        | Observation::PrimitiveReturn
        | Observation::ArgumentContainers(_)
        | Observation::NotCallable
        | Observation::DefaultLibraryAlias(_)
        | Observation::DescribedCallbacks(_)
        | Observation::DescribedProtocols(_)
        | Observation::DescribedMembers(..)
        | Observation::DescribedReads(_) => unreachable!(),
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

/// ADR 0100: the module for a described `callbacks` enumeration. Every callable
/// slot is sampled with its own recording callable, so the module knows which
/// slot ran; the marker fires when a slot outside the description runs at any
/// time, or a described slot runs while no sample call is on the stack. A
/// described slot running inside its sample call is the described item and
/// observes nothing -- the census, not this module, is what proves it.
/// [`sample_tuples`] for a described `reads` enumeration (ADR 0101): every
/// described slot, and every slot whose candidate is an object, array or
/// callable, is rendered as `tripwireAt(<slot>)` -- a recording value whose
/// members are all callable -- so the module can tell which slot's member ran.
/// Primitive candidates stay primitives: a member invocation on a number is
/// not what the enumeration is about.
fn reads_sample_tuples(signatures: &[typefacts::SelectedSignature], mask: u64) -> Vec<Vec<String>> {
    let mut tuples = Vec::new();
    for signature in signatures {
        let slots = signature
            .parameters
            .iter()
            .filter(|parameter| !parameter.rest)
            .map(|parameter| (parameter.index, slot_candidates(parameter)))
            .collect::<Vec<_>>();
        let widest = slots
            .iter()
            .map(|(_, (candidates, _))| candidates.len())
            .max()
            .unwrap_or(1)
            .clamp(1, MAX_SAMPLE_CALLS);
        for index in 0..widest {
            let tuple = slots
                .iter()
                .map(|(slot, (candidates, literals))| {
                    let sample = candidates[index % candidates.len()];
                    let described = u32::try_from(*slot).is_ok_and(|bit| bit < u64::BITS)
                        && mask & (1 << slot) != 0;
                    if described
                        || matches!(sample, Sample::Object | Sample::Array | Sample::Callable)
                    {
                        format!("tripwireAt({slot})")
                    } else {
                        render(sample, literals)
                    }
                })
                .collect::<Vec<_>>();
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
    }
    tuples
}

/// ADR 0101: the module for a described `reads` enumeration. Every recorded
/// slot is a tripwire -- a callable proxy whose every string-keyed member is
/// another callable proxy remembering the slot -- so invoking a member of the
/// argument is what records, and a plain property read, a bare call of the
/// argument itself (the `callbacks` domain's item), or an engine-protocol
/// member the runtime reaches on its own while coercing or awaiting records
/// nothing. Quiet for a described slot's member run inside the sample call;
/// loud for an undescribed slot's member at any time, or a described slot's
/// member after the call returned.
fn described_reads_module_source(
    specifier: &str,
    export: &str,
    mask: u64,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let described = (0..u64::BITS)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| bit.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let tuples = reads_sample_tuples(signatures, mask)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Synthesized veto (ADR 0101) for the described `reads` closure of `{export}`.\n\
         // Derived from the export's Type Facts call signature; deterministic in it.\n\
         // It observes, it never proves: the implementation census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         const described = new Set([{described}]);\n\
         let emitter = null;\n\
         let inCall = false;\n\
         let emitted = false;\n\
         const record = (slot) => {{\n\
         \x20 if (!emitted && emitter && (!described.has(slot) || !inCall)) {{\n\
         \x20   emitted = true;\n\
         \x20   emitter.emit({{ marker: \"read-operation\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         }};\n\
         // Members the engine reads on its own while coercing, awaiting or\n\
         // serializing a value; a member the export itself invokes is never one.\n\
         const protocolKeys = new Set([\"then\", \"valueOf\", \"toString\", \"toJSON\", \"constructor\"]);\n\
         // A member chain ends after a bounded depth, so a walk such as\n\
         // `while (node) node = node.parentNode` terminates instead of running\n\
         // to the session's budget; the export's own invocations sit far above it.\n\
         const MEMBER_DEPTH = 8;\n\
         const memberAt = (slot, depth) => new Proxy(() => undefined, {{\n\
         \x20 get(_target, key) {{\n\
         \x20   if (typeof key === \"symbol\" || protocolKeys.has(key) || depth >= MEMBER_DEPTH) return undefined;\n\
         \x20   return memberAt(slot, depth + 1);\n\
         \x20 }},\n\
         \x20 apply() {{ record(slot); return undefined; }}\n\
         }});\n\
         const tripwireAt = (slot) => new Proxy(() => undefined, {{\n\
         \x20 get(_target, key) {{\n\
         \x20   return typeof key === \"symbol\" || protocolKeys.has(key) ? undefined : memberAt(slot, 1);\n\
         \x20 }},\n\
         \x20 // A bare invocation of the argument is the callbacks domain's item.\n\
         \x20 apply() {{ return undefined; }}\n\
         }});\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 emitter = harness;\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject !== \"function\") {{\n\
         \x20   throw new Error(\"synthesized veto: the export is not callable in this realm\");\n\
         \x20 }}\n\
         \x20 let threw = 0;\n\
         \x20 for (const args of samples) {{\n\
         \x20   inCall = true;\n\
         \x20   try {{\n\
         \x20     subject(...args);\n\
         \x20   }} catch {{\n\
         \x20     threw += 1;\n\
         \x20   }} finally {{\n\
         \x20     inCall = false;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 if (threw > 0) harness.emit({{ marker: \"sample-threw\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}

/// [`sample_tuples`] for a described enumeration with non-call items: every
/// slot whose sample is an object, array or callable is rendered as
/// `slotAt(<slot>, <shape>)`, and a described slot also gets the object
/// sample when its signature offers none, so each described protocol meets a
/// recording value in at least one tuple. Other primitive samples stay
/// primitives: nothing of the caller's runs on a number.
fn protocol_sample_tuples(
    signatures: &[typefacts::SelectedSignature],
    masks: ProtocolMasks,
) -> Vec<Vec<String>> {
    protocol_sample_tuples_with(signatures, masks, 0)
}

/// [`protocol_sample_tuples`] for an enumeration with member call items: a
/// slot a member item names is also sampled with the array Proxy, whose
/// target carries the recording members, when its signature offers no array.
fn member_sample_tuples(
    signatures: &[typefacts::SelectedSignature],
    masks: ProtocolMasks,
    members: MemberCalls,
) -> Vec<Vec<String>> {
    protocol_sample_tuples_with(signatures, masks, members.slots())
}

fn protocol_sample_tuples_with(
    signatures: &[typefacts::SelectedSignature],
    masks: ProtocolMasks,
    member_slots: u64,
) -> Vec<Vec<String>> {
    let described = masks.any() | member_slots;
    let mut tuples = Vec::new();
    for signature in signatures {
        let slots = signature
            .parameters
            .iter()
            .filter(|parameter| !parameter.rest)
            .map(|parameter| {
                let (mut candidates, literals) = slot_candidates(parameter);
                let recorded = u32::try_from(parameter.index).is_ok_and(|bit| bit < u64::BITS)
                    && described & (1 << parameter.index) != 0;
                if recorded
                    && !candidates.iter().any(|sample| {
                        matches!(sample, Sample::Object | Sample::Array | Sample::Callable)
                    })
                {
                    candidates.push(Sample::Object);
                }
                let member = recorded && member_slots & (1 << parameter.index) != 0;
                if member && !candidates.contains(&Sample::Array) {
                    candidates.push(Sample::Array);
                }
                (parameter.index, (candidates, literals))
            })
            .collect::<Vec<_>>();
        let widest = slots
            .iter()
            .map(|(_, (candidates, _))| candidates.len())
            .max()
            .unwrap_or(1)
            .clamp(1, MAX_SAMPLE_CALLS);
        for index in 0..widest {
            let tuple = slots
                .iter()
                .map(
                    |(slot, (candidates, literals))| match candidates[index % candidates.len()] {
                        Sample::Object => format!("slotAt({slot}, \"object\")"),
                        Sample::Array => format!("slotAt({slot}, \"array\")"),
                        Sample::Callable => format!("slotAt({slot}, \"function\")"),
                        sample => render(sample, literals),
                    },
                )
                .collect::<Vec<_>>();
            if !tuples.contains(&tuple) {
                tuples.push(tuple);
            }
        }
    }
    tuples
}

/// The module for a described `callbacks` enumeration with non-call items
/// (item A of ways-to-improve § 3.3). One recording mechanism for every
/// protocol: `slotAt(slot, shape)` is a `Proxy` over a fresh empty object,
/// array or zero-arity function whose every trap records the slot and the
/// protocol it classifies the use as, then answers with the target's own
/// `Reflect.*` result -- so the value behaves as the plain value it wraps,
/// `typeof` and `.length` included, and the export takes the branch it would
/// take on that value. It is the `DescribedReads` tripwire's per-slot member
/// recording widened from "a member was invoked" to "which protocol ran",
/// with the engine-protocol members it deliberately ignored (`valueOf`,
/// `toString`, the symbols) now the very uses it classifies; that module is
/// left byte-identical so its receipts keep their construction digests.
fn described_protocols_module_source(
    specifier: &str,
    export: &str,
    masks: ProtocolMasks,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let tuples = protocol_sample_tuples(signatures, masks)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Synthesized veto (ADR 0100, per protocol) for the described `callbacks`\n\
         // closure of `{export}`, whose items include non-call uses of an argument.\n\
         // Derived from the export's Type Facts call signature; deterministic in it.\n\
         // It observes, it never proves: the implementation census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         const described = {described};\n\
         let emitter = null;\n\
         let inCall = false;\n\
         let emitted = false;\n\
         const record = (slot, protocol) => {{\n\
         \x20 if (!emitted && emitter && (!described[protocol].has(slot) || !inCall)) {{\n\
         \x20   emitted = true;\n\
         \x20   emitter.emit({{ marker: \"callback-invocation\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         }};\n\
         // Which protocol of the value a property key reaches. A string key is a\n\
         // property read, except the two ToPrimitive reaches; the iteration,\n\
         // coercion and hasInstance symbols are their protocols; any other\n\
         // symbol is the engine's own bookkeeping and records nothing.\n\
         const protocolOf = (key) => {{\n\
         \x20 if (key === Symbol.iterator || key === Symbol.asyncIterator) return \"iterate\";\n\
         \x20 if (key === Symbol.toPrimitive || key === \"valueOf\" || key === \"toString\") return \"coerce\";\n\
         \x20 if (key === Symbol.hasInstance) return \"has-instance\";\n\
         \x20 return typeof key === \"string\" ? \"get\" : null;\n\
         }};\n\
         const use = (slot, key) => {{\n\
         \x20 const protocol = protocolOf(key);\n\
         \x20 if (protocol !== null) record(slot, protocol);\n\
         }};\n\
         const slotAt = (slot, shape) => {{\n\
         \x20 const target = shape === \"function\" ? function () {{ return undefined; }} : shape === \"array\" ? [] : {{}};\n\
         \x20 return new Proxy(target, {{\n\
         \x20   apply(t, self, args) {{ record(slot, \"call\"); return Reflect.apply(t, self, args); }},\n\
         \x20   construct(t, args, next) {{ record(slot, \"call\"); return Reflect.construct(t, args, next); }},\n\
         \x20   get(t, key, receiver) {{ use(slot, key); return Reflect.get(t, key, receiver); }},\n\
         \x20   has(t, key) {{ use(slot, key); return Reflect.has(t, key); }},\n\
         \x20   ownKeys(t) {{ record(slot, \"get\"); return Reflect.ownKeys(t); }},\n\
         \x20   getOwnPropertyDescriptor(t, key) {{ use(slot, key); return Reflect.getOwnPropertyDescriptor(t, key); }},\n\
         \x20 }});\n\
         }};\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 emitter = harness;\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject !== \"function\") {{\n\
         \x20   throw new Error(\"synthesized veto: the export is not callable in this realm\");\n\
         \x20 }}\n\
         \x20 let threw = 0;\n\
         \x20 for (const args of samples) {{\n\
         \x20   inCall = true;\n\
         \x20   try {{\n\
         \x20     subject(...args);\n\
         \x20   }} catch {{\n\
         \x20     threw += 1;\n\
         \x20   }} finally {{\n\
         \x20     inCall = false;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 if (threw > 0) harness.emit({{ marker: \"sample-threw\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        described = masks.javascript(),
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}

/// The module for a described `callbacks` enumeration with member call items
/// (item B of ways-to-improve § 3.3): [`described_protocols_module_source`]'s
/// per-protocol Proxy, with the target of every slot a member item names
/// carrying `memberAt(slot, index)` -- a recording function -- at each index
/// up to one past the highest described index for that slot. A call of a
/// described member inside the sample call is the item and observes nothing;
/// a call of an undescribed one, or of any of them outside the call, emits the
/// contradiction. The member is read through the Proxy, so the read is the
/// slot's `get` as it is for any other key. A separate function so that
/// module stays byte-identical for every enumeration with no member item.
fn described_members_module_source(
    specifier: &str,
    export: &str,
    masks: ProtocolMasks,
    members: MemberCalls,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let tuples = member_sample_tuples(signatures, masks, members)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Synthesized veto (ADR 0100, per protocol and member) for the described\n\
         // `callbacks` closure of `{export}`, whose items include calls of a member\n\
         // of an argument. Derived from the export's Type Facts call signature;\n\
         // deterministic in it. It observes, it never proves: the implementation\n\
         // census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         const described = {described};\n\
         const members = {members};\n\
         const memberTops = {tops};\n\
         let emitter = null;\n\
         let inCall = false;\n\
         let emitted = false;\n\
         const contradict = () => {{\n\
         \x20 if (!emitted && emitter) {{\n\
         \x20   emitted = true;\n\
         \x20   emitter.emit({{ marker: \"callback-invocation\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         }};\n\
         const record = (slot, protocol) => {{\n\
         \x20 if (!described[protocol].has(slot) || !inCall) contradict();\n\
         }};\n\
         // A call of the member the sample installed at `index` of `slot`.\n\
         const memberAt = (slot, index) => function () {{\n\
         \x20 if (!members.has(`${{slot}}:${{index}}`) || !inCall) contradict();\n\
         \x20 return undefined;\n\
         }};\n\
         // Which protocol of the value a property key reaches. A string key is a\n\
         // property read, except the two ToPrimitive reaches; the iteration,\n\
         // coercion and hasInstance symbols are their protocols; any other\n\
         // symbol is the engine's own bookkeeping and records nothing.\n\
         const protocolOf = (key) => {{\n\
         \x20 if (key === Symbol.iterator || key === Symbol.asyncIterator) return \"iterate\";\n\
         \x20 if (key === Symbol.toPrimitive || key === \"valueOf\" || key === \"toString\") return \"coerce\";\n\
         \x20 if (key === Symbol.hasInstance) return \"has-instance\";\n\
         \x20 return typeof key === \"string\" ? \"get\" : null;\n\
         }};\n\
         const use = (slot, key) => {{\n\
         \x20 const protocol = protocolOf(key);\n\
         \x20 if (protocol !== null) record(slot, protocol);\n\
         }};\n\
         const slotAt = (slot, shape) => {{\n\
         \x20 const target = shape === \"function\" ? function () {{ return undefined; }} : shape === \"array\" ? [] : {{}};\n\
         \x20 const top = memberTops[slot];\n\
         \x20 if (top !== undefined) {{\n\
         \x20   for (let index = 0; index <= top + 1; index += 1) target[index] = memberAt(slot, index);\n\
         \x20 }}\n\
         \x20 return new Proxy(target, {{\n\
         \x20   apply(t, self, args) {{ record(slot, \"call\"); return Reflect.apply(t, self, args); }},\n\
         \x20   construct(t, args, next) {{ record(slot, \"call\"); return Reflect.construct(t, args, next); }},\n\
         \x20   get(t, key, receiver) {{ use(slot, key); return Reflect.get(t, key, receiver); }},\n\
         \x20   has(t, key) {{ use(slot, key); return Reflect.has(t, key); }},\n\
         \x20   ownKeys(t) {{ record(slot, \"get\"); return Reflect.ownKeys(t); }},\n\
         \x20   getOwnPropertyDescriptor(t, key) {{ use(slot, key); return Reflect.getOwnPropertyDescriptor(t, key); }},\n\
         \x20 }});\n\
         }};\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 emitter = harness;\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject !== \"function\") {{\n\
         \x20   throw new Error(\"synthesized veto: the export is not callable in this realm\");\n\
         \x20 }}\n\
         \x20 let threw = 0;\n\
         \x20 for (const args of samples) {{\n\
         \x20   inCall = true;\n\
         \x20   try {{\n\
         \x20     subject(...args);\n\
         \x20   }} catch {{\n\
         \x20     threw += 1;\n\
         \x20   }} finally {{\n\
         \x20     inCall = false;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 if (threw > 0) harness.emit({{ marker: \"sample-threw\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        described = masks.javascript(),
        members = members.javascript_described(),
        tops = members.javascript_tops(),
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}

fn described_callbacks_module_source(
    specifier: &str,
    export: &str,
    mask: u64,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let described = (0..u64::BITS)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| bit.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let tuples = sample_tuples_with(signatures, true)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Synthesized veto (ADR 0100) for the described `callbacks` closure of `{export}`.\n\
         // Derived from the export's Type Facts call signature; deterministic in it.\n\
         // It observes, it never proves: the implementation census is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         const described = new Set([{described}]);\n\
         let emitter = null;\n\
         let inCall = false;\n\
         let emitted = false;\n\
         const callbackAt = (slot) => () => {{\n\
         \x20 if (!emitted && emitter && (!described.has(slot) || !inCall)) {{\n\
         \x20   emitted = true;\n\
         \x20   emitter.emit({{ marker: \"callback-invocation\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         }};\n\
         const samples = [\n{tuples}\n];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 emitter = harness;\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject !== \"function\") {{\n\
         \x20   throw new Error(\"synthesized veto: the export is not callable in this realm\");\n\
         \x20 }}\n\
         \x20 let threw = 0;\n\
         \x20 for (const args of samples) {{\n\
         \x20   inCall = true;\n\
         \x20   try {{\n\
         \x20     subject(...args);\n\
         \x20   }} catch {{\n\
         \x20     threw += 1;\n\
         \x20   }} finally {{\n\
         \x20     inCall = false;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 if (threw > 0) harness.emit({{ marker: \"sample-threw\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}

/// ADR 0099: the module for a not-callable export. It samples nothing -- there
/// is no call to make -- and emits `callable-value` when the runtime value is a
/// function after all, which is the one observation that falsifies the
/// producer's stated fact. A non-function value is a clean non-observation and
/// the closure rests on the fact.
fn not_callable_module_source(specifier: &str, export: &str) -> String {
    format!(
        "// Synthesized veto (ADR 0099) for the empty call domains of `{export}`.\n\
         // The producer states the export's value cannot be invoked; this observes\n\
         // the runtime value's typeof and emits if it is a function after all.\n\
         // It observes, it never proves: the stated type fact is the proof.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 if (typeof subject === \"function\") {{\n\
         \x20   harness.emit({{ marker: \"callable-value\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
    )
}

fn default_library_alias_module_source(specifier: &str, export: &str, index: u16) -> String {
    // The index was assigned from the reviewed table, so this cannot name a
    // member the certifier has not reviewed. A slot out of range would be a
    // construction bug rather than a claim about the package, and refusing to
    // emit a module is the fail-closed answer: the gate stays incomplete and
    // the candidate stays withheld.
    let Some(qualified) = super::type_facts::reviewed_default_library_alias(index) else {
        return String::new();
    };
    let (container, member) = qualified
        .split_once('.')
        .expect("a reviewed alias is spelled Container.member");
    format!(
        "// Synthesized veto (ADR 0103) for the empty `reads`, `creates` and\n\
         // `callbacks` domains of `{export}`.\n\
         // The producer states this export *is* `{qualified}`, by identity. This\n\
         // observes that identity directly -- one Object.is, no sample -- and emits\n\
         // only if the runtime value is some other function after all.\n\
         // It observes, it never proves: the stated identity fact is the proof, and\n\
         // that `{qualified}` may close these domains is a reviewed decision made in\n\
         // the certifier, not here.\n\
         import * as subjectModule from {specifier_json};\n\
         \n\
         const subject = subjectModule[{export_json}];\n\
         \n\
         export async function runProbeSession(_session, harness) {{\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 const builtin = typeof {container} === \"undefined\" ? undefined : {container}[{member_json}];\n\
         \x20 if (builtin === undefined || !Object.is(subject, builtin)) {{\n\
         \x20   harness.emit({{ marker: \"alias-identity\", kind: \"call\", phase: \"enter\" }});\n\
         \x20 }}\n\
         \x20 harness.emit({{ marker: \"call\", kind: \"call\", phase: \"exit\" }});\n\
         }}\n",
        specifier_json = serde_json::to_string(specifier).unwrap_or_default(),
        export_json = serde_json::to_string(export).unwrap_or_default(),
        member_json = serde_json::to_string(member).unwrap_or_default(),
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

/// ADR 0115: the module for returns of argument containers.
///
/// Every sample is called and the marker fires when a normal completion is
/// none of the claimed containers. SameValue is written with operators and the
/// array check reads only the result's own `length` and elements in a plain
/// loop, so a package that replaces an intrinsic during its import cannot bend
/// the argument half. A throwing sample observes nothing, and a run with no
/// normal completion throws so the gate is incomplete rather than satisfied by
/// silence.
fn argument_container_module_source(
    specifier: &str,
    export: &str,
    set: ContainerSet,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let tuples = Observation::ArgumentContainers(set)
        .sample_tuples(signatures)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"// Synthesized argument-container return veto (ADR 0115, ADR 0116).
// Finite observations only falsify; the authenticated census proves closure.
import * as subjectModule from {specifier};
const subject = subjectModule[{export}];
// SameValue without reading a mutable intrinsic after the subject's import.
const sameValue = (left, right) => left === right
  ? left !== 0 || 1 / left === 1 / right
  : left !== left && right !== right;
const containers = [{containers}];
// ADR 0116: a claimed invocation slot's callable is replaced by a recording
// function of the same arity, and only the tokens it returned during this
// sample call are what that invocation returned.
const invocationSlots = [{invocation_slots}];
const tokens = {{}};
const recorder = (slot, arity) => arity === 0
  ? function () {{ const token = {{}}; tokens[slot][tokens[slot].length] = token; return token; }}
  : function (_) {{ const token = {{}}; tokens[slot][tokens[slot].length] = token; return token; }};
const holds = (result, args) => {{
  for (const container of containers) {{
    if (container.invocation) {{
      const returned = tokens[container.items[0]];
      for (let position = 0; position < returned.length; position += 1) {{
        if (sameValue(result, returned[position])) return true;
      }}
      continue;
    }}
    if (!container.array) {{
      if (sameValue(result, args[container.items[0]])) return true;
      continue;
    }}
    if (result === null || typeof result !== "object") continue;
    let length;
    try {{ length = result.length; }} catch {{ continue; }}
    if (length !== container.items.length) continue;
    let every = true;
    for (let position = 0; position < container.items.length; position += 1) {{
      let element;
      try {{ element = result[position]; }} catch {{ every = false; break; }}
      if (!sameValue(element, args[container.items[position]])) {{ every = false; break; }}
    }}
    if (every) return true;
  }}
  return false;
}};
const samples = [
{tuples}
];
export async function runProbeSession(_session, harness) {{
  harness.emit({{ marker: "call", kind: "call", phase: "enter" }});
  if (typeof subject !== "function") throw new Error("synthesized veto: export is not callable");
  let completed = 0;
  let threw = 0;
  for (const sample of samples) {{
    const args = [];
    for (let position = 0; position < sample.length; position += 1) args[position] = sample[position];
    for (const slot of invocationSlots) {{
      tokens[slot] = [];
      if (typeof args[slot] === "function") args[slot] = recorder(slot, args[slot].length === 0 ? 0 : 1);
    }}
    let result;
    try {{ result = subject(...args); }} catch {{ threw += 1; continue; }}
    completed += 1;
    if (!holds(result, args)) {{
      harness.emit({{ marker: "return-outside-containers", kind: "call", phase: "enter" }});
    }}
  }}
  if (threw > 0) harness.emit({{ marker: "sample-threw", kind: "call", phase: "enter" }});
  if (completed === 0) throw new Error("synthesized container veto: no sample completed normally");
  harness.emit({{ marker: "call", kind: "call", phase: "exit" }});
}}
"#,
        specifier = serde_json::to_string(specifier).unwrap(),
        export = serde_json::to_string(export).unwrap(),
        containers = set.javascript(),
        invocation_slots = set
            .invocation_indices()
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// ADR 0113: the module for one `plain` return over a primitive completion.
///
/// Every sample the signature admits is called, and the marker fires when a
/// normal completion hands back anything a primitive cannot be. `typeof` is an
/// operator, so a package that replaces a global during its import cannot bend
/// the check, and a `Proxy` still reports `"object"` or `"function"`. A throwing
/// sample observes nothing, and a run with no normal completion throws so the
/// gate is incomplete rather than satisfied by silence.
fn primitive_return_module_source(
    specifier: &str,
    export: &str,
    signatures: &[typefacts::SelectedSignature],
) -> String {
    let tuples = sample_tuples(signatures)
        .into_iter()
        .map(|arguments| format!("  [{}],", arguments.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"// Synthesized primitive-return veto (ADR 0113).
// Finite observations only falsify; the authenticated census proves closure.
import * as subjectModule from {specifier};
const subject = subjectModule[{export}];
const samples = [
{tuples}
];
export async function runProbeSession(_session, harness) {{
  harness.emit({{ marker: "call", kind: "call", phase: "enter" }});
  if (typeof subject !== "function") throw new Error("synthesized veto: export is not callable");
  let completed = 0;
  let threw = 0;
  for (const args of samples) {{
    let result;
    try {{ result = subject(...args); }} catch {{ threw += 1; continue; }}
    completed += 1;
    const kind = typeof result;
    if (kind === "function" || (kind === "object" && result !== null)) {{
      harness.emit({{ marker: "return-not-primitive", kind: "call", phase: "enter" }});
    }}
  }}
  if (threw > 0) harness.emit({{ marker: "sample-threw", kind: "call", phase: "enter" }});
  if (completed === 0) throw new Error("synthesized primitive-return veto: no sample completed normally");
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
