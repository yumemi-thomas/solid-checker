//! The Solid 1.x rule catalog: projects the reactive IR's `Program` onto the
//! dialect's findings.
//!
//! The engine's analysis is shared with the 2.0 dialect; what is dialect-
//! specific here is which tables can fire at all under 1.x vocabulary, the
//! external rule names (`v1/<rule>`), and every sentence of message and hint —
//! a 1.x diagnostic never tells its reader to call an API their Solid version
//! does not have.

mod rules;

use solid_reactive_ir::{ExecutionRole, Program};
use std::time::Instant;

pub use rules::{DOCS_BASE_URL, Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

#[must_use]
pub fn solve(program: &Program) -> Vec<Finding> {
    solve_measured(program).0
}

#[must_use]
pub fn solve_measured(program: &Program) -> (Vec<Finding>, SolveTimings) {
    let total_started = Instant::now();
    let construction_started = Instant::now();
    // Tables the 1.x vocabulary can never populate are deliberately not read:
    // `actions` and `async_reads` come from 2.0-only primitives, and the
    // cleanup-return tables from the returned-cleanup form 1.x does not have
    // (`accepts_cleanup_return` is empty for every 1.x primitive).
    let mut findings = program
        .reads
        .iter()
        .filter(|read| {
            matches!(
                read.execution,
                ExecutionRole::UntrackedRendering | ExecutionRole::EffectApply
            )
        })
        .map(|read| Finding {
            analysis_context: read.context.to_string(),
            subject_kind: read.kind.to_string(),
            related_locations: strict_read_related_locations(read),
            evidence: strict_read_evidence(read),
            hint: "Move the read into a tracking scope: JSX, a createMemo, or the callback of createEffect(fn). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit.".into(),
            ..Finding::new(
                Rule::StrictReadUntracked.metadata(),
                strict_read_message(read),
                read.location.clone(),
            )
        })
        .collect::<Vec<_>>();
    findings.extend(
        program
            .writes
            .iter()
            .filter(|write| !write.allowed_by_option && !allowed_write_role(write.execution))
            .map(|write| {
                let context = if write.context.is_empty() {
                    "owned scope"
                } else {
                    &write.context
                };
                Finding {
                    analysis_context: context.into(),
                    related_locations: vec![write.declaration.clone()],
                    evidence: vec![
                        EvidenceStep {
                            message: format!(
                                "{:?} is the setter returned by createSignal or createStore",
                                write.setter
                            ),
                            location: Some(write.declaration.clone()),
                        },
                        EvidenceStep {
                            message: "this scope is tracked; writes are only allowed in event handlers, onMount, and other callbacks that run outside the current computation"
                                .into(),
                            location: Some(write.location.clone()),
                        },
                    ],
                    hint: "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, onMount, or a callback that runs after the current computation.".into(),
                    ..Finding::new(
                        Rule::ReactiveWriteInOwnedScope.metadata(),
                        format!(
                            "signal setter {:?} is called inside tracked scope {context}; writes during the tracking phase re-trigger the computation that made them and can loop the reactive graph",
                            write.setter
                        ),
                        write.location.clone(),
                    )
                }
            }),
    );
    findings.extend(program.leaf_operations.iter().map(|operation| {
        let (rule, message, hint) = if operation.primitive == "onCleanup" {
            (
                Rule::CleanupInForbiddenScope,
                format!(
                    "onCleanup is called inside {}, whose callback runs as a leaf with no owner to register cleanup on; the cleanup function will never run",
                    operation.owner
                ),
                format!(
                    "Register the cleanup in the computation that owns the {} instead, or create the surrounding scope with createRoot so disposal exists.",
                    operation.owner
                ),
            )
        } else {
            (
                Rule::PrimitiveInLeafOwner,
                format!(
                    "reactive primitive {} is created inside {}; {} runs its callback as a leaf owner with no children, so nested primitives are never tracked or disposed",
                    operation.primitive, operation.owner, operation.owner
                ),
                format!(
                    "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                    operation.owner
                ),
            )
        };
        Finding {
            evidence: vec![EvidenceStep {
                message: format!(
                    "the call is lexically contained by the {} callback",
                    operation.owner
                ),
                location: Some(operation.location.clone()),
            }],
            fixes: operation.fix.clone().into_iter().collect(),
            hint,
            ..Finding::new(rule.metadata(), message, operation.location.clone())
        }
    }));
    findings.extend(program.static_violations.iter().map(|violation| {
        let rule = Rule::from_identity(&violation.id, &violation.rule).unwrap_or_else(|| {
            panic!(
                "diagnostic identity is missing from the v1 rule catalog: {} [{}]",
                violation.id, violation.rule
            )
        });
        Finding {
            analysis_context: violation.analysis_context.clone(),
            evidence: vec![EvidenceStep {
                message: match rule {
                    Rule::NoDestructure => {
                        "the destructuring pattern is bound to proven component props".into()
                    }
                    Rule::ComponentsReturnOnce => {
                        "a proven reactive read controls the component's return shape".into()
                    }
                    Rule::PackageContractExportMissing => {
                        "the imported package has a contract, but this export has no effect summary"
                            .into()
                    }
                    _ => "the invalid API shape is statically present at this call".into(),
                },
                location: Some(violation.location.clone()),
            }],
            fixes: violation.fixes.clone(),
            hint: violation.hint.clone(),
            ..Finding::new(
                rule.metadata(),
                violation.message.clone(),
                violation.location.clone(),
            )
        }
    }));
    findings.extend(program.directive_creations.iter().map(|creation| Finding {
        evidence: vec![EvidenceStep {
            message: if creation.returned_closure {
                "the primitive is created inside the callback returned to a compiler-recognized ref application".into()
            } else {
                "the primitive is created inside a compiler-recognized ref application callback".into()
            },
            location: Some(creation.location.clone()),
        }],
        hint: "Use the two-phase directive factory: create primitives and subscriptions in the setup phase (the factory body, which runs in an owned scope) and keep the returned ref callback to DOM work only.".into(),
        ..Finding::new(
            Rule::PrimitiveInDirectiveApplication.metadata(),
            format!(
                "reactive primitive {} is created in a directive application callback; the apply phase runs per element as an unowned leaf, so primitives created here are never tracked or disposed",
                creation.primitive
            ),
            creation.location.clone(),
        )
    }));
    findings.extend(program.missing_owners.iter().filter_map(|requirement| {
        if !requirement.report {
            return None;
        }
        let (rule, message, hint) = match requirement.operation.as_str() {
            "cleanup" => (
                Rule::NoOwnerCleanup,
                "onCleanup is called without a reactive owner; no scope's disposal can trigger it, so this cleanup function will never run",
                "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists.",
            ),
            "boundary" => (
                Rule::NoOwnerBoundary,
                "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
            ),
            _ => (
                Rule::NoOwnerEffect,
                "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
            ),
        };
        let uncertain = requirement.uncertain;
        Some(Finding {
            kind: if uncertain {
                "uncertifiable".into()
            } else {
                "violation".into()
            },
            severity: if uncertain {
                "error".into()
            } else {
                rule.metadata().severity.into()
            },
            evidence: vec![EvidenceStep {
                message: "no containing component, computation, or root owner dominates this operation".into(),
                location: Some(requirement.location.clone()),
            }],
            hint: if uncertain {
                format!(
                    "{hint} If every caller runs this exported function under an owner, document that in the package's reactivity contract."
                )
            } else {
                hint.into()
            },
            ..Finding::new(
                rule.metadata(),
                if uncertain {
                    format!(
                        "{message}; this function is exported, so solid-checker cannot prove its callers provide an owner"
                    )
                } else {
                    message.into()
                },
                requirement.location.clone(),
            )
        })
    }));
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

const fn allowed_write_role(role: ExecutionRole) -> bool {
    matches!(
        role,
        ExecutionRole::EventCallback
            | ExecutionRole::DeferredCallback
            | ExecutionRole::EffectApply
            | ExecutionRole::DirectiveApply
    )
}

fn strict_read_message(read: &solid_reactive_ir::ReactiveRead) -> String {
    let context = if read.context.is_empty() {
        "rendering function"
    } else {
        &read.context
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

fn strict_read_evidence(read: &solid_reactive_ir::ReactiveRead) -> Vec<EvidenceStep> {
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
                if read.context.is_empty() {
                    "rendering function"
                } else {
                    &read.context
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

fn strict_read_related_locations(
    read: &solid_reactive_ir::ReactiveRead,
) -> Vec<typefacts::Location> {
    let mut locations = vec![read.declaration.clone()];
    if let Some(origin) = &read.origin {
        locations.push(origin.clone());
    }
    locations
}
