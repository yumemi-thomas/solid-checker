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

fn operation(id: &str, min: u32) -> Operation {
    Operation {
        id: OperationId(id.into()),
        kind: OperationKind::Read,
        guard: None,
        trigger: Some(Trigger::Event(Event::Call)),
        at: Some(Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: OwnerRelation {
            source: OwnerSource::None,
            lifetime: Some(Lifetime::Call),
            productions: KnowledgeSet::complete(vec![]),
            ..OwnerRelation::default()
        },
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(min),
            max: Some(UpperBound::Finite(1)),
        },
        inputs: vec![],
        output: None,
        resources: BTreeSet::new(),
    }
}

fn accepted() -> AcceptedContract {
    let runtime = artifact("./dist/index.js", 'b');
    let declarations = artifact("./types/index.d.ts", 'c');
    let identity = ExportIdentity {
        entrypoint: ".".into(),
        public_name: "run".into(),
        runtime: ExportTargetIdentity {
            module: runtime.clone(),
            export_name: "run".into(),
        },
        declarations: ExportTargetIdentity {
            module: declarations.clone(),
            export_name: "run".into(),
        },
    };
    let export = ExportSemantics {
        identity,
        shape: ValueShape::Callable,
        stability: StabilityKnowledge::Unknown,
        call: CallSemantics::new(
            CallClaims {
                reads: KnowledgeSet::complete(vec![
                    OperationId("read-a".into()),
                    OperationId("read-b".into()),
                ]),
                callbacks: KnowledgeSet::complete(vec![]),
                writes: KnowledgeSet::complete(vec![]),
                creates: KnowledgeSet::complete(vec![]),
                invalidates: KnowledgeSet::complete(vec![]),
                throws: KnowledgeSet::complete(vec![]),
                returns: KnowledgeSet::complete(vec![]),
                cleanups: KnowledgeSet::complete(vec![]),
                disposals: KnowledgeSet::complete(vec![]),
            },
            vec![operation("read-a", 1), operation("read-b", 0)],
            vec![],
            vec![],
            GuardPartition {
                cases: KnowledgeSet::complete(vec![
                    GuardedCase::When {
                        guard: Guard(vec![GuardAtom::Signature("sig:a".into())]),
                        operations: KnowledgeSet::complete(vec![OperationId("read-a".into())]),
                    },
                    GuardedCase::Otherwise {
                        operations: KnowledgeSet::complete(vec![OperationId("read-b".into())]),
                    },
                ]),
            },
        ),
    };
    let case = ArtifactCase {
        id: "case-a".into(),
        entrypoint: ".".into(),
        resolution_trace: vec![],
        runtime,
        declarations,
        dependency_closure: digest('d'),
        transform: None,
        stability: StabilityKnowledge::Unknown,
        exports: BTreeMap::from([("run".into(), export)]),
    };
    AcceptedContract {
        package: PackageIdentity {
            name: "pkg".into(),
            version: "1.0.0".into(),
            integrity: "sha512:test".into(),
            manifest: artifact("./package.json", 'a'),
        },
        selected_case: case,
        receipt: AcceptanceReceipt {
            receipt_version: 1,
            wire_digest: digest('1'),
            semantic_model_version: 1,
            semantic_digest: digest('2'),
            artifacts_digest: digest('3'),
            closure_digest: digest('d'),
            proof_root: digest('4'),
            closed_claims_root: digest('5'),
            verifier: VerifierIdentity {
                build: "test".into(),
                policy: 1,
            },
        },
    }
}

#[test]
fn exact_export_identity_refuses_same_spelling_from_another_artifact() {
    let accepted = accepted();
    let identity = accepted.export("run").unwrap().identity.clone();
    assert!(accepted.resolve_export(&identity).is_ok());
    let mut wrong = identity;
    wrong.runtime.module.digest = digest('9');
    assert!(matches!(
        accepted.resolve_export(&wrong),
        Err(SemanticQueryError::MissingExport { .. })
    ));
}

#[test]
fn exact_signature_selects_one_case_and_preserves_strength() {
    let accepted = accepted();
    let identity = accepted.export("run").unwrap().identity.clone();
    let facts = CallSiteFacts {
        selected_signatures: FiniteFact::exact("sig:a".into()),
        ..CallSiteFacts::default()
    };
    let export = accepted.instantiate_export(&identity, &facts).unwrap();
    assert_eq!(
        export.operation_claim(ClaimDomain::Reads).knowledge,
        KnowledgeSet::complete(vec![OperationId("read-a".into())])
    );
    assert_eq!(
        export
            .guaranteed_operations(ClaimDomain::Reads)
            .iter()
            .map(|operation| operation.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["read-a"]
    );
}

#[test]
fn unresolved_guard_selection_joins_monotonically_without_inventing_guarantees() {
    let accepted = accepted();
    let identity = accepted.export("run").unwrap().identity.clone();
    let facts = CallSiteFacts::default();
    let export = accepted.instantiate_export(&identity, &facts).unwrap();
    let claim = export.operation_claim(ClaimDomain::Reads);
    assert_eq!(
        claim.knowledge,
        KnowledgeSet::complete(vec![
            OperationId("read-a".into()),
            OperationId("read-b".into()),
        ])
    );
    assert_eq!(claim.open_reasons, vec![OpenDomainReason::GuardSelection]);
    assert!(export.guaranteed_operations(ClaimDomain::Reads).is_empty());
}

#[test]
fn one_open_domain_does_not_weaken_a_closed_sibling() {
    let mut accepted = accepted();
    accepted
        .selected_case
        .exports
        .get_mut("run")
        .unwrap()
        .call
        .claims
        .writes = KnowledgeSet::Unknown;
    let identity = accepted.export("run").unwrap().identity.clone();
    let facts = CallSiteFacts {
        selected_signatures: FiniteFact::exact("sig:a".into()),
        ..CallSiteFacts::default()
    };
    let export = accepted.instantiate_export(&identity, &facts).unwrap();
    assert!(matches!(
        export.operation_claim(ClaimDomain::Writes).knowledge,
        KnowledgeSet::Unknown
    ));
    assert_eq!(
        export.operation_claim(ClaimDomain::Reads).knowledge,
        KnowledgeSet::complete(vec![OperationId("read-a".into())])
    );
}

#[test]
fn native_dialect_wins_only_when_contract_is_compatible() {
    let native = KnowledgeSet::complete(vec![OperationId("read-a".into())]);
    let compatible = KnowledgeSet::partial(vec![OperationId("read-a".into())]).unwrap();
    assert_eq!(
        native_claim_precedence(ClaimDomain::Reads, Some(&native), &compatible).unwrap(),
        native
    );
    let contradictory = KnowledgeSet::complete(vec![OperationId("read-b".into())]);
    assert_eq!(
        native_claim_precedence(ClaimDomain::Reads, Some(&native), &contradictory),
        Err(SemanticQueryError::NativeContractConflict {
            domain: ClaimDomain::Reads,
        })
    );
}

#[test]
fn analyzer_index_binds_semantics_to_the_exact_import_occurrence() {
    let contract = accepted();
    let export = contract.export("run").unwrap().identity.clone();
    let index = AcceptedContractIndex::new([AcceptedContractInput {
        importer: "/project/a.ts".into(),
        specifier: "pkg".into(),
        contract,
    }])
    .unwrap();

    assert_eq!(index.semantic_identity()[0].importer, "/project/a.ts");
    assert!(index.resolve("/project/a.ts", "pkg", &export).is_ok());
    assert_eq!(
        index.resolve("/project/b.ts", "pkg", &export).unwrap_err(),
        SemanticQueryError::MissingImport {
            importer: "/project/b.ts".into(),
            specifier: "pkg".into(),
        }
    );
}

#[test]
fn analyzer_cache_identity_is_deterministic_across_acquisition_order() {
    let inputs = || {
        [
            AcceptedContractInput {
                importer: "/project/a.ts".into(),
                specifier: "pkg".into(),
                contract: accepted(),
            },
            AcceptedContractInput {
                importer: "/project/b.ts".into(),
                specifier: "pkg".into(),
                contract: accepted(),
            },
        ]
    };
    let forward = AcceptedContractIndex::new(inputs()).unwrap();
    let reverse = AcceptedContractIndex::new(inputs().into_iter().rev()).unwrap();
    assert_eq!(forward.semantic_identity(), reverse.semantic_identity());
    assert_eq!(forward.cache_fingerprint(), reverse.cache_fingerprint());
}

#[test]
fn duplicate_exact_import_is_refused_and_receipt_policy_is_cache_identity() {
    let first = accepted();
    let mut changed_policy = first.clone();
    changed_policy.receipt.verifier.policy += 1;
    assert_ne!(
        first.semantic_identity(),
        changed_policy.semantic_identity()
    );
    let first_index = AcceptedContractIndex::new([AcceptedContractInput {
        importer: "/project/a.ts".into(),
        specifier: "pkg".into(),
        contract: first.clone(),
    }])
    .unwrap();
    let changed_index = AcceptedContractIndex::new([AcceptedContractInput {
        importer: "/project/a.ts".into(),
        specifier: "pkg".into(),
        contract: changed_policy.clone(),
    }])
    .unwrap();
    assert_ne!(
        first_index.cache_fingerprint(),
        changed_index.cache_fingerprint()
    );

    assert_eq!(
        AcceptedContractIndex::new([
            AcceptedContractInput {
                importer: "/project/a.ts".into(),
                specifier: "pkg".into(),
                contract: first,
            },
            AcceptedContractInput {
                importer: "/project/a.ts".into(),
                specifier: "pkg".into(),
                contract: changed_policy,
            },
        ])
        .unwrap_err(),
        SemanticQueryError::AmbiguousImport {
            importer: "/project/a.ts".into(),
            specifier: "pkg".into(),
        }
    );
}

#[test]
fn accepted_artifact_case_decides_artifact_guards_without_optional_call_facts() {
    let mut contract = accepted();
    contract
        .selected_case
        .exports
        .get_mut("run")
        .unwrap()
        .call
        .operations
        .iter_mut()
        .find(|operation| operation.id.0 == "read-a")
        .unwrap()
        .guard = Some(Guard(vec![GuardAtom::ArtifactCase("case-a".into())]));
    let identity = contract.export("run").unwrap().identity.clone();
    let facts = CallSiteFacts {
        selected_signatures: FiniteFact::exact("sig:a".into()),
        ..CallSiteFacts::default()
    };
    let export = contract.instantiate_export(&identity, &facts).unwrap();
    assert_eq!(
        export.operation_claim(ClaimDomain::Reads).knowledge,
        KnowledgeSet::complete(vec![OperationId("read-a".into())])
    );
}

#[test]
fn complete_absence_and_unknown_remain_distinct_at_one_domain() {
    let mut contract = accepted();
    let export = contract.selected_case.exports.get_mut("run").unwrap();
    export.call.claims.writes = KnowledgeSet::complete(vec![]);
    export.call.claims.throws = KnowledgeSet::Unknown;
    let identity = export.identity.clone();
    let facts = CallSiteFacts {
        selected_signatures: FiniteFact::exact("sig:a".into()),
        ..CallSiteFacts::default()
    };
    let export = contract.instantiate_export(&identity, &facts).unwrap();

    assert!(
        export
            .operation_claim(ClaimDomain::Writes)
            .knowledge
            .proves_absence()
    );
    assert!(matches!(
        export.operation_claim(ClaimDomain::Throws).knowledge,
        KnowledgeSet::Unknown
    ));
    assert_eq!(
        export.operation_claim(ClaimDomain::Throws).diagnostics(),
        vec![OpenDomainDiagnostic {
            code: "open-claim-domain",
            claim: Some(ClaimPath::Call(ClaimDomain::Throws)),
            operation: None,
        }]
    );
}

#[test]
fn typefacts_exact_constants_close_only_their_literal_leaf() {
    let entity: typefacts::EntityFact = serde_json::from_value(serde_json::json!({
        "location": { "path": "/project/a.ts", "startByte": 5, "endByte": 11 },
        "runtimeValueDomain": { "mayBeOther": true },
        "constantValue": { "kind": "string", "string": "sync" }
    }))
    .unwrap();
    let mut facts = CallSiteFacts::default();
    facts.set_argument_entity(0, vec!["mode".into()], &entity);

    assert_eq!(
        facts.evaluate(
            &GuardAtom::Literal {
                argument: 0,
                path: vec!["mode".into()],
                value: Literal::String("sync".into()),
            },
            "case-a",
        ),
        GuardTruth::True
    );
    assert_eq!(
        facts.evaluate(
            &GuardAtom::Literal {
                argument: 0,
                path: vec!["other".into()],
                value: Literal::String("sync".into()),
            },
            "case-a",
        ),
        GuardTruth::Unknown
    );
    assert_eq!(
        facts.evaluate(
            &GuardAtom::ValueKind {
                argument: 0,
                path: vec!["mode".into()],
                kind: ValueKind::Plain,
            },
            "case-a",
        ),
        GuardTruth::Unknown,
        "runtime other also permits promise and async-iterable values"
    );
}

#[test]
fn every_restricted_guard_axis_reads_its_own_exact_call_site_fact() {
    let mut facts = CallSiteFacts {
        selected_signatures: FiniteFact::exact("sig:a".into()),
        argument_counts: FiniteFact::exact(2),
        tuple_alternatives: BTreeMap::from([(1, FiniteFact::exact(3))]),
        result_protocols: FiniteFact::exact(ValueKind::Promise),
        ..CallSiteFacts::default()
    };
    facts.set_literal(
        0,
        vec!["mode".into()],
        FiniteFact::exact(Literal::String("sync".into())),
    );
    facts.set_value_kind(0, vec![], FiniteFact::exact(ValueKind::Callable));
    facts.set_property(
        0,
        vec![],
        "apply".into(),
        PropertyFact {
            present: FiniteFact::exact(true),
            callable: FiniteFact::exact(false),
        },
    );

    for atom in [
        GuardAtom::Signature("sig:a".into()),
        GuardAtom::ArgumentCount {
            min: 1,
            max: Some(2),
        },
        GuardAtom::Literal {
            argument: 0,
            path: vec!["mode".into()],
            value: Literal::String("sync".into()),
        },
        GuardAtom::ValueKind {
            argument: 0,
            path: vec![],
            kind: ValueKind::Callable,
        },
        GuardAtom::Property {
            argument: 0,
            path: vec![],
            name: "apply".into(),
            callable: Some(false),
        },
        GuardAtom::TupleAlternative {
            argument: 1,
            alternative: 3,
        },
        GuardAtom::ResultProtocol(ValueKind::Promise),
        GuardAtom::ArtifactCase("case-a".into()),
    ] {
        assert_eq!(
            facts.evaluate(&atom, "case-a"),
            GuardTruth::True,
            "{atom:?}"
        );
    }
    assert_eq!(
        facts.evaluate(&GuardAtom::Signature("sig:b".into()), "case-a"),
        GuardTruth::False
    );
}

#[test]
fn invocation_transcript_supplies_exact_signature_arity_and_result_protocol() {
    let location = serde_json::json!({
        "path": "/project/a.ts",
        "startByte": 1,
        "endByte": 8
    });
    let transcript: typefacts::InvocationTranscript = serde_json::from_value(serde_json::json!({
        "location": location,
        "validity": "valid",
        "kind": "call",
        "selectedSignature": {
            "identity": "sig:a",
            "declaration": {
                "kind": "function",
                "location": location
            },
            "overloadOrdinal": 0,
            "overloadCount": 1,
            "minimumArgumentCount": 1,
            "result": {
                "callability": "nonCallable",
                "constructability": "nonConstructable",
                "primitive": {},
                "partitions": [{
                    "axis": "protocol",
                    "complete": true,
                    "cases": [{ "kind": "promise", "protocol": "promise" }]
                }]
            }
        },
        "bindings": [{
            "argumentIndex": 0,
            "location": location,
            "disposition": "direct",
            "slots": [{ "expandedIndex": 0, "parameterIndex": 0 }]
        }],
        "complete": ["signature", "bindings", "result"]
    }))
    .unwrap();
    let facts = CallSiteFacts::from_invocation(&transcript);

    assert_eq!(
        facts.evaluate(&GuardAtom::Signature("sig:a".into()), "case-a"),
        GuardTruth::True
    );
    assert_eq!(
        facts.evaluate(
            &GuardAtom::ArgumentCount {
                min: 1,
                max: Some(1),
            },
            "case-a",
        ),
        GuardTruth::True
    );
    assert_eq!(
        facts.evaluate(&GuardAtom::ResultProtocol(ValueKind::Promise), "case-a",),
        GuardTruth::True
    );
}
