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
            hint: "Move the read into a tracking scope: JSX, a createMemo, or the compute function of createEffect(compute, apply). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit. Solid warns STRICT_READ_UNTRACKED here in dev.".into(),
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
                let refresh = write.setter.starts_with("refresh(");
                let (message, hint, provenance) = if refresh {
                    (
                        format!(
                            "refresh() is called inside owned scope {context}; a write transaction cannot start while the graph is tracking, and Solid throws here in dev"
                        ),
                        "Move the refresh() call to an event handler, an action, onSettled, or another imperative scope; a recompute cannot be requested from inside the tracking phase.".to_owned(),
                        "the refresh target is a proven Solid source accessor or store".to_owned(),
                    )
                } else {
                    (
                        format!(
                            "signal setter {:?} is called inside owned scope {context}; writes during the tracking phase create feedback loops in the reactive graph, and Solid throws SIGNAL_WRITE_IN_OWNED_SCOPE here in dev",
                            write.setter
                        ),
                        "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, an action, onSettled, or the apply function of createEffect(compute, apply). For internal signals only, opt in with createSignal(value, { ownedWrite: true }).".to_owned(),
                        format!(
                            "{:?} is the setter returned by createSignal or createStore",
                            write.setter
                        ),
                    )
                };
                Finding {
                    analysis_context: context.into(),
                    related_locations: vec![write.declaration.clone()],
                    evidence: vec![
                        EvidenceStep {
                            message: provenance,
                            location: Some(write.declaration.clone()),
                        },
                        EvidenceStep {
                            message: "this scope is owned (tracking phase); writes are only allowed in event handlers, actions, onSettled, and effect apply callbacks"
                                .into(),
                            location: Some(write.location.clone()),
                        },
                    ],
                    hint,
                    ..Finding::new(
                        Rule::ReactiveWriteInOwnedScope.metadata(),
                        message,
                        write.location.clone(),
                    )
                }
            }),
    );
    findings.extend(program.leaf_operations.iter().map(|operation| {
        let (rule, message, hint) = match operation.primitive.as_str() {
            "onCleanup" => (
                Rule::CleanupInForbiddenScope,
                format!(
                    "onCleanup is called inside {}, a leaf owner that manages cleanup through its return value; Solid throws CLEANUP_IN_FORBIDDEN_SCOPE here in dev",
                    operation.owner
                ),
                format!(
                    "Return the cleanup function from the {} callback instead: do the setup, then return () => teardown().",
                    operation.owner
                ),
            ),
            "flush" => (
                Rule::FlushInForbiddenScope,
                format!(
                    "flush() is called inside {}, which runs as part of the flush cycle itself; the call would re-enter the scheduler, and Solid throws here in dev",
                    operation.owner
                ),
                format!(
                    "Inside {} the graph has already settled, so signal values and the DOM are current and the flush() is usually unnecessary. If you need to observe a write you just made, move both the write and the flush() to the event handler or imperative boundary that triggered this scope.",
                    operation.owner
                ),
            ),
            _ => (
                Rule::PrimitiveInLeafOwner,
                format!(
                    "reactive primitive {} is created inside {}; {} is a leaf owner with no children, so nested primitives are never tracked or disposed, and Solid throws in dev",
                    operation.primitive, operation.owner, operation.owner
                ),
                format!(
                    "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                    operation.owner
                ),
            ),
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
    findings.extend(
        program
            .invalid_cleanup_returns
            .iter()
            .map(|invalid| Finding {
                evidence: vec![EvidenceStep {
                    message: "the callback statically returns a non-function value, including an implicit Promise from an async callback".into(),
                    location: Some(invalid.location.clone()),
                }],
                hint: "Return a cleanup function or nothing at all. An async callback can never return valid cleanup because it implicitly returns a Promise; make the callback synchronous and start the async work inside it.".into(),
                ..Finding::new(
                    Rule::InvalidCleanupReturn.metadata(),
                    format!(
                        "{} callback returns a value that is not a cleanup function; Solid treats this return value as cleanup, and anything other than a function or undefined throws in dev",
                        invalid.primitive
                    ),
                    invalid.location.clone(),
                )
            }),
    );
    findings.extend(
        program
            .unresolved_cleanup_returns
            .iter()
            .map(|unresolved| Finding {
                evidence: vec![EvidenceStep {
                    message: format!(
                        "the return value of the {} callback cannot be resolved statically",
                        unresolved.primitive
                    ),
                    location: Some(unresolved.location.clone()),
                }],
                hint: "Make the return shape explicit at each return site: return a function literal, a named local function, or nothing. Returns of member expressions, call results, or values that cross files defeat this analysis.".into(),
                ..Finding::new(
                    Rule::CleanupReturnUnresolved.metadata(),
                    format!(
                        "cannot prove that the {} callback returns only a cleanup function or undefined; an unresolved return value may throw at runtime",
                        unresolved.primitive
                    ),
                    unresolved.location.clone(),
                )
            }),
    );
    findings.extend(program.static_violations.iter().map(|violation| {
        let rule = Rule::from_identity(&violation.id, &violation.rule).unwrap_or_else(|| {
            panic!(
                "diagnostic identity is missing from the rule catalog: {} [{}]",
                violation.id, violation.rule
            )
        });
        Finding {
            analysis_context: violation.analysis_context.clone(),
            evidence: vec![EvidenceStep {
                message: if violation.rule == "component-props-destructure" {
                    "the destructuring pattern is bound to proven component props".into()
                } else if violation.rule == "component-returns-conditionally" {
                    "a proven reactive read controls the component's return shape".into()
                } else if violation.rule == "package-contract-export-missing" {
                    "the imported package has a contract, but this export has no effect summary"
                        .into()
                } else {
                    "the invalid API shape is statically present at this call".into()
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
                "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists. For one-time setup with teardown, use onSettled with a returned cleanup in a component.",
            ),
            "boundary" => (
                Rule::NoOwnerBoundary,
                "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
            ),
            "settled-cleanup" => (
                Rule::SettledCleanupUnowned,
                "onSettled returns a cleanup function in a scope with no owner to register it on; the cleanup is silently dropped and will never run",
                "Call onSettled where an owner is active (a component body or computation), or wrap the scope in createRoot. Inside event handlers a returned cleanup is not supported; do the teardown explicitly instead.",
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
    findings.extend(program.async_reads.iter().filter_map(|read| {
        let (rule, message, hint) = if let Some(owner) = &read.leaf_owner {
            (
                Rule::PendingAsyncForbiddenScope,
                format!(
                    "pending async accessor {:?} is read inside {}, which runs after the graph settles and cannot suspend; a pending read here throws at runtime",
                    read.accessor, owner
                ),
                format!(
                    "Settle the value before it reaches {owner}: read the accessor in the compute function of createEffect(compute, apply) and pass the resolved value through, or guard the scope so it only runs once the data is ready."
                ),
            )
        } else if read.execution == ExecutionRole::UntrackedRendering {
            (
                Rule::PendingAsyncUntrackedRead,
                format!(
                    "pending async accessor {:?} is read outside a tracking scope; an untracked read cannot suspend or retry, and Solid throws PENDING_ASYNC_UNTRACKED_READ in dev",
                    read.accessor
                ),
                "Read async values where the graph can wait for them: JSX, a createMemo, or an effect's compute function. The read then suspends to the nearest <Loading> boundary and re-runs when the value settles.".to_owned(),
            )
        } else if read.execution == ExecutionRole::TrackedJsx && !read.under_loading {
            (
                Rule::AsyncOutsideLoadingBoundary,
                format!(
                    "async accessor {:?} is rendered without a Loading boundary above it; while it is pending nothing renders, and the mount is deferred until all uncaught async settles (Solid dev warning ASYNC_OUTSIDE_LOADING_BOUNDARY)",
                    read.accessor
                ),
                "This is safe but shows nothing while loading. Wrap the reading subtree in <Loading fallback={...}> for visible fallback UI, or leave it as is if an empty container during load is intended. For a revalidation indicator, use isPending(() => ...) under the same boundary.".to_owned(),
            )
        } else {
            return None;
        };
        Some(Finding {
            related_locations: vec![read.declaration.clone()],
            evidence: vec![
                EvidenceStep {
                    message: "the accessor is returned by an async computation".into(),
                    location: Some(read.declaration.clone()),
                },
                EvidenceStep {
                    message: message.clone(),
                    location: Some(read.location.clone()),
                },
            ],
            hint,
            ..Finding::new(rule.metadata(), message, read.location.clone())
        })
    }));
    findings.extend(
        program
            .actions
            .iter()
            .filter(|action| !allowed_write_role(action.execution))
            .map(|action| Finding {
                evidence: vec![EvidenceStep {
                    message: "invoking an action starts a write transaction while an owner is active"
                        .into(),
                    location: Some(action.location.clone()),
                }],
                hint: "Call the action from an event handler, onSettled, or another imperative boundary. To load data reactively you don't need an action: return the Promise from a computation and read it under a <Loading> boundary.".into(),
                ..Finding::new(
                    Rule::ActionCalledInOwnedScope.metadata(),
                    format!(
                        "action {:?} is called inside owned scope {}; invoking an action starts a write transaction (optimistic writes, refresh) while the graph is still tracking, which re-triggers the scope that called it",
                        action.action, action.context
                    ),
                    action.location.clone(),
                )
            }),
    );
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
