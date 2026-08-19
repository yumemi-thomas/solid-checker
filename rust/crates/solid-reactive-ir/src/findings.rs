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
        out.push_str(&format!(
            "    {{ \"code\": \"{}\", \"name\": \"{}\", \"severity\": \"{}\", \"uncertifiable\": {} }}{}\n",
            metadata.code,
            metadata.name,
            metadata.severity,
            metadata.uncertifiable,
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
        // `uncertain` predates the reason fields. Treat a deserialized legacy
        // row with no explicit reason as caller uncertainty.
        let caller_uncertain = requirement.caller_uncertain
            || (uncertain && !runtime_uncertain && !conditional_owner && !component_uncertain);
        let mut message = message.to_string();
        let mut hint = hint.to_string();
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
            message: "the call is outside every compiler-tracked JSX region and deferred callback"
                .into(),
            location: Some(read.location.clone()),
        });
    } else {
        evidence.push(EvidenceStep {
            message: "the cross-file reference resolves to that accessor declaration".into(),
            location: Some(read.location.clone()),
        });
        evidence.push(EvidenceStep {
            message: "the read is outside every compiler-tracked JSX region and deferred callback"
                .into(),
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
