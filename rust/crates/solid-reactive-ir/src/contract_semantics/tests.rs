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
        computations: KnowledgeSet::Unknown,
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
        composed_from: None,
        protocol: None,
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
            OperationKind::Compute => claims.computations = KnowledgeSet::Partial(vec![id]),
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
        initialization: None,
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

/// Withdrawing an operation the census could not certify has to leave the
/// export publishable and the domain that listed it *open*: a shorter list
/// still marked closed would assert an absence nothing established, which is a
/// stronger claim than the one being withdrawn.
#[test]
fn withholding_an_operation_opens_its_domain_and_takes_its_dependents() {
    let case = artifact_case("server-import");
    let mut read = operation("read-0", OperationKind::Read);
    let mut dependent = operation("write-0", OperationKind::Write);
    dependent.trigger = Some(Trigger::Operation(OperationId("read-0".into())));
    let mut untouched = operation("create-0", OperationKind::Create);
    untouched.trigger = Some(Trigger::Event(Event::Call));
    read.inputs.push(ValueShape::Parameter {
        index: 0,
        path: vec!["contains".into()],
    });
    let mut semantics = call(vec![read, dependent, untouched], vec![]);
    semantics.edges.push(OperationEdge {
        kind: EdgeKind::Orders,
        from: OperationId("read-0".into()),
        to: OperationId("create-0".into()),
    });
    let mut export = export(&case, "contains", ValueShape::Unknown, semantics);
    assert!(export.call.claims.reads.is_closed());

    let withdrawn = export.withhold_operations(&BTreeSet::from([OperationId("read-0".into())]));

    assert_eq!(
        withdrawn,
        BTreeSet::from([OperationId("read-0".into()), OperationId("write-0".into())]),
        "an operation triggered by a withdrawn one describes nothing either"
    );
    let ids = export
        .call
        .operations
        .iter()
        .map(|operation| operation.id.0.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec!["create-0"],
        "only the unrelated operation survives"
    );
    assert!(
        export.call.edges.is_empty(),
        "an edge touching a withdrawn operation is removed"
    );
    assert!(
        !export.call.claims.reads.is_closed(),
        "the domain that listed the withdrawn operation is opened"
    );
    assert!(
        export.call.claims.reads.items().is_empty(),
        "and no longer lists it"
    );
    assert!(
        !export.call.claims.writes.is_closed(),
        "so is the domain that listed the cascade"
    );
    assert!(
        export.call.claims.creates.is_closed(),
        "a domain that listed nothing withdrawn keeps its closure"
    );

    // An id this export does not carry is not a withdrawal, and changes
    // nothing: the caller learns that from the empty return rather than from a
    // document that quietly lost a closure.
    let mut untouched_export = export.clone();
    let none =
        untouched_export.withhold_operations(&BTreeSet::from([OperationId("absent".into())]));
    assert!(none.is_empty());
    assert_eq!(untouched_export.call.operations.len(), 1);
    assert!(untouched_export.call.claims.creates.is_closed());
}

/// A `compute` operation in the shape the generator publishes an `Effect`
/// owner requirement (ADR 0114): it requires the caller's ambient owner and
/// child owners of it, and what it produces is left unknown.
fn compute(id: &str) -> Operation {
    let mut compute = operation(id, OperationKind::Compute);
    compute.owner = OwnerRelation {
        source: OwnerSource::AmbientAtCall,
        requirements: OwnerRequirements {
            owner: Requirement::Required,
            child_owners: Requirement::Required,
            cleanup: Requirement::Unconstrained,
        },
        capabilities: OwnerCapabilities::default(),
        lifetime: None,
        productions: KnowledgeSet::Unknown,
    };
    compute
}

/// ADR 0114's domain: stated by item only, by `compute` operations only, and
/// never an unresolved claim, since nothing in version 1 could resolve it.
#[test]
fn computations_are_stated_by_item_only_and_by_compute_operations() {
    let id = || OperationId("compute-0".into());
    let semantics = call(vec![compute("compute-0")], vec![]);
    assert_eq!(
        semantics.claims.computations,
        KnowledgeSet::Partial(vec![id()])
    );
    let export = normalized_export(proposal_with(ValueShape::Callable, semantics.clone()));
    assert_eq!(
        export.claim_state(ClaimDomain::Computations),
        KnowledgeState::PartialPositive
    );
    assert!(
        !export
            .unresolved_claims()
            .contains(&ClaimPath::Call(ClaimDomain::Computations)),
        "a domain no document may close is not a claim anything could resolve"
    );
    let silent = normalized_export(proposal_with(ValueShape::Callable, call(vec![], vec![])));
    assert_eq!(
        silent.claim_state(ClaimDomain::Computations),
        KnowledgeState::Unknown
    );
    assert!(
        !silent
            .unresolved_claims()
            .contains(&ClaimPath::Call(ClaimDomain::Computations)),
        "and silence about it is not one either"
    );

    let refused = |semantics: CallSemantics, why: &str| {
        assert!(
            matches!(
                proposal_with(ValueShape::Callable, semantics).normalize(),
                Err(ModelError::Contradiction { .. })
            ),
            "{why}"
        );
    };
    let mut closed = semantics.clone();
    closed.claims.computations = KnowledgeSet::Complete(vec![id()]);
    refused(
        closed,
        "no census decides computations, so no document closes it",
    );
    let mut unlisted = semantics.clone();
    unlisted.claims.computations = KnowledgeSet::Unknown;
    refused(unlisted, "a compute operation outside computations");
    let mut misfiled = call(vec![operation("cleanup-0", OperationKind::Cleanup)], vec![]);
    misfiled.claims.computations = KnowledgeSet::Partial(vec![OperationId("cleanup-0".into())]);
    refused(
        misfiled,
        "a computations item that is not a compute operation",
    );
    let mut childless = compute("compute-0");
    childless.owner.requirements.child_owners = Requirement::Unconstrained;
    refused(
        call(vec![childless], vec![]),
        "a compute that does not require child owners states no registration",
    );
    let mut ownerless = compute("compute-0");
    ownerless.owner.source = OwnerSource::Unknown;
    ownerless.owner.requirements.owner = Requirement::Unconstrained;
    refused(
        call(vec![ownerless], vec![]),
        "a compute that does not require an owner states no registration",
    );
}

/// A consumer reads a closed `creates` as "no owner requirement beyond the
/// published items", so withdrawing an operation that imposes one opens
/// `creates` too; an operation that imposes none leaves it closed, which is
/// what `withholding_an_operation_opens_its_domain_and_takes_its_dependents`
/// already pins.
#[test]
fn withdrawing_an_owner_requirement_opens_creates_with_it() {
    let case = artifact_case("server-import");
    let mut cleanup = operation("cleanup-0", OperationKind::Cleanup);
    cleanup.owner = compute("unused").owner;
    cleanup.owner.requirements.child_owners = Requirement::Unconstrained;
    cleanup.owner.requirements.cleanup = Requirement::Required;
    for (seed, requirement) in [("compute-0", compute("compute-0")), ("cleanup-0", cleanup)] {
        let mut export = export(
            &case,
            "track",
            ValueShape::Callable,
            call(vec![requirement], vec![]),
        );
        assert!(export.call.claims.creates.is_closed());
        let withdrawn = export.withhold_operations(&BTreeSet::from([OperationId(seed.into())]));
        assert_eq!(withdrawn.len(), 1, "{seed}");
        assert!(
            export.call.claims.computations.items().is_empty()
                && export.call.claims.cleanups.items().is_empty(),
            "{seed} is no longer listed"
        );
        assert!(
            !export.call.claims.creates.is_closed(),
            "{seed}: creates is the consumer's completeness signal, so it opens with the requirement"
        );
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

fn claim_subject(path: SemanticClaimPath) -> SemanticClaimSubject {
    SemanticClaimSubject {
        artifact_case: "server-import".into(),
        export: "createResource".into(),
        path,
    }
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
fn semantic_claim_ids_ignore_unrelated_meaning_but_bind_exact_identity_and_path() {
    let first = proposal_with(
        ValueShape::Plain,
        call(vec![operation("read", OperationKind::Read)], vec![]),
    )
    .normalize()
    .unwrap();
    let second = proposal_with(
        ValueShape::Callable,
        call(
            vec![
                operation("read", OperationKind::Read),
                operation("write", OperationKind::Write),
            ],
            vec![],
        ),
    )
    .normalize()
    .unwrap();
    let reads = claim_subject(SemanticClaimPath::Domain(ClaimPath::Call(
        ClaimDomain::Reads,
    )));
    let writes = claim_subject(SemanticClaimPath::Domain(ClaimPath::Call(
        ClaimDomain::Writes,
    )));

    let first_id = first.claim_id(&reads).unwrap();
    assert_eq!(first_id, second.claim_id(&reads).unwrap());
    assert_ne!(first_id, first.claim_id(&writes).unwrap());
    assert_eq!(SemanticClaimId::parse(first_id.as_str()).unwrap(), first_id);

    let mut other_package = package();
    other_package.version = "2.0.0-rc.4".into();
    let mut case = artifact_case("server-import");
    case.exports.insert(
        "createResource".into(),
        export(
            &case,
            "createResource",
            ValueShape::Plain,
            call(vec![operation("read", OperationKind::Read)], vec![]),
        ),
    );
    let other = ContractProposal::new(other_package, vec![case])
        .normalize()
        .unwrap();
    assert_ne!(first_id, other.claim_id(&reads).unwrap());

    let mut changed_closure = artifact_case("server-import");
    changed_closure.dependency_closure = digest('e');
    changed_closure.exports.insert(
        "createResource".into(),
        export(
            &changed_closure,
            "createResource",
            ValueShape::Plain,
            call(vec![operation("read", OperationKind::Read)], vec![]),
        ),
    );
    let changed_closure = ContractProposal::new(package(), vec![changed_closure])
        .normalize()
        .unwrap();
    assert_ne!(first_id, changed_closure.claim_id(&reads).unwrap());
}

#[test]
fn closed_claim_lookup_is_exact_across_domain_package_and_knowledge_state() {
    let contract = proposal_with(
        ValueShape::Plain,
        CallSemantics::new(
            CallClaims {
                callbacks: KnowledgeSet::complete(vec![]),
                ..CallClaims::default()
            },
            vec![],
            vec![],
            vec![],
            GuardPartition::default(),
        ),
    )
    .normalize()
    .unwrap();
    let callbacks = contract
        .claim_id(&claim_subject(SemanticClaimPath::Domain(ClaimPath::Call(
            ClaimDomain::Callbacks,
        ))))
        .unwrap();
    let throws = contract
        .claim_id(&claim_subject(SemanticClaimPath::Domain(ClaimPath::Call(
            ClaimDomain::Throws,
        ))))
        .unwrap();

    assert!(contract.contains_closed_claim_id(callbacks.as_str()));
    assert!(!contract.contains_closed_claim_id(throws.as_str()));
    assert!(!contract.contains_closed_claim_id("sha256:not-a-claim"));

    let mut other_package = package();
    other_package.version = "2.0.0-rc.4".into();
    let mut case = artifact_case("server-import");
    case.exports.insert(
        "createResource".into(),
        export(
            &case,
            "createResource",
            ValueShape::Plain,
            CallSemantics::new(
                CallClaims {
                    callbacks: KnowledgeSet::complete(vec![]),
                    ..CallClaims::default()
                },
                vec![],
                vec![],
                vec![],
                GuardPartition::default(),
            ),
        ),
    );
    let other = ContractProposal::new(other_package, vec![case])
        .normalize()
        .unwrap();
    assert!(!other.contains_closed_claim_id(callbacks.as_str()));

    let mut other_case = artifact_case("client-import");
    other_case.exports.insert(
        "createResource".into(),
        export(
            &other_case,
            "createResource",
            ValueShape::Plain,
            CallSemantics::new(
                CallClaims {
                    callbacks: KnowledgeSet::complete(vec![]),
                    ..CallClaims::default()
                },
                vec![],
                vec![],
                vec![],
                GuardPartition::default(),
            ),
        ),
    );
    let other_case = ContractProposal::new(package(), vec![other_case])
        .normalize()
        .unwrap();
    assert!(!other_case.contains_closed_claim_id(callbacks.as_str()));

    let mut other_export = artifact_case("server-import");
    other_export.exports.insert(
        "other".into(),
        export(
            &other_export,
            "other",
            ValueShape::Plain,
            CallSemantics::new(
                CallClaims {
                    callbacks: KnowledgeSet::complete(vec![]),
                    ..CallClaims::default()
                },
                vec![],
                vec![],
                vec![],
                GuardPartition::default(),
            ),
        ),
    );
    let other_export = ContractProposal::new(package(), vec![other_export])
        .normalize()
        .unwrap();
    assert!(!other_export.contains_closed_claim_id(callbacks.as_str()));

    let one_item = proposal_with(
        ValueShape::Tuple(KnowledgeSet::complete(vec![ValueShape::Tuple(
            KnowledgeSet::complete(vec![]),
        )])),
        call(vec![], vec![]),
    )
    .normalize()
    .unwrap();
    let first_item = one_item
        .claim_id(&claim_subject(SemanticClaimPath::Domain(
            ClaimPath::Value {
                root: ValueRoot::Export,
                path: ValuePath(vec![ValuePathSegment::TupleItem(0)]),
                domain: ValueClaimDomain::TupleItems,
            },
        )))
        .unwrap();
    let two_items = proposal_with(
        ValueShape::Tuple(KnowledgeSet::complete(vec![
            ValueShape::Plain,
            ValueShape::Tuple(KnowledgeSet::complete(vec![])),
        ])),
        call(vec![], vec![]),
    )
    .normalize()
    .unwrap();
    let second_item = two_items
        .claim_id(&claim_subject(SemanticClaimPath::Domain(
            ClaimPath::Value {
                root: ValueRoot::Export,
                path: ValuePath(vec![ValuePathSegment::TupleItem(1)]),
                domain: ValueClaimDomain::TupleItems,
            },
        )))
        .unwrap();
    assert!(one_item.contains_closed_claim_id(first_item.as_str()));
    assert!(two_items.contains_closed_claim_id(second_item.as_str()));
    assert!(!one_item.contains_closed_claim_id(second_item.as_str()));
}

#[test]
fn semantic_claim_ids_reject_orphan_subjects_and_noncanonical_spellings() {
    let contract = proposal_with(
        ValueShape::Tuple(KnowledgeSet::Complete(vec![ValueShape::Plain])),
        call(vec![operation("read", OperationKind::Read)], vec![]),
    )
    .normalize()
    .unwrap();
    let missing_operation =
        claim_subject(SemanticClaimPath::Operation(OperationId("missing".into())));
    assert!(matches!(
        contract.claim_id(&missing_operation),
        Err(ClaimIdentityError::InvalidSubject { .. })
    ));

    let missing_leaf = claim_subject(SemanticClaimPath::Domain(ClaimPath::Value {
        root: ValueRoot::Export,
        path: ValuePath(vec![ValuePathSegment::TupleItem(1)]),
        domain: ValueClaimDomain::Shape,
    }));
    assert!(matches!(
        contract.claim_id(&missing_leaf),
        Err(ClaimIdentityError::InvalidSubject { .. })
    ));

    let valid = contract
        .claim_id(&claim_subject(SemanticClaimPath::Operation(OperationId(
            "read".into(),
        ))))
        .unwrap();
    assert!(SemanticClaimId::parse(valid.as_str().to_ascii_uppercase()).is_err());
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

    let mut trigger_left = operation("trigger-left", OperationKind::Read);
    trigger_left.trigger = Some(Trigger::Operation(OperationId("trigger-right".into())));
    let mut trigger_right = operation("trigger-right", OperationKind::Write);
    trigger_right.trigger = Some(Trigger::Operation(OperationId("trigger-left".into())));
    assert!(matches!(
        proposal_with(
            ValueShape::Plain,
            call(vec![trigger_left, trigger_right], vec![]),
        )
        .normalize(),
        Err(ModelError::OperationCycle { .. })
    ));

    let mut mixed_left = operation("mixed-left", OperationKind::Read);
    mixed_left.trigger = Some(Trigger::Operation(OperationId("mixed-right".into())));
    let mixed_right = operation("mixed-right", OperationKind::Write);
    let mut mixed = call(vec![mixed_left, mixed_right], vec![]);
    mixed.edges.push(OperationEdge {
        kind: EdgeKind::Data,
        from: OperationId("mixed-left".into()),
        to: OperationId("mixed-right".into()),
    });
    assert!(matches!(
        proposal_with(ValueShape::Plain, mixed).normalize(),
        Err(ModelError::OperationCycle { .. })
    ));
}

#[test]
fn resource_lifetime_graph_allows_self_anchor_but_rejects_indirect_cycles() {
    let mut self_bound = resource("self", ResourceKind::ReactiveSource);
    self_bound.lifetime = Some(Lifetime::Resource(ResourceId("self".into())));
    assert!(
        proposal_with(ValueShape::Plain, call(vec![], vec![self_bound]))
            .normalize()
            .is_ok()
    );

    let mut first = resource("first", ResourceKind::ReactiveSource);
    first.lifetime = Some(Lifetime::Resource(ResourceId("second".into())));
    let mut second = resource("second", ResourceKind::ReactiveSource);
    second.lifetime = Some(Lifetime::Resource(ResourceId("first".into())));
    assert!(matches!(
        proposal_with(ValueShape::Plain, call(vec![], vec![first, second])).normalize(),
        Err(ModelError::ResourceCycle { .. })
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
fn open_guard_partition_retains_fallback_only_as_possible_behavior() {
    let mut guarded = call(vec![], vec![]);
    guarded.guards.cases = KnowledgeSet::Partial(vec![GuardedCase::Otherwise {
        operations: KnowledgeSet::Complete(vec![]),
    }]);
    let normalized = proposal_with(ValueShape::Plain, guarded)
        .normalize()
        .unwrap();
    let partition = &normalized.artifact_cases()[0].exports["createResource"]
        .call
        .guards;
    assert!(matches!(
        partition.select_operations(|_| GuardTruth::False),
        KnowledgeSet::Unknown
    ));

    let positive = GuardPartition {
        cases: KnowledgeSet::Partial(vec![
            GuardedCase::When {
                guard: literal_guard(true),
                operations: KnowledgeSet::Complete(vec![OperationId("read".into())]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![OperationId("write".into())]),
            },
        ]),
    }
    .select_operations(|_| GuardTruth::True);
    assert_eq!(
        positive,
        KnowledgeSet::Partial(vec![OperationId("read".into())])
    );
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
fn semantic_model_v1_digest_algorithm_and_golden_vector_are_frozen() {
    assert_eq!(SEMANTIC_MODEL_VERSION, 1);
    assert_eq!(SEMANTIC_DIGEST_ALGORITHM, "sha256");
    assert_eq!(
        SEMANTIC_DIGEST_DOMAIN,
        "solid-checker:normalized-package-contract"
    );

    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let mut behavior = call(vec![read.clone(), write.clone()], vec![owner, cleanup]);
    behavior.edges = vec![OperationEdge {
        kind: EdgeKind::Data,
        from: read.id,
        to: write.id,
    }];
    let normalized = proposal_with(ValueShape::Plain, behavior)
        .normalize()
        .unwrap();
    assert_eq!(
        normalized.semantic_digest().as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9"
    );
}

/// The provenance digest family is separate, frozen, and disjoint from the
/// legacy one.
///
/// The vector above is what pins the half that matters most: a contract with
/// no composed operation hashes exactly the bytes it hashed before
/// `composed_from` existed, so every policy-2 receipt already issued for such
/// a contract keeps authenticating. This pins the other half — that a contract
/// which *does* state provenance lands in its own family, under its own
/// domain, with its own frozen vector.
#[test]
fn composed_provenance_digest_family_is_separate_and_frozen() {
    assert_eq!(
        SEMANTIC_DIGEST_DOMAIN_COMPOSED,
        "solid-checker:normalized-package-contract:composed-provenance"
    );

    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let plain = |composed: Option<ComposedFrom>| {
        let mut read = read.clone();
        read.composed_from = composed;
        let mut behavior = call(
            vec![read.clone(), write.clone()],
            vec![owner.clone(), cleanup.clone()],
        );
        behavior.edges = vec![OperationEdge {
            kind: EdgeKind::Data,
            from: read.id.clone(),
            to: write.id.clone(),
        }];
        proposal_with(ValueShape::Plain, behavior)
            .normalize()
            .unwrap()
    };
    // The same contract, with and without provenance on one row. The two
    // digests are in different families and neither is the other's.
    assert_eq!(
        plain(None).semantic_digest().as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9",
        "a provenance-free contract must keep the legacy vector byte for byte"
    );
    assert_eq!(
        plain(Some(ComposedFrom {
            export: "createPolled".into(),
            operation: OperationId("case:createPolled:operation:read-0".into()),
        }))
        .semantic_digest()
        .as_str(),
        "sha256:6d2c93ab74d0543599ce2729ae2d705a197bc70242eeee1d7ff69d08a2563700"
    );
}

/// The proposed-closure families are separate, frozen, and disjoint from both
/// families above.
///
/// A proposed closure states no knowledge — the domain it names stays open —
/// but it is what `inspect_candidates` derives the planner's candidate
/// universe from, so two documents that differ only in what they propose plan
/// different demand graphs and must not share the identity a receipt binds.
/// The features are independent, so the four combinations are four domains.
#[test]
fn proposed_closure_digest_family_is_separate_and_frozen() {
    assert_eq!(
        SEMANTIC_DIGEST_DOMAIN_PROPOSED_CLOSURE,
        "solid-checker:normalized-package-contract:proposed-closure"
    );
    assert_eq!(
        SEMANTIC_DIGEST_DOMAIN_COMPOSED_PROPOSED_CLOSURE,
        "solid-checker:normalized-package-contract:composed-provenance:proposed-closure"
    );

    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let contract = |propose: bool, composed: bool| {
        let mut read = read.clone();
        if composed {
            read.composed_from = Some(ComposedFrom {
                export: "createPolled".into(),
                operation: OperationId("case:createPolled:operation:read-0".into()),
            });
        }
        let mut behavior = call(
            vec![read.clone(), write.clone()],
            vec![owner.clone(), cleanup.clone()],
        );
        behavior.edges = vec![OperationEdge {
            kind: EdgeKind::Data,
            from: read.id.clone(),
            to: write.id.clone(),
        }];
        // The label is over a closure the document states, which the helper
        // already closes empty, so the two variants differ in the label alone.
        let behavior = if propose {
            behavior.with_proposed_closures([ClaimDomain::Creates])
        } else {
            behavior
        };
        proposal_with(ValueShape::Plain, behavior)
            .normalize()
            .unwrap()
    };
    assert_eq!(
        contract(false, false).semantic_digest().as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9",
        "a contract that proposes nothing keeps the legacy vector byte for byte"
    );
    assert_eq!(
        contract(true, false).semantic_digest().as_str(),
        "sha256:46711b6a1ccebc437a1beb44d90854c7a53c8f5bf45fac89421ee6e935732a05"
    );
    assert_eq!(
        contract(true, true).semantic_digest().as_str(),
        "sha256:c2906640684350d1053c1b3409c2b69db35822fb3d7ffd4055a394cdd7395249"
    );
}

/// ADR 0114's family: a contract stating a `computations` item hashes under a
/// marker of its own, written before anything else, so every contract stating
/// none keeps the stream it had -- the golden vector, again, is that half.
#[test]
fn computations_digest_family_is_separate_and_frozen() {
    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let contract = |computes: bool| {
        let mut operations = vec![read.clone(), write.clone()];
        if computes {
            operations.push(compute("compute"));
        }
        let mut behavior = call(operations, vec![owner.clone(), cleanup.clone()]);
        behavior.edges = vec![OperationEdge {
            kind: EdgeKind::Data,
            from: read.id.clone(),
            to: write.id.clone(),
        }];
        proposal_with(ValueShape::Plain, behavior)
            .normalize()
            .unwrap()
    };
    assert_eq!(
        contract(false).semantic_digest().as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9",
        "a contract stating no computation keeps the legacy vector byte for byte"
    );
    assert_eq!(
        contract(true).semantic_digest().as_str(),
        "sha256:9ae7a327fbb8ab15d31649a1e0c34acbee00f35698cb2d53627c4ceab014be67"
    );
}

/// Item B round 2 of ways-to-improve § 3.3: `undefined` is appended to the
/// canonical value encoding as tag 20, and a member of the caller's argument
/// is the `parameter` shape's existing path. No document before it carries the
/// tag, so it needs no digest family of its own: every other document keeps
/// the bytes it had (the legacy vector, asserted again here), and one that
/// states the new shape hashes to this frozen vector.
#[test]
fn an_undefined_output_is_an_appended_tag_with_a_frozen_vector() {
    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let mut legacy = call(vec![read.clone(), write.clone()], vec![owner, cleanup]);
    legacy.edges = vec![OperationEdge {
        kind: EdgeKind::Data,
        from: read.id,
        to: write.id,
    }];
    assert_eq!(
        proposal_with(ValueShape::Plain, legacy)
            .normalize()
            .unwrap()
            .semantic_digest()
            .as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9",
        "a contract stating no undefined output keeps the legacy vector byte for byte"
    );

    let mut member = operation("return-0", OperationKind::Return);
    member.output = Some(ValueShape::Parameter {
        index: 0,
        path: vec!["defaultPrevented".into()],
    });
    let mut undefined = operation("return-1", OperationKind::Return);
    undefined.output = Some(ValueShape::Undefined);
    let mut behavior = call(vec![member.clone(), undefined.clone()], vec![]);
    behavior.claims.returns = KnowledgeSet::Complete(vec![member.id, undefined.id]);
    let stated = proposal_with(ValueShape::Callable, behavior)
        .normalize()
        .unwrap();
    assert_eq!(
        stated.semantic_digest().as_str(),
        "sha256:9f1c2830a0b0d90a16f7bf375001f527b4c6a979cfeb7839b1564cd5494378e1"
    );
}

/// The label is over a closure this document states, so it is well-formed only
/// where the domain really is closed and only where a certifier can decide it.
#[test]
fn a_proposed_closure_is_refused_over_an_open_domain_and_over_an_undecidable_one() {
    let mut behavior = call(vec![], vec![]);
    behavior.claims.creates = KnowledgeSet::Unknown;
    let open = proposal_with(
        ValueShape::Plain,
        behavior.with_proposed_closures([ClaimDomain::Creates]),
    )
    .normalize()
    .expect_err("an open domain states no closure to propose");
    assert!(
        matches!(&open, ModelError::Contradiction { reason, .. } if reason.contains("creates")),
        "{open}"
    );

    // `writes`, not `reads`: `reads` gained a closure proof mode on
    // 2026-09-10 and this row needs a domain that still has none.
    let undecidable = proposal_with(
        ValueShape::Plain,
        call(vec![], vec![]).with_proposed_closures([ClaimDomain::Writes]),
    )
    .normalize()
    .expect_err("a domain with no closure proof mode cannot be proposed");
    assert!(
        matches!(
            &undecidable,
            ModelError::InvalidKnowledge { reason, .. }
                if reason.contains("writes") && reason.contains("no closure proof mode")
        ),
        "{undecidable}"
    );
    assert!(ClaimDomain::Creates.is_proposable());
    assert!(ClaimDomain::Returns.is_proposable());
    assert!(ClaimDomain::Reads.is_proposable());
    assert!(ClaimDomain::Callbacks.is_proposable());
    assert!(!ClaimDomain::Writes.is_proposable());
    assert_eq!(
        ClaimDomain::PROPOSABLE,
        [
            ClaimDomain::Creates,
            ClaimDomain::Returns,
            ClaimDomain::Reads,
            ClaimDomain::Callbacks
        ]
    );
}

/// Opening a domain withdraws its proposal.
///
/// This is what keeps an opaque closure frontier and a recipe-gated
/// withholding effective: both reopen the domain, and a marker that survived
/// would let the certifier rediscover the candidate it had just withdrawn.
#[test]
fn opening_a_call_domain_withdraws_its_proposed_closure() {
    let mut export = normalized_export(proposal_with(
        ValueShape::Plain,
        call(vec![], vec![]).with_proposed_closures([ClaimDomain::Creates]),
    ));
    assert_eq!(
        export.call.proposed_closures(),
        &BTreeSet::from([ClaimDomain::Creates])
    );
    let mut weakened = export.clone();
    assert!(
        weakened
            .open_proposed_closure()
            .contains(&ClaimPath::Call(ClaimDomain::Creates))
    );
    assert!(weakened.call.proposed_closures().is_empty());

    export.open_call_domains([ClaimDomain::Creates]);
    assert!(export.call.proposed_closures().is_empty());
    assert!(export.claim_state(ClaimDomain::Creates).is_open());
    // And back again: the generator republishes exactly this pair.
    export.propose_closures([ClaimDomain::Creates]);
    assert!(!export.claim_state(ClaimDomain::Creates).is_open());
    assert_eq!(
        export.call.proposed_closures(),
        &BTreeSet::from([ClaimDomain::Creates])
    );
}

/// Provenance is part of the operation's identity, so it is part of the
/// digest.
///
/// The two proposals state the same rows and differ only in where one of them
/// says the read happens. A digest that could not tell them apart would let an
/// acceptance receipt for the plain row authenticate the composed one, whose
/// evidence is an entirely different premise.
#[test]
fn composed_provenance_moves_the_semantic_digest() {
    let plain = operation("read", OperationKind::Read);
    let mut composed = plain.clone();
    composed.composed_from = Some(ComposedFrom {
        export: "createPolled".into(),
        operation: OperationId("case:createPolled:operation:read-0".into()),
    });
    let digest = |operation| {
        proposal_with(ValueShape::Plain, call(vec![operation], vec![]))
            .normalize()
            .unwrap()
            .semantic_digest()
            .as_str()
            .to_owned()
    };
    assert_ne!(digest(plain), digest(composed));
}

/// Only a `read` operation may state provenance, and never its own export's
/// operation.
///
/// Both refusals are the model's, not the certifier's: a published
/// `composedFrom` on an `invoke` row would be a fact whose premise was never
/// reviewed, and one naming this export's own operation is a cycle wearing a
/// proof's clothes.
#[test]
fn composed_provenance_is_refused_on_a_non_read_and_on_its_own_export() {
    let mut invoke = operation("invoke", OperationKind::Invoke);
    // The control: the same row without provenance normalizes, so the refusal
    // below is the guard's and not the proposal's shape.
    assert!(
        proposal_with(
            ValueShape::Plain,
            call(vec![operation("invoke", OperationKind::Invoke)], vec![]),
        )
        .normalize()
        .is_ok()
    );
    invoke.composed_from = Some(ComposedFrom {
        export: "other".into(),
        operation: OperationId("case:other:operation:read-0".into()),
    });
    let error = proposal_with(ValueShape::Plain, call(vec![invoke], vec![]))
        .normalize()
        .expect_err("provenance on an invoke operation");
    assert!(
        format!("{error:?}").contains("only a read operation may state composed provenance"),
        "{error:?}"
    );

    // Two well-formed reads, both in the closed `reads` claim, so validation
    // reaches the provenance guard instead of stopping at a node with no
    // positive claim behind it. The `call` helper cannot build this: it
    // *overwrites* `claims.reads` per operation, so a two-read proposal built
    // with it is malformed and every assertion about it passes for the wrong
    // reason.
    let mut read = operation("read", OperationKind::Read);
    let sibling = operation("sibling", OperationKind::Read);
    read.composed_from = Some(ComposedFrom {
        export: "self".into(),
        operation: sibling.id.clone(),
    });
    let two_reads = |operations: Vec<Operation>| {
        let mut claims = closed_claims();
        claims.reads =
            KnowledgeSet::Complete(operations.iter().map(|row| row.id.clone()).collect());
        CallSemantics::new(
            claims,
            operations,
            vec![],
            vec![],
            GuardPartition {
                cases: KnowledgeSet::complete(vec![]),
            },
        )
    };
    // The control: the same two rows, with no provenance, normalize.
    assert!(
        proposal_with(
            ValueShape::Plain,
            two_reads(vec![
                operation("read", OperationKind::Read),
                sibling.clone(),
            ]),
        )
        .normalize()
        .is_ok(),
        "the two-read proposal itself must be well formed, or the guard below is untested"
    );
    let error = proposal_with(ValueShape::Plain, two_reads(vec![read, sibling]))
        .normalize()
        .expect_err("a provenance naming this export's own operation");
    assert!(
        format!("{error:?}").contains("composed provenance names an operation of the composing"),
        "{error:?}"
    );
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

/// One export whose `reads` lists `reads` operations, each id carrying the
/// artifact-case prefix a generated document gives it.
fn addressed_contract(
    case_id: &str,
    closure: char,
    tracking: Tracking,
    reads: usize,
) -> NormalizedContract {
    let mut case = artifact_case(case_id);
    case.dependency_closure = digest(closure);
    let operations = (0..reads)
        .map(|index| {
            let mut read = operation(
                &format!("{case_id}:createResource:operation:read-{index}"),
                OperationKind::Read,
            );
            read.tracking = tracking;
            read
        })
        .collect::<Vec<_>>();
    let mut call = call(vec![], vec![]);
    call.claims.reads = KnowledgeSet::complete(operations.iter().map(|op| op.id.clone()).collect());
    call.operations = operations;
    let export = export(&case, "createResource", ValueShape::Callable, call);
    case.exports.insert("createResource".into(), export);
    ContractProposal::new(package(), vec![case])
        .normalize()
        .unwrap()
}

fn addressed_subject(case_id: &str, path: SemanticClaimPath) -> SemanticClaimSubject {
    SemanticClaimSubject {
        artifact_case: case_id.into(),
        export: "createResource".into(),
        path,
    }
}

fn address_of(
    contract: &NormalizedContract,
    case_id: &str,
    path: SemanticClaimPath,
    closure_bytes: &str,
) -> RecipeAddress {
    let bytes = contract
        .artifact_case_byte_identity(case_id, closure_bytes)
        .unwrap();
    contract
        .recipe_address(&addressed_subject(case_id, path), &bytes)
        .unwrap()
}

const READS: SemanticClaimPath = SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Reads));

/// Ways-to-improve § 3.2: a dependency whose accepted contract moved changes
/// the dependent's artifact case id and dependency-closure digest, and with
/// them every claim id. The recipe address is the same claim stated over bytes
/// and value alone, so it does not move.
#[test]
fn a_recipe_address_ignores_the_case_id_and_the_dependency_closure_digest() {
    let before = addressed_contract("case-a", 'd', Tracking::Untracked, 1);
    let after = addressed_contract("case-b", 'e', Tracking::Untracked, 1);
    assert_eq!(
        before
            .artifact_case_byte_identity("case-a", "closure")
            .unwrap(),
        after
            .artifact_case_byte_identity("case-b", "closure")
            .unwrap(),
    );
    assert_ne!(
        before
            .claim_id(&addressed_subject("case-a", READS))
            .unwrap(),
        after.claim_id(&addressed_subject("case-b", READS)).unwrap(),
    );
    let address = address_of(&before, "case-a", READS, "closure");
    assert_eq!(address, address_of(&after, "case-b", READS, "closure"));
    assert_eq!(RecipeAddress::parse(address.as_str()).unwrap(), address);
    assert!(address.as_str().starts_with("recipe-address:v1:sha256:"));

    let operation = |case: &str| {
        SemanticClaimPath::Operation(OperationId(format!(
            "{case}:createResource:operation:read-0"
        )))
    };
    let operation_address = address_of(&before, "case-a", operation("case-a"), "closure");
    assert_eq!(
        operation_address,
        address_of(&after, "case-b", operation("case-b"), "closure")
    );
    assert_ne!(operation_address, address);
}

#[test]
fn a_recipe_address_binds_the_claim_value() {
    let one_read = addressed_contract("case-a", 'd', Tracking::Untracked, 1);
    let address = address_of(&one_read, "case-a", READS, "closure");
    // `reads` closed over nothing versus closed over `read-0`.
    let no_read = addressed_contract("case-a", 'd', Tracking::Untracked, 0);
    assert_ne!(address, address_of(&no_read, "case-a", READS, "closure"));
    // The same list, but the operation it names is tracked.
    let tracked = addressed_contract("case-a", 'd', Tracking::Tracked, 1);
    assert_ne!(address, address_of(&tracked, "case-a", READS, "closure"));
    // A different call domain of the same export is a different subject.
    let writes = SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Writes));
    assert_ne!(address, address_of(&one_read, "case-a", writes, "closure"));
}

#[test]
fn a_recipe_address_binds_the_closure_bytes_and_refuses_unaddressable_subjects() {
    let contract = addressed_contract("case-a", 'd', Tracking::Untracked, 1);
    assert_ne!(
        address_of(&contract, "case-a", READS, "closure-one"),
        address_of(&contract, "case-a", READS, "closure-two"),
    );
    let bytes = contract
        .artifact_case_byte_identity("case-a", "closure")
        .unwrap();
    assert!(matches!(
        contract.recipe_address(
            &addressed_subject(
                "case-a",
                SemanticClaimPath::Domain(ClaimPath::GuardPartition)
            ),
            &bytes
        ),
        Err(ModelError::Unaddressable { .. })
    ));
    assert!(matches!(
        contract.recipe_address(
            &addressed_subject(
                "case-a",
                SemanticClaimPath::Operation(OperationId(
                    "case-a:createResource:operation:gone".into()
                ))
            ),
            &bytes
        ),
        Err(ModelError::Unaddressable { .. })
    ));
    assert!(
        contract
            .artifact_case_byte_identity("missing", "closure")
            .is_err()
    );
    let claim = contract
        .claim_id(&addressed_subject("case-a", READS))
        .unwrap();
    assert_eq!(
        RecipeAddress::parse(claim.as_str()),
        Err(ModelError::RecipeAddressFormat)
    );
}

/// One export whose `callbacks` closes over one bare-parameter `invoke`
/// operation, with the artifact-case prefix a generated document gives it.
fn invoking_contract(case_id: &str) -> NormalizedContract {
    let mut case = artifact_case(case_id);
    case.dependency_closure = digest('d');
    let mut invoke = operation(
        &format!("{case_id}:createResource:operation:callback-0"),
        OperationKind::Invoke,
    );
    invoke.tracking = Tracking::AmbientAtExecution;
    let call = call(vec![invoke], vec![]);
    let export = export(&case, "createResource", ValueShape::Callable, call);
    case.exports.insert("createResource".into(), export);
    ContractProposal::new(package(), vec![case])
        .normalize()
        .unwrap()
}

const CALLBACKS: SemanticClaimPath =
    SemanticClaimPath::Domain(ClaimPath::Call(ClaimDomain::Callbacks));

/// ADR 0117's addresses and the semantic digest of a contract stating no
/// non-call protocol, frozen before the invoke-protocol family existed: the
/// 95 addresses migrated into the recipe corpus stay valid only if the stream
/// a call-only claim writes never moves.
#[test]
fn call_only_recipe_addresses_and_digests_are_frozen() {
    let reads = addressed_contract("case-a", 'd', Tracking::Untracked, 1);
    assert_eq!(
        address_of(&reads, "case-a", READS, "closure").as_str(),
        "recipe-address:v1:sha256:c1a7b40c824ccff08c8e393eaa5996846847cca99ebac44fec4a7252f8d9b148"
    );
    let invoking = invoking_contract("case-a");
    assert_eq!(
        address_of(&invoking, "case-a", CALLBACKS, "closure").as_str(),
        "recipe-address:v1:sha256:f0cb2480183d4f915cf277d21a8eb6efa14d5ec3e021688275171b9333c14428"
    );
    assert_eq!(
        invoking.semantic_digest().as_str(),
        "sha256:2e2e4199fc90e25040d2a2f45950a871cef1cd551b83b0c508b994c8d437120f"
    );
}

/// `invoking_contract` with the one `invoke` restated as a non-call protocol.
fn protocol_contract(
    case_id: &str,
    protocol: Option<InvokeProtocol>,
) -> Result<NormalizedContract, ModelError> {
    let mut case = artifact_case(case_id);
    case.dependency_closure = digest('d');
    let mut invoke = operation(
        &format!("{case_id}:createResource:operation:callback-0"),
        OperationKind::Invoke,
    );
    invoke.tracking = Tracking::AmbientAtExecution;
    invoke.cardinality = Cardinality {
        scope: Some(CardinalityScope::Call),
        min: Some(0),
        max: Some(UpperBound::Many),
    };
    invoke.protocol = protocol;
    let call = call(vec![invoke], vec![]);
    let export = export(&case, "createResource", ValueShape::Callable, call);
    case.exports.insert("createResource".into(), export);
    ContractProposal::new(package(), vec![case]).normalize()
}

/// Item A of ways-to-improve § 3.3: a non-call protocol is its own digest
/// family. A stated `call` is the absent protocol, so it hashes as a
/// call-only contract does; a `get` writes the marker first and the protocol
/// of every operation, under a frozen vector of its own; each protocol is a
/// distinct claim.
#[test]
fn invoke_protocol_digest_family_is_separate_and_frozen() {
    assert_eq!(
        SEMANTIC_INVOKE_PROTOCOL_MARKER,
        "solid-checker:semantic-invoke-protocol:v1"
    );
    let call = protocol_contract("case-a", None).unwrap();
    let stated_call = protocol_contract("case-a", Some(InvokeProtocol::Call)).unwrap();
    assert_eq!(
        stated_call.artifact_cases()[0].exports["createResource"]
            .call
            .operations[0]
            .protocol,
        None,
        "a decoded `call` normalizes to the absent protocol"
    );
    assert_eq!(call.semantic_digest(), stated_call.semantic_digest());
    let get = protocol_contract("case-a", Some(InvokeProtocol::Get)).unwrap();
    assert_ne!(call.semantic_digest(), get.semantic_digest());
    assert_eq!(
        get.semantic_digest().as_str(),
        "sha256:3d793b66c3a616cee35f786386d1ffb8eb6ac0fba98e278a51b1dae05976c375"
    );
    let digests = [
        InvokeProtocol::Get,
        InvokeProtocol::Iterate,
        InvokeProtocol::Coerce,
        InvokeProtocol::HasInstance,
    ]
    .map(|protocol| {
        protocol_contract("case-a", Some(protocol))
            .unwrap()
            .semantic_digest()
            .clone()
    });
    for (index, digest) in digests.iter().enumerate() {
        assert!(
            digests[index + 1..].iter().all(|other| other != digest),
            "each protocol is a distinct claim"
        );
    }
}

/// ADR 0117's address, per claim: a call-only callbacks claim keeps the
/// frozen address above, and a claim naming a non-call protocol is addressed
/// in the invoke-protocol family, differently per protocol.
#[test]
fn a_recipe_address_binds_the_invoke_protocol() {
    let call = address_of(
        &protocol_contract("case-a", None).unwrap(),
        "case-a",
        CALLBACKS,
        "closure",
    );
    let get = address_of(
        &protocol_contract("case-a", Some(InvokeProtocol::Get)).unwrap(),
        "case-a",
        CALLBACKS,
        "closure",
    );
    let coerce = address_of(
        &protocol_contract("case-a", Some(InvokeProtocol::Coerce)).unwrap(),
        "case-a",
        CALLBACKS,
        "closure",
    );
    assert_ne!(call, get);
    assert_ne!(get, coerce);
    assert_eq!(
        get.as_str(),
        "recipe-address:v1:sha256:8e315333ad3d92c5e421d0baad2fac181723605f9860ded22a9b972638143cdb"
    );
}

/// A non-call invocation states one shape: an `invoke`, at the call event on
/// the same stack, `ambient-at-execution` (never `untracked`), counted per
/// call from zero to many, unguarded, named by exactly one `callbacks` item
/// from a bare parameter.
#[test]
fn a_non_call_invocation_is_validated_to_its_one_shape() {
    let refused = |mutate: &dyn Fn(&mut CallSemantics), needle: &str| {
        let mut invoke = operation("callback-0", OperationKind::Invoke);
        invoke.tracking = Tracking::AmbientAtExecution;
        invoke.cardinality = Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(0),
            max: Some(UpperBound::Many),
        };
        invoke.protocol = Some(InvokeProtocol::Get);
        let mut behavior = call(vec![invoke], vec![]);
        mutate(&mut behavior);
        let error = proposal_with(ValueShape::Callable, behavior)
            .normalize()
            .expect_err("the shape is refused");
        assert!(error.to_string().contains(needle), "{needle:?} in {error}");
    };
    refused(
        &|call| call.operations[0].tracking = Tracking::Untracked,
        "ambient-at-execution",
    );
    refused(
        &|call| call.operations[0].tracking = Tracking::Tracked,
        "ambient-at-execution",
    );
    refused(
        &|call| call.operations[0].tracking = Tracking::Unknown,
        "ambient-at-execution",
    );
    refused(
        &|call| call.operations[0].schedule = Some(Schedule::Queued),
        "at the call event on the same stack",
    );
    refused(
        &|call| call.operations[0].cardinality.min = Some(1),
        "from zero to many",
    );
    refused(
        &|call| {
            call.claims.callbacks = KnowledgeSet::Complete(vec![CallbackInvocation {
                from: ValueSource::Parameter {
                    index: 0,
                    path: vec!["length".into()],
                },
                operation: OperationId("callback-0".into()),
            }]);
        },
        "exactly one callbacks item from a bare parameter",
    );
    refused(
        &|call| {
            let item = call.claims.callbacks.items()[0].clone();
            let second = CallbackInvocation {
                from: ValueSource::Parameter {
                    index: 1,
                    path: vec![],
                },
                ..item.clone()
            };
            call.claims.callbacks = KnowledgeSet::Complete(vec![item, second]);
        },
        "exactly one callbacks item from a bare parameter",
    );
    // Only an invoke may state a protocol.
    let mut read = operation("read", OperationKind::Read);
    read.protocol = Some(InvokeProtocol::Coerce);
    let error = proposal_with(ValueShape::Callable, call(vec![read], vec![]))
        .normalize()
        .expect_err("a read states no protocol");
    assert!(
        error
            .to_string()
            .contains("only an invoke operation may state a protocol"),
        "{error}"
    );
}

/// A non-call item withdrawn by narrowing leaves `callbacks` closed and its
/// proposal standing, down to the empty enumeration; a call item, or a non-call
/// item not marked as narrowing, opens the domain exactly as before.
#[test]
fn a_narrowed_non_call_item_keeps_its_callbacks_closure() {
    let invoke = |id: &str, protocol: Option<InvokeProtocol>| {
        let mut invoke = operation(id, OperationKind::Invoke);
        invoke.tracking = Tracking::AmbientAtExecution;
        invoke.cardinality = Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(0),
            max: Some(UpperBound::Many),
        };
        invoke.protocol = protocol;
        invoke
    };
    let item = |index: u16, id: &str| CallbackInvocation {
        from: ValueSource::Parameter {
            index,
            path: vec![],
        },
        operation: OperationId(id.into()),
    };
    let mixed = || {
        let mut call = call(
            vec![
                invoke("callback-0", None),
                invoke("callback-1", Some(InvokeProtocol::Coerce)),
            ],
            vec![],
        );
        call.claims.callbacks =
            KnowledgeSet::Complete(vec![item(0, "callback-0"), item(1, "callback-1")]);
        normalized_export(proposal_with(
            ValueShape::Callable,
            call.with_proposed_closures([ClaimDomain::Callbacks]),
        ))
    };
    let ids = |ids: &[&str]| {
        ids.iter()
            .map(|id| OperationId((*id).into()))
            .collect::<BTreeSet<_>>()
    };
    let case_id = |export: &ExportSemantics, id: &str| {
        export
            .call
            .operations
            .iter()
            .find(|operation| operation.id.0.ends_with(id))
            .unwrap()
            .id
            .0
            .clone()
    };

    // The coerce narrows: closed, proposed, and only the call is left.
    let mut narrowed = mixed();
    let coerce = case_id(&narrowed, "callback-1");
    narrowed.withhold_operations_narrowing(&ids(&[&coerce]), &ids(&[&coerce]));
    assert!(narrowed.callbacks().is_closed());
    assert_eq!(narrowed.callbacks().items().len(), 1);
    assert!(
        narrowed
            .call
            .proposed_closures()
            .contains(&ClaimDomain::Callbacks)
    );

    // The same withdrawal without the narrowing mark opens the domain.
    let mut opened = mixed();
    opened.withhold_operations_narrowing(&ids(&[&coerce]), &BTreeSet::new());
    assert!(!opened.callbacks().is_closed());

    // A call item is never narrowed, whatever the caller asks.
    let mut call_item = mixed();
    let call = case_id(&call_item, "callback-0");
    call_item.withhold_operations_narrowing(&ids(&[&call]), &ids(&[&call]));
    assert!(!call_item.callbacks().is_closed());

    // Narrowed to nothing is the closed empty enumeration, not unknown.
    let mut empty = mixed();
    empty.withhold_operations_narrowing(&ids(&[&call, &coerce]), &ids(&[&coerce]));
    assert!(
        !empty.callbacks().is_closed(),
        "the call's withdrawal opens it"
    );
    let mut only_protocols = {
        let mut call = call_semantics_with_one_coerce();
        call.claims.callbacks = KnowledgeSet::Complete(vec![item(0, "callback-0")]);
        normalized_export(proposal_with(
            ValueShape::Callable,
            call.with_proposed_closures([ClaimDomain::Callbacks]),
        ))
    };
    let only = case_id(&only_protocols, "callback-0");
    only_protocols.withhold_operations_narrowing(&ids(&[&only]), &ids(&[&only]));
    assert_eq!(only_protocols.callbacks(), &KnowledgeSet::Complete(vec![]));
    assert!(
        only_protocols
            .call
            .proposed_closures()
            .contains(&ClaimDomain::Callbacks)
    );
}

fn call_semantics_with_one_coerce() -> CallSemantics {
    let mut invoke = operation("callback-0", OperationKind::Invoke);
    invoke.tracking = Tracking::AmbientAtExecution;
    invoke.cardinality = Cardinality {
        scope: Some(CardinalityScope::Call),
        min: Some(0),
        max: Some(UpperBound::Many),
    };
    invoke.protocol = Some(InvokeProtocol::Coerce);
    call(vec![invoke], vec![])
}

/// ADR 0139's one shape: an `invoke` triggered by and at `result-access`, on
/// an external schedule, ambient for tracking and owner, counted per trigger
/// from zero to many, unguarded.
fn result_access_operation(id: &str) -> Operation {
    let mut invoke = operation(id, OperationKind::Invoke);
    invoke.trigger = Some(Trigger::Event(Event::ResultAccess));
    invoke.at = Some(Event::ResultAccess);
    invoke.schedule = Some(Schedule::External);
    invoke.tracking = Tracking::AmbientAtExecution;
    invoke.owner = OwnerRelation {
        source: OwnerSource::AmbientAtExecution,
        productions: KnowledgeSet::complete(vec![]),
        ..OwnerRelation::default()
    };
    invoke.cardinality = Cardinality {
        scope: Some(CardinalityScope::Trigger),
        min: Some(0),
        max: Some(UpperBound::Many),
    };
    invoke
}

/// `invoking_contract`'s shape with its one `invoke` a `result-access` item.
fn result_access_contract(case_id: &str) -> Result<NormalizedContract, ModelError> {
    let mut case = artifact_case(case_id);
    case.dependency_closure = digest('d');
    let invoke = result_access_operation(&format!("{case_id}:createResource:operation:callback-0"));
    let call = call(vec![invoke], vec![]);
    let export = export(&case, "createResource", ValueShape::Callable, call);
    case.exports.insert("createResource".into(), export);
    ContractProposal::new(package(), vec![case]).normalize()
}

/// ADR 0139: a contract that states a `result-access` operation is its own
/// digest family, under a frozen vector, and distinct from the same item at
/// any other event; a claim naming one is addressed in that family.
#[test]
fn result_access_digest_family_is_separate_and_frozen() {
    assert_eq!(
        SEMANTIC_RESULT_ACCESS_MARKER,
        "solid-checker:semantic-result-access:v1"
    );
    let kept = result_access_contract("case-a").unwrap();
    assert_eq!(
        kept.semantic_digest().as_str(),
        "sha256:382b278d9347bc91f55fa1d8ac1185e1ec9ae622bab42b121ba8212b18cb63c7"
    );
    let call = protocol_contract("case-a", None).unwrap();
    assert_ne!(kept.semantic_digest(), call.semantic_digest());
    let kept_address = address_of(&kept, "case-a", CALLBACKS, "closure");
    assert_ne!(
        kept_address,
        address_of(&call, "case-a", CALLBACKS, "closure")
    );
    assert_eq!(
        kept_address.as_str(),
        "recipe-address:v1:sha256:334480c0c7d4ef149f0e4291a93b0d72d909338ce61c0a2af53e0e2331001497"
    );
}

/// ADR 0153 part 3: a context premise is its own digest family under a frozen
/// vector. The premise conditions the export's claims without naming one, so a
/// claim id and a recipe address stay where they were: a corpus recipe keyed
/// by either still binds after the transaction states the premise.
#[test]
fn context_premise_digest_family_is_separate_frozen_and_moves_no_claim() {
    assert_eq!(
        SEMANTIC_CONTEXT_PREMISES_MARKER,
        "solid-checker:semantic-context-premises:v1"
    );
    let plain = result_access_contract("case-a").unwrap();
    let premised = |names: &[&str]| {
        let mut cases = plain.artifact_cases().to_vec();
        cases[0]
            .exports
            .get_mut("createResource")
            .unwrap()
            .add_context_premises(names.iter().map(|name| ContextPremise {
                export: (*name).into(),
            }));
        ContractProposal::new(plain.package().clone(), cases)
            .normalize()
            .unwrap()
    };
    let one = premised(&["RouterContext"]);
    let two = premised(&["RouterContext", "OtherContext"]);
    assert_ne!(one.semantic_digest(), plain.semantic_digest());
    assert_ne!(one.semantic_digest(), two.semantic_digest());
    assert_eq!(
        one.semantic_digest().as_str(),
        "sha256:f2cbb8d12d8786923dcc8fac6b0b4714e6efc9983128c46f4f88fbe0140224f6"
    );
    assert_eq!(
        address_of(&one, "case-a", CALLBACKS, "closure"),
        address_of(&plain, "case-a", CALLBACKS, "closure")
    );
    let subject = addressed_subject("case-a", CALLBACKS);
    assert_eq!(
        one.claim_id(&subject).unwrap(),
        plain.claim_id(&subject).unwrap()
    );
}

/// ADR 0153 item C: an accessor-installation bound is its own digest family
/// under a frozen vector, moves no claim id or recipe address, survives the
/// candidate weakening's knowledge-level round trip, and is cleared by every
/// opening that withdraws `reads`.
#[test]
fn accessor_bound_digest_family_is_separate_frozen_and_cleared_with_reads() {
    assert_eq!(
        SEMANTIC_ACCESSOR_BOUNDS_MARKER,
        "solid-checker:semantic-accessor-bounds:v1"
    );
    let plain = result_access_contract("case-a").unwrap();
    let bounded = |sources: &[&str]| {
        let mut cases = plain.artifact_cases().to_vec();
        cases[0]
            .exports
            .get_mut("createResource")
            .unwrap()
            .add_accessor_bounds(sources.iter().map(|source| (*source).to_owned()));
        ContractProposal::new(plain.package().clone(), cases)
            .normalize()
            .unwrap()
    };
    let one = bounded(&["./index.js:10-15"]);
    let two = bounded(&["./index.js:10-15", "./other.js:1-6"]);
    assert_ne!(one.semantic_digest(), plain.semantic_digest());
    assert_ne!(one.semantic_digest(), two.semantic_digest());
    assert_eq!(
        one.semantic_digest().as_str(),
        "sha256:098092b7feb71eaed5260bc30e9c64142ca598c74862789b1cee588ce24de478"
    );
    assert_eq!(
        address_of(&one, "case-a", CALLBACKS, "closure"),
        address_of(&plain, "case-a", CALLBACKS, "closure")
    );
    let subject = addressed_subject("case-a", CALLBACKS);
    assert_eq!(
        one.claim_id(&subject).unwrap(),
        plain.claim_id(&subject).unwrap()
    );
    // Withdrawing `reads` withdraws what bounded it; any other domain keeps
    // it.
    let mut export = one.artifact_cases()[0].exports["createResource"].clone();
    export.open_call_domains([ClaimDomain::Creates]);
    assert_eq!(export.call.accessor_bounds().len(), 1);
    export.open_call_domains([ClaimDomain::Reads]);
    assert!(export.call.accessor_bounds().is_empty());
    // An empty source bounds nothing and is refused.
    let mut cases = plain.artifact_cases().to_vec();
    cases[0]
        .exports
        .get_mut("createResource")
        .unwrap()
        .add_accessor_bounds([String::new()]);
    assert!(
        ContractProposal::new(plain.package().clone(), cases)
            .normalize()
            .is_err()
    );
}

/// A `result-access` operation states exactly one shape, and exactly one
/// `callbacks` item names it, from a bare parameter.
#[test]
fn a_result_access_operation_is_validated_to_its_one_shape() {
    let refused = |mutate: &dyn Fn(&mut CallSemantics), needle: &str| {
        let mut behavior = call(vec![result_access_operation("callback-0")], vec![]);
        mutate(&mut behavior);
        let error = proposal_with(ValueShape::Callable, behavior)
            .normalize()
            .expect_err("the shape is refused");
        assert!(error.to_string().contains(needle), "{needle:?} in {error}");
    };
    normalized_export(proposal_with(
        ValueShape::Callable,
        call(vec![result_access_operation("callback-0")], vec![]),
    ));
    let at_event = "is triggered by and happens at the result-access event";
    refused(
        &|call| call.operations[0].trigger = Some(Trigger::Event(Event::Call)),
        at_event,
    );
    refused(&|call| call.operations[0].at = Some(Event::Call), at_event);
    refused(
        &|call| call.operations[0].schedule = Some(Schedule::Queued),
        at_event,
    );
    let ambient = "ambient-at-execution and unconstrained";
    refused(
        &|call| call.operations[0].tracking = Tracking::Untracked,
        ambient,
    );
    refused(
        &|call| call.operations[0].owner.source = OwnerSource::AmbientAtCall,
        ambient,
    );
    refused(
        &|call| call.operations[0].owner.requirements.owner = Requirement::Required,
        ambient,
    );
    refused(
        &|call| call.operations[0].cardinality.scope = Some(CardinalityScope::Call),
        "per trigger, from zero to many",
    );
    refused(
        &|call| call.operations[0].protocol = Some(InvokeProtocol::Get),
        "at the call event on the same stack",
    );
    refused(
        &|call| call.operations[0].inputs = vec![ValueShape::Plain],
        "states no inputs, output or resources",
    );
    let one_item = "exactly one callbacks item from a bare parameter";
    refused(
        &|call| {
            call.claims.callbacks = KnowledgeSet::Complete(vec![CallbackInvocation {
                from: ValueSource::Parameter {
                    index: 0,
                    path: vec!["handler".into()],
                },
                operation: OperationId("callback-0".into()),
            }]);
        },
        one_item,
    );
    refused(
        &|call| {
            let item = call.claims.callbacks.items()[0].clone();
            let second = CallbackInvocation {
                from: ValueSource::Parameter {
                    index: 1,
                    path: vec![],
                },
                ..item.clone()
            };
            call.claims.callbacks = KnowledgeSet::Complete(vec![item, second]);
        },
        one_item,
    );
    // Only an invoke happens at the event.
    let mut read = result_access_operation("read");
    read.kind = OperationKind::Read;
    let error = proposal_with(ValueShape::Callable, call(vec![read], vec![]))
        .normalize()
        .expect_err("a read is not kept for later invocation");
    assert!(
        error
            .to_string()
            .contains("a result-access operation is an invoke"),
        "{error}"
    );
}

/// ADR 0145: a described callable is appended to the canonical value encoding
/// as tag 21, both of its lists written in canonical order. No document before
/// it carries the tag, so it needs no digest family: every other document keeps
/// its bytes (the legacy vector, asserted again here), and one that states it
/// hashes to this frozen vector.
#[test]
fn a_described_callable_output_is_an_appended_tag_with_a_frozen_vector() {
    let read = operation("read", OperationKind::Read);
    let write = operation("write", OperationKind::Write);
    let owner = resource("owner", ResourceKind::Owner);
    let cleanup = resource("cleanup", ResourceKind::Cleanup);
    let mut legacy = call(vec![read.clone(), write.clone()], vec![owner, cleanup]);
    legacy.edges = vec![OperationEdge {
        kind: EdgeKind::Data,
        from: read.id,
        to: write.id,
    }];
    assert_eq!(
        proposal_with(ValueShape::Plain, legacy)
            .normalize()
            .unwrap()
            .semantic_digest()
            .as_str(),
        "sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9",
        "a contract stating no described callable keeps the legacy vector byte for byte"
    );

    let described = |returns: Vec<ValueShape>| {
        let mut returned = operation("return", OperationKind::Return);
        returned.output = Some(ValueShape::DescribedCallable(Box::new(DescribedCall {
            reads: Vec::new(),
            returns,
            callbacks: Vec::new(),
        })));
        let mut behavior = call(vec![returned.clone()], vec![]);
        behavior.claims.returns = KnowledgeSet::Complete(vec![returned.id]);
        proposal_with(ValueShape::Callable, behavior).normalize()
    };
    let plain = described(vec![ValueShape::Plain]).unwrap();
    assert_eq!(
        plain.semantic_digest().as_str(),
        "sha256:76ff18f124569912553ecc6480fe2cf494923280c61dee04db4dcfa15dd8bc97"
    );
    let valueless = described(Vec::new()).unwrap();
    assert_ne!(
        valueless.semantic_digest(),
        plain.semantic_digest(),
        "what the returned callable hands back is part of the claim"
    );
}

/// ADR 0145: a described callable is valid only as the whole output of a
/// `return`, its returns are drawn from `plain` alone and neither list repeats.
#[test]
fn a_described_callable_is_validated_to_its_one_position_and_vocabulary() {
    let callable = |reads: Vec<DescribedRead>, returns: Vec<ValueShape>| {
        ValueShape::DescribedCallable(Box::new(DescribedCall {
            reads,
            returns,
            callbacks: Vec::new(),
        }))
    };
    let with_output = |kind: OperationKind, output: ValueShape| {
        let mut operation = operation("subject", kind);
        operation.output = Some(output);
        let mut behavior = call(vec![operation.clone()], vec![]);
        match kind {
            OperationKind::Return => {
                behavior.claims.returns = KnowledgeSet::Complete(vec![operation.id]);
            }
            OperationKind::Invoke => {
                behavior.claims.callbacks = KnowledgeSet::Unknown;
            }
            _ => {}
        }
        proposal_with(ValueShape::Callable, behavior).normalize()
    };
    assert!(with_output(OperationKind::Return, callable(Vec::new(), Vec::new())).is_ok());
    assert!(
        with_output(
            OperationKind::Return,
            callable(vec![DescribedRead::OwnedSignal], vec![ValueShape::Plain])
        )
        .is_ok()
    );
    // ADR 0162: a memo read is a described read, and a read value may be
    // returned beside it.
    assert!(
        with_output(
            OperationKind::Return,
            callable(vec![DescribedRead::OwnedMemo], vec![ValueShape::ReadValue])
        )
        .is_ok()
    );
    for (kind, output, needle) in [
        (
            OperationKind::Return,
            callable(
                vec![DescribedRead::OwnedMemo, DescribedRead::OwnedMemo],
                Vec::new(),
            ),
            "a described read is repeated",
        ),
        (
            OperationKind::Return,
            callable(Vec::new(), vec![ValueShape::Undefined]),
            "may return only `plain`",
        ),
        (
            OperationKind::Return,
            callable(Vec::new(), vec![ValueShape::Plain, ValueShape::Plain]),
            "a described return is repeated",
        ),
        (
            OperationKind::Return,
            callable(
                vec![DescribedRead::OwnedSignal, DescribedRead::OwnedSignal],
                Vec::new(),
            ),
            "a described read is repeated",
        ),
        (
            OperationKind::Return,
            ValueShape::Tuple(KnowledgeSet::Complete(vec![callable(
                Vec::new(),
                Vec::new(),
            )])),
            "whole output of a return",
        ),
    ] {
        let error = with_output(kind, output).unwrap_err().to_string();
        assert!(error.contains(needle), "{error}");
    }
    let error = proposal_with(
        callable(Vec::new(), Vec::new()),
        call(Vec::new(), Vec::new()),
    )
    .normalize()
    .unwrap_err()
    .to_string();
    assert!(error.contains("whole output of a return"), "{error}");
}

/// ADR 0152: a described callable's callback items are the one invocation the
/// census proves, of a bare export argument, one item per argument; its
/// returns may name what such an argument returned; and only a described
/// callable that states an item moves off ADR 0145's encoding.
#[test]
fn a_described_callables_callback_items_are_validated_and_hashed_apart() {
    use crate::contract_semantics::DescribedCallback;
    let normalized = |callbacks: Vec<DescribedCallback>, returns: Vec<ValueShape>| {
        let mut returned = operation("return", OperationKind::Return);
        returned.output = Some(ValueShape::DescribedCallable(Box::new(DescribedCall {
            reads: Vec::new(),
            returns,
            callbacks,
        })));
        let mut behavior = call(vec![returned.clone()], vec![]);
        behavior.claims.returns = KnowledgeSet::Complete(vec![returned.id]);
        proposal_with(ValueShape::Callable, behavior).normalize()
    };
    let pipe = normalized(
        vec![
            DescribedCallback::same_stack_once(1),
            DescribedCallback::same_stack_once(0),
        ],
        vec![ValueShape::InvocationResult { parameter: 1 }],
    )
    .expect("pipe's claim is the admitted shape");
    let without = normalized(Vec::new(), vec![ValueShape::Plain]).unwrap();
    assert_eq!(
        without.semantic_digest().as_str(),
        "sha256:76ff18f124569912553ecc6480fe2cf494923280c61dee04db4dcfa15dd8bc97",
        "a described callable with no item keeps ADR 0145's vector byte for byte"
    );
    assert_ne!(pipe.semantic_digest(), without.semantic_digest());
    let reordered = normalized(
        vec![
            DescribedCallback::same_stack_once(0),
            DescribedCallback::same_stack_once(1),
        ],
        vec![ValueShape::InvocationResult { parameter: 1 }],
    )
    .unwrap();
    assert_eq!(
        pipe.semantic_digest(),
        reordered.semantic_digest(),
        "items are canonically sorted"
    );

    let mut deferred = DescribedCallback::same_stack_once(0);
    deferred.schedule = Some(Schedule::Queued);
    let mut possible = DescribedCallback::same_stack_once(0);
    possible.cardinality.min = Some(0);
    let mut untracked = DescribedCallback::same_stack_once(0);
    untracked.tracking = Tracking::Untracked;
    let mut member = DescribedCallback::same_stack_once(0);
    member.from = ValueSource::Parameter {
        index: 0,
        path: vec!["run".into()],
    };
    for (callbacks, returns, needle) in [
        (vec![deferred], Vec::new(), "on the same stack"),
        (vec![possible], Vec::new(), "exactly once per call"),
        (vec![untracked], Vec::new(), "tracking context"),
        (vec![member], Vec::new(), "bare argument of its export"),
        (
            vec![
                DescribedCallback::same_stack_once(0),
                DescribedCallback::same_stack_once(0),
            ],
            Vec::new(),
            "at most one callback item",
        ),
        (
            vec![DescribedCallback::same_stack_once(0)],
            vec![ValueShape::InvocationResult { parameter: 1 }],
            "may return only `plain`",
        ),
        (
            Vec::new(),
            vec![ValueShape::InvocationResult { parameter: 0 }],
            "may return only `plain`",
        ),
    ] {
        let error = normalized(callbacks, returns).unwrap_err().to_string();
        assert!(error.contains(needle), "{needle}: {error}");
    }
}

/// ADR 0177: weakening names members of a bare return's literal output and
/// leaves exactly those undescribed; anything else changes nothing.
#[test]
fn a_structural_return_member_is_weakened_in_place() {
    let accessor = || ValueShape::Reactive {
        role: ReactiveRole::Accessor,
        resource: None,
        capabilities: KnowledgeSet::Unknown,
    };
    let mut returned = operation("return", OperationKind::Return);
    returned.owner = OwnerRelation::default();
    returned.cardinality = Cardinality {
        scope: Some(CardinalityScope::Call),
        min: Some(0),
        max: Some(UpperBound::Many),
    };
    returned.output = Some(ValueShape::Tuple(KnowledgeSet::Complete(vec![
        accessor(),
        accessor(),
        ValueShape::Unknown,
    ])));
    let mut export = normalized_export(proposal_with(
        ValueShape::Callable,
        call(vec![returned], vec![]),
    ));
    let id = export.call.operations[0].id.clone();
    assert!(export.call.operations[0].is_bare_return());
    let member = |index| ValuePath(vec![ValuePathSegment::TupleItem(index)]);

    assert!(
        !export.weaken_return_members(&id, &[member(3)]),
        "no fourth member"
    );
    assert!(
        !export.weaken_return_members(&id, &[ValuePath(vec![])]),
        "the root is no member"
    );
    assert!(
        !export.weaken_return_members(&OperationId("absent".into()), &[member(1)]),
        "no such operation"
    );
    assert_eq!(
        export.call.operations[0].output,
        Some(ValueShape::Tuple(KnowledgeSet::Complete(vec![
            accessor(),
            accessor(),
            ValueShape::Unknown,
        ]))),
        "a refused weakening changes nothing"
    );

    assert!(export.weaken_return_members(&id, &[member(1)]));
    assert_eq!(
        export.call.operations[0].output,
        Some(ValueShape::Tuple(KnowledgeSet::Complete(vec![
            accessor(),
            ValueShape::Unknown,
            ValueShape::Unknown,
        ])))
    );
    assert!(
        export
            .operation_claim(ClaimDomain::Returns)
            .is_some_and(|claim| claim.items().len() == 1),
        "the return itself is kept"
    );
}

// ADR 0183: withdrawing a created owner keeps the invoke, resets its owner and
// lower bound, and drops the owner resource only when nothing else names it.
#[test]
fn weakening_a_created_owner_keeps_the_invoke_and_drops_an_unnamed_resource() {
    let case = artifact_case("case");
    let created = |id: &str, owner: &str| {
        let mut invoke = operation(id, OperationKind::Invoke);
        invoke.owner.source = OwnerSource::Created(ResourceId(owner.into()));
        invoke.cardinality.min = Some(1);
        invoke
    };
    let mut lone = export(
        &case,
        "lone",
        ValueShape::Plain,
        call(
            vec![created("invoke", "owner")],
            vec![resource("owner", ResourceKind::Owner)],
        ),
    );
    assert!(lone.weaken_created_owner(&OperationId("invoke".into())));
    let invoke = &lone.call.operations[0];
    assert_eq!(invoke.owner, OwnerRelation::default());
    assert_eq!(invoke.cardinality.min, Some(0));
    assert!(lone.call.resources.is_empty());

    let mut lifetime = operation("read", OperationKind::Read);
    lifetime.cardinality.scope = Some(CardinalityScope::Resource(ResourceId("owner".into())));
    let mut shared = export(
        &case,
        "shared",
        ValueShape::Plain,
        call(
            vec![created("invoke", "owner"), lifetime],
            vec![resource("owner", ResourceKind::Owner)],
        ),
    );
    assert!(shared.weaken_created_owner(&OperationId("invoke".into())));
    assert_eq!(shared.call.resources.len(), 1, "a named resource is kept");

    let mut plain = export(
        &case,
        "plain",
        ValueShape::Plain,
        call(vec![operation("invoke", OperationKind::Invoke)], vec![]),
    );
    assert!(!plain.weaken_created_owner(&OperationId("invoke".into())));
}

/// ADR 0207: `event-handler-props` is exactly the keys the Solid runtime's
/// spread attaches as event listeners.
#[test]
fn the_event_handler_member_class_is_every_unnamespaced_on_key() {
    let class = MemberClass::EventHandlerProps;
    assert_eq!(class.wire(), "event-handler-props");
    for key in ["onClick", "onkeydown", "on", "onPointerDown"] {
        assert!(class.contains(key), "{key}");
    }
    for key in [
        "on:click",
        "oncapture:click",
        "ref",
        "as",
        "children",
        "render",
        "Onclick",
    ] {
        assert!(!class.contains(key), "{key}");
    }
}
