use super::*;
use crate::{
    EvidenceCatalog, SandboxIdentity, emit_evidence_sidecars,
    evidence_sidecars::EvidenceSidecarError,
};
use solid_reactive_ir::contract_semantics::{ClaimDomain, ClaimPath};

const SIGNAL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../benchmarks/package-contract-v2/phase6/signal-pair-complete.json"
));

fn digest(byte: char) -> Digest {
    Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

fn tool(name: &str, byte: char) -> ToolIdentity {
    ToolIdentity {
        name: name.into(),
        version: "1.0.0".into(),
        build: digest(byte),
        protocol: Some("1".into()),
    }
}

fn contract() -> NormalizedContract {
    crate::contract_document::decode(SIGNAL)
        .unwrap()
        .normalize()
        .unwrap()
}

fn subjects(
    contract: &NormalizedContract,
) -> (SemanticClaimSubject, SemanticClaimSubject, OperationId) {
    let case = &contract.artifact_cases()[0];
    let operation = case.exports["createSignal"].call.operations[0].id.clone();
    (
        SemanticClaimSubject {
            artifact_case: case.id.clone(),
            export: "createSignal".into(),
            path: SemanticClaimPath::Operation(operation.clone()),
        },
        SemanticClaimSubject {
            artifact_case: case.id.clone(),
            export: "createSignal".into(),
            path: SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Reads)),
        },
        operation,
    )
}

fn environment(conditions: &[&str]) -> EnvironmentIdentity {
    EnvironmentIdentity {
        runtime: tool("node", '1'),
        os: "linux".into(),
        architecture: "x64".into(),
        conditions: conditions
            .iter()
            .map(|condition| (*condition).into())
            .collect(),
        sandbox: SandboxIdentity {
            kind: SandboxKind::Process,
            policy: Some(digest('2')),
        },
    }
}

fn mode(contract: &NormalizedContract, name: &str, conditions: &[&str]) -> ProbeMode {
    ProbeMode {
        name: name.into(),
        artifact_case: contract.artifact_cases()[0].id.clone(),
        environment: environment(conditions),
    }
}

fn policy() -> ProbePolicy {
    ProbePolicy {
        repeat_runs: 2,
        timeout_millis: 5_000,
        max_microtask_turns: 8,
        max_macrotask_turns: 4,
        max_events: 128,
    }
}

fn recipe(
    subject: SemanticClaimSubject,
    authority: ProbeAuthority,
    scenario: ProbeScenario,
    class: ProbeEventClass,
    marker: &str,
    operation: Option<OperationId>,
) -> ProbeRecipe {
    ProbeRecipe {
        subject,
        authority,
        scenario,
        construction: digest('3'),
        expected_event: ProbeEventMatch {
            marker: marker.into(),
            class,
            operation,
        },
        drain: vec![
            DrainStep::Flush,
            DrainStep::Microtasks { max_turns: 2 },
            DrainStep::Macrotasks { max_turns: 1 },
        ],
        coverage_limitations: vec!["one exact mode".into()],
    }
}

fn plan_with(
    modes: Vec<ProbeMode>,
    recipe: ProbeRecipe,
    witness: SemanticClaimSubject,
    closure: SemanticClaimSubject,
) -> RuntimeProbePlan {
    let contract = contract();
    let matrix = ArtifactModeMatrix::new(&contract, modes).unwrap();
    RuntimeProbePlan::build(
        contract,
        [witness].into_iter().collect(),
        [closure].into_iter().collect(),
        matrix,
        vec![recipe],
        policy(),
    )
    .unwrap()
}

fn event(
    sequence: u32,
    marker: &str,
    operation: Option<OperationId>,
    kind: ProbeEventKind,
) -> ProbeEvent {
    ProbeEvent {
        sequence,
        marker: marker.into(),
        operation,
        kind,
    }
}

fn operation_events(operation: &OperationId, marker: &str) -> Vec<ProbeEvent> {
    vec![
        event(
            0,
            "call-enter",
            None,
            ProbeEventKind::Call {
                phase: BoundaryPhase::Enter,
            },
        ),
        event(
            1,
            marker,
            Some(operation.clone()),
            ProbeEventKind::Callback { ordinal: 0 },
        ),
        event(
            2,
            "call-exit",
            None,
            ProbeEventKind::Call {
                phase: BoundaryPhase::Exit,
            },
        ),
    ]
}

fn runs_with(
    plan: &RuntimeProbePlan,
    events: impl Fn(&ProbeSessionRequest) -> Vec<ProbeEvent>,
) -> Vec<ProbeRun> {
    plan.sessions
        .iter()
        .enumerate()
        .map(|(index, session)| ProbeRun {
            session: session.id.clone(),
            environment: session.mode.environment.clone(),
            isolation: IsolationIdentity {
                process: format!("process-{index}"),
                realm: format!("realm-{index}"),
                module_instance: format!("module-{index}"),
            },
            drained_microtasks: 2,
            drained_macrotasks: 1,
            outcome: ProbeRunOutcome::Completed {
                events: events(session),
            },
        })
        .collect()
}

#[test]
fn exact_artifact_mode_matrix_builds_bounded_isolated_repeat_sessions() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let modes = vec![
        mode(&contract, "browser-production", &["production", "browser"]),
        mode(
            &contract,
            "browser-development",
            &["development", "browser"],
        ),
    ];
    let plan = plan_with(
        modes,
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "callback",
            Some(operation),
        ),
        witness,
        closure,
    );
    assert_eq!(plan.sessions().len(), 4);
    assert_eq!(
        plan.sessions()
            .iter()
            .map(|session| session.id())
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert!(
        plan.sessions()
            .iter()
            .all(|session| session.policy().repeat_runs == 2)
    );
    assert!(plan.sessions().iter().all(|session| {
        session.drain().iter().all(|step| {
            matches!(
                step,
                DrainStep::Flush | DrainStep::Microtasks { .. } | DrainStep::Macrotasks { .. }
            )
        })
    }));
}

#[test]
fn transcript_uses_semantic_markers_for_all_event_families_and_is_deterministic() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "server", &["node"])],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );
    let resource = ResourceId("resource".into());
    let all_events = vec![
        event(
            0,
            "call",
            None,
            ProbeEventKind::Call {
                phase: BoundaryPhase::Enter,
            },
        ),
        event(
            1,
            "render",
            None,
            ProbeEventKind::Render {
                phase: BoundaryPhase::Enter,
            },
        ),
        event(2, "flush", None, ProbeEventKind::Flush { ordinal: 0 }),
        event(
            3,
            "witness",
            Some(operation),
            ProbeEventKind::Callback { ordinal: 0 },
        ),
        event(
            4,
            "cleanup",
            None,
            ProbeEventKind::Cleanup {
                phase: CleanupPhase::Invoked,
                root_lifetime: false,
            },
        ),
        event(
            5,
            "settlement",
            None,
            ProbeEventKind::Settlement {
                resource: Some(resource.clone()),
                state: SettlementState::Settled,
            },
        ),
        event(
            6,
            "emission",
            None,
            ProbeEventKind::Emission {
                resource: resource.clone(),
                index: 0,
            },
        ),
        event(
            7,
            "transition",
            None,
            ProbeEventKind::Transition {
                resource: resource.clone(),
                state: TransitionState::Active,
            },
        ),
        event(
            8,
            "request",
            None,
            ProbeEventKind::Request {
                resource: resource.clone(),
                phase: BoundaryPhase::Enter,
            },
        ),
        event(
            9,
            "response",
            None,
            ProbeEventKind::Response {
                resource: resource.clone(),
                state: ResponseState::Committed,
            },
        ),
        event(
            10,
            "stream",
            None,
            ProbeEventKind::Stream {
                resource,
                state: StreamState::Closed,
            },
        ),
    ];
    let runs = runs_with(&plan, |_| all_events.clone());
    let first = evaluate_runtime_probes(&plan, runs.clone(), tool("probe-runner", '4')).unwrap();
    let mut reversed = runs;
    reversed.reverse();
    let second = evaluate_runtime_probes(&plan, reversed, tool("probe-runner", '4')).unwrap();
    assert_eq!(first, second);
    assert!(matches!(
        first.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Witness { .. }
    ));
    let text = std::str::from_utf8(first.transcripts()[0].bytes()).unwrap();
    for kind in [
        "call",
        "render",
        "flush",
        "callback",
        "cleanup",
        "settlement",
        "emission",
        "transition",
        "request",
        "response",
        "stream",
    ] {
        assert!(text.contains(&format!("\"kind\": \"{kind}\"")), "{kind}");
    }
    assert!(text.contains(PROBE_TRANSCRIPT_FORMAT));
    assert!(text.contains(&format!(
        "\"transcriptVersion\": {PROBE_TRANSCRIPT_VERSION}"
    )));
}

#[test]
fn finite_absence_timeout_error_and_inconsistent_repeats_never_promote_negative_facts() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "client", &["browser"])],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "required",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );

    let missing = evaluate_runtime_probes(
        &plan,
        runs_with(&plan, |_| operation_events(&operation, "different-marker")),
        tool("runner", '5'),
    )
    .unwrap();
    assert!(matches!(
        missing.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));
    assert!(missing.contradictions().is_empty());

    let mut timeout_runs = runs_with(&plan, |_| operation_events(&operation, "required"));
    timeout_runs[0].outcome = ProbeRunOutcome::Timeout;
    let timeout = evaluate_runtime_probes(&plan, timeout_runs, tool("runner", '5')).unwrap();
    assert_eq!(
        timeout.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Timeout {
            limit_millis: 5_000
        }
    );

    let mut error_runs = runs_with(&plan, |_| operation_events(&operation, "required"));
    error_runs[0].outcome = ProbeRunOutcome::Error {
        details: digest('6'),
    };
    let error = evaluate_runtime_probes(&plan, error_runs, tool("runner", '5')).unwrap();
    assert_eq!(
        error.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Error {
            details: digest('6')
        }
    );

    let mut inconsistent = runs_with(&plan, |_| operation_events(&operation, "required"));
    let ProbeRunOutcome::Completed { events } = &mut inconsistent[1].outcome else {
        unreachable!()
    };
    events[1].marker = "nondeterministic".into();
    let inconsistent = evaluate_runtime_probes(&plan, inconsistent, tool("runner", '5')).unwrap();
    assert!(matches!(
        inconsistent.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));
}

#[test]
fn process_realm_and_module_state_must_be_fresh_for_every_repeat() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "client", &["browser"])],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );
    for field in ["process", "realm", "module"] {
        let mut runs = runs_with(&plan, |_| operation_events(&operation, "witness"));
        match field {
            "process" => runs[1].isolation.process = runs[0].isolation.process.clone(),
            "realm" => runs[1].isolation.realm = runs[0].isolation.realm.clone(),
            "module" => {
                runs[1].isolation.module_instance = runs[0].isolation.module_instance.clone()
            }
            _ => unreachable!(),
        }
        let evaluated = evaluate_runtime_probes(&plan, runs, tool("runner", '7')).unwrap();
        assert!(matches!(
            evaluated.claim_material()[0].observations[0].outcome,
            ProbeOutcome::Refused { .. }
        ));
    }
}

#[test]
fn plan_and_transcript_digests_ignore_input_order_but_bind_exact_mode_identity() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let mut first_mode = mode(
        &contract,
        "browser-development",
        &["browser", "development"],
    );
    let second_mode = mode(&contract, "browser-production", &["browser", "production"]);
    let mut first_recipe = recipe(
        witness.clone(),
        ProbeAuthority::PossiblePositiveWitness,
        ProbeScenario::Operation,
        ProbeEventClass::Callback,
        "witness",
        Some(operation.clone()),
    );
    first_recipe.coverage_limitations = vec!["z limitation".into(), "a limitation".into()];
    let first = plan_with(
        vec![first_mode.clone(), second_mode.clone()],
        first_recipe,
        witness.clone(),
        closure.clone(),
    );

    first_mode.environment.conditions.reverse();
    let mut second_recipe = recipe(
        witness.clone(),
        ProbeAuthority::PossiblePositiveWitness,
        ProbeScenario::Operation,
        ProbeEventClass::Callback,
        "witness",
        Some(operation.clone()),
    );
    second_recipe.coverage_limitations = vec!["a limitation".into(), "z limitation".into()];
    let second = plan_with(
        vec![second_mode, first_mode],
        second_recipe,
        witness,
        closure,
    );
    assert_eq!(first.digest(), second.digest());
    assert_eq!(
        first
            .sessions()
            .iter()
            .map(ProbeSessionRequest::id)
            .collect::<Vec<_>>(),
        second
            .sessions()
            .iter()
            .map(ProbeSessionRequest::id)
            .collect::<Vec<_>>()
    );

    let first_evaluation = evaluate_runtime_probes(
        &first,
        runs_with(&first, |_| operation_events(&operation, "witness")),
        tool("runner", '7'),
    )
    .unwrap();
    let second_evaluation = evaluate_runtime_probes(
        &second,
        runs_with(&second, |_| operation_events(&operation, "witness")),
        tool("runner", '7'),
    )
    .unwrap();
    assert_eq!(first_evaluation, second_evaluation);

    let changed = plan_with(
        vec![mode(
            &contract,
            "browser-development",
            &["browser", "production"],
        )],
        recipe(
            subjects(&contract).0,
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation),
        ),
        subjects(&contract).0,
        subjects(&contract).1,
    );
    assert_ne!(first.digest(), changed.digest());
}

#[test]
fn wrong_environment_excess_drain_and_malformed_sequences_fail_closed_locally() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "client", &["browser"])],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );

    let mut wrong_environment = runs_with(&plan, |_| operation_events(&operation, "witness"));
    wrong_environment[0].environment.conditions = vec!["node".into()];
    let evaluated = evaluate_runtime_probes(&plan, wrong_environment, tool("runner", '7')).unwrap();
    assert!(matches!(
        evaluated.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));

    let mut excessive_drain = runs_with(&plan, |_| operation_events(&operation, "witness"));
    excessive_drain[0].drained_microtasks = 3;
    let evaluated = evaluate_runtime_probes(&plan, excessive_drain, tool("runner", '7')).unwrap();
    assert!(matches!(
        evaluated.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));

    let malformed = runs_with(&plan, |_| {
        let mut events = operation_events(&operation, "witness");
        events[1].sequence = 9;
        events
    });
    assert!(matches!(
        evaluate_runtime_probes(&plan, malformed, tool("runner", '7')),
        Err(RuntimeProbeError::InvalidPlan { .. })
    ));
}

#[test]
fn one_mode_inconsistency_does_not_contaminate_a_known_sibling_mode() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![
            mode(&contract, "development", &["browser", "development"]),
            mode(&contract, "production", &["browser", "production"]),
        ],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );
    let mut runs = runs_with(&plan, |_| operation_events(&operation, "witness"));
    let development = plan
        .sessions()
        .iter()
        .find(|session| session.mode().name == "development" && session.repeat() == 1)
        .unwrap()
        .id()
        .clone();
    let run = runs
        .iter_mut()
        .find(|run| run.session == development)
        .unwrap();
    let ProbeRunOutcome::Completed { events } = &mut run.outcome else {
        unreachable!()
    };
    events[1].marker = "different".into();
    let evaluated = evaluate_runtime_probes(&plan, runs, tool("runner", '7')).unwrap();
    let observations = &evaluated.claim_material()[0].observations;
    assert_eq!(observations[0].mode, "development");
    assert!(matches!(
        observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));
    assert_eq!(observations[1].mode, "production");
    assert!(matches!(
        observations[1].outcome,
        ProbeOutcome::Witness { .. }
    ));
}

#[test]
fn positive_marker_can_falsify_proposed_closure_but_its_absence_cannot_confirm_it() {
    let contract = contract();
    let (witness, closure, _) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "client", &["browser"])],
        recipe(
            closure.clone(),
            ProbeAuthority::ClosureFalsification,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "unlisted-callback",
            None,
        ),
        witness,
        closure,
    );
    let observed = evaluate_runtime_probes(
        &plan,
        runs_with(&plan, |_| {
            vec![event(
                0,
                "unlisted-callback",
                None,
                ProbeEventKind::Callback { ordinal: 0 },
            )]
        }),
        tool("runner", '8'),
    )
    .unwrap();
    assert!(matches!(
        observed.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Falsification { .. }
    ));
    assert_eq!(observed.contradictions().len(), 1);

    let absent = evaluate_runtime_probes(
        &plan,
        runs_with(&plan, |_| {
            vec![event(
                0,
                "call-only",
                None,
                ProbeEventKind::Call {
                    phase: BoundaryPhase::Enter,
                },
            )]
        }),
        tool("runner", '8'),
    )
    .unwrap();
    assert!(matches!(
        absent.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));
    assert!(absent.contradictions().is_empty());
}

#[test]
fn cleanup_async_iterable_transition_request_and_root_scenarios_require_lifecycle_markers() {
    let scenarios = [
        ProbeScenario::CleanupLifecycle,
        ProbeScenario::RepeatedAsyncIterable,
        ProbeScenario::TransitionLifecycle,
        ProbeScenario::RequestResponseLifecycle,
        ProbeScenario::RootLifetime,
    ];
    for scenario in scenarios {
        let contract = contract();
        let (witness, closure, operation) = subjects(&contract);
        let class = match scenario {
            ProbeScenario::CleanupLifecycle | ProbeScenario::RootLifetime => {
                ProbeEventClass::Cleanup
            }
            ProbeScenario::RepeatedAsyncIterable => ProbeEventClass::Emission,
            ProbeScenario::TransitionLifecycle => ProbeEventClass::Transition,
            ProbeScenario::RequestResponseLifecycle => ProbeEventClass::Request,
            ProbeScenario::Operation => unreachable!(),
        };
        let plan = plan_with(
            vec![mode(&contract, "client", &["browser"])],
            recipe(
                witness.clone(),
                ProbeAuthority::PossiblePositiveWitness,
                scenario,
                class,
                "witness",
                Some(operation.clone()),
            ),
            witness,
            closure,
        );
        let resource = ResourceId("scenario-resource".into());
        let events = match scenario {
            ProbeScenario::CleanupLifecycle => vec![
                event(
                    0,
                    "cleanup-produced",
                    None,
                    ProbeEventKind::Cleanup {
                        phase: CleanupPhase::Produced,
                        root_lifetime: false,
                    },
                ),
                event(
                    1,
                    "witness",
                    Some(operation),
                    ProbeEventKind::Cleanup {
                        phase: CleanupPhase::Invoked,
                        root_lifetime: false,
                    },
                ),
            ],
            ProbeScenario::RepeatedAsyncIterable => vec![
                event(
                    0,
                    "witness",
                    Some(operation),
                    ProbeEventKind::Emission {
                        resource: resource.clone(),
                        index: 0,
                    },
                ),
                event(
                    1,
                    "emission-1",
                    None,
                    ProbeEventKind::Emission {
                        resource: resource.clone(),
                        index: 1,
                    },
                ),
                event(
                    2,
                    "settled",
                    None,
                    ProbeEventKind::Settlement {
                        resource: Some(resource),
                        state: SettlementState::Settled,
                    },
                ),
            ],
            ProbeScenario::TransitionLifecycle => vec![
                event(
                    0,
                    "witness",
                    Some(operation),
                    ProbeEventKind::Transition {
                        resource: resource.clone(),
                        state: TransitionState::Active,
                    },
                ),
                event(
                    1,
                    "transition-settled",
                    None,
                    ProbeEventKind::Transition {
                        resource,
                        state: TransitionState::Settled,
                    },
                ),
            ],
            ProbeScenario::RequestResponseLifecycle => vec![
                event(
                    0,
                    "witness",
                    Some(operation),
                    ProbeEventKind::Request {
                        resource: resource.clone(),
                        phase: BoundaryPhase::Enter,
                    },
                ),
                event(
                    1,
                    "response-open",
                    None,
                    ProbeEventKind::Response {
                        resource: resource.clone(),
                        state: ResponseState::Uncommitted,
                    },
                ),
                event(
                    2,
                    "response-commit",
                    None,
                    ProbeEventKind::Response {
                        resource,
                        state: ResponseState::Committed,
                    },
                ),
            ],
            ProbeScenario::RootLifetime => vec![event(
                0,
                "witness",
                Some(operation),
                ProbeEventKind::Cleanup {
                    phase: CleanupPhase::Invoked,
                    root_lifetime: true,
                },
            )],
            ProbeScenario::Operation => unreachable!(),
        };
        let evaluated = evaluate_runtime_probes(
            &plan,
            runs_with(&plan, |_| events.clone()),
            tool("runner", '9'),
        )
        .unwrap();
        assert!(
            matches!(
                evaluated.claim_material()[0].observations[0].outcome,
                ProbeOutcome::Witness { .. }
            ),
            "{scenario:?}"
        );
    }
}

#[test]
fn lifecycle_scenarios_refuse_unordered_or_incomplete_positive_markers() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![mode(&contract, "client", &["browser"])],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::CleanupLifecycle,
            ProbeEventClass::Cleanup,
            "witness",
            Some(operation.clone()),
        ),
        witness,
        closure,
    );
    let evaluated = evaluate_runtime_probes(
        &plan,
        runs_with(&plan, |_| {
            vec![event(
                0,
                "witness",
                Some(operation.clone()),
                ProbeEventKind::Cleanup {
                    phase: CleanupPhase::Invoked,
                    root_lifetime: false,
                },
            )]
        }),
        tool("runner", '9'),
    )
    .unwrap();
    assert!(matches!(
        evaluated.claim_material()[0].observations[0].outcome,
        ProbeOutcome::Refused { .. }
    ));
}

#[test]
fn unauthorized_cardinality_or_closure_promotion_is_rejected_at_plan_time() {
    let first_contract = contract();
    let (witness, closure, operation) = subjects(&first_contract);
    let matrix = ArtifactModeMatrix::new(
        &first_contract,
        vec![mode(&first_contract, "client", &["browser"])],
    )
    .unwrap();
    let error = RuntimeProbePlan::build(
        first_contract,
        [witness.clone()].into_iter().collect(),
        [closure.clone()].into_iter().collect(),
        matrix,
        vec![recipe(
            closure,
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "not-authorized",
            Some(operation),
        )],
        policy(),
    )
    .unwrap_err();
    assert!(matches!(error, RuntimeProbeError::UnplannedTarget { .. }));

    let second_contract = contract();
    let (witness, closure, _) = subjects(&second_contract);
    let matrix = ArtifactModeMatrix::new(
        &second_contract,
        vec![mode(&second_contract, "client", &["browser"])],
    )
    .unwrap();
    let error = RuntimeProbePlan::build(
        second_contract,
        [witness].into_iter().collect(),
        [closure.clone()].into_iter().collect(),
        matrix,
        vec![recipe(
            closure,
            ProbeAuthority::ClosureFalsification,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "not-an-operation",
            Some(OperationId("invented-operation".into())),
        )],
        policy(),
    )
    .unwrap_err();
    assert!(matches!(error, RuntimeProbeError::InvalidPlan { .. }));
}

#[test]
fn evaluation_material_integrates_with_multi_mode_phase9_sidecars() {
    let contract = contract();
    let (witness, closure, operation) = subjects(&contract);
    let plan = plan_with(
        vec![
            mode(&contract, "development", &["browser", "development"]),
            mode(&contract, "production", &["browser", "production"]),
        ],
        recipe(
            witness.clone(),
            ProbeAuthority::PossiblePositiveWitness,
            ProbeScenario::Operation,
            ProbeEventClass::Callback,
            "witness",
            Some(operation.clone()),
        ),
        witness.clone(),
        closure,
    );
    let evaluated = evaluate_runtime_probes(
        &plan,
        runs_with(&plan, |_| operation_events(&operation, "witness")),
        tool("runner", 'a'),
    )
    .unwrap();
    let catalog = EvidenceCatalog::new(contract, [], [witness]).unwrap();
    let documents = emit_evidence_sidecars(
        &catalog,
        tool("evidence", 'b'),
        vec![],
        evaluated.claim_material().to_vec(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(documents.probes().unwrap()).unwrap();
    assert_eq!(
        value["claims"][0]["observations"].as_array().unwrap().len(),
        2
    );
    assert!(matches!(
        crate::validate_evidence_sidecars(SIGNAL, &catalog, None, documents.probes()),
        Err(EvidenceSidecarError::OrphanDocument { .. })
    ));
}
