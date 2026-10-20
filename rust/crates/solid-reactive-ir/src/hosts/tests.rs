use super::*;

#[test]
fn static_linking_and_completion_are_conjunctive_and_transitive() {
    let module = |path: &str, dependencies: &[&str], refused, completion| HostModule {
        path: path.into(),
        static_dependencies: dependencies.iter().map(|path| (*path).into()).collect(),
        refused,
        completion,
    };
    let build = |modules: &[HostModule]| {
        let scopes = [
            HostScope {
                path: "entry".into(),
                ..scope("entry", None, &["barrel"])
            },
            HostScope {
                path: "barrel".into(),
                ..scope("barrel", None, &["leaf"])
            },
            HostScope {
                path: "leaf".into(),
                ..scope("leaf", None, &[])
            },
        ];
        ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["entry".into()]),
                ..Default::default()
            },
            modules,
            &scopes,
        )
    };
    let mut modules = [
        module("entry", &["barrel"], false, Some(true)),
        module("barrel", &["leaf"], false, Some(true)),
        module("leaf", &[], false, Some(true)),
    ];
    assert!(build(&modules).browser_site("entry", 0, 1));
    assert!(build(&modules).browser_site("leaf", 0, 1));
    modules[2].refused = true;
    assert!(build(&modules).is_empty());
    modules[2].refused = false;
    modules[2].completion = Some(false);
    assert!(build(&modules).is_empty());
    modules[2].completion = None;
    assert!(build(&modules).is_empty());
    modules[2].completion = Some(true);
    modules[2].static_dependencies.insert("barrel".into());
    assert!(build(&modules).is_empty());
    modules[2].static_dependencies = BTreeSet::from(["not-in-program".into()]);
    assert!(build(&modules).is_empty());
    // A distinct root can still execute the throwing module's live prefix.
    modules[0].static_dependencies.clear();
    modules[1].static_dependencies.clear();
    modules[2].static_dependencies.clear();
    modules[2].completion = Some(false);
    let index = build(&modules);
    assert!(index.browser_site("entry", 0, 1));
    assert!(index.browser_site("leaf", 0, 1));
}

#[test]
fn exact_outgoing_site_also_requires_containing_expression_completion() {
    let mut top = scope("top", None, &[]);
    let call = Span::new(20, 25);
    top.execution.extend([
        HostExecutionRegion {
            span: Span::new(10, 30),
            client: None,
            server: None,
        },
        HostExecutionRegion {
            span: call,
            client: Some(true),
            server: Some(true),
        },
    ]);
    assert!(!top.client_live_site(call));
}

fn scope(id: &str, span: Option<Span>, edges: &[&str]) -> HostScope {
    HostScope {
        id: id.into(),
        path: "shared".into(),
        span,
        edges: edges.iter().map(|edge| (*edge).into()).collect(),
        execution: vec![HostExecutionRegion {
            span: span.unwrap_or(Span::new(0, u32::MAX)),
            client: Some(true),
            server: Some(true),
        }],
        ..HostScope::default()
    }
}

#[test]
fn incoming_server_callers_do_not_cancel_browser_and_uncalled_bodies_stay_none() {
    let scopes = [
        scope("top", None, &["called", "missing"]),
        scope("called", Some(Span::new(10, 30)), &[]),
        scope("server-only", Some(Span::new(40, 60)), &["called"]),
    ];
    let index = ProjectHostIndex::build(
        BrowserRootManifest {
            roots: BTreeSet::from(["top".into()]),
            ..BrowserRootManifest::default()
        },
        &[],
        &scopes,
    );
    assert!(index.browser_site("shared", 0, 1));
    assert!(index.browser_site("shared", 15, 16));
    assert!(!index.browser_site("shared", 45, 46));
}

#[test]
fn server_boundary_does_not_propagate_or_cancel_its_browser_caller() {
    let mut server = scope("server", Some(Span::new(10, 30)), &["hidden"]);
    server.refused = true;
    let scopes = [
        scope("top", None, &["server"]),
        server,
        scope("hidden", Some(Span::new(40, 60)), &[]),
    ];
    let index = ProjectHostIndex::build(
        BrowserRootManifest {
            roots: BTreeSet::from(["top".into()]),
            ..BrowserRootManifest::default()
        },
        &[],
        &scopes,
    );
    assert!(index.browser_site("shared", 0, 1));
    assert!(!index.browser_site("shared", 15, 16));
    assert!(!index.browser_site("shared", 45, 46));
}

fn callback(
    delivery: crate::callback_host::CallbackDelivery,
    invoker: Span,
) -> crate::callback_host::BrowserCallbackInvocation {
    use crate::callback_host::{BrowserCallbackInvocation, InvocationSite};
    let site = |span| InvocationSite {
        path: "shared".into(),
        span,
    };
    BrowserCallbackInvocation {
        supplied: site(Span::new(1, 2)),
        invoked: site(invoker),
        target: site(Span::new(70, 90)),
        delivery,
        cancelled_by: None,
        premise: "exact-local-path".into(),
    }
}

#[test]
fn callback_needs_supplier_and_exact_browser_invoker() {
    use crate::callback_host::CallbackDelivery;
    let build = |invoker_edge: bool| {
        ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..BrowserRootManifest::default()
            },
            &[],
            &[
                scope("top", None, if invoker_edge { &["invoker"] } else { &[] }),
                scope("invoker", Some(Span::new(10, 30)), &[]),
                scope("callback", Some(Span::new(70, 90)), &[]),
            ],
        )
    };
    let fact = callback(CallbackDelivery::SameStack, Span::new(15, 16));
    assert!(
        !build(false)
            .with_callback_invocations(vec![fact.clone()])
            .browser_site("shared", 75, 76)
    );
    assert!(
        build(true)
            .with_callback_invocations(vec![fact])
            .browser_site("shared", 75, 76)
    );
}

#[test]
fn feasible_trigger_upgrades_but_exact_client_live_cancellation_refuses() {
    use crate::callback_host::{CallbackDelivery, InvocationSite};
    let build = || {
        ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..Default::default()
            },
            &[],
            &[
                scope("top", None, &[]),
                scope("callback", Some(Span::new(70, 90)), &[]),
            ],
        )
    };
    let mut fact = callback(
        CallbackDelivery::FeasibleHostTrigger(solid_dialect::BrowserCallbackTrigger::TimerTask),
        Span::new(1, 2),
    );
    assert!(
        build()
            .with_callback_invocations(vec![fact.clone()])
            .browser_site("shared", 75, 76)
    );
    fact.cancelled_by = Some(InvocationSite {
        path: "shared".into(),
        span: Span::new(3, 4),
    });
    assert!(
        !build()
            .with_callback_invocations(vec![fact])
            .browser_site("shared", 75, 76)
    );
}

#[test]
fn cancellation_registration_and_dead_invokers_never_upgrade_callbacks() {
    use crate::callback_host::CallbackDelivery;
    use solid_dialect::BrowserCallbackTrigger;
    let mut invoker = scope("invoker", Some(Span::new(10, 30)), &[]);
    invoker.refused = true;
    let build = || {
        ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..BrowserRootManifest::default()
            },
            &[],
            &[
                scope("top", None, &["invoker"]),
                invoker.clone(),
                scope("callback", Some(Span::new(70, 90)), &[]),
            ],
        )
    };
    assert!(
        !build()
            .with_callback_invocations(vec![callback(
                CallbackDelivery::SameStack,
                Span::new(15, 16)
            )])
            .browser_site("shared", 75, 76)
    );
    for trigger in [
        BrowserCallbackTrigger::TimerTask,
        BrowserCallbackTrigger::DomEvent,
        BrowserCallbackTrigger::OwnerDisposal,
        BrowserCallbackTrigger::RenderSelection,
    ] {
        assert!(
            !build()
                .with_callback_invocations(vec![callback(
                    CallbackDelivery::Pending(trigger),
                    Span::new(1, 2)
                )])
                .browser_site("shared", 75, 76)
        );
    }
}

#[test]
fn dead_unknown_and_unreached_default_sites_withhold_only_their_own_authority() {
    let mut top = scope("top", None, &[]);
    top.execution.extend([
        HostExecutionRegion {
            span: Span::new(10, 20),
            client: Some(false),
            server: Some(true),
        },
        HostExecutionRegion {
            span: Span::new(30, 40),
            client: None,
            server: None,
        },
    ]);
    let scopes = [top, scope("default", Some(Span::new(50, 60)), &[])];
    let index = ProjectHostIndex::build(
        BrowserRootManifest {
            roots: BTreeSet::from(["top".into()]),
            ..BrowserRootManifest::default()
        },
        &[],
        &scopes,
    );
    assert!(index.browser_site("shared", 1, 2));
    assert!(!index.browser_site("shared", 11, 12));
    assert!(!index.browser_site("shared", 31, 32));
    assert!(!index.browser_site("shared", 51, 52));
}
