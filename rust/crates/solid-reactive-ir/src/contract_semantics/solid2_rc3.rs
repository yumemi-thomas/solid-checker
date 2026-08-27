//! Exact published Solid 2.0.0-rc.3 semantic conformance corpus.
//!
//! This is an internal normalized-model corpus, not a public package-contract
//! wire format and not a bundled-contract cutover.  Each case binds the exact
//! published manifest, runtime, declaration, and tarball file-manifest
//! identities before describing the independently knowable semantic leaves.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ArrayLength, ArtifactCase, ArtifactIdentity, CallClaims, CallSemantics, CallbackInvocation,
    CapabilityClaim, CapabilityKnowledge, Cardinality, CardinalityScope, ClaimDomain,
    ContractProposal, Digest, EdgeKind, Event, ExportIdentity, ExportSemantics,
    ExportTargetIdentity, Guard, GuardAtom, GuardPartition, GuardedCase, KnowledgeSet, Lifetime,
    Literal, ObservableCapability, Operation, OperationEdge, OperationId, OperationKind,
    OwnerCapabilities, OwnerProduction, OwnerRelation, OwnerRequirements, OwnerSource,
    PackageIdentity, ReactiveRole, Requirement, ResolutionStep, Resource, ResourceCapability,
    ResourceId, ResourceKind, ResourceState, Schedule, StabilityKnowledge, Tracking, Trigger,
    UpperBound, ValueShape, ValueSource,
};

pub const SOLID_RC3_VERSION: &str = "2.0.0-rc.3";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConformanceRow {
    SplitCreateEffect,
    TrackedEffectAndOnSettled,
    BatchingAndFlush,
    ControlFlowCallbacks,
    AsyncComputations,
    LoadingAndRefresh,
    ActionsAndOptimism,
    StoresAndProjections,
    RefsAndDirectives,
    RootEventDelegation,
    BrowserAndServerRendering,
    RequestResponseMutation,
    ServerFunctions,
    ExperimentalServerComponents,
    ConditionalAdapters,
    MixedFrameworkSelection,
}

impl ConformanceRow {
    pub const ALL: [Self; 16] = [
        Self::SplitCreateEffect,
        Self::TrackedEffectAndOnSettled,
        Self::BatchingAndFlush,
        Self::ControlFlowCallbacks,
        Self::AsyncComputations,
        Self::LoadingAndRefresh,
        Self::ActionsAndOptimism,
        Self::StoresAndProjections,
        Self::RefsAndDirectives,
        Self::RootEventDelegation,
        Self::BrowserAndServerRendering,
        Self::RequestResponseMutation,
        Self::ServerFunctions,
        Self::ExperimentalServerComponents,
        Self::ConditionalAdapters,
        Self::MixedFrameworkSelection,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SplitCreateEffect => "split-create-effect",
            Self::TrackedEffectAndOnSettled => "tracked-effect-on-settled",
            Self::BatchingAndFlush => "batching-flush",
            Self::ControlFlowCallbacks => "control-flow-callbacks",
            Self::AsyncComputations => "async-computations",
            Self::LoadingAndRefresh => "loading-refresh",
            Self::ActionsAndOptimism => "actions-optimism",
            Self::StoresAndProjections => "stores-projections",
            Self::RefsAndDirectives => "refs-directives",
            Self::RootEventDelegation => "root-event-delegation",
            Self::BrowserAndServerRendering => "browser-server-rendering",
            Self::RequestResponseMutation => "request-response-mutation",
            Self::ServerFunctions => "server-functions",
            Self::ExperimentalServerComponents => "experimental-server-components",
            Self::ConditionalAdapters => "conditional-adapters",
            Self::MixedFrameworkSelection => "mixed-framework-selection",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceCase {
    pub row: ConformanceRow,
    pub proposal: ContractProposal,
    pub selected_artifact_case: &'static str,
    pub exports: Vec<&'static str>,
}

#[derive(Clone, Copy)]
enum Authority {
    SolidBrowser,
    SignalsDevelopment,
    WebBrowser,
    WebServer,
    ServerFunctionsClient,
    ServerFunctionsServer,
    FramesClient,
    FramesServer,
    AutoAnimateSolid,
}

impl Authority {
    fn package(self) -> PackageIdentity {
        match self {
            Self::SolidBrowser => package(
                "solid-js",
                SOLID_RC3_VERSION,
                "sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==",
                "e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b",
            ),
            Self::SignalsDevelopment => package(
                "@solidjs/signals",
                SOLID_RC3_VERSION,
                "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==",
                "22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76",
            ),
            Self::WebBrowser
            | Self::WebServer
            | Self::ServerFunctionsClient
            | Self::ServerFunctionsServer
            | Self::FramesClient
            | Self::FramesServer => package(
                "@solidjs/web",
                SOLID_RC3_VERSION,
                "sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==",
                "ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e",
            ),
            Self::AutoAnimateSolid => package(
                "@formkit/auto-animate",
                "0.10.0",
                "sha512-KGomRttjUfORuPUaR/ZGQw+6xfMrTM+sxnILv7JAd9AmabU9rg9i6gF/iC0Ih+QpKCubJpCA/1DX9UHKE8cX+A==",
                "7413f50676f5e08e24c84cd1ae6cba0cf163bb5b2021997d6b4d6c905f4e4d85",
            ),
        }
    }

    fn case(self, id: &str) -> ArtifactCase {
        let (
            entrypoint,
            trace,
            runtime_path,
            runtime_digest,
            declarations_path,
            declarations_digest,
            closure,
        ) = match self {
            Self::SolidBrowser => (
                ".",
                vec!["browser", "development", "import"],
                "dist/dev.js",
                "dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18",
                "types/index.d.ts",
                "76b94bfb3a95099405a8cae461fff7b83c5a3cd61667cf72c23e7f850cf52740",
                "626ff782ace96d12380cc222f22c537ada35b87bdc681416ed59aba527e39bbe",
            ),
            Self::SignalsDevelopment => (
                ".",
                vec!["import", "development"],
                "dist/dev.js",
                "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                "dist/types/index.d.ts",
                "e4157c4caba48476db4e7649b5a50827687c2d90c0a09fa091f0e65d0a63cfb4",
                "f14cb312787bee8c09948a5beb8cf35956519c5846e36857daf1c6609ad7462c",
            ),
            Self::WebBrowser => (
                ".",
                vec!["browser", "development", "import"],
                "dist/dev.js",
                "d848d00341ac8195e191404ace7dd8b4c650f47befb0cfecac78ddcf01587851",
                "types/index.d.ts",
                "5870c51be7674969670ccb084077d3df29ed732db8e8ad03527d384285c99635",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::WebServer => (
                ".",
                vec!["node", "import"],
                "dist/server.js",
                "80abb46a98a9d6695b7d2c42725ccfb538f8e941d6aa3a8ec5343d6d002d54b1",
                "types/index.d.ts",
                "5870c51be7674969670ccb084077d3df29ed732db8e8ad03527d384285c99635",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::ServerFunctionsClient => (
                "./server-functions",
                vec!["browser", "import"],
                "server-functions/dist/client.js",
                "f7e754c2119449c94a01b16760efe1cc9fd4bf53f3f17033392a07b4d6bd00a1",
                "types/server-functions/client.d.ts",
                "fdce7ef458dc83b823f883e0af58a4cfa75dd77f1889423d1fa03bd771d17eb6",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::ServerFunctionsServer => (
                "./server-functions",
                vec!["node", "development", "import"],
                "server-functions/dist/server.dev.js",
                "e1fc68d86022e2d26d9e6a24150001c1e69aaeadedf58398f1480304d948040c",
                "types/server-functions/server.d.ts",
                "0459be58eb62d4ba980d06864b11df7b59e8d81fe8c490b81b64412f83b37fbf",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::FramesClient => (
                "./frames",
                vec!["browser", "development", "import"],
                "frames/dist/client.dev.js",
                "fdd6836e1dee13ac4f04d8c78eae7def8c85d997fdc2bd0b5cdc9419754cfe53",
                "types/frames/client.d.ts",
                "e83afca019249516e25679192d747a2de2f0f93684ac38d25b091717516df33b",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::FramesServer => (
                "./frames",
                vec!["node", "import"],
                "frames/dist/server.js",
                "620ff6ce77756a3151f654b06b0acc9492048ef8ca306289278715117b750717",
                "types/frames/server.d.ts",
                "c58fa5f4ec79dfc0707d1f01ddc7b464105710ccb9df60fd7db8a3004fb5a5f6",
                "8ffc3f3c194f3408a9e46f020b7729170a0b42fc6bd4108cb977b88e725873af",
            ),
            Self::AutoAnimateSolid => (
                "./solid",
                vec!["import"],
                "solid/index.mjs",
                "0e0bdd64956a915698f898a16adf6edb1c031cacf1f7608845781ee0a7246d10",
                "solid/index.d.ts",
                "604e208469e4dc03235da3807d5f2e1e0507f163b040b7ac11fadb70d7520920",
                "7011b9171c91c3b6db26f1c5587928cc6310ccb8dc2f02e56ea242bb503a4c7c",
            ),
        };
        ArtifactCase {
            id: id.into(),
            entrypoint: entrypoint.into(),
            resolution_trace: trace
                .into_iter()
                .map(|condition| ResolutionStep {
                    condition: condition.into(),
                    target: runtime_path.into(),
                })
                .collect(),
            runtime: artifact(runtime_path, runtime_digest),
            declarations: artifact(declarations_path, declarations_digest),
            dependency_closure: digest(closure),
            transform: None,
            stability: StabilityKnowledge::Unknown,
            exports: BTreeMap::new(),
        }
    }
}

fn digest(value: &str) -> Digest {
    Digest::parse(format!("sha256:{value}")).expect("published RC.3 digest is valid")
}

fn artifact(path: &str, value: &str) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.into(),
        digest: digest(value),
    }
}

fn package(name: &str, version: &str, integrity: &str, manifest: &str) -> PackageIdentity {
    PackageIdentity {
        name: name.into(),
        version: version.into(),
        integrity: integrity.into(),
        manifest: artifact("package.json", manifest),
    }
}

fn resource(id: &str, kind: ResourceKind, states: Vec<ResourceState>) -> Resource {
    Resource {
        id: ResourceId(id.into()),
        kind,
        states: KnowledgeSet::Complete(states),
        capabilities: KnowledgeSet::Complete(vec![]),
        lifetime: None,
    }
}

fn reactive_resource(id: &str, capabilities: Vec<ResourceCapability>) -> Resource {
    Resource {
        id: ResourceId(id.into()),
        kind: ResourceKind::ReactiveSource,
        states: KnowledgeSet::Complete(vec![]),
        capabilities: KnowledgeSet::Complete(capabilities),
        lifetime: None,
    }
}

fn owner_none() -> OwnerRelation {
    OwnerRelation {
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
        productions: KnowledgeSet::Complete(vec![]),
    }
}

fn owner_ambient(source: OwnerSource) -> OwnerRelation {
    OwnerRelation {
        source,
        requirements: OwnerRequirements::default(),
        capabilities: OwnerCapabilities::default(),
        lifetime: None,
        productions: KnowledgeSet::Complete(vec![]),
    }
}

fn owner_leaf(id: &str, created: bool) -> OwnerRelation {
    let resource = ResourceId(id.into());
    let source = if created {
        OwnerSource::Created(resource.clone())
    } else {
        OwnerSource::Captured(resource.clone())
    };
    OwnerRelation {
        source,
        requirements: OwnerRequirements {
            owner: Requirement::Required,
            child_owners: Requirement::Forbidden,
            cleanup: Requirement::Forbidden,
        },
        capabilities: OwnerCapabilities {
            child_owners: CapabilityKnowledge::Forbidden,
            cleanup: CapabilityKnowledge::Forbidden,
        },
        lifetime: Some(Lifetime::Owner(resource.clone())),
        productions: if created {
            KnowledgeSet::Complete(vec![OwnerProduction {
                resource: resource.clone(),
                capabilities: OwnerCapabilities {
                    child_owners: CapabilityKnowledge::Forbidden,
                    cleanup: CapabilityKnowledge::Forbidden,
                },
                lifetime: Some(Lifetime::Owner(resource)),
            }])
        } else {
            KnowledgeSet::Complete(vec![])
        },
    }
}

fn operation(id: &str, kind: OperationKind, event: Event, min: u32) -> Operation {
    Operation {
        id: OperationId(id.into()),
        kind,
        guard: None,
        trigger: Some(Trigger::Event(event)),
        at: Some(event),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        owner: owner_none(),
        cardinality: Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(min),
            max: Some(UpperBound::Many),
        },
        inputs: vec![],
        output: None,
        resources: BTreeSet::new(),
    }
}

fn ids(operations: &[Operation], kind: OperationKind) -> Vec<OperationId> {
    operations
        .iter()
        .filter(|operation| operation.kind == kind)
        .map(|operation| operation.id.clone())
        .collect()
}

fn knowledge<T>(items: Vec<T>, open: bool) -> KnowledgeSet<T> {
    if open {
        KnowledgeSet::partial(items).unwrap_or(KnowledgeSet::Unknown)
    } else {
        KnowledgeSet::Complete(items)
    }
}

fn callback_source(operation: &Operation) -> ValueSource {
    match operation.id.0.as_str() {
        "queued-apply" => ValueSource::Parameter {
            index: 1,
            path: vec![],
        },
        "queued-error" => ValueSource::Parameter {
            index: 1,
            path: vec!["error".into()],
        },
        "raw-child" | "accessor-child" => ValueSource::Parameter {
            index: 0,
            path: vec!["children".into()],
        },
        "ref-application" => ValueSource::OperationOutput {
            operation: OperationId("ref-factory".into()),
            path: vec![],
        },
        "delegated-event" => ValueSource::Resource {
            resource: ResourceId("render-root".into()),
            path: vec!["listeners".into()],
        },
        "invoke-reference" | "transport" => ValueSource::Resource {
            resource: ResourceId("server-reference".into()),
            path: vec!["callable".into()],
        },
        "yield-resume" => ValueSource::OperationOutput {
            operation: OperationId("invoke-action".into()),
            path: vec![],
        },
        "render-component" => ValueSource::OperationOutput {
            operation: OperationId("resolve-module".into()),
            path: vec![],
        },
        _ => ValueSource::Parameter {
            index: 0,
            path: vec![],
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn semantic_export(
    case: &ArtifactCase,
    name: &str,
    shape: ValueShape,
    operations: Vec<Operation>,
    edges: Vec<OperationEdge>,
    resources: Vec<Resource>,
    guards: KnowledgeSet<GuardedCase>,
    open_domains: &[ClaimDomain],
) -> ExportSemantics {
    let callbacks = operations
        .iter()
        .filter(|operation| operation.kind == OperationKind::Invoke)
        .map(|operation| CallbackInvocation {
            from: callback_source(operation),
            operation: operation.id.clone(),
        })
        .collect::<Vec<_>>();
    let open = |domain| open_domains.contains(&domain);
    let claims = CallClaims {
        callbacks: knowledge(callbacks, open(ClaimDomain::Callbacks)),
        reads: knowledge(
            ids(&operations, OperationKind::Read),
            open(ClaimDomain::Reads),
        ),
        writes: knowledge(
            ids(&operations, OperationKind::Write),
            open(ClaimDomain::Writes),
        ),
        creates: knowledge(
            ids(&operations, OperationKind::Create),
            open(ClaimDomain::Creates),
        ),
        invalidates: knowledge(
            ids(&operations, OperationKind::Invalidate),
            open(ClaimDomain::Invalidates),
        ),
        throws: if open(ClaimDomain::Throws) {
            KnowledgeSet::Unknown
        } else {
            KnowledgeSet::Complete(vec![])
        },
        returns: knowledge(
            ids(&operations, OperationKind::Return),
            open(ClaimDomain::Returns),
        ),
        cleanups: knowledge(
            ids(&operations, OperationKind::Cleanup),
            open(ClaimDomain::Cleanups),
        ),
        disposals: knowledge(
            ids(&operations, OperationKind::Dispose),
            open(ClaimDomain::Disposals),
        ),
    };
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
        stability: case.stability,
        call: CallSemantics::new(
            claims,
            operations,
            edges,
            resources,
            GuardPartition { cases: guards },
        ),
    }
}

fn edge(kind: EdgeKind, from: &str, to: &str) -> OperationEdge {
    OperationEdge {
        kind,
        from: OperationId(from.into()),
        to: OperationId(to.into()),
    }
}

fn insert(case: &mut ArtifactCase, export: ExportSemantics) {
    case.exports
        .insert(export.identity.public_name.clone(), export);
}

fn split_effect() -> ConformanceCase {
    let authority = Authority::SolidBrowser;
    let mut case = authority.case("solid-browser-development");
    let owner = resource(
        "effect-owner",
        ResourceKind::Owner,
        vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
    );
    let cleanup = resource(
        "effect-cleanup",
        ResourceKind::Cleanup,
        vec![
            ResourceState::CleanupInstalled,
            ResourceState::CleanupDisposed,
        ],
    );
    let mut initial = operation("initial-compute", OperationKind::Invoke, Event::Call, 1);
    initial.tracking = Tracking::Tracked;
    initial.owner = owner_ambient(OwnerSource::AmbientAtCall);
    let mut repeated = operation(
        "repeated-compute",
        OperationKind::Invoke,
        Event::External,
        0,
    );
    repeated.tracking = Tracking::Tracked;
    repeated.owner = owner_ambient(OwnerSource::Captured(ResourceId("effect-owner".into())));
    let mut apply = operation("queued-apply", OperationKind::Invoke, Event::Flush, 0);
    apply.trigger = Some(Trigger::Operation(OperationId("initial-compute".into())));
    apply.schedule = Some(Schedule::Queued);
    apply.owner = owner_ambient(OwnerSource::AmbientAtExecution);
    let mut error = operation("queued-error", OperationKind::Invoke, Event::Flush, 0);
    error.trigger = Some(Trigger::Operation(OperationId("initial-compute".into())));
    error.schedule = Some(Schedule::Queued);
    error.owner = owner_ambient(OwnerSource::AmbientAtExecution);
    error.guard = Some(Guard(vec![GuardAtom::Property {
        argument: 1,
        path: vec![],
        name: "error".into(),
        callable: Some(true),
    }]));
    let mut cleanup_op = operation("replace-cleanup", OperationKind::Cleanup, Event::Cleanup, 0);
    cleanup_op.trigger = Some(Trigger::Operation(OperationId("queued-apply".into())));
    cleanup_op
        .resources
        .insert(ResourceId("effect-cleanup".into()));
    let mut dispose = operation("dispose-effect", OperationKind::Dispose, Event::Cleanup, 0);
    dispose.resources.insert(ResourceId("effect-owner".into()));
    let operations = vec![initial, repeated, apply, error, cleanup_op, dispose];
    let export = semantic_export(
        &case,
        "createEffect",
        ValueShape::Plain,
        operations,
        vec![
            edge(EdgeKind::Data, "initial-compute", "queued-apply"),
            edge(EdgeKind::Data, "repeated-compute", "queued-apply"),
            edge(EdgeKind::Error, "initial-compute", "queued-error"),
            edge(EdgeKind::Error, "repeated-compute", "queued-error"),
            edge(EdgeKind::Cleanup, "queued-apply", "replace-cleanup"),
            edge(EdgeKind::Lifetime, "replace-cleanup", "dispose-effect"),
        ],
        vec![owner, cleanup],
        KnowledgeSet::Complete(vec![
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 2,
                    path: vec!["defer".into()],
                    value: Literal::Bool(true),
                }]),
                operations: KnowledgeSet::Complete(vec![OperationId("initial-compute".into())]),
            },
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 2,
                    path: vec!["defer".into()],
                    value: Literal::Bool(false),
                }]),
                operations: KnowledgeSet::Complete(vec![
                    OperationId("initial-compute".into()),
                    OperationId("queued-apply".into()),
                    OperationId("queued-error".into()),
                ]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![
                    OperationId("initial-compute".into()),
                    OperationId("queued-apply".into()),
                    OperationId("queued-error".into()),
                ]),
            },
        ]),
        &[ClaimDomain::Throws],
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::SplitCreateEffect,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "solid-browser-development",
        exports: vec!["createEffect"],
    }
}

fn tracked_effect() -> ConformanceCase {
    let authority = Authority::SignalsDevelopment;
    let mut case = authority.case("signals-development");
    for (name, owned) in [("createTrackedEffect", true), ("onSettled", false)] {
        let owner_id = format!("{name}-leaf-owner");
        let owner = resource(
            &owner_id,
            ResourceKind::Owner,
            vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
        );
        let event = if name == "onSettled" {
            Event::Settle
        } else {
            Event::Call
        };
        let mut callback = operation("callback", OperationKind::Invoke, event, 0);
        callback.tracking = if name == "onSettled" {
            Tracking::Untracked
        } else {
            Tracking::Tracked
        };
        callback.output = if owned {
            Some(ValueShape::Cleanup {
                resource: None,
                lifetime: Some(Lifetime::Owner(ResourceId(owner_id.clone()))),
            })
        } else {
            Some(ValueShape::Unknown)
        };
        callback.owner = if owned {
            owner_leaf(&owner_id, true)
        } else {
            owner_ambient(OwnerSource::AmbientAtExecution)
        };
        let mut cleanup = operation(
            "returned-cleanup",
            OperationKind::Cleanup,
            Event::Cleanup,
            0,
        );
        cleanup.trigger = Some(Trigger::Operation(OperationId("callback".into())));
        let export = semantic_export(
            &case,
            name,
            ValueShape::Plain,
            vec![callback, cleanup],
            vec![edge(EdgeKind::Cleanup, "callback", "returned-cleanup")],
            vec![owner],
            KnowledgeSet::Complete(vec![]),
            if owned { &[] } else { &[ClaimDomain::Cleanups] },
        );
        insert(&mut case, export);
    }
    ConformanceCase {
        row: ConformanceRow::TrackedEffectAndOnSettled,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "signals-development",
        exports: vec!["createTrackedEffect", "onSettled"],
    }
}

fn batching() -> ConformanceCase {
    let authority = Authority::SignalsDevelopment;
    let mut case = authority.case("signals-development");
    let mut write = operation("stage-write", OperationKind::Write, Event::Call, 1);
    write.schedule = Some(Schedule::Queued);
    let mut invalidate = operation(
        "commit-invalidation",
        OperationKind::Invalidate,
        Event::Flush,
        0,
    );
    invalidate.trigger = Some(Trigger::Operation(OperationId("stage-write".into())));
    let mut callback = operation("flush-callback", OperationKind::Invoke, Event::Call, 0);
    callback.guard = Some(Guard(vec![GuardAtom::ArgumentCount {
        min: 1,
        max: Some(1),
    }]));
    let mut drain = operation("flush-drain", OperationKind::Invalidate, Event::Flush, 1);
    drain.trigger = Some(Trigger::Operation(OperationId("flush-callback".into())));
    let export = semantic_export(
        &case,
        "flush",
        ValueShape::Callable,
        vec![write, invalidate, callback, drain],
        vec![
            edge(EdgeKind::Invalidates, "stage-write", "commit-invalidation"),
            edge(EdgeKind::Orders, "flush-callback", "flush-drain"),
        ],
        vec![],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Throws],
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::BatchingAndFlush,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "signals-development",
        exports: vec!["flush"],
    }
}

fn control_flow_export(case: &ArtifactCase, name: &str) -> ExportSemantics {
    let row_owner = resource(
        "row-owner",
        ResourceKind::Owner,
        vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
    );
    let mut raw = operation("raw-child", OperationKind::Invoke, Event::Render, 0);
    raw.guard = Some(Guard(vec![GuardAtom::Literal {
        argument: 0,
        path: vec!["keyed".into()],
        value: Literal::String("value".into()),
    }]));
    raw.inputs = vec![ValueShape::Plain, ValueShape::Plain];
    raw.owner = owner_leaf("row-owner", true);
    let mut accessor = operation("accessor-child", OperationKind::Invoke, Event::Render, 0);
    accessor.guard = Some(Guard(vec![GuardAtom::Literal {
        argument: 0,
        path: vec!["keyed".into()],
        value: Literal::String("index".into()),
    }]));
    accessor.inputs = vec![ValueShape::Reactive {
        role: ReactiveRole::Accessor,
        resource: None,
        capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
            capability: ObservableCapability::Readable,
            resource: None,
        }]),
    }];
    accessor.owner = owner_leaf("row-owner", true);
    semantic_export(
        case,
        name,
        ValueShape::Component,
        vec![raw, accessor],
        vec![],
        vec![row_owner],
        KnowledgeSet::Complete(vec![
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 0,
                    path: vec!["keyed".into()],
                    value: Literal::String("value".into()),
                }]),
                operations: KnowledgeSet::Complete(vec![OperationId("raw-child".into())]),
            },
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 0,
                    path: vec!["keyed".into()],
                    value: Literal::String("index".into()),
                }]),
                operations: KnowledgeSet::Complete(vec![OperationId("accessor-child".into())]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![
                    OperationId("raw-child".into()),
                    OperationId("accessor-child".into()),
                ]),
            },
        ]),
        &[],
    )
}

fn control_flow() -> ConformanceCase {
    let authority = Authority::SolidBrowser;
    let mut case = authority.case("solid-browser-development");
    for name in ["For", "Repeat", "Show", "Match"] {
        let export = control_flow_export(&case, name);
        insert(&mut case, export);
    }
    ConformanceCase {
        row: ConformanceRow::ControlFlowCallbacks,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "solid-browser-development",
        exports: vec!["For", "Repeat", "Show", "Match"],
    }
}

fn async_computations() -> ConformanceCase {
    let authority = Authority::SignalsDevelopment;
    let mut case = authority.case("signals-development");
    let async_resource = resource(
        "async-computation",
        ResourceKind::AsyncComputation,
        vec![
            ResourceState::AsyncPending,
            ResourceState::AsyncSettled,
            ResourceState::AsyncErrored,
            ResourceState::AsyncCancelled,
        ],
    );
    let mut compute = operation("compute", OperationKind::Invoke, Event::Call, 1);
    compute.tracking = Tracking::Tracked;
    compute.output = Some(ValueShape::Choice(KnowledgeSet::Complete(vec![
        ValueShape::Plain,
        ValueShape::Promise(Box::new(ValueShape::Plain)),
        ValueShape::AsyncIterable(Box::new(ValueShape::Plain)),
    ])));
    compute
        .resources
        .insert(ResourceId("async-computation".into()));
    let mut emission = operation("emission", OperationKind::Return, Event::AsyncEmission, 0);
    emission.trigger = Some(Trigger::Resource {
        resource: ResourceId("async-computation".into()),
        event: Event::AsyncEmission,
    });
    emission.output = Some(ValueShape::Plain);
    let mut cancel = operation("cancel", OperationKind::Dispose, Event::Cleanup, 0);
    cancel
        .resources
        .insert(ResourceId("async-computation".into()));
    let export = semantic_export(
        &case,
        "createMemo",
        ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: Some(ResourceId("async-computation".into())),
            capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                capability: ObservableCapability::Readable,
                resource: None,
            }]),
        },
        vec![compute, emission, cancel],
        vec![
            edge(EdgeKind::Data, "compute", "emission"),
            edge(EdgeKind::Lifetime, "emission", "cancel"),
        ],
        vec![async_resource],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Throws],
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::AsyncComputations,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "signals-development",
        exports: vec!["createMemo"],
    }
}

fn loading_refresh() -> ConformanceCase {
    let authority = Authority::SolidBrowser;
    let mut case = authority.case("solid-browser-development");
    let mut loading_target = resource(
        "async-target",
        ResourceKind::AsyncComputation,
        vec![
            ResourceState::AsyncPending,
            ResourceState::AsyncSettled,
            ResourceState::AsyncErrored,
            ResourceState::AsyncCancelled,
        ],
    );
    loading_target.capabilities = KnowledgeSet::Partial(vec![ResourceCapability::Refreshable]);
    let loading_boundary = resource(
        "loading-boundary",
        ResourceKind::Owner,
        vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
    );
    let mut loading_read = operation("pending-read", OperationKind::Read, Event::Render, 0);
    loading_read.resources.extend([
        ResourceId("async-target".into()),
        ResourceId("loading-boundary".into()),
    ]);
    let loading = semantic_export(
        &case,
        "Loading",
        ValueShape::Component,
        vec![loading_read],
        vec![],
        vec![loading_target, loading_boundary],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Reads],
    );
    insert(&mut case, loading);
    for name in ["isPending", "latest", "refresh", "affects"] {
        let mut target = reactive_resource(
            "async-target",
            if name == "refresh" {
                vec![ResourceCapability::Refreshable]
            } else {
                vec![]
            },
        );
        if name == "isPending" {
            target.states = KnowledgeSet::Unknown;
        }
        let kind = if matches!(name, "refresh" | "affects") {
            OperationKind::Invalidate
        } else {
            OperationKind::Read
        };
        let operation_id = match name {
            "isPending" => "pending-read",
            "latest" => "latest-read",
            "refresh" => "refresh-target",
            "affects" => "affects-key",
            _ => unreachable!(),
        };
        let mut op = operation(operation_id, kind, Event::Call, 1);
        op.resources.insert(ResourceId("async-target".into()));
        op.inputs = vec![ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: Some(ResourceId("async-target".into())),
            capabilities: if name == "refresh" {
                KnowledgeSet::Complete(vec![
                    CapabilityClaim {
                        capability: ObservableCapability::Readable,
                        resource: None,
                    },
                    CapabilityClaim {
                        capability: ObservableCapability::Refreshable,
                        resource: Some(ResourceId("async-target".into())),
                    },
                ])
            } else {
                KnowledgeSet::Unknown
            },
        }];
        let export = semantic_export(
            &case,
            name,
            ValueShape::Plain,
            vec![op],
            vec![],
            vec![target],
            KnowledgeSet::Complete(vec![]),
            if name == "isPending" {
                &[ClaimDomain::Reads]
            } else {
                &[]
            },
        );
        insert(&mut case, export);
    }
    ConformanceCase {
        row: ConformanceRow::LoadingAndRefresh,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "solid-browser-development",
        exports: vec!["Loading", "isPending", "latest", "refresh", "affects"],
    }
}

fn actions_optimism() -> ConformanceCase {
    let authority = Authority::SignalsDevelopment;
    let mut case = authority.case("signals-development");
    let transition = resource(
        "action-transition",
        ResourceKind::Transition,
        vec![
            ResourceState::TransitionActive,
            ResourceState::TransitionSettled,
            ResourceState::TransitionReverted,
        ],
    );
    let mut invoke = operation("invoke-action", OperationKind::Invoke, Event::Call, 1);
    invoke.output = Some(ValueShape::Promise(Box::new(ValueShape::Plain)));
    invoke
        .resources
        .insert(ResourceId("action-transition".into()));
    let mut resume = operation("yield-resume", OperationKind::Invoke, Event::Settle, 0);
    resume.trigger = Some(Trigger::Operation(OperationId("invoke-action".into())));
    resume.schedule = Some(Schedule::Queued);
    resume
        .resources
        .insert(ResourceId("action-transition".into()));
    let mut optimistic = operation(
        "optimistic-write",
        OperationKind::Write,
        Event::Transition,
        0,
    );
    optimistic.trigger = Some(Trigger::Resource {
        resource: ResourceId("action-transition".into()),
        event: Event::Transition,
    });
    optimistic
        .resources
        .insert(ResourceId("action-transition".into()));
    let mut settle = operation(
        "settle-or-revert",
        OperationKind::Invalidate,
        Event::Settle,
        0,
    );
    settle
        .resources
        .insert(ResourceId("action-transition".into()));
    let export = semantic_export(
        &case,
        "action",
        ValueShape::Action {
            transition: Some(ResourceId("action-transition".into())),
        },
        vec![invoke, optimistic, resume, settle],
        vec![
            edge(EdgeKind::Orders, "invoke-action", "optimistic-write"),
            edge(EdgeKind::Orders, "invoke-action", "yield-resume"),
            edge(
                EdgeKind::Invalidates,
                "optimistic-write",
                "settle-or-revert",
            ),
            edge(EdgeKind::Error, "yield-resume", "settle-or-revert"),
        ],
        vec![transition],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Throws],
    );
    insert(&mut case, export);

    for (name, shape) in [
        (
            "createOptimistic",
            ValueShape::Tuple(KnowledgeSet::Complete(vec![
                ValueShape::Reactive {
                    role: ReactiveRole::Accessor,
                    resource: Some(ResourceId("optimistic-source".into())),
                    capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                        capability: ObservableCapability::Readable,
                        resource: None,
                    }]),
                },
                ValueShape::Reactive {
                    role: ReactiveRole::Setter,
                    resource: Some(ResourceId("optimistic-source".into())),
                    capabilities: KnowledgeSet::Complete(vec![
                        CapabilityClaim {
                            capability: ObservableCapability::Writable,
                            resource: None,
                        },
                        CapabilityClaim {
                            capability: ObservableCapability::Optimistic,
                            resource: Some(ResourceId("action-transition".into())),
                        },
                    ]),
                },
            ])),
        ),
        (
            "createOptimisticStore",
            ValueShape::Tuple(KnowledgeSet::Complete(vec![
                ValueShape::Store {
                    resource: Some(ResourceId("optimistic-source".into())),
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
                            resource: Some(ResourceId("action-transition".into())),
                        },
                    ]),
                },
                ValueShape::Reactive {
                    role: ReactiveRole::Setter,
                    resource: Some(ResourceId("optimistic-source".into())),
                    capabilities: KnowledgeSet::Complete(vec![
                        CapabilityClaim {
                            capability: ObservableCapability::Writable,
                            resource: None,
                        },
                        CapabilityClaim {
                            capability: ObservableCapability::Optimistic,
                            resource: Some(ResourceId("action-transition".into())),
                        },
                    ]),
                },
            ])),
        ),
    ] {
        let mut optimistic_resource = resource(
            "action-transition",
            ResourceKind::Transition,
            vec![
                ResourceState::TransitionActive,
                ResourceState::TransitionSettled,
                ResourceState::TransitionReverted,
            ],
        );
        optimistic_resource.capabilities =
            KnowledgeSet::Complete(vec![ResourceCapability::Writable]);
        let optimistic_source =
            reactive_resource("optimistic-source", vec![ResourceCapability::Writable]);
        let mut write = operation(
            "optimistic-write",
            OperationKind::Write,
            Event::Transition,
            0,
        );
        write.trigger = Some(Trigger::Resource {
            resource: ResourceId("action-transition".into()),
            event: Event::Transition,
        });
        write.resources.extend([
            ResourceId("action-transition".into()),
            ResourceId("optimistic-source".into()),
        ]);
        let optimistic_export = semantic_export(
            &case,
            name,
            shape,
            vec![write],
            vec![],
            vec![optimistic_resource, optimistic_source],
            KnowledgeSet::Complete(vec![]),
            &[ClaimDomain::Throws],
        );
        insert(&mut case, optimistic_export);
    }
    ConformanceCase {
        row: ConformanceRow::ActionsAndOptimism,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "signals-development",
        exports: vec!["action", "createOptimistic", "createOptimisticStore"],
    }
}

fn stores_projections() -> ConformanceCase {
    let authority = Authority::SignalsDevelopment;
    let mut case = authority.case("signals-development");
    let store = reactive_resource("store", vec![ResourceCapability::Writable]);
    let mut read = operation("read-store", OperationKind::Read, Event::Call, 0);
    read.resources.insert(ResourceId("store".into()));
    let mut write = operation("write-draft", OperationKind::Write, Event::Call, 0);
    write.resources.insert(ResourceId("store".into()));
    let mut shallow = operation("shallow-read", OperationKind::Read, Event::External, 0);
    shallow.resources.insert(ResourceId("store".into()));
    let mut deep = operation("deep-read", OperationKind::Read, Event::External, 0);
    deep.resources.insert(ResourceId("store".into()));
    let create_store = semantic_export(
        &case,
        "createStore",
        ValueShape::Tuple(KnowledgeSet::Complete(vec![
            ValueShape::Store {
                resource: Some(ResourceId("store".into())),
                capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                    capability: ObservableCapability::Readable,
                    resource: None,
                }]),
            },
            ValueShape::Reactive {
                role: ReactiveRole::Setter,
                resource: Some(ResourceId("store".into())),
                capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                    capability: ObservableCapability::Writable,
                    resource: None,
                }]),
            },
        ])),
        vec![read, write, shallow, deep],
        vec![],
        vec![store],
        KnowledgeSet::Complete(vec![
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 1,
                    path: vec!["shallow".into()],
                    value: Literal::Bool(true),
                }]),
                operations: KnowledgeSet::Complete(vec![
                    OperationId("read-store".into()),
                    OperationId("write-draft".into()),
                    OperationId("shallow-read".into()),
                ]),
            },
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 1,
                    path: vec!["shallow".into()],
                    value: Literal::Bool(false),
                }]),
                operations: KnowledgeSet::Complete(vec![
                    OperationId("read-store".into()),
                    OperationId("write-draft".into()),
                    OperationId("deep-read".into()),
                ]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![
                    OperationId("read-store".into()),
                    OperationId("write-draft".into()),
                    OperationId("deep-read".into()),
                ]),
            },
        ]),
        &[],
    );
    insert(&mut case, create_store);

    let projection = reactive_resource("projection", vec![ResourceCapability::Refreshable]);
    let mut compute = operation("projection-compute", OperationKind::Invoke, Event::Call, 1);
    compute.tracking = Tracking::Tracked;
    compute.resources.insert(ResourceId("projection".into()));
    let mut projection_write = operation("write-draft", OperationKind::Write, Event::Call, 0);
    projection_write
        .resources
        .insert(ResourceId("projection".into()));
    let mut emission = operation(
        "projection-emission",
        OperationKind::Return,
        Event::AsyncEmission,
        0,
    );
    emission.trigger = Some(Trigger::Resource {
        resource: ResourceId("projection".into()),
        event: Event::AsyncEmission,
    });
    emission.output = Some(ValueShape::Plain);
    emission.resources.insert(ResourceId("projection".into()));
    let mut projection_read = operation("read-store", OperationKind::Read, Event::External, 0);
    projection_read
        .resources
        .insert(ResourceId("projection".into()));
    let create_projection = semantic_export(
        &case,
        "createProjection",
        ValueShape::Store {
            resource: Some(ResourceId("projection".into())),
            capabilities: KnowledgeSet::Complete(vec![
                CapabilityClaim {
                    capability: ObservableCapability::Readable,
                    resource: None,
                },
                CapabilityClaim {
                    capability: ObservableCapability::Refreshable,
                    resource: Some(ResourceId("projection".into())),
                },
            ]),
        },
        vec![compute, projection_write, emission, projection_read],
        vec![
            edge(EdgeKind::Data, "projection-compute", "write-draft"),
            edge(EdgeKind::Data, "write-draft", "projection-emission"),
            edge(EdgeKind::Invalidates, "projection-emission", "read-store"),
        ],
        vec![projection],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Throws, ClaimDomain::Returns],
    );
    insert(&mut case, create_projection);

    let snapshot_store = reactive_resource("store", vec![]);
    let mut snapshot_read = operation("snapshot-read", OperationKind::Read, Event::Call, 1);
    snapshot_read.resources.insert(ResourceId("store".into()));
    let snapshot_export = semantic_export(
        &case,
        "snapshot",
        ValueShape::Plain,
        vec![snapshot_read],
        vec![],
        vec![snapshot_store],
        KnowledgeSet::Complete(vec![]),
        &[],
    );
    insert(&mut case, snapshot_export);

    let reconcile_store = reactive_resource("store", vec![ResourceCapability::Writable]);
    let mut reconcile_write =
        operation("reconcile-write", OperationKind::Write, Event::External, 0);
    reconcile_write.schedule = Some(Schedule::External);
    reconcile_write.resources.insert(ResourceId("store".into()));
    let reconcile_export = semantic_export(
        &case,
        "reconcile",
        ValueShape::Callable,
        vec![reconcile_write],
        vec![],
        vec![reconcile_store],
        KnowledgeSet::Complete(vec![]),
        &[],
    );
    insert(&mut case, reconcile_export);
    ConformanceCase {
        row: ConformanceRow::StoresAndProjections,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "signals-development",
        exports: vec!["createStore", "createProjection", "snapshot", "reconcile"],
    }
}

fn refs_directives() -> ConformanceCase {
    let authority = Authority::WebBrowser;
    let mut case = authority.case("web-browser-development");
    let setup_owner = resource(
        "setup-owner",
        ResourceKind::Owner,
        vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
    );
    let mut factory = operation("ref-factory", OperationKind::Invoke, Event::Render, 1);
    factory.tracking = Tracking::Untracked;
    factory.owner = owner_ambient(OwnerSource::Captured(ResourceId("setup-owner".into())));
    factory.output = Some(ValueShape::Array {
        element: Box::new(ValueShape::RefApplication),
        length: ArrayLength {
            min: Some(1),
            max: Some(UpperBound::Many),
        },
    });
    let mut apply = operation("ref-application", OperationKind::Invoke, Event::Render, 0);
    apply.trigger = Some(Trigger::Operation(OperationId("ref-factory".into())));
    apply.owner = owner_none();
    let mut cleanup = operation("ref-cleanup", OperationKind::Cleanup, Event::Cleanup, 0);
    cleanup.trigger = Some(Trigger::Operation(OperationId("ref-application".into())));
    let export = semantic_export(
        &case,
        "applyRef",
        ValueShape::Plain,
        vec![factory, apply, cleanup],
        vec![
            edge(EdgeKind::Data, "ref-factory", "ref-application"),
            edge(EdgeKind::Cleanup, "ref-application", "ref-cleanup"),
        ],
        vec![setup_owner],
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Cleanups],
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::RefsAndDirectives,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "web-browser-development",
        exports: vec!["applyRef"],
    }
}

fn event_delegation() -> ConformanceCase {
    let authority = Authority::WebBrowser;
    let mut case = authority.case("web-browser-development");
    let root = resource(
        "render-root",
        ResourceKind::Owner,
        vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
    );
    let mut register = operation(
        "register-delegation",
        OperationKind::Create,
        Event::Render,
        1,
    );
    register.owner = owner_leaf("render-root", true);
    register.resources.insert(ResourceId("render-root".into()));
    let mut event = operation("delegated-event", OperationKind::Invoke, Event::External, 0);
    event.schedule = Some(Schedule::External);
    event.owner = owner_ambient(OwnerSource::Captured(ResourceId("render-root".into())));
    let mut dispose = operation("unregister-root", OperationKind::Dispose, Event::Cleanup, 0);
    dispose.resources.insert(ResourceId("render-root".into()));
    let export = semantic_export(
        &case,
        "render",
        ValueShape::Cleanup {
            resource: None,
            lifetime: Some(Lifetime::Owner(ResourceId("render-root".into()))),
        },
        vec![register, event, dispose],
        vec![
            edge(EdgeKind::Lifetime, "register-delegation", "delegated-event"),
            edge(EdgeKind::Lifetime, "delegated-event", "unregister-root"),
        ],
        vec![root],
        KnowledgeSet::Complete(vec![]),
        &[],
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::RootEventDelegation,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "web-browser-development",
        exports: vec!["render"],
    }
}

fn rendering() -> ConformanceCase {
    let browser_authority = Authority::WebBrowser;
    let server_authority = Authority::WebServer;
    let mut browser = browser_authority.case("web-browser-development");
    for name in ["render", "hydrate"] {
        let root = resource(
            "browser-root",
            ResourceKind::Owner,
            vec![ResourceState::OwnerActive, ResourceState::OwnerDisposed],
        );
        let operation_id = if name == "hydrate" {
            "hydrate-callback"
        } else {
            "render-callback"
        };
        let mut callback = operation(operation_id, OperationKind::Invoke, Event::Render, 1);
        callback.owner = owner_leaf("browser-root", true);
        let export = semantic_export(
            &browser,
            name,
            ValueShape::Cleanup {
                resource: None,
                lifetime: Some(Lifetime::Owner(ResourceId("browser-root".into()))),
            },
            vec![callback],
            vec![],
            vec![root],
            KnowledgeSet::Complete(vec![]),
            if name == "hydrate" {
                &[ClaimDomain::Callbacks]
            } else {
                &[]
            },
        );
        insert(&mut browser, export);
    }
    let mut server = server_authority.case("web-node-server");
    for name in ["renderToString", "renderToStream"] {
        let mut callback = operation("ssr-callback", OperationKind::Invoke, Event::Render, 1);
        callback.owner = owner_ambient(OwnerSource::AmbientAtExecution);
        let mut operations = vec![callback];
        let mut resources = vec![];
        let shape = if name == "renderToStream" {
            resources.push(resource(
                "ssr-stream",
                ResourceKind::Stream,
                vec![ResourceState::StreamUnclaimed, ResourceState::StreamClaimed],
            ));
            let mut consume = operation("claim-stream", OperationKind::Read, Event::External, 0);
            consume.schedule = Some(Schedule::External);
            consume.cardinality.max = Some(UpperBound::Finite(1));
            consume.resources.insert(ResourceId("ssr-stream".into()));
            operations.push(consume);
            ValueShape::Object(KnowledgeSet::Unknown)
        } else {
            ValueShape::Plain
        };
        let export = semantic_export(
            &server,
            name,
            shape,
            operations,
            if name == "renderToStream" {
                vec![edge(EdgeKind::Orders, "ssr-callback", "claim-stream")]
            } else {
                vec![]
            },
            resources,
            KnowledgeSet::Complete(vec![]),
            &[ClaimDomain::Throws],
        );
        insert(&mut server, export);
    }
    ConformanceCase {
        row: ConformanceRow::BrowserAndServerRendering,
        proposal: ContractProposal::new(browser_authority.package(), vec![browser, server]),
        selected_artifact_case: "web-browser-development",
        exports: vec!["render", "hydrate", "renderToString", "renderToStream"],
    }
}

fn request_response() -> ConformanceCase {
    let server_authority = Authority::WebServer;
    let browser_authority = Authority::WebBrowser;
    let mut server = server_authority.case("web-node-server");
    for name in ["httpStatus", "httpHeader"] {
        let request = resource("request", ResourceKind::Request, vec![]);
        let response = resource(
            "response",
            ResourceKind::Response,
            vec![
                ResourceState::ResponseUncommitted,
                ResourceState::ResponseCommitted,
            ],
        );
        let mut declare = operation("declare-response", OperationKind::Write, Event::Request, 0);
        declare.trigger = Some(Trigger::Resource {
            resource: ResourceId("request".into()),
            event: Event::Request,
        });
        declare.owner = owner_ambient(OwnerSource::AmbientAtExecution);
        declare
            .resources
            .extend([ResourceId("request".into()), ResourceId("response".into())]);
        let mut retract = operation(
            "retract-declaration",
            OperationKind::Cleanup,
            Event::Cleanup,
            0,
        );
        retract.trigger = Some(Trigger::Operation(OperationId("declare-response".into())));
        retract.resources.insert(ResourceId("response".into()));
        let export = semantic_export(
            &server,
            name,
            ValueShape::Plain,
            vec![declare, retract],
            vec![edge(
                EdgeKind::Cleanup,
                "declare-response",
                "retract-declaration",
            )],
            vec![request, response],
            KnowledgeSet::Complete(vec![]),
            &[ClaimDomain::Writes, ClaimDomain::Cleanups],
        );
        insert(&mut server, export);
    }
    let mut browser = browser_authority.case("web-browser-development");
    for name in ["httpStatus", "httpHeader"] {
        let no_op = semantic_export(
            &browser,
            name,
            ValueShape::Plain,
            vec![],
            vec![],
            vec![],
            KnowledgeSet::Complete(vec![]),
            &[],
        );
        insert(&mut browser, no_op);
    }
    ConformanceCase {
        row: ConformanceRow::RequestResponseMutation,
        proposal: ContractProposal::new(server_authority.package(), vec![server, browser]),
        selected_artifact_case: "web-node-server",
        exports: vec!["httpStatus", "httpHeader"],
    }
}

fn server_function_export(case: &ArtifactCase, client: bool) -> ExportSemantics {
    let reference = resource(
        "server-reference",
        ResourceKind::ServerFunctionReference,
        vec![],
    );
    let mut transform = operation("transform-reference", OperationKind::Create, Event::Call, 0);
    transform.guard = Some(Guard(vec![GuardAtom::Signature(
        "compiler:server-function-reference".into(),
    )]));
    transform
        .resources
        .insert(ResourceId("server-reference".into()));
    let mut invoke = operation("invoke-reference", OperationKind::Invoke, Event::Call, 0);
    invoke.output = Some(if client {
        ValueShape::Promise(Box::new(ValueShape::Unknown))
    } else {
        ValueShape::Choice(KnowledgeSet::Partial(vec![
            ValueShape::Plain,
            ValueShape::Promise(Box::new(ValueShape::Unknown)),
        ]))
    });
    invoke
        .resources
        .insert(ResourceId("server-reference".into()));
    let mut operations = vec![transform];
    let mut edges = vec![edge(
        EdgeKind::Data,
        "transform-reference",
        if client {
            "invoke-reference"
        } else {
            "register-reference"
        },
    )];
    let mut resources = vec![reference];
    if client {
        let mut request = resource("request", ResourceKind::Request, vec![]);
        request.lifetime = Some(Lifetime::Call);
        let mut stream = resource(
            "stream",
            ResourceKind::Stream,
            vec![ResourceState::StreamUnclaimed, ResourceState::StreamClaimed],
        );
        stream.lifetime = Some(Lifetime::Resource(ResourceId("request".into())));
        let mut transport = operation("transport", OperationKind::Invoke, Event::External, 0);
        transport.trigger = Some(Trigger::Operation(OperationId("invoke-reference".into())));
        transport.schedule = Some(Schedule::External);
        transport.output = Some(ValueShape::Promise(Box::new(ValueShape::Unknown)));
        transport.resources.extend([
            ResourceId("server-reference".into()),
            ResourceId("request".into()),
            ResourceId("stream".into()),
        ]);
        let mut cancel = operation("cancel-stream", OperationKind::Dispose, Event::Cleanup, 0);
        cancel.trigger = Some(Trigger::Resource {
            resource: ResourceId("stream".into()),
            event: Event::Cleanup,
        });
        cancel.resources.insert(ResourceId("stream".into()));
        operations.extend([invoke, transport, cancel]);
        edges.extend([
            edge(EdgeKind::Data, "invoke-reference", "transport"),
            edge(EdgeKind::Error, "transport", "cancel-stream"),
            edge(EdgeKind::Lifetime, "transport", "cancel-stream"),
        ]);
        resources.extend([request, stream]);
    } else {
        let mut register = operation("register-reference", OperationKind::Create, Event::Call, 0);
        register
            .resources
            .insert(ResourceId("server-reference".into()));
        operations.extend([register, invoke]);
        edges.push(edge(
            EdgeKind::Data,
            "register-reference",
            "invoke-reference",
        ));
    }
    semantic_export(
        case,
        "createServerReference",
        ValueShape::ServerFunctionReference {
            resource: Some(ResourceId("server-reference".into())),
        },
        operations,
        edges,
        resources,
        KnowledgeSet::Complete(vec![]),
        &[ClaimDomain::Throws, ClaimDomain::Returns],
    )
}

fn server_functions() -> ConformanceCase {
    let client_authority = Authority::ServerFunctionsClient;
    let server_authority = Authority::ServerFunctionsServer;
    let mut client = client_authority.case("server-functions-browser-client");
    let mut server = server_authority.case("server-functions-node-server");
    let client_export = server_function_export(&client, true);
    let server_export = server_function_export(&server, false);
    insert(&mut client, client_export);
    insert(&mut server, server_export);
    ConformanceCase {
        row: ConformanceRow::ServerFunctions,
        proposal: ContractProposal::new(client_authority.package(), vec![client, server]),
        selected_artifact_case: "server-functions-browser-client",
        exports: vec!["createServerReference"],
    }
}

fn server_components() -> ConformanceCase {
    let client_authority = Authority::FramesClient;
    let server_authority = Authority::FramesServer;
    let mut client = client_authority.case("frames-browser-client");
    client.stability = StabilityKnowledge::Experimental;
    let mut server = server_authority.case("frames-node-server");
    server.stability = StabilityKnowledge::Experimental;
    for (case, name) in [
        (&mut client, "installServerComponents"),
        (&mut server, "renderServerComponent"),
    ] {
        let mut export = semantic_export(
            case,
            name,
            ValueShape::Component,
            vec![],
            vec![],
            vec![],
            KnowledgeSet::Unknown,
            &ClaimDomain::ALL,
        );
        export.stability = StabilityKnowledge::Experimental;
        insert(case, export);
    }
    ConformanceCase {
        row: ConformanceRow::ExperimentalServerComponents,
        proposal: ContractProposal::new(client_authority.package(), vec![client, server]),
        selected_artifact_case: "frames-browser-client",
        exports: vec!["installServerComponents", "renderServerComponent"],
    }
}

fn conditional_adapters() -> ConformanceCase {
    let browser_authority = Authority::WebBrowser;
    let server_authority = Authority::WebServer;
    let mut browser = browser_authority.case("web-browser-development");
    let module_load = resource(
        "module-load",
        ResourceKind::AsyncComputation,
        vec![
            ResourceState::AsyncPending,
            ResourceState::AsyncSettled,
            ResourceState::AsyncErrored,
            ResourceState::AsyncCancelled,
        ],
    );
    let mut eager = operation("eager-load", OperationKind::Invoke, Event::Call, 0);
    eager.resources.insert(ResourceId("module-load".into()));
    let mut lazy = operation("lazy-load", OperationKind::Invoke, Event::Render, 0);
    lazy.guard = Some(Guard(vec![GuardAtom::Literal {
        argument: 1,
        path: vec!["lazy".into()],
        value: Literal::Bool(true),
    }]));
    lazy.schedule = Some(Schedule::External);
    lazy.resources.insert(ResourceId("module-load".into()));
    let mut resolve = operation("resolve-module", OperationKind::Return, Event::Settle, 0);
    resolve.trigger = Some(Trigger::Resource {
        resource: ResourceId("module-load".into()),
        event: Event::Settle,
    });
    resolve.schedule = Some(Schedule::Queued);
    resolve.output = Some(ValueShape::Object(KnowledgeSet::Unknown));
    resolve.resources.insert(ResourceId("module-load".into()));
    let mut fallback = operation("render-fallback", OperationKind::Return, Event::Render, 0);
    fallback.schedule = Some(Schedule::External);
    fallback.output = Some(ValueShape::Plain);
    let mut component = operation("render-component", OperationKind::Invoke, Event::Settle, 0);
    component.trigger = Some(Trigger::Operation(OperationId("resolve-module".into())));
    component.schedule = Some(Schedule::Queued);
    component.output = Some(ValueShape::Component);
    let export = semantic_export(
        &browser,
        "clientOnly",
        ValueShape::Component,
        vec![eager, lazy, resolve, fallback, component],
        vec![
            edge(EdgeKind::Data, "eager-load", "resolve-module"),
            edge(EdgeKind::Data, "lazy-load", "resolve-module"),
            edge(EdgeKind::Data, "resolve-module", "render-component"),
            edge(EdgeKind::Orders, "render-fallback", "render-component"),
        ],
        vec![module_load],
        KnowledgeSet::Complete(vec![
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 1,
                    path: vec!["lazy".into()],
                    value: Literal::Bool(true),
                }]),
                operations: KnowledgeSet::Complete(vec![
                    OperationId("lazy-load".into()),
                    OperationId("resolve-module".into()),
                    OperationId("render-fallback".into()),
                    OperationId("render-component".into()),
                ]),
            },
            GuardedCase::When {
                guard: Guard(vec![GuardAtom::Literal {
                    argument: 1,
                    path: vec!["lazy".into()],
                    value: Literal::Bool(false),
                }]),
                operations: KnowledgeSet::Complete(vec![
                    OperationId("eager-load".into()),
                    OperationId("resolve-module".into()),
                    OperationId("render-fallback".into()),
                    OperationId("render-component".into()),
                ]),
            },
            GuardedCase::Otherwise {
                operations: KnowledgeSet::Complete(vec![
                    OperationId("eager-load".into()),
                    OperationId("resolve-module".into()),
                    OperationId("render-fallback".into()),
                    OperationId("render-component".into()),
                ]),
            },
        ]),
        &[ClaimDomain::Callbacks, ClaimDomain::Returns],
    );
    insert(&mut browser, export);

    let mut server = server_authority.case("web-node-server");
    let mut server_fallback = operation("render-fallback", OperationKind::Return, Event::Render, 0);
    server_fallback.schedule = Some(Schedule::External);
    server_fallback.output = Some(ValueShape::Plain);
    let server_export = semantic_export(
        &server,
        "clientOnly",
        ValueShape::Component,
        vec![server_fallback],
        vec![],
        vec![],
        KnowledgeSet::Complete(vec![]),
        &[],
    );
    insert(&mut server, server_export);
    ConformanceCase {
        row: ConformanceRow::ConditionalAdapters,
        proposal: ContractProposal::new(browser_authority.package(), vec![browser, server]),
        selected_artifact_case: "web-browser-development",
        exports: vec!["clientOnly"],
    }
}

fn mixed_framework() -> ConformanceCase {
    let authority = Authority::AutoAnimateSolid;
    let mut case = authority.case("auto-animate-solid");
    let export = semantic_export(
        &case,
        "createAutoAnimate",
        ValueShape::Tuple(KnowledgeSet::Partial(vec![
            ValueShape::Reactive {
                role: ReactiveRole::Setter,
                resource: None,
                capabilities: KnowledgeSet::Complete(vec![CapabilityClaim {
                    capability: ObservableCapability::Writable,
                    resource: None,
                }]),
            },
            ValueShape::Callable,
        ])),
        vec![],
        vec![],
        vec![],
        KnowledgeSet::Unknown,
        &ClaimDomain::ALL,
    );
    insert(&mut case, export);
    ConformanceCase {
        row: ConformanceRow::MixedFrameworkSelection,
        proposal: ContractProposal::new(authority.package(), vec![case]),
        selected_artifact_case: "auto-animate-solid",
        exports: vec!["createAutoAnimate"],
    }
}

#[must_use]
pub fn conformance_corpus() -> Vec<ConformanceCase> {
    vec![
        split_effect(),
        tracked_effect(),
        batching(),
        control_flow(),
        async_computations(),
        loading_refresh(),
        actions_optimism(),
        stores_projections(),
        refs_directives(),
        event_delegation(),
        rendering(),
        request_response(),
        server_functions(),
        server_components(),
        conditional_adapters(),
        mixed_framework(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_semantics::{ClaimPath, KnowledgeState, ValueClaimDomain};

    #[test]
    fn every_matrix_row_normalizes_against_exact_published_artifact_identity() {
        let corpus = conformance_corpus();
        assert_eq!(corpus.len(), ConformanceRow::ALL.len());
        let rows = corpus.iter().map(|case| case.row).collect::<BTreeSet<_>>();
        assert_eq!(rows, BTreeSet::from(ConformanceRow::ALL));
        for case in corpus {
            let row = case.row.id();
            let contract = case
                .proposal
                .normalize()
                .unwrap_or_else(|error| panic!("{row} did not normalize: {error}"));
            let selected = contract
                .artifact_case(case.selected_artifact_case)
                .unwrap_or_else(|| panic!("{row} lost its selected artifact case"));
            assert!(!selected.runtime.path.is_empty());
            assert!(!selected.declarations.path.is_empty());
            assert!(selected.runtime.digest.as_str().starts_with("sha256:"));
            assert!(selected.declarations.digest.as_str().starts_with("sha256:"));
            for export in case.exports {
                assert!(
                    contract
                        .artifact_cases()
                        .iter()
                        .any(|artifact| artifact.exports.contains_key(export)),
                    "{row} does not expose its declared conformance export {export}"
                );
            }
        }
    }

    #[test]
    fn conformance_digests_are_deterministic_and_bind_artifact_drift() {
        for case in conformance_corpus() {
            let row = case.row.id();
            let first = case.proposal.clone().normalize().unwrap();
            let second = case.proposal.normalize().unwrap();
            assert_eq!(first.semantic_digest(), second.semantic_digest(), "{row}");

            let mut artifacts = first.artifact_cases().to_vec();
            artifacts[0].runtime.digest = digest(&"0".repeat(64));
            let drifted = ContractProposal::new(first.package().clone(), artifacts)
                .normalize()
                .unwrap();
            assert_ne!(first.semantic_digest(), drifted.semantic_digest(), "{row}");
        }
    }

    #[test]
    fn normalized_cases_bind_the_checked_transitive_closure_census() {
        let report: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../benchmarks/package-contract-v2/phase13/conformance.json"
        ))
        .unwrap();
        for case in conformance_corpus() {
            let row = case.row.id();
            let report_row = report["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|candidate| candidate["id"] == row)
                .unwrap();
            let closure_name = report_row["closureIdentity"].as_str().unwrap();
            let expected = report["closureIdentities"][closure_name]["digest"]
                .as_str()
                .unwrap();
            let contract = case.proposal.normalize().unwrap();
            assert!(contract.artifact_cases().iter().all(|artifact| {
                artifact.dependency_closure.as_str() == format!("sha256:{expected}")
            }));
        }
    }

    #[test]
    fn checked_corpus_and_normalized_model_have_the_same_semantic_census() {
        let report: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../benchmarks/package-contract-v2/phase13/conformance.json"
        ))
        .unwrap();
        for case in conformance_corpus() {
            let row = case.row.id();
            let report_row = report["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|candidate| candidate["id"] == row)
                .unwrap();
            let expected_exports = report_row["apis"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                expected_exports,
                case.exports.iter().copied().collect(),
                "{row}: export census drifted"
            );

            let contract = case.proposal.normalize().unwrap();
            let actual_operations = contract
                .artifact_cases()
                .iter()
                .flat_map(|artifact| artifact.exports.values())
                .flat_map(|export| export.call.operations.iter())
                .map(|operation| operation.id.0.as_str())
                .collect::<BTreeSet<_>>();
            let expected_operations = report_row["normalized"]["operations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                expected_operations, actual_operations,
                "{row}: operation census drifted"
            );

            let actual_resources = contract
                .artifact_cases()
                .iter()
                .flat_map(|artifact| artifact.exports.values())
                .flat_map(|export| export.call.resources.iter())
                .map(|resource| resource.id.0.as_str())
                .collect::<BTreeSet<_>>();
            let expected_resources = report_row["normalized"]["resources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                expected_resources, actual_resources,
                "{row}: resource census drifted"
            );
        }
    }

    #[test]
    fn experimental_and_incompatible_domains_remain_local_and_open() {
        for case in conformance_corpus() {
            let row = case.row;
            let contract = case.proposal.normalize().unwrap();
            if row == ConformanceRow::ExperimentalServerComponents {
                assert!(contract.artifact_cases().iter().all(|artifact| {
                    artifact.stability == StabilityKnowledge::Experimental
                        && artifact.exports.values().all(|export| {
                            export.stability == StabilityKnowledge::Experimental
                                && export.claim_state(ClaimDomain::Callbacks)
                                    == KnowledgeState::Unknown
                        })
                }));
            }
            if row == ConformanceRow::MixedFrameworkSelection {
                let export = &contract.artifact_cases()[0].exports["createAutoAnimate"];
                assert_eq!(
                    export.claim_state(ClaimDomain::Callbacks),
                    KnowledgeState::Unknown
                );
                assert!(export.unresolved_claims().iter().any(|claim| matches!(
                    claim,
                    ClaimPath::Value {
                        domain: ValueClaimDomain::TupleItems,
                        ..
                    }
                )));
            }
        }
    }

    #[test]
    fn unresolved_adapter_guard_joins_eager_and_lazy_operations_without_a_guarantee() {
        let case = conformance_corpus()
            .into_iter()
            .find(|case| case.row == ConformanceRow::ConditionalAdapters)
            .unwrap();
        let contract = case.proposal.normalize().unwrap();
        let export = &contract.artifact_cases()[0].exports["clientOnly"];
        assert!(matches!(
            export.call.guards.select_operations(|_| super::super::GuardTruth::Unknown),
            KnowledgeSet::Complete(operations)
                if operations.len() == 5
                    && operations.contains(&OperationId("eager-load".into()))
                    && operations.contains(&OperationId("lazy-load".into()))
        ));
    }
}
