use super::*;

fn digest(byte: char) -> Digest {
    Digest::parse(format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

fn artifact(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.into(),
        digest: digest(byte),
    }
}

fn package() -> PackageIdentity {
    PackageIdentity {
        name: "solid-js".into(),
        version: "2.0.0-rc.3".into(),
        integrity: "sha512-authoritative".into(),
        manifest: artifact("package.json", 'a'),
    }
}

fn closed_claims() -> CallClaims {
    CallClaims {
        callbacks: KnowledgeSet::complete(vec![]),
        reads: KnowledgeSet::complete(vec![]),
        writes: KnowledgeSet::complete(vec![]),
        creates: KnowledgeSet::complete(vec![]),
        invalidates: KnowledgeSet::complete(vec![]),
        throws: KnowledgeSet::complete(vec![]),
        returns: KnowledgeSet::complete(vec![]),
        cleanups: KnowledgeSet::complete(vec![]),
        disposals: KnowledgeSet::complete(vec![]),
    }
}

fn operation(id: &str, kind: OperationKind) -> Operation {
    Operation {
        id: OperationId(id.into()),
        kind,
        guard: None,
        trigger: Some(Trigger::Event(Event::Call)),
        at: Some(Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: OwnerRelation {
            source: OwnerSource::None,
            requirements: OwnerRequirements {
                owner: Requirement::Forbidden,
                child_owners: Requirement::Unconstrained,
                cleanup: Requirement::Unconstrained,
            },
            capabilities: OwnerCapabilities {
                child_owners: CapabilityKnowledge::Forbidden,
                cleanup: CapabilityKnowledge::Forbidden,
            },
            lifetime: Some(Lifetime::Call),
            productions: KnowledgeSet::complete(vec![]),
        },
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(1),
            max: Some(UpperBound::Finite(1)),
        },
        inputs: vec![],
        output: None,
        resources: BTreeSet::new(),
    }
}

fn resource(id: &str, kind: ResourceKind) -> Resource {
    Resource {
        id: ResourceId(id.into()),
        kind,
        states: KnowledgeSet::Unknown,
        capabilities: KnowledgeSet::Unknown,
        lifetime: Some(Lifetime::Call),
    }
}

fn call(operations: Vec<Operation>, resources: Vec<Resource>) -> CallSemantics {
    let mut claims = closed_claims();
    for operation in &operations {
        let id = operation.id.clone();
        match operation.kind {
            OperationKind::Invoke => {
                claims.callbacks = KnowledgeSet::Complete(vec![CallbackInvocation {
                    from: ValueSource::Parameter {
                        index: 0,
                        path: vec![],
                    },
                    operation: id,
                }])
            }
            OperationKind::Return => claims.returns = KnowledgeSet::Complete(vec![id]),
            OperationKind::Read => claims.reads = KnowledgeSet::Complete(vec![id]),
            OperationKind::Write => claims.writes = KnowledgeSet::Complete(vec![id]),
            OperationKind::Invalidate => claims.invalidates = KnowledgeSet::Complete(vec![id]),
            OperationKind::Create => claims.creates = KnowledgeSet::Complete(vec![id]),
            OperationKind::Cleanup => claims.cleanups = KnowledgeSet::Complete(vec![id]),
            OperationKind::Dispose => claims.disposals = KnowledgeSet::Complete(vec![id]),
        }
    }
    CallSemantics::new(
        claims,
        operations,
        vec![],
        resources,
        GuardPartition {
            cases: KnowledgeSet::complete(vec![]),
        },
    )
}

fn export(
    case: &ArtifactCase,
    name: &str,
    shape: ValueShape,
    call: CallSemantics,
) -> ExportSemantics {
    ExportSemantics {
        identity: ExportIdentity {
            entrypoint: case.entrypoint.clone(),
            public_name: name.into(),
            runtime: ExportTargetIdentity {
                module: case.runtime.clone(),
                export_name: name.into(),
            },
            declarations: ExportTargetIdentity {
                module: case.declarations.clone(),
                export_name: name.into(),
            },
        },
        shape,
        stability: StabilityKnowledge::Unknown,
        call,
    }
}

fn artifact_case(id: &str) -> ArtifactCase {
    ArtifactCase {
        id: id.into(),
        entrypoint: ".".into(),
        resolution_trace: vec![ResolutionStep {
            condition: "import".into(),
            target: "./dist/server.js".into(),
        }],
        runtime: artifact("dist/server.js", 'b'),
        declarations: artifact("dist/server.d.ts", 'c'),
        dependency_closure: digest('d'),
        transform: None,
        stability: StabilityKnowledge::Unknown,
        exports: BTreeMap::new(),
    }
}

fn proposal_with(shape: ValueShape, call: CallSemantics) -> ContractProposal {
    let mut case = artifact_case("server-import");
    let export = export(&case, "createResource", shape, call);
    case.exports.insert("createResource".into(), export);
    ContractProposal::new(package(), vec![case])
}

fn normalized_export(proposal: ContractProposal) -> ExportSemantics {
    proposal
        .normalize()
        .unwrap()
        .artifact_case("server-import")
        .unwrap()
        .exports["createResource"]
        .clone()
}

#[test]
fn four_knowledge_states_keep_unknown_distinct_from_negative() {
    assert_eq!(
        KnowledgeSet::<OperationId>::Unknown.state(),
        KnowledgeState::Unknown
    );
    assert_eq!(
        KnowledgeSet::Partial(vec![OperationId("read".into())]).state(),
        KnowledgeState::PartialPositive
    );
    assert_eq!(
        KnowledgeSet::Complete(vec![OperationId("read".into())]).state(),
        KnowledgeState::CompletePositive
    );
    assert_eq!(
        KnowledgeSet::<OperationId>::Complete(vec![]).state(),
        KnowledgeState::CompleteNegative
    );
    assert!(!KnowledgeSet::<OperationId>::Unknown.proves_absence());
    assert!(KnowledgeSet::<OperationId>::Complete(vec![]).proves_absence());
}

#[test]
fn partial_empty_is_rejected_as_false_closure() {
    let mut claims = closed_claims();
    claims.reads = KnowledgeSet::Partial(vec![]);
    let proposal = proposal_with(
        ValueShape::Plain,
        CallSemantics::new(claims, vec![], vec![], vec![], GuardPartition::default()),
    );
    assert!(matches!(
        proposal.normalize(),
        Err(ModelError::InvalidKnowledge { .. })
    ));
}

#[test]
fn property_join_is_monotone_and_does_not_invent_negative_proof() {
    let read = OperationId("read".into());
    let write = OperationId("write".into());
    let alternatives = [
        KnowledgeSet::Unknown,
        KnowledgeSet::Partial(vec![read.clone()]),
        KnowledgeSet::Complete(vec![write.clone()]),
        KnowledgeSet::Complete(vec![]),
    ];
    for mask in 1_usize..1 << alternatives.len() {
        let selected = alternatives
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, knowledge)| knowledge.clone())
            .collect::<Vec<_>>();
        let joined = KnowledgeSet::join(selected.clone());
        for positive in selected.iter().flat_map(KnowledgeSet::items) {
            assert!(joined.items().contains(positive));
        }
        if selected.iter().any(|item| !item.is_closed()) {
            assert!(!joined.proves_absence());
        }
    }
}

#[test]
fn recursive_unknown_leaf_does_not_contaminate_known_sibling() {
    let shape = ValueShape::Object(KnowledgeSet::Complete(vec![
        ObjectProperty {
            name: "known".into(),
            value: ValueShape::Plain,
        },
        ObjectProperty {
            name: "open".into(),
            value: ValueShape::Promise(Box::new(ValueShape::Unknown)),
        },
    ]));
    let export = normalized_export(proposal_with(shape, call(vec![], vec![])));
    let value_claims = export
        .unresolved_claims()
        .into_iter()
        .filter_map(|claim| match claim {
            ClaimPath::Value { path, domain, .. } => Some((path, domain)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        value_claims,
        vec![(
            ValuePath(vec![
                ValuePathSegment::ObjectProperty("open".into()),
                ValuePathSegment::PromiseValue,
            ]),
            ValueClaimDomain::Shape,
        )]
    );
}

#[test]
fn operation_axes_are_independently_unresolved() {
    let mut read = operation("read", OperationKind::Read);
    read.trigger = None;
    read.schedule = None;
    read.tracking = Tracking::Unknown;
    read.cardinality.min = None;
    let export = normalized_export(proposal_with(ValueShape::Plain, call(vec![read], vec![])));
    let unresolved = export.unresolved_claims();
    for domain in [
        OperationClaimDomain::Trigger,
        OperationClaimDomain::Schedule,
        OperationClaimDomain::Tracking,
        OperationClaimDomain::CardinalityMinimum,
    ] {
        assert!(unresolved.contains(&ClaimPath::Operation {
            operation: OperationId("read".into()),
            domain,
        }));
    }
    assert!(!unresolved.contains(&ClaimPath::Operation {
        operation: OperationId("read".into()),
        domain: OperationClaimDomain::ExecutionPoint,
    }));
}

#[test]
fn operation_graph_rejects_missing_nodes_and_cycles() {
    let read = operation("read", OperationKind::Read);
    let mut missing = call(vec![read.clone()], vec![]);
    missing.edges.push(OperationEdge {
        kind: EdgeKind::Data,
        from: OperationId("read".into()),
        to: OperationId("missing".into()),
    });
    assert!(matches!(
        proposal_with(ValueShape::Plain, missing).normalize(),
        Err(ModelError::MissingOperation { .. })
    ));

    let write = operation("write", OperationKind::Write);
    let mut cyclic = call(vec![read, write], vec![]);
    cyclic.edges = vec![
        OperationEdge {
            kind: EdgeKind::Data,
            from: OperationId("read".into()),
            to: OperationId("write".into()),
        },
        OperationEdge {
            kind: EdgeKind::Invalidates,
            from: OperationId("write".into()),
            to: OperationId("read".into()),
        },
    ];
    assert!(matches!(
        proposal_with(ValueShape::Plain, cyclic).normalize(),
        Err(ModelError::OperationCycle { .. })
    ));
}

#[test]
fn cardinality_requires_a_valid_explicit_scope() {
    let mut read = operation("read", OperationKind::Read);
    read.cardinality.scope = None;
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![read], vec![])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));

    let mut read = operation("read", OperationKind::Read);
    read.cardinality.min = Some(2);
    read.cardinality.max = Some(UpperBound::Finite(1));
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![read], vec![])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));

    let mut read = operation("read", OperationKind::Read);
    read.cardinality.scope = Some(CardinalityScope::Resource(ResourceId("absent".into())));
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![read], vec![])).normalize(),
        Err(ModelError::MissingResource { .. })
    ));
}

#[test]
fn owner_source_requirements_and_production_are_separate_invariants() {
    let mut create = operation("create", OperationKind::Create);
    create.owner.requirements.owner = Requirement::Required;
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![create], vec![])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));

    let owner = resource("owner", ResourceKind::Owner);
    let mut create = operation("create", OperationKind::Create);
    create.owner.source = OwnerSource::Created(ResourceId("owner".into()));
    create.owner.requirements.owner = Requirement::Required;
    create.owner.capabilities = OwnerCapabilities::default();
    create.owner.productions = KnowledgeSet::Unknown;
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![create], vec![owner])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));
}

#[test]
fn compatible_owner_production_normalizes_without_inference() {
    let owner = resource("owner", ResourceKind::Owner);
    let mut create = operation("create", OperationKind::Create);
    create.owner.source = OwnerSource::Created(ResourceId("owner".into()));
    create.owner.requirements.owner = Requirement::Required;
    create.owner.capabilities = OwnerCapabilities::default();
    create.owner.productions = KnowledgeSet::Partial(vec![OwnerProduction {
        resource: ResourceId("owner".into()),
        capabilities: OwnerCapabilities::default(),
        lifetime: Some(Lifetime::Owner(ResourceId("owner".into()))),
    }]);
    assert!(
        proposal_with(ValueShape::Plain, call(vec![create], vec![owner]))
            .normalize()
            .is_ok()
    );
}

#[test]
fn capabilities_reject_role_and_resource_contradictions() {
    let writable_accessor = ValueShape::Reactive {
        role: ReactiveRole::Accessor,
        resource: None,
        capabilities: KnowledgeSet::Complete(vec![
            CapabilityClaim {
                capability: ObservableCapability::Readable,
                resource: None,
            },
            CapabilityClaim {
                capability: ObservableCapability::Writable,
                resource: None,
            },
        ]),
    };
    assert!(matches!(
        proposal_with(writable_accessor, call(vec![], vec![])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));

    let transition = resource("transition", ResourceKind::Transition);
    let optimistic_without_writable = ValueShape::Store {
        resource: None,
        capabilities: KnowledgeSet::Partial(vec![CapabilityClaim {
            capability: ObservableCapability::Optimistic,
            resource: Some(ResourceId("transition".into())),
        }]),
    };
    assert!(matches!(
        proposal_with(optimistic_without_writable, call(vec![], vec![transition])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));

    let response = resource("response", ResourceKind::Response);
    let refreshable_response = ValueShape::Store {
        resource: None,
        capabilities: KnowledgeSet::Partial(vec![CapabilityClaim {
            capability: ObservableCapability::Refreshable,
            resource: Some(ResourceId("response".into())),
        }]),
    };
    assert!(matches!(
        proposal_with(refreshable_response, call(vec![], vec![response])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));
}

#[test]
fn resource_state_partitions_reject_cross_kind_states() {
    let mut response = resource("response", ResourceKind::Response);
    response.states = KnowledgeSet::Complete(vec![ResourceState::OwnerDisposed]);
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![], vec![response])).normalize(),
        Err(ModelError::Contradiction { .. })
    ));
}

fn literal_guard(value: bool) -> Guard {
    Guard(vec![GuardAtom::Literal {
        argument: 0,
        path: vec![],
        value: Literal::Bool(value),
    }])
}

#[test]
fn restricted_guards_reject_overlap_and_unsatisfiable_conjunctions() {
    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let mut overlapping = call(vec![read, write], vec![]);
    overlapping.guards.cases = KnowledgeSet::Complete(vec![
        GuardedCase::When {
            guard: Guard(vec![GuardAtom::Signature("source".into())]),
            operations: KnowledgeSet::Complete(vec![OperationId("read".into())]),
        },
        GuardedCase::When {
            guard: Guard(vec![GuardAtom::ArgumentCount { min: 0, max: None }]),
            operations: KnowledgeSet::Complete(vec![OperationId("write".into())]),
        },
        GuardedCase::Otherwise {
            operations: KnowledgeSet::Complete(vec![]),
        },
    ]);
    assert!(matches!(
        proposal_with(ValueShape::Plain, overlapping).normalize(),
        Err(ModelError::OverlappingGuards { .. })
    ));

    let mut unsatisfiable = call(vec![operation("read", OperationKind::Read)], vec![]);
    unsatisfiable.guards.cases = KnowledgeSet::Partial(vec![GuardedCase::When {
        guard: Guard(vec![
            GuardAtom::Signature("source".into()),
            GuardAtom::Signature("options".into()),
        ]),
        operations: KnowledgeSet::Complete(vec![OperationId("read".into())]),
    }]);
    assert!(matches!(
        proposal_with(ValueShape::Plain, unsatisfiable).normalize(),
        Err(ModelError::InvalidGuard { .. })
    ));
}

#[test]
fn unresolved_guard_selection_joins_possible_cases_monotonically() {
    let partition = GuardPartition {
        cases: KnowledgeSet::Complete(vec![
            GuardedCase::When {
                guard: literal_guard(true),
                operations: KnowledgeSet::Complete(vec![OperationId("read".into())]),
            },
            GuardedCase::When {
                guard: literal_guard(false),
                operations: KnowledgeSet::Partial(vec![OperationId("write".into())]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![]),
            },
        ]),
    };
    let selected = partition.select_operations(|_| GuardTruth::Unknown);
    assert_eq!(
        selected,
        KnowledgeSet::Partial(vec![
            OperationId("read".into()),
            OperationId("write".into())
        ])
    );

    let negative = GuardPartition {
        cases: KnowledgeSet::Complete(vec![GuardedCase::Otherwise {
            operations: KnowledgeSet::Complete(vec![]),
        }]),
    }
    .select_operations(|_| GuardTruth::Unknown);
    assert!(negative.proves_absence());

    assert!(
        GuardPartition {
            cases: KnowledgeSet::Complete(vec![]),
        }
        .select_operations(|_| GuardTruth::Unknown)
        .proves_absence()
    );
}

#[test]
fn open_guard_partition_cannot_claim_exhaustive_otherwise() {
    let mut guarded = call(vec![], vec![]);
    guarded.guards.cases = KnowledgeSet::Partial(vec![GuardedCase::Otherwise {
        operations: KnowledgeSet::Complete(vec![]),
    }]);
    assert!(matches!(
        proposal_with(ValueShape::Plain, guarded).normalize(),
        Err(ModelError::InvalidGuard { .. })
    ));
}

#[test]
fn exact_export_identity_and_artifact_selection_are_validated() {
    let mut wrong_identity = proposal_with(ValueShape::Plain, call(vec![], vec![]));
    wrong_identity.artifact_cases[0]
        .exports
        .get_mut("createResource")
        .unwrap()
        .identity
        .public_name = "resource".into();
    assert!(matches!(
        wrong_identity.normalize(),
        Err(ModelError::ExportIdentity { .. })
    ));

    let mut first = artifact_case("first");
    let export = export(
        &first,
        "createSignal",
        ValueShape::Plain,
        call(vec![], vec![]),
    );
    first.exports.insert("createSignal".into(), export);
    let mut second = first.clone();
    second.id = "second".into();
    assert!(matches!(
        ContractProposal::new(package(), vec![first, second]).normalize(),
        Err(ModelError::DuplicateArtifactSelection { .. })
    ));
}

#[test]
fn experimental_status_is_local_to_case_and_export() {
    let mut stable_unknown = artifact_case("server");
    let export_unknown = export(
        &stable_unknown,
        "createSignal",
        ValueShape::Plain,
        call(vec![], vec![]),
    );
    stable_unknown
        .exports
        .insert("createSignal".into(), export_unknown);

    let mut experimental = artifact_case("browser");
    experimental.entrypoint = "./web".into();
    experimental.stability = StabilityKnowledge::Experimental;
    experimental.runtime = artifact("dist/web.js", 'e');
    let mut export_experimental = export(
        &experimental,
        "createSignal",
        ValueShape::Plain,
        call(vec![], vec![]),
    );
    export_experimental.stability = StabilityKnowledge::Experimental;
    experimental
        .exports
        .insert("createSignal".into(), export_experimental);

    let normalized = ContractProposal::new(package(), vec![experimental, stable_unknown])
        .normalize()
        .unwrap();
    assert_eq!(
        normalized.artifact_case("server").unwrap().stability,
        StabilityKnowledge::Unknown
    );
    assert_eq!(
        normalized.artifact_case("browser").unwrap().exports["createSignal"].stability,
        StabilityKnowledge::Experimental
    );
}

#[test]
fn property_semantic_digest_is_deterministic_and_order_equivalent() {
    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);

    let mut forward = call(
        vec![read.clone(), write.clone()],
        vec![owner.clone(), cleanup.clone()],
    );
    forward.edges = vec![OperationEdge {
        kind: EdgeKind::Data,
        from: read.id.clone(),
        to: write.id.clone(),
    }];
    let mut reverse = call(vec![write, read], vec![cleanup, owner]);
    reverse.edges = forward.edges.iter().cloned().rev().collect();

    let first = proposal_with(ValueShape::Plain, forward)
        .normalize()
        .unwrap();
    let second = proposal_with(ValueShape::Plain, reverse)
        .normalize()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.semantic_digest(), second.semantic_digest());
    for _ in 0..32 {
        assert_eq!(
            first.semantic_digest(),
            proposal_with(
                ValueShape::Plain,
                first.artifact_cases[0].exports["createResource"]
                    .call
                    .clone()
            )
            .normalize()
            .unwrap()
            .semantic_digest()
        );
    }
}

#[test]
fn property_equivalent_numeric_guards_normalize_to_one_digest() {
    let with_number = |number: &str| {
        let mut read = operation("read", OperationKind::Read);
        read.guard = Some(Guard(vec![GuardAtom::Literal {
            argument: 0,
            path: vec![],
            value: Literal::Number(number.into()),
        }]));
        proposal_with(ValueShape::Plain, call(vec![read], vec![]))
            .normalize()
            .unwrap()
    };
    assert_eq!(with_number("1.0"), with_number("1e0"));
}

#[test]
fn canonical_digest_distinguishes_all_local_knowledge_states() {
    let mut digests = BTreeSet::new();
    for knowledge in [
        KnowledgeSet::Unknown,
        KnowledgeSet::Partial(vec![OperationId("read".into())]),
        KnowledgeSet::Complete(vec![OperationId("read".into())]),
        KnowledgeSet::Complete(vec![]),
    ] {
        let mut read = operation("read", OperationKind::Read);
        read.output = Some(ValueShape::Unknown);
        let mut call = call(vec![read], vec![]);
        call.claims.throws = knowledge;
        digests.insert(
            proposal_with(ValueShape::Plain, call)
                .normalize()
                .unwrap()
                .semantic_digest()
                .clone(),
        );
    }
    assert_eq!(digests.len(), 4);
}

#[test]
fn property_leaf_locality_survives_every_sibling_permutation() {
    for names in [["a", "b", "c"], ["c", "a", "b"], ["b", "c", "a"]] {
        let shape = ValueShape::Object(KnowledgeSet::Complete(
            names
                .into_iter()
                .map(|name| ObjectProperty {
                    name: name.into(),
                    value: if name == "b" {
                        ValueShape::Unknown
                    } else {
                        ValueShape::Plain
                    },
                })
                .collect(),
        ));
        let export = normalized_export(proposal_with(shape, call(vec![], vec![])));
        let unknown_values = export
            .unresolved_claims()
            .into_iter()
            .filter(|claim| matches!(claim, ClaimPath::Value { .. }))
            .collect::<Vec<_>>();
        assert_eq!(
            unknown_values,
            vec![ClaimPath::Value {
                root: ValueRoot::Export,
                path: ValuePath(vec![ValuePathSegment::ObjectProperty("b".into())]),
                domain: ValueClaimDomain::Shape,
            }]
        );
    }
}

#[test]
fn solid_two_conformance_matrix_rows_have_normalized_representations() {
    let mut rows = Vec::new();

    let invoke = operation("compute", OperationKind::Invoke);
    let mut cleanup = operation("cleanup", OperationKind::Cleanup);
    cleanup.trigger = Some(Trigger::Operation(invoke.id.clone()));
    cleanup.at = Some(Event::Cleanup);
    let mut effect = call(vec![invoke, cleanup], vec![]);
    effect.edges.push(OperationEdge {
        kind: EdgeKind::Cleanup,
        from: OperationId("compute".into()),
        to: OperationId("cleanup".into()),
    });
    rows.push((
        "split effects",
        ValueShape::Cleanup {
            resource: None,
            lifetime: Some(Lifetime::Call),
        },
        effect,
    ));

    let owner = resource("leaf-owner", ResourceKind::Owner);
    let mut settled = operation("settled", OperationKind::Invoke);
    settled.owner.source = OwnerSource::Captured(ResourceId("leaf-owner".into()));
    settled.owner.requirements.owner = Requirement::Required;
    settled.owner.requirements.child_owners = Requirement::Forbidden;
    settled.owner.requirements.cleanup = Requirement::Forbidden;
    settled.owner.capabilities.child_owners = CapabilityKnowledge::Forbidden;
    settled.owner.capabilities.cleanup = CapabilityKnowledge::Forbidden;
    settled.owner.lifetime = Some(Lifetime::Owner(ResourceId("leaf-owner".into())));
    rows.push((
        "tracked effect and onSettled ownership",
        ValueShape::Callable,
        call(vec![settled], vec![owner]),
    ));

    let write = operation("write", OperationKind::Write);
    let mut invalidate = operation("invalidate", OperationKind::Invalidate);
    invalidate.at = Some(Event::Flush);
    invalidate.schedule = Some(Schedule::Queued);
    let mut batch = call(vec![write, invalidate], vec![]);
    batch.edges.push(OperationEdge {
        kind: EdgeKind::Invalidates,
        from: OperationId("write".into()),
        to: OperationId("invalidate".into()),
    });
    rows.push(("batched writes and flush", ValueShape::Callable, batch));

    let mut control = operation("child", OperationKind::Invoke);
    control.inputs = vec![ValueShape::Choice(KnowledgeSet::Complete(vec![
        ValueShape::Tuple(KnowledgeSet::Complete(vec![ValueShape::Plain])),
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                capability: ObservableCapability::Readable,
                resource: None,
            }]),
        },
    ]))];
    rows.push((
        "control flow keyed modes",
        ValueShape::Component,
        call(vec![control], vec![]),
    ));

    rows.push((
        "promise and async iterable computations",
        ValueShape::Choice(KnowledgeSet::Complete(vec![
            ValueShape::Promise(Box::new(ValueShape::Unknown)),
            ValueShape::AsyncIterable(Box::new(ValueShape::Plain)),
        ])),
        call(vec![], vec![]),
    ));

    let mut async_resource = resource("async", ResourceKind::AsyncComputation);
    async_resource.capabilities = KnowledgeSet::Complete(vec![ResourceCapability::Refreshable]);
    rows.push((
        "loading pending latest refresh affects",
        ValueShape::Store {
            resource: Some(ResourceId("async".into())),
            capabilities: KnowledgeSet::Complete(vec![
                CapabilityClaim {
                    capability: ObservableCapability::Readable,
                    resource: None,
                },
                CapabilityClaim {
                    capability: ObservableCapability::Refreshable,
                    resource: Some(ResourceId("async".into())),
                },
                CapabilityClaim {
                    capability: ObservableCapability::PendingAware,
                    resource: Some(ResourceId("async".into())),
                },
            ]),
        },
        call(vec![], vec![async_resource]),
    ));

    let mut transition = resource("transition", ResourceKind::Transition);
    transition.capabilities = KnowledgeSet::Complete(vec![ResourceCapability::Writable]);
    rows.push((
        "actions and optimistic state",
        ValueShape::Choice(KnowledgeSet::Complete(vec![
            ValueShape::Action {
                transition: Some(ResourceId("transition".into())),
            },
            ValueShape::Store {
                resource: None,
                capabilities: KnowledgeSet::Complete(vec![
                    CapabilityClaim {
                        capability: ObservableCapability::Readable,
                        resource: None,
                    },
                    CapabilityClaim {
                        capability: ObservableCapability::Writable,
                        resource: None,
                    },
                    CapabilityClaim {
                        capability: ObservableCapability::Optimistic,
                        resource: Some(ResourceId("transition".into())),
                    },
                ]),
            },
        ])),
        call(vec![], vec![transition]),
    ));

    rows.push((
        "store drafts projections snapshots",
        ValueShape::Store {
            resource: None,
            capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                capability: ObservableCapability::Readable,
                resource: None,
            }]),
        },
        call(vec![], vec![]),
    ));
    rows.push((
        "two-phase refs directives",
        ValueShape::RefApplication,
        call(vec![], vec![]),
    ));

    let root_owner = resource("root", ResourceKind::Owner);
    let mut event = operation("event", OperationKind::Invoke);
    event.trigger = Some(Trigger::Event(Event::External));
    event.at = Some(Event::External);
    event.schedule = Some(Schedule::External);
    event.owner.source = OwnerSource::Captured(ResourceId("root".into()));
    event.owner.requirements.owner = Requirement::Required;
    event.owner.requirements.child_owners = Requirement::Unconstrained;
    event.owner.capabilities.child_owners = CapabilityKnowledge::Unknown;
    event.owner.capabilities.cleanup = CapabilityKnowledge::Unknown;
    event.owner.lifetime = Some(Lifetime::Owner(ResourceId("root".into())));
    rows.push((
        "root-owned event delegation",
        ValueShape::Component,
        call(vec![event], vec![root_owner]),
    ));
    rows.push((
        "browser render hydrate and SSR artifacts",
        ValueShape::Component,
        call(vec![], vec![]),
    ));

    let request = resource("request", ResourceKind::Request);
    let mut response = resource("response", ResourceKind::Response);
    response.states = KnowledgeSet::Complete(vec![
        ResourceState::ResponseUncommitted,
        ResourceState::ResponseCommitted,
    ]);
    rows.push((
        "HTTP response mutation",
        ValueShape::Callable,
        call(vec![], vec![request, response]),
    ));

    let reference = resource("server-reference", ResourceKind::ServerFunctionReference);
    rows.push((
        "server-function references",
        ValueShape::ServerFunctionReference {
            resource: Some(ResourceId("server-reference".into())),
        },
        call(vec![], vec![reference]),
    ));
    rows.push((
        "experimental server components",
        ValueShape::Component,
        call(vec![], vec![]),
    ));

    let mut conditional = call(
        vec![
            operation("read", OperationKind::Read),
            operation("write", OperationKind::Write),
        ],
        vec![],
    );
    conditional.guards.cases = KnowledgeSet::Partial(vec![
        GuardedCase::When {
            guard: literal_guard(true),
            operations: KnowledgeSet::Complete(vec![OperationId("read".into())]),
        },
        GuardedCase::When {
            guard: literal_guard(false),
            operations: KnowledgeSet::Complete(vec![OperationId("write".into())]),
        },
    ]);
    rows.push((
        "conditional adapters",
        ValueShape::Choice(KnowledgeSet::Unknown),
        conditional,
    ));
    rows.push((
        "mixed-framework exact artifact closure",
        ValueShape::Plain,
        call(vec![], vec![]),
    ));

    assert_eq!(rows.len(), 16);
    for (row, shape, call) in rows {
        proposal_with(shape, call)
            .normalize()
            .unwrap_or_else(|error| panic!("{row} was not representable: {error}"));
    }
}
