//! The dialect-neutral diagnostic model.
//!
//! Every dialect's rule catalog produces these findings from a [`Program`],
//! and the backend consumes them without knowing which catalog ran. The
//! catalogs themselves — which rules exist, their codes, messages, and when
//! they fire — live in the dialect crates.
//!
//! [`Program`]: crate::Program

use crate::ExecutionRole;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use typefacts::Location;

use crate::{DirectMutationTarget, Fix, OwnerRequirement, ReactiveRead};

/// Where a catalog run spent its time.
#[derive(Clone, Copy, Debug, Default)]
pub struct SolveTimings {
    pub total: Duration,
    pub finding_construction: Duration,
    pub final_ordering: Duration,
}

/// The externally visible identity of one rule: what a catalog entry
/// contributes to every finding it constructs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleMetadata {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: &'static str,
    pub uncertifiable: bool,
    pub default_enabled: bool,
    /// Whether a project may configure the rule by name. `false` for an
    /// obligation (SC9005, SC9012): an internal uncertifiable result that no
    /// option reaches and no key names (ADR 0440).
    pub configurable: bool,
    pub presets: &'static [&'static str],
}

/// Base URL of the per-rule documentation pages in `docs/rules/`. Every
/// dialect catalog addresses its pages under it; the per-dialect part is the
/// directory the rule name itself carries — none, for the default surface.
pub const DOCS_BASE_URL: &str =
    "https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules";

/// Serializes a rule catalog into the stable manifest consumed by the npm
/// adapter. Catalogs remain responsible for ordering and metadata; this owns
/// the shared wire shape and formatting.
///
/// One entry per rule *name*, the configurable identity (ADR 0426): codes that
/// share a name become one entry whose `code` is the first and whose `codes`
/// lists every one. Two rules of one name must agree on every other field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleManifestIdentity {
    /// Stable checker dialect id (`solid-v2`).
    pub dialect: &'static str,
    /// Backward-compatible ESLint flat-config name (`v2`).
    pub config: &'static str,
    /// Rule-name namespace without the slash, or empty for the default surface.
    pub namespace: &'static str,
}

#[must_use]
pub fn rule_manifest_json(
    identity: RuleManifestIdentity,
    docs_base_url: &str,
    rules: impl ExactSizeIterator<Item = RuleMetadata>,
) -> String {
    let mut out = format!(
        "{{\n  \"schemaVersion\": 1,\n  \"dialect\": \"{}\",\n  \"config\": \"{}\",\n  \"namespace\": \"{}\",\n  \"docsBaseUrl\": \"{docs_base_url}\",\n  \"rules\": [\n",
        identity.dialect, identity.config, identity.namespace
    );
    let mut entries: Vec<(RuleMetadata, Vec<&'static str>)> = Vec::new();
    for metadata in rules {
        // An obligation has no key to configure, so the plugin has no entry
        // for it (ADR 0440).
        if !metadata.configurable {
            continue;
        }
        if let Some((first, codes)) = entries
            .iter_mut()
            .find(|(first, _)| first.name == metadata.name)
        {
            assert!(
                first.severity == metadata.severity
                    && first.uncertifiable == metadata.uncertifiable
                    && first.default_enabled == metadata.default_enabled
                    && first.presets == metadata.presets,
                "rules named {} must share severity, certifiability and presets",
                metadata.name
            );
            codes.push(metadata.code);
        } else {
            entries.push((metadata, vec![metadata.code]));
        }
    }
    let count = entries.len();
    for (index, (metadata, codes)) in entries.into_iter().enumerate() {
        let presets = metadata
            .presets
            .iter()
            .map(|preset| format!("\"{preset}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let all_codes = if codes.len() > 1 {
            let listed = codes
                .iter()
                .map(|code| format!("\"{code}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!(" \"codes\": [{listed}],")
        } else {
            String::new()
        };
        out.push_str(&format!(
            "    {{ \"code\": \"{}\",{all_codes} \"name\": \"{}\", \"severity\": \"{}\", \"uncertifiable\": {}, \"defaultEnabled\": {}, \"presets\": [{}] }}{}\n",
            metadata.code,
            metadata.name,
            metadata.severity,
            metadata.uncertifiable,
            metadata.default_enabled,
            presets,
            if index + 1 == count { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceStep {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    pub severity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_context: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject_kind: String,
    /// ADR 0205: for an analysis-coverage finding, the kind of gap it is
    /// (`package-boundary`, `caller-supplied-member`, …) and what it is about
    /// (the package, helper or owner primitive). A reporter groups by these
    /// instead of by message text. Empty for every other finding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coverage_family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coverage_subject: String,
    pub primary_location: Location,
    pub related_locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
}

impl Finding {
    pub fn new(metadata: RuleMetadata, message: String, primary_location: Location) -> Self {
        Self {
            id: metadata.code.into(),
            rule: metadata.name.into(),
            kind: if metadata.uncertifiable {
                "uncertifiable".into()
            } else {
                "violation".into()
            },
            message,
            hint: String::new(),
            severity: metadata.severity.into(),
            analysis_context: String::new(),
            subject_kind: String::new(),
            coverage_family: String::new(),
            coverage_subject: String::new(),
            primary_location,
            related_locations: vec![],
            evidence: vec![],
            fixes: vec![],
        }
    }

    /// Decorates a missing-owner finding with the shared uncertainty
    /// vocabulary. The catalog supplies the rule identity and the per-dialect
    /// `message` and `hint`; this owns the escalation an unproven owner
    /// forces — kind flips to `uncertifiable` and severity to `error` when
    /// the requirement is uncertain — and the clause each uncertainty source
    /// appends to the message and hint.
    #[must_use]
    pub fn for_owner_requirement(
        metadata: RuleMetadata,
        requirement: &OwnerRequirement,
        message: &str,
        hint: &str,
    ) -> Self {
        let uncertain = requirement.uncertain;
        let conditional_owner = requirement.conditional_owner;
        let later_run_unowned = requirement.later_run_unowned;
        let runtime_uncertain = requirement.runtime_uncertain;
        let component_uncertain = requirement.component_uncertain;
        let missing_jsx_census = requirement.missing_jsx_census;
        // `uncertain` predates the reason fields. Treat a deserialized legacy
        // row with no explicit reason as caller uncertainty.
        let through_dependency = requirement.through_dependency.as_ref();
        let caller_uncertain = requirement.caller_uncertain
            || (uncertain
                && through_dependency.is_none()
                && !runtime_uncertain
                && !conditional_owner
                && !later_run_unowned
                && !component_uncertain
                && !missing_jsx_census);
        let mut message = message.to_string();
        let mut hint = hint.to_string();
        if missing_jsx_census {
            message.push_str(
                "; but this call is written inside a JSX expression the Solid compiler's execution census does not cover, so whether the operation executes — and under which owner — cannot be proven",
            );
            hint.push_str(
                " Move the call to a compiler-censused position, or extend the compiler trace so its execution and owner context can be certified.",
            );
            return Self {
                kind: "uncertifiable".into(),
                severity: "error".into(),
                evidence: vec![EvidenceStep {
                    message: "the operation sits inside a JSX expression the compiler's execution census does not cover, so no compiler fact proves that a live unowned operation exists"
                        .into(),
                    location: Some(requirement.location.clone()),
                }],
                hint,
                ..Self::new(metadata, message, requirement.location.clone())
            };
        }
        let mut evidence = vec![EvidenceStep {
            message: if requirement.operation == crate::OwnerRequirementOperation::ContextProvider {
                "the read runs under a reactive owner, and the closed program has no provider for this context, so Solid finds no value".into()
            } else if requirement.after_await {
                "this call runs after an `await`, from a promise continuation: the owner current before the `await` is not current after it, and no other owner is"
                    .into()
            } else if component_uncertain {
                "component identity is unresolved, so this operation may execute with or without a reactive owner"
                    .into()
            } else if conditional_owner && requirement.caller_uncertain {
                // An exported function is owned by its owned callers and unowned
                // to any caller this build cannot see (its open-world seed); the
                // runWithOwner wording would name a cause that is not there.
                "this function is exported, and a caller solid-checker cannot see may supply no owner, so this operation may execute with or without a reactive owner"
                    .into()
            } else if conditional_owner {
                "runWithOwner receives a nullable owner, so this operation may execute detached"
                    .into()
            } else if later_run_unowned {
                "the enclosing callback runs under its caller's owner only on its first run; a later run comes from the scheduler's flush, where no owner is current"
                    .into()
            } else {
                "no containing component, computation, or root owner dominates this operation"
                    .into()
            },
            location: Some(requirement.location.clone()),
        }];
        // ADR 0290: the operation is in a package analyzed as source, and
        // this is the project's call that runs it.
        if let Some(operation) = through_dependency {
            message.push_str(
                "; this call runs it inside a package analyzed as source, under this call's owner context",
            );
            if uncertain
                && !runtime_uncertain
                && !conditional_owner
                && !later_run_unowned
                && !component_uncertain
                && !requirement.caller_uncertain
            {
                message.push_str(
                    ", which solid-checker cannot prove provides an owner on every path to it",
                );
            }
            hint.push_str(
                " The operation is in the package; give this call an owner (call it in a component or computation, or under createRoot).",
            );
            evidence.insert(
                0,
                EvidenceStep {
                    message: "this call enters package code that performs the operation".into(),
                    location: Some(requirement.location.clone()),
                },
            );
            evidence.push(EvidenceStep {
                message: "the operation, in the package's source".into(),
                location: Some(operation.clone()),
            });
        }
        if runtime_uncertain {
            let (clause, guidance, evidence_message) = match requirement.operation {
                crate::OwnerRequirementOperation::SettledCleanup => (
                    "; available runtime facts do not prove whether this callback returns a cleanup that requires owner registration",
                    " Resolve the callback's runtime return value to a definite cleanup or definite void before treating this as a violation.",
                    "cleanup registration depends on an unresolved callback return value",
                ),
                _ => (
                    "; available runtime facts do not prove whether this effect reaches owner registration",
                    " Resolve the effective runtime entry and argument shape before treating this as a violation.",
                    "effect allocation depends on an unavailable runtime-entry or argument-shape fact",
                ),
            };
            message.push_str(clause);
            hint.push_str(guidance);
            evidence.push(EvidenceStep {
                message: evidence_message.into(),
                location: Some(requirement.location.clone()),
            });
        }
        // The exported-caller clause below carries the open-world cause; the
        // runWithOwner clause is for a nullable owner only (ADR 0349).
        if conditional_owner && !requirement.caller_uncertain {
            message.push_str(
                "; runWithOwner may receive null, so solid-checker cannot prove this execution has an owner",
            );
            hint.push_str(
                " Narrow the owner to a non-null value before runWithOwner, or handle the detached lifetime explicitly.",
            );
        }
        if later_run_unowned {
            message.push_str(
                "; the enclosing callback is owned only on its first run and runs with no owner on any later run, so solid-checker cannot prove every execution has an owner",
            );
            hint.push_str(
                " Create owned work in the effect's compute, or keep the apply callback free of primitives and cleanups that a later run would leave detached.",
            );
        }
        if component_uncertain {
            message.push_str(
                "; the containing function may be a Solid component or an ordinary helper, so its owner context cannot be certified",
            );
            hint.push_str(
                " Use the function through JSX or annotate it with Solid's Component type to establish component ownership; otherwise handle the helper's lifetime explicitly.",
            );
        }
        if caller_uncertain {
            message.push_str(
                "; this function is exported, so solid-checker cannot prove its callers provide an owner",
            );
            hint.push_str(
                " If every caller runs this exported function under an owner, make that ownership visible to solid-checker, for example by calling it from a component or computation in the project.",
            );
        }
        Self {
            kind: if uncertain {
                "uncertifiable".into()
            } else {
                "violation".into()
            },
            severity: if uncertain {
                "error".into()
            } else {
                metadata.severity.into()
            },
            evidence,
            hint,
            ..Self::new(metadata, message, requirement.location.clone())
        }
    }
}

/// The shared tail of every catalog's `solve_measured`: deterministic
/// ordering, de-duplication, and the timing assembly. `construction_started`
/// is sampled here — construction ends where ordering begins.
#[must_use]
pub fn finish_findings(
    mut findings: Vec<Finding>,
    total_started: Instant,
    construction_started: Instant,
) -> (Vec<Finding>, SolveTimings) {
    let finding_construction = construction_started.elapsed();
    let ordering_started = Instant::now();
    findings.sort_by(|left, right| {
        (
            &left.primary_location.path,
            left.primary_location.start_byte,
            &left.id,
            &left.message,
        )
            .cmp(&(
                &right.primary_location.path,
                right.primary_location.start_byte,
                &right.id,
                &right.message,
            ))
    });
    findings.dedup();
    let final_ordering = ordering_started.elapsed();
    (
        findings,
        SolveTimings {
            total: total_started.elapsed(),
            finding_construction,
            final_ordering,
        },
    )
}

/// The context a read with no named context is described in, from its role.
/// A module-scope read has no enclosing function, and an effect's apply
/// callback is not a rendering function; naming either one so would describe
/// a context that does not exist.
const fn unnamed_read_context(execution: crate::ExecutionRole) -> &'static str {
    match execution {
        crate::ExecutionRole::ModuleInitialization => "module scope",
        crate::ExecutionRole::EffectApply => "effect apply callback",
        crate::ExecutionRole::Unknown => "code the analysis cannot place",
        crate::ExecutionRole::UntrackedUnwindowed => "a callback that runs with tracking off",
        _ => "rendering function",
    }
}

/// The strict-read message: how an untracked read of a reactive value is
/// described. The sentence is version-independent; what differs per dialect
/// is the hint, which the catalogs word themselves.
#[must_use]
pub fn strict_read_message(read: &ReactiveRead) -> String {
    strict_read_message_with_compute(read, false)
}

/// The compute observation affects only wording, never the read's proof.
#[must_use]
pub fn strict_read_message_with_compute(read: &ReactiveRead, compute_reads_source: bool) -> String {
    let context = if read.context.is_empty() {
        unnamed_read_context(read.execution)
    } else {
        &read.context
    };
    // No fact places the read: neither the compiler's census nor the semantic
    // roles say whether it runs tracked, untracked, in a callback, or at all.
    if read.execution == ExecutionRole::Unknown {
        let through = if read.via.is_empty() {
            String::new()
        } else {
            format!(" through {}", read.via)
        };
        return format!(
            "{} {:?} is read{through} in {context}, but no compiler or semantic fact places where this code runs (tracked, untracked, in a callback, or never), so whether this read is untracked cannot be proven either way",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
    if read.callback_invocation_unproven {
        let tracking = if read.missing_jsx_census
            || read.host_callback_timing
            || read.callee_callback_timing
        {
            "its execution or tracking context is also unresolved"
        } else {
            "the call is outside a tracking scope"
        };
        return format!(
            "{} {:?} may be read through {} at a call written in {context}; the callee's summary states an inline invocation but does not prove this call invokes the accessor; {tracking}, so the untracked read is uncertifiable",
            reactive_value_label(&read.kind),
            read.accessor,
            read.via,
        );
    }
    // A census gap unmakes the second half of the ordinary sentence. "Which
    // does not track" and "never updates" are claims about the execution
    // context, and the only evidence for them here would be the compiler's
    // silence about this JSX region — which is equally consistent with the
    // expression having been dropped, or lowered into something tracked that
    // the producer did not census. So the message says what is actually known.
    if read.missing_jsx_census {
        let through = if read.via.is_empty() {
            String::new()
        } else {
            format!(" through {}", read.via)
        };
        return format!(
            "{} {:?} is read{through} in {context}, inside a JSX expression the Solid compiler's execution census does not cover; whether that read is tracked cannot be proven either way, because the compiler reported no execution site for this JSX region and its silence is not evidence that the read never updates",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
    // The same honesty for the execution window. The callback is written in
    // `context` but runs wherever the host invokes it, and the host may do so
    // inside the strict-read window (a synchronous dispatch, a bound call, a
    // synchronous thenable) or after it; only the first is the untracked read
    // the ordinary sentence describes.
    if read.host_callback_timing {
        let through = if read.via.is_empty() {
            String::new()
        } else {
            format!(" through {}", read.via)
        };
        return format!(
            "{} {:?} is read{through} in a callback written in {context} that a host API retains (an event listener, a bound argument, a thenable callback or a Geolocation callback); the host may invoke this callback inside the component body's strict-read window, or after it, so whether this read runs untracked in {context} cannot be proven either way",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
    // The same honesty when the invoker is project code: the literal is
    // handed to a function whose body is not proven to call it during the
    // call, so it may run then, later from a closure the function returns or
    // keeps, or never.
    if read.callee_callback_timing {
        if read.execution == ExecutionRole::EffectApply {
            return format!(
                "{} {:?} may be read in {context}; whether apply executes is unresolved for these options and compute dependencies, so this untracked read is uncertifiable",
                reactive_value_label(&read.kind),
                read.accessor,
            );
        }
        let through = if read.via.is_empty() {
            String::new()
        } else {
            format!(" through {}", read.via)
        };
        return format!(
            "{} {:?} is read{through} in a callback written in {context} and passed to a consumer that is not proven to invoke it during the call or component rendering; that consumer may invoke it then, later from a closure it returns or keeps, or never, so whether this read runs untracked in {context} cannot be proven either way",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
    if compute_reads_source
        && read.execution == crate::ExecutionRole::EffectApply
        && !read.is_uncertifiable()
        && !read.summary_attributed
    {
        let through = if read.via.is_empty() {
            "directly".to_owned()
        } else {
            format!("through {}", read.via)
        };
        return format!(
            "{} {:?} is read {through} in {context}, which does not track; this effect's compute also reads the same source, so the effect can re-run when it changes, but this apply read is untracked and Solid warns in development",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
    if read.via.is_empty() {
        format!(
            "{} {:?} is read directly in {context}, which does not track; the read sees the current value once and never updates when {:?} changes",
            reactive_value_label(&read.kind),
            read.accessor,
            read.accessor
        )
    } else {
        format!(
            "{} {:?} is read through {} in {context}, which does not track; the read sees the current value once and never updates when {:?} changes",
            reactive_value_label(&read.kind),
            read.accessor,
            read.via,
            read.accessor
        )
    }
}

fn reactive_value_label(kind: &str) -> &'static str {
    match kind {
        "store-path" => "reactive store path",
        "reactive-getter" => "reactive getter",
        "component-props" => "component prop",
        _ => "reactive accessor",
    }
}

/// The last evidence step: either the proof that the site is outside every
/// tracked region, or — when the compiler censused nothing in the containing
/// JSX region — the statement that no such proof exists. "Outside every
/// compiler-tracked JSX region" reads as a completed search of the compiler's
/// facts; over a census hole it would be an overstatement, since the compiler
/// never reported on that region at all.
fn untracked_evidence_sentence(read: &ReactiveRead, subject: &str) -> String {
    if read.execution == ExecutionRole::Unknown {
        format!("no compiler or semantic fact places where {subject} runs")
    } else if read.callback_invocation_unproven {
        format!(
            "{subject} is an accessor passed to an accepted inline callback slot whose call-scoped cardinality does not guarantee an invocation"
        )
    } else if read.missing_jsx_census {
        format!(
            "{subject} sits inside a JSX expression the compiler's execution census does not cover, so no compiler fact places it inside or outside a tracked region"
        )
    } else if read.host_callback_timing {
        format!(
            "{subject} sits in a callback a host API retains and may invoke on its invoker's stack, so no fact places its execution inside or after the component body's strict-read window"
        )
    } else if read.callee_callback_timing {
        if read.execution == ExecutionRole::EffectApply {
            format!(
                "{subject} is in an effect apply whose invocation is not proven by its defer options and compute dependencies"
            )
        } else {
            format!(
                "{subject} sits in a callback passed to a consumer that is not proven to invoke it during the call or component rendering, so no fact places its execution inside or after the component body's strict-read window"
            )
        }
    } else {
        format!("{subject} is outside every compiler-tracked JSX region and deferred callback")
    }
}

/// The evidence chain behind a strict-read finding: the declaration, the
/// propagation through an intermediate function when there is one, and the
/// proof the read site is outside every tracked region.
#[must_use]
pub fn strict_read_evidence(read: &ReactiveRead) -> Vec<EvidenceStep> {
    let mut evidence = vec![EvidenceStep {
        message: format!(
            "{:?} is a {}",
            read.accessor,
            reactive_value_label(&read.kind)
        ),
        location: Some(read.declaration.clone()),
    }];
    if let Some(origin) = &read.origin {
        let origin_context = if read.origin_context.is_empty() {
            &read.via
        } else {
            &read.origin_context
        };
        evidence.push(EvidenceStep {
            message: format!(
                "{origin_context} {} the {}",
                if read.callback_invocation_unproven {
                    "may read"
                } else {
                    "reads"
                },
                reactive_value_label(&read.kind)
            ),
            location: Some(origin.clone()),
        });
        evidence.push(EvidenceStep {
            message: format!(
                "the call to {} propagates that {}read into {}",
                read.via,
                if read.callback_invocation_unproven {
                    "possible "
                } else {
                    ""
                },
                if read.context.is_empty() {
                    unnamed_read_context(read.execution)
                } else {
                    &read.context
                }
            ),
            location: Some(read.location.clone()),
        });
        evidence.push(EvidenceStep {
            message: untracked_evidence_sentence(read, "the call"),
            location: Some(read.location.clone()),
        });
    } else {
        evidence.push(EvidenceStep {
            message: "the cross-file reference resolves to that accessor declaration".into(),
            location: Some(read.location.clone()),
        });
        evidence.push(EvidenceStep {
            message: untracked_evidence_sentence(read, "the read"),
            location: Some(read.location.clone()),
        });
    }
    evidence
}

/// The locations a strict-read finding relates to beyond its primary one:
/// the accessor's declaration, and the interprocedural origin when the read
/// arrives through another function.
#[must_use]
pub fn strict_read_related_locations(read: &ReactiveRead) -> Vec<Location> {
    let mut locations = vec![read.declaration.clone()];
    if let Some(origin) = &read.origin {
        locations.push(origin.clone());
    }
    locations
}

/// The `(message, hint)` pair for a direct mutation of a reactive value.
/// Every sentence here is shared by both dialects verbatim; the one hint
/// whose wording is dialect-specific — how to write through a store's
/// setter — is supplied by the catalog as `store_hint`.
#[must_use]
pub fn direct_mutation_wording(
    name: &str,
    target: DirectMutationTarget,
    store_hint: impl FnOnce(&str) -> String,
) -> (String, String) {
    match target {
        DirectMutationTarget::Store => (
            format!(
                "{name:?} is a reactive store and is written through directly; Solid hands out a readonly proxy, so the write is dropped and nothing re-runs"
            ),
            store_hint(name),
        ),
        DirectMutationTarget::Projection => (
            format!(
                "{name:?} is a projection, a read-only derived store; a write through it is dropped and nothing re-runs"
            ),
            format!(
                "Derive the value inside the projection's compute function instead of mutating the result {name}."
            ),
        ),
        DirectMutationTarget::Props => (
            format!(
                "{name:?} is a reactive props object and is written through directly; Solid hands out a readonly proxy, so the write is dropped and nothing re-runs"
            ),
            format!(
                "Props are readonly by design: the parent owns the value. Lift the state to the parent and pass a callback down, rather than assigning to {name}."
            ),
        ),
    }
}

/// Test support for the dialect catalogs: asserts every rule name has a
/// documentation page under `docs_dir`. Not part of the crate's API.
#[doc(hidden)]
pub fn assert_rules_have_documentation(
    docs_dir: &std::path::Path,
    names: impl Iterator<Item = &'static str>,
) {
    for name in names {
        let page = docs_dir.join(format!("{name}.md"));
        assert!(
            page.is_file(),
            "rule {} has no documentation page at {}",
            name,
            page.display()
        );
    }
}

#[cfg(test)]
mod tests;
