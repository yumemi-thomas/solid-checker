//! The dialect-neutral diagnostic model.
//!
//! Every dialect's rule catalog produces these findings from a [`Program`],
//! and the backend consumes them without knowing which catalog ran. The
//! catalogs themselves — which rules exist, their codes, messages, and when
//! they fire — live in the dialect crates.
//!
//! [`Program`]: crate::Program

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
    pub presets: &'static [&'static str],
}

/// Base URL of the per-rule documentation pages in `docs/rules/`. Both
/// dialect catalogs address their pages under it; the per-dialect part is the
/// directory the rule name itself carries (`v1/...` or none).
pub const DOCS_BASE_URL: &str =
    "https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules";

/// Serializes a rule catalog into the stable manifest consumed by the npm
/// adapter. Catalogs remain responsible for ordering and metadata; this owns
/// the shared wire shape and formatting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleManifestIdentity {
    /// Stable checker dialect id (`solid-v1`, `solid-v2`).
    pub dialect: &'static str,
    /// Backward-compatible ESLint flat-config name (`v1`, `v2`).
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
    let count = rules.len();
    for (index, metadata) in rules.enumerate() {
        let presets = metadata
            .presets
            .iter()
            .map(|preset| format!("\"{preset}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "    {{ \"code\": \"{}\", \"name\": \"{}\", \"severity\": \"{}\", \"uncertifiable\": {}, \"defaultEnabled\": {}, \"presets\": [{}] }}{}\n",
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
        let runtime_uncertain = requirement.runtime_uncertain;
        let component_uncertain = requirement.component_uncertain;
        let divergent_lowering = requirement.divergent_lowering;
        // `uncertain` predates the reason fields. Treat a deserialized legacy
        // row with no explicit reason as caller uncertainty. A divergence is an
        // explicit reason and must not be re-read as that legacy default, or the
        // message would also blame callers it has no complaint about.
        let caller_uncertain = requirement.caller_uncertain
            || (uncertain
                && !runtime_uncertain
                && !conditional_owner
                && !component_uncertain
                && divergent_lowering.is_none());
        let mut message = message.to_string();
        let mut hint = hint.to_string();
        // The divergence returns early instead of appending a clause like the
        // other reasons, because it disagrees with the sentence they decorate.
        // "No containing owner dominates this operation" is a completed search of
        // the compiler's facts; here the facts are not incomplete but
        // contradictory, and that sentence would assert the half of them that
        // happens to be this producer's. Appending would leave the evidence chain
        // still claiming the search finished.
        //
        // It also subsumes the other reasons when they coincide: if the call
        // cannot be shown to run at all, whether its unenumerable callers would
        // have supplied an owner is not the reader's next question. The fields
        // stay on the requirement, so the serialized row still records them; only
        // the prose leads with the reason that dominates.
        if let Some(divergence) = divergent_lowering {
            let (position, shipped) = match divergence {
                crate::DivergentLowering::VoidElementChild => (
                    "inside the children of an HTML void element",
                    "the compiler Solid ships deletes a void element's child list in every position",
                ),
                crate::DivergentLowering::NoscriptChild => (
                    "inside the children of a `<noscript>`",
                    "the compiler Solid ships never lowers `<noscript>` children in any position",
                ),
            };
            message.push_str(&format!(
                "; but this call is written {position}, and {shipped} while the pinned Solid compiler lowers that child into a reactive insert that owns it — so whether this call runs at all, and under which owner, depends on which compiler builds this project"
            ));
            hint.push_str(
                " Move the call out of that element's children: while it sits there, no compiler fact decides its owner, and the two candidate compilers disagree about whether it executes.",
            );
            return Self {
                kind: "uncertifiable".into(),
                severity: "error".into(),
                evidence: vec![EvidenceStep {
                    message: format!(
                        "the operation sits {position}: the pinned compiler reports that child as a reactive JSX site and owns it, the compiler Solid ships emits nothing there, and no fact available here decides which one builds this project"
                    ),
                    location: Some(requirement.location.clone()),
                }],
                hint,
                ..Self::new(metadata, message, requirement.location.clone())
            };
        }
        let mut evidence = vec![EvidenceStep {
            message: if component_uncertain {
                "component identity is unresolved, so this operation may execute with or without a reactive owner"
                    .into()
            } else if conditional_owner {
                "runWithOwner receives a nullable owner, so this operation may execute detached"
                    .into()
            } else {
                "no containing component, computation, or root owner dominates this operation"
                    .into()
            },
            location: Some(requirement.location.clone()),
        }];
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
        if conditional_owner {
            message.push_str(
                "; runWithOwner may receive null, so solid-checker cannot prove this execution has an owner",
            );
            hint.push_str(
                " Narrow the owner to a non-null value before runWithOwner, or handle the detached lifetime explicitly.",
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
                " If every caller runs this exported function under an owner, document that in the package's reactivity contract.",
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

/// The strict-read message: how an untracked read of a reactive value is
/// described. The sentence is version-independent; what differs per dialect
/// is the hint, which the catalogs word themselves.
#[must_use]
pub fn strict_read_message(read: &ReactiveRead) -> String {
    let context = if !read.context.is_empty() {
        &read.context
    } else if read.execution == crate::ExecutionRole::ModuleInitialization {
        // A module-scope read has no enclosing function to name; calling it a
        // "rendering function" would describe a context that does not exist.
        "module scope"
    } else {
        "rendering function"
    };
    // A census gap unmakes the second half of the ordinary sentence. "Which
    // does not track" and "never updates" are claims about the execution
    // context, and the only evidence for them here would be the compiler's
    // silence about this JSX region — which is equally consistent with the
    // expression having been dropped, or lowered into something tracked that
    // the producer did not census. So the message says what is actually known.
    // Checked before the census gap, and they are mutually exclusive by
    // construction: the divergence requires a census entry the producer really
    // emitted, and the gap requires there to be none. The order is fixed
    // anyway, because this is the more specific claim — the census here is not
    // silent, it speaks for a compiler that may not be the one building this
    // project.
    //
    // The two divergences share a shape and differ in why they are true, so the
    // sentence differs in exactly that clause. Saying "deletes it" of
    // `<noscript>` would be wrong: the shipped compiler does not delete a
    // `<noscript>` child it decided to keep, it never lowers the subtree at all.
    if let Some(divergence) = read.divergent_lowering {
        let through = if read.via.is_empty() {
            String::new()
        } else {
            format!(" through {}", read.via)
        };
        let (position, shipped) = match divergence {
            crate::DivergentLowering::VoidElementChild => (
                "inside the children of an HTML void element",
                "the compiler Solid ships deletes a void element's child list in every position",
            ),
            crate::DivergentLowering::NoscriptChild => (
                "inside the children of a `<noscript>`",
                "the compiler Solid ships never lowers `<noscript>` children in any position",
            ),
        };
        return format!(
            "{} {:?} is read{through} in {context}, {position}; the pinned Solid compiler lowers that child into a reactive insert while {shipped}, so whether this read exists at runtime — let alone whether it is tracked — cannot be certified either way",
            reactive_value_label(&read.kind),
            read.accessor,
        );
    }
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
    if let Some(divergence) = read.divergent_lowering {
        let element = match divergence {
            crate::DivergentLowering::VoidElementChild => "a void element's child",
            crate::DivergentLowering::NoscriptChild => "a `<noscript>`'s child",
        };
        return format!(
            "{subject} is {element}: the pinned compiler reports it as a reactive JSX site, the compiler Solid ships emits nothing there, and no fact available here decides which one builds this project"
        );
    }
    if read.missing_jsx_census {
        format!(
            "{subject} sits inside a JSX expression the compiler's execution census does not cover, so no compiler fact places it inside or outside a tracked region"
        )
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
                "{origin_context} reads the {}",
                reactive_value_label(&read.kind)
            ),
            location: Some(origin.clone()),
        });
        evidence.push(EvidenceStep {
            message: format!(
                "the call to {} propagates that read into {}",
                read.via,
                if !read.context.is_empty() {
                    &read.context
                } else if read.execution == crate::ExecutionRole::ModuleInitialization {
                    "module scope"
                } else {
                    "rendering function"
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
        DirectMutationTarget::AccessorBinding => (
            format!(
                "reactive accessor {name:?} is reassigned; the binding is replaced and every subscriber keeps reading the old accessor"
            ),
            format!("Call the setter returned beside it instead of rebinding {name}."),
        ),
        DirectMutationTarget::Store => (
            format!(
                "{name:?} is a reactive store and is written through directly; Solid hands out a readonly proxy, so the write is dropped and nothing re-runs"
            ),
            store_hint(name),
        ),
        DirectMutationTarget::Props | DirectMutationTarget::ReactiveValue => {
            let object = if target == DirectMutationTarget::Props {
                "props object"
            } else {
                "value"
            };
            (
                format!(
                    "{name:?} is a reactive {object} and is written through directly; Solid hands out a readonly proxy, so the write is dropped and nothing re-runs"
                ),
                format!(
                    "Props are readonly by design: the parent owns the value. Lift the state to the parent and pass a callback down, rather than assigning to {name}."
                ),
            )
        }
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
mod tests {
    use super::{Finding, RuleMetadata, strict_read_evidence, strict_read_message};
    use crate::{ExecutionRole, OwnerRequirement, ReactiveRead};
    use typefacts::Location;

    fn location(start: u64) -> Location {
        Location {
            path: "app.tsx".into(),
            start_byte: start,
            end_byte: start + 1,
        }
    }

    fn read(missing_jsx_census: bool) -> ReactiveRead {
        ReactiveRead {
            kind: "accessor".into(),
            accessor: "count".into(),
            location: location(20),
            declaration: location(10),
            execution: ExecutionRole::UntrackedRendering,
            context: "Panel".into(),
            via: "".into(),
            origin: None,
            origin_context: "".into(),
            uncertain: false,
            missing_jsx_census,
            divergent_lowering: None,
        }
    }

    /// The census-gap wording *is* the precision claim of the escalation: a
    /// finding that says "outside every compiler-tracked JSX region" asserts a
    /// completed search of facts that were never collected. The two census-gap
    /// fixtures pin the message through the snapshot (they are in coverage's
    /// `KEEPS_WORDING` set), but nothing in the snapshot carries evidence
    /// steps, so the last evidence sentence is pinned here or nowhere.
    #[test]
    fn a_census_gap_never_claims_a_completed_search_of_compiler_facts() {
        let overstatement = "outside every compiler-tracked JSX region";

        let ordinary = read(false);
        assert!(strict_read_message(&ordinary).contains("which does not track"));
        assert!(
            strict_read_evidence(&ordinary)
                .last()
                .unwrap()
                .message
                .contains(overstatement),
            "a read with a census entry keeps the completed-search evidence"
        );

        let gap = read(true);
        let message = strict_read_message(&gap);
        assert!(
            message.contains("execution census does not cover"),
            "the census gap must be named in the message: {message}"
        );
        assert!(
            !message.contains("which does not track") && !message.contains("never updates when"),
            "the message must not claim the read never updates: {message}"
        );
        let evidence = strict_read_evidence(&gap);
        let last = &evidence.last().unwrap().message;
        assert!(
            !last.contains(overstatement),
            "the evidence must not claim a completed search: {last}"
        );
        assert!(
            last.contains("no compiler fact places it inside or outside a tracked region"),
            "the evidence must state the missing fact: {last}"
        );
    }

    /// The interprocedural arm reaches the same sentence through a different
    /// subject, so it gets its own assertion rather than riding on the direct
    /// one.
    #[test]
    fn the_census_gap_evidence_covers_the_propagated_read_too() {
        let mut gap = read(true);
        gap.via = "title".into();
        gap.origin = Some(location(4));
        let evidence = strict_read_evidence(&gap);
        let last = &evidence.last().unwrap().message;
        assert!(
            last.starts_with("the call sits inside a JSX expression the compiler's execution census does not cover"),
            "unexpected evidence for the propagated read: {last}"
        );
        assert!(strict_read_message(&gap).contains("read through title"));
    }

    /// The void-child divergence and the census gap are two different claims
    /// and must not borrow each other's sentence. The gap says the compiler was
    /// silent; the divergence says two compilers spoke and disagreed. Reporting
    /// the second with the first's wording would tell the user to look for a
    /// missing fact that is in fact present.
    #[test]
    fn a_void_child_divergence_names_the_disagreement_not_a_missing_fact() {
        let mut divergent = read(false);
        divergent.divergent_lowering = Some(crate::DivergentLowering::VoidElementChild);
        // The role the pinned fork's census actually assigns. The wording must
        // not depend on it: the whole point is that this role is not evidence.
        divergent.execution = ExecutionRole::TrackedJsx;

        let message = strict_read_message(&divergent);
        assert!(
            message.contains("void element")
                && message.contains("deletes a void element's child list"),
            "the divergence must be named in the message: {message}"
        );
        assert!(
            !message.contains("does not cover"),
            "the divergence is not a census gap; the census entry is present: {message}"
        );
        assert!(
            !message.contains("which does not track") && !message.contains("never updates when"),
            "the message must not claim the read never updates: {message}"
        );

        let evidence = strict_read_evidence(&divergent);
        let last = &evidence.last().unwrap().message;
        assert!(
            !last.contains("outside every compiler-tracked JSX region"),
            "the evidence must not claim a completed search: {last}"
        );
        assert!(
            last.contains("void element's child") && last.contains("which one builds this project"),
            "the evidence must state the disagreement: {last}"
        );
    }

    /// The ownership consumer of the same divergence. An owner requirement whose
    /// call sits in a divergently lowered child must not be a **violation**: the
    /// pinned producer wraps the insert it emits (so the call runs owned) and the
    /// parity target deletes the child (so it never runs), and neither compiler
    /// produces the unowned live operation the violation asserts.
    ///
    /// Pinned here as well as in the fixture because the snapshot carries no
    /// evidence steps and no hint, and because the *absence* of the ordinary
    /// no-owner sentence is the claim — an appended clause would leave the
    /// finding still asserting a completed search for an owner.
    #[test]
    fn a_divergent_child_makes_an_owner_requirement_uncertifiable_not_a_violation() {
        let metadata = RuleMetadata {
            code: "SC4001",
            name: "missing-owner",
            severity: "error",
            uncertifiable: false,
            default_enabled: true,
            presets: &[],
        };
        let requirement = |divergent_lowering: Option<crate::DivergentLowering>| OwnerRequirement {
            operation: crate::OwnerRequirementOperation::Cleanup,
            location: location(20),
            uncertain: divergent_lowering.is_some(),
            runtime_uncertain: false,
            caller_uncertain: false,
            conditional_owner: false,
            component_uncertain: false,
            divergent_lowering,
            report: true,
        };

        let ordinary = Finding::for_owner_requirement(
            metadata,
            &requirement(None),
            "onCleanup is called without a reactive owner",
            "Register it under a component or root.",
        );
        assert_eq!(ordinary.kind, "violation");
        assert!(
            ordinary.evidence[0]
                .message
                .contains("no containing component, computation, or root owner dominates"),
            "the ordinary requirement keeps its completed-search evidence: {:?}",
            ordinary.evidence[0].message
        );

        let divergent = Finding::for_owner_requirement(
            metadata,
            &requirement(Some(crate::DivergentLowering::VoidElementChild)),
            "onCleanup is called without a reactive owner",
            "Register it under a component or root.",
        );
        assert_eq!(divergent.kind, "uncertifiable");
        assert_eq!(divergent.severity, "error");
        assert!(
            divergent
                .message
                .contains("children of an HTML void element")
                && divergent
                    .message
                    .contains("depends on which compiler builds this project"),
            "the divergence must be named in the message: {}",
            divergent.message
        );
        assert!(
            !divergent.message.contains("this function is exported"),
            "a divergence is not caller uncertainty: {}",
            divergent.message
        );
        assert_eq!(divergent.evidence.len(), 1);
        assert!(
            !divergent.evidence[0]
                .message
                .contains("no containing component, computation, or root owner dominates"),
            "the evidence must not assert a completed search for an owner: {:?}",
            divergent.evidence[0].message
        );

        let noscript = Finding::for_owner_requirement(
            metadata,
            &requirement(Some(crate::DivergentLowering::NoscriptChild)),
            "onCleanup is called without a reactive owner",
            "Register it under a component or root.",
        );
        assert!(
            noscript.message.contains("children of a `<noscript>`")
                && noscript
                    .message
                    .contains("never lowers `<noscript>` children in any position"),
            "the noscript arm keeps its own reason here too: {}",
            noscript.message
        );
    }

    /// The `<noscript>` arm (the fork's divergence 3) must not borrow the void
    /// arm's sentence. "Deletes it" is false of `<noscript>`: the shipped
    /// compiler does not delete a child it kept, it never lowers the subtree at
    /// all. Two divergences, two reasons, two sentences.
    ///
    /// Pinned here because the fixture coverage for this arm is narrower than
    /// the void arm's: the 1.x producer exits 2 on the `<noscript>` fast-path
    /// shape, so no fixture pins the retracting position under 1.x.
    #[test]
    fn the_noscript_divergence_has_its_own_reason_not_the_void_ones() {
        let mut noscript = read(false);
        noscript.divergent_lowering = Some(crate::DivergentLowering::NoscriptChild);
        noscript.execution = ExecutionRole::TrackedJsx;

        let message = strict_read_message(&noscript);
        assert!(
            message.contains("`<noscript>`")
                && message.contains("never lowers `<noscript>` children in any position"),
            "the noscript divergence must state its own reason: {message}"
        );
        assert!(
            !message.contains("void element") && !message.contains("does not cover"),
            "it is neither the void arm nor a census gap: {message}"
        );

        let evidence = strict_read_evidence(&noscript);
        let last = &evidence.last().unwrap().message;
        assert!(
            last.contains("`<noscript>`'s child")
                && !last.contains("outside every compiler-tracked JSX region"),
            "unexpected noscript evidence: {last}"
        );
    }
}
