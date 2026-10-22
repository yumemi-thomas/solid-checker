use super::{
    OWNER_CONTEXT_COMPONENT_UNCERTAIN, OWNER_CONTEXT_LATER_RUN_UNOWNED, OWNER_CONTEXT_LEAF,
    OWNER_CONTEXT_LEAF_OWNED, OWNER_CONTEXT_MAY_RESUME_UNOWNED, OWNER_CONTEXT_OWNED,
    OWNER_CONTEXT_PROVEN_UNOWNED, OWNER_CONTEXT_UNOWNED, OwnerEdgeKind, OwnerNode,
    binding_returns_reactive_source, callback_owner_edge_kind, compiler_owner_context,
    compose_owner_edge, owner_edge_context, possible_await_continuation_context,
    propagate_owner_contexts, providing_region_chain, returned_arrow_function, root_owned_at,
    seed_contexts, seed_unnamed_hand_outs, settle_eager_owners,
};
use solid_facts::ast;
use solid_facts::compiler::{
    COMPILER_FACTS_PROTOCOL, ExecutionMap, OwnershipRegion, OwnershipRegionKind,
};
use solid_facts::core::SourceHash;
use solid_facts::core::Span;

fn returned_arrow(source: &str) -> bool {
    let source = format!("function outer() {{ return {source}; }}");
    let facts = ast::extract("test.tsx", &source).unwrap();
    facts
        .returns
        .first()
        .and_then(|returned| returned.argument)
        .is_some_and(|argument| returned_arrow_function(&facts, argument))
}

#[test]
fn returned_arrow_identity_comes_from_ast_facts() {
    assert!(returned_arrow("(a = (0)) /* run */ => a"));
    assert!(returned_arrow("async => async"));
    assert!(!returned_arrow("function () { return 1; }"));
}

fn binding_accepts(source: &str) -> bool {
    let facts = ast::extract("test.tsx", source).unwrap();
    binding_returns_reactive_source(
        facts.bindings.first().unwrap(),
        facts.calls.first().unwrap(),
    )
}

#[test]
fn returned_source_binding_identity_comes_from_ast_facts() {
    assert!(binding_accepts(
        "const [go] = /* gate */ makeGate<Array<T>>();"
    ));
    assert!(!binding_accepts("let [go] = makeGate();"));
    assert!(!binding_accepts("const [, go] = makeGate();"));
}

/// One file with a real AST and a hand-built execution map.
fn owner_context_file(source: &str, compiler: ExecutionMap) -> solid_facts::FileFacts {
    const PATH: &str = "test.tsx";
    solid_facts::FileFacts {
        generation: solid_facts::core::Generation::new(1).unwrap(),
        path: solid_facts::core::SourcePath::new(PATH).unwrap(),
        source_hash: SourceHash::of(source),
        source: std::sync::Arc::from(source),
        ast: std::sync::Arc::new(ast::extract(PATH, source).unwrap()),
        compiler: std::sync::Arc::new(compiler),
    }
}

#[test]
fn compiler_ownership_regions_seed_owner_analysis() {
    let source = "function run() {}";
    let facts = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(source),
        semantic_model: Default::default(),
        tracked_regions: vec![],
        untracked_regions: vec![],
        discarded_regions: vec![],
        ownership_regions: vec![OwnershipRegion {
            span: Span::new(0, 17),
            kind: OwnershipRegionKind::Leaf,
        }],
        callback_roles: vec![],
        jsx_operations: vec![],
    };
    assert_eq!(
        compiler_owner_context(&owner_context_file(source, facts), Span::new(15, 17)),
        OWNER_CONTEXT_LEAF
    );
}

#[test]
fn possible_suspension_preserves_the_owner_without_inventing_an_invocation() {
    let source = "async function run() { before(); if (flag) await pending; after(); }";
    let compiler = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(source),
        semantic_model: Default::default(),
        tracked_regions: vec![],
        untracked_regions: vec![],
        discarded_regions: vec![],
        ownership_regions: vec![],
        callback_roles: vec![],
        jsx_operations: vec![],
    };
    let file = owner_context_file(source, compiler);
    let site = |name| {
        file.ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some(name))
            .unwrap()
            .callee
    };
    let leaf = OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED;
    assert_eq!(
        possible_await_continuation_context(&file, site("before"), leaf),
        None
    );
    assert_eq!(
        possible_await_continuation_context(&file, site("after"), leaf),
        Some(leaf | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_MAY_RESUME_UNOWNED)
    );
    assert_eq!(
        possible_await_continuation_context(&file, site("after"), 0),
        None
    );
    let unowned = OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
    assert_eq!(
        possible_await_continuation_context(&file, site("after"), unowned),
        Some(unowned)
    );
}

#[test]
fn owner_edge_context_transforms_the_source_bits_per_edge_kind() {
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::Preserve, OWNER_CONTEXT_OWNED),
        OWNER_CONTEXT_OWNED
    );
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::Owned, OWNER_CONTEXT_UNOWNED),
        OWNER_CONTEXT_OWNED
    );
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::Unowned, OWNER_CONTEXT_OWNED),
        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
    );
    assert_eq!(
        owner_edge_context(
            OwnerEdgeKind::Preserve,
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN,
        ),
        OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_COMPONENT_UNCERTAIN
    );
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::Conditional, 0),
        OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED
    );
    // A createTrackedEffect body runs under the tracked node whatever its
    // creation owner: its cleanup is SC3001's (`LEAF_OWNED`), and no unowned
    // bit survives, so it is never SC4001's `NO_OWNER_CLEANUP`. A conditional
    // creation is live on its owned path too.
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::Leaf, OWNER_CONTEXT_OWNED),
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
    );
    assert_eq!(
        owner_edge_context(
            OwnerEdgeKind::Leaf,
            OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
        ),
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
    );
    assert_eq!(
        owner_edge_context(
            OwnerEdgeKind::Leaf,
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED
        ),
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
    );
    // A tracked effect inside a creation-owned leaf body is dead: it throws
    // before its callback runs.
    assert_eq!(
        owner_edge_context(
            OwnerEdgeKind::Leaf,
            OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
        ),
        0
    );
    // An onSettled inside a leaf body is queued out of band: proven unowned
    // whatever the body's owner (rc.13). Outside one it is an ordinary leaf.
    assert_eq!(
        owner_edge_context(
            OwnerEdgeKind::QueuedLeaf,
            OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
        ),
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
    );
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::QueuedLeaf, OWNER_CONTEXT_OWNED),
        OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED
    );
    // A ref application proves the unowned run; a may-be-unowned callback
    // carries only the open-world bit, whatever its source.
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::RefApplied, 0),
        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
    );
    assert_eq!(
        owner_edge_context(OwnerEdgeKind::MayBeUnowned, OWNER_CONTEXT_OWNED),
        OWNER_CONTEXT_UNOWNED
    );
    // ADR 0362: an apply a flush from an owned scope may drain is owned on
    // some runs and unowned on others, whatever the creation site proves, and
    // it leaves a region written around it.
    for source in [
        0,
        OWNER_CONTEXT_OWNED,
        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
    ] {
        let context = owner_edge_context(OwnerEdgeKind::FlushDrained, source);
        assert_eq!(
            context,
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED
        );
        assert_eq!(context & OWNER_CONTEXT_PROVEN_UNOWNED, 0);
        assert_ne!(
            context & super::owner_context::OWNER_CONTEXT_LEAVES_REGION,
            0
        );
    }
}

#[test]
fn callback_owner_edge_kinds_map_one_to_one() {
    use solid_dialect::CallbackOwner;
    assert_eq!(
        callback_owner_edge_kind(CallbackOwner::Creates),
        OwnerEdgeKind::Owned
    );
    assert_eq!(
        callback_owner_edge_kind(CallbackOwner::Conditional),
        OwnerEdgeKind::Conditional
    );
    assert_eq!(
        callback_owner_edge_kind(CallbackOwner::Inherits),
        OwnerEdgeKind::Preserve
    );
    assert_eq!(
        callback_owner_edge_kind(CallbackOwner::None),
        OwnerEdgeKind::Unowned
    );
    assert_eq!(
        callback_owner_edge_kind(CallbackOwner::Leaf),
        OwnerEdgeKind::Leaf
    );
}

#[test]
fn composing_owner_edges_lets_an_explicit_callback_owner_dominate() {
    assert_eq!(
        compose_owner_edge(OwnerEdgeKind::Unowned, OwnerEdgeKind::Preserve),
        OwnerEdgeKind::Unowned
    );
    assert_eq!(
        compose_owner_edge(OwnerEdgeKind::Unowned, OwnerEdgeKind::Owned),
        OwnerEdgeKind::Owned
    );
    assert_eq!(
        compose_owner_edge(OwnerEdgeKind::Owned, OwnerEdgeKind::Leaf),
        OwnerEdgeKind::Leaf
    );
}

fn node(seed_context: u8) -> OwnerNode {
    OwnerNode {
        path: "app.tsx".to_owned(),
        span: Span::new(0, 10),
        body: Span::new(0, 10),
        symbol: None,
        exported: false,
        generator: false,
        component: seed_context & OWNER_CONTEXT_OWNED != 0,
        component_uncertain: false,
        seed_context,
        namespace_candidate: false,
    }
}

#[test]
fn semantic_seeds_preserve_proven_owner_states() {
    let nodes = [
        node(OWNER_CONTEXT_OWNED),
        node(OWNER_CONTEXT_UNOWNED),
        node(0),
        node(OWNER_CONTEXT_LEAF),
    ];
    assert_eq!(
        seed_contexts(&nodes),
        vec![
            OWNER_CONTEXT_OWNED,
            OWNER_CONTEXT_UNOWNED,
            0,
            OWNER_CONTEXT_LEAF
        ]
    );
}

#[test]
fn propagation_reaches_a_fixed_point_and_accumulates_bits() {
    // 0 --preserve--> 1 --conditional--> 2, plus a cycle 1 <-> 0.
    let mut contexts = vec![OWNER_CONTEXT_OWNED, 0, OWNER_CONTEXT_LEAF];
    let outgoing = vec![
        vec![(1, OwnerEdgeKind::Preserve)],
        vec![
            (0, OwnerEdgeKind::Preserve),
            (2, OwnerEdgeKind::Conditional),
        ],
        vec![],
    ];
    propagate_owner_contexts(&mut contexts, &outgoing, &[false; 3]);
    assert_eq!(
        contexts,
        vec![
            OWNER_CONTEXT_OWNED,
            OWNER_CONTEXT_OWNED,
            OWNER_CONTEXT_LEAF | OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
        ]
    );
}

/// Only nodes whose seeded context is nonzero enter the worklist, so even an
/// `Owned` edge (whose contribution does not depend on the source's bits) stays
/// dormant when its source has no context of its own. That dormancy is the
/// closed program's, kept on purpose (ADR 0347): an unexported function nothing
/// calls, and an export no namespace import hands out, have no caller at all,
/// so the callbacks they pass are not reached. An export a namespace import
/// does hand out is not context-free: `seed_namespace_hand_outs` gives it an
/// unknown caller first, and its edges walk (the test below).
#[test]
fn propagation_does_not_walk_edges_out_of_context_free_nodes() {
    let mut contexts = vec![0, 0];
    let outgoing = vec![vec![(1, OwnerEdgeKind::Owned)], vec![]];
    propagate_owner_contexts(&mut contexts, &outgoing, &[false; 2]);
    assert_eq!(contexts, vec![0, 0]);
}

/// ADR 0347: a `runWithOwner(null)` callback of an export that a namespace
/// import hands out is reached through the unknown-caller seed, and the
/// callback is proven unowned whatever that caller does.
#[test]
fn a_namespace_handed_out_export_reaches_its_detached_callback() {
    let mut contexts = vec![OWNER_CONTEXT_UNOWNED, 0];
    let outgoing = vec![vec![(1, OwnerEdgeKind::Unowned)], vec![]];
    propagate_owner_contexts(&mut contexts, &outgoing, &[false; 2]);
    assert_eq!(
        contexts,
        vec![
            OWNER_CONTEXT_UNOWNED,
            OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
        ]
    );
}

/// ADR 0347 amendment: a detached callback whose function is reached only through an
/// unnamed namespace escape is not proven unowned: whether the function runs is
/// unproven, so the callback's unowned context has no proof bit.
#[test]
fn an_unnamed_escape_reaches_a_detached_callback_without_proof() {
    let mut contexts = vec![0, 0];
    let outgoing = vec![vec![(1, OwnerEdgeKind::Unowned)], vec![]];
    let uncertain = seed_unnamed_hand_outs(&mut contexts, &outgoing, &[true, false]);
    propagate_owner_contexts(&mut contexts, &outgoing, &uncertain);
    assert_eq!(contexts, vec![OWNER_CONTEXT_UNOWNED, OWNER_CONTEXT_UNOWNED]);
}

/// ADR 0347 amendment: a function the program also reaches with a certain caller keeps
/// its proof: the unnamed escape adds no uncertainty to a certain reach.
#[test]
fn a_certain_reach_keeps_the_proof_an_unnamed_escape_also_gives() {
    // 0 is an unnamed candidate and calls 1; 1 is certainly reached (proven
    // unowned); 1's detached callback 2 is proven.
    let mut contexts = vec![0, OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED, 0];
    let outgoing = vec![
        vec![(1, OwnerEdgeKind::Preserve)],
        vec![(2, OwnerEdgeKind::Unowned)],
        vec![],
    ];
    let uncertain = seed_unnamed_hand_outs(&mut contexts, &outgoing, &[true, false, false]);
    // The candidate has no certain reach of its own; 1 and 2 have one.
    assert_eq!(uncertain, vec![true, false, false]);
    propagate_owner_contexts(&mut contexts, &outgoing, &uncertain);
    assert_eq!(
        contexts[2],
        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED
    );
}

/// An eager ref call from a function with no owner context settles to the
/// uncertain unowned bit, and what that call leads to inherits it. From a
/// proven source the call inherits the proof, as `Preserve` does.
#[test]
fn an_eager_call_settles_uncertain_from_a_context_free_source() {
    // 0 (context-free) --eager--> 1 --preserve--> 2; 3 (proven) --eager--> 4.
    let mut contexts = vec![
        0,
        0,
        0,
        OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        0,
    ];
    let outgoing = vec![
        vec![(1, OwnerEdgeKind::EagerCall)],
        vec![(2, OwnerEdgeKind::Preserve)],
        vec![],
        vec![(4, OwnerEdgeKind::EagerCall)],
        vec![],
    ];
    propagate_owner_contexts(&mut contexts, &outgoing, &[false; 5]);
    settle_eager_owners(&mut contexts, &outgoing, &[false; 5]);
    let uncertain = OWNER_CONTEXT_UNOWNED;
    let proven = OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED;
    assert_eq!(contexts, vec![0, uncertain, uncertain, proven, proven]);
}

#[test]
fn owner_providing_regions_contain_spans_inclusively() {
    let regions = [Span::new(10, 20), Span::new(40, 50)];
    let none = std::iter::empty::<((), Span, Span)>;
    assert_eq!(
        providing_region_chain(&regions, none(), Span::new(12, 18)),
        Some(vec![])
    );
    assert_eq!(
        providing_region_chain(&regions, none(), Span::new(40, 50)),
        Some(vec![])
    );
    assert_eq!(
        providing_region_chain(&regions, none(), Span::new(19, 21)),
        None
    );
    assert_eq!(providing_region_chain(&[], none(), Span::new(0, 0)), None);
}

/// `createRoot(() => { createEffect(c, () => { createRoot(() => { OP }) }) })`
/// as spans: the outer root callback 10..100, the effect apply 20..90, the
/// inner root callback 30..80, and an operation in each.
#[test]
fn providing_region_chain_stops_at_the_innermost_region() {
    let regions = [Span::new(10, 100), Span::new(30, 80)];
    let functions = [
        ("outer-root", Span::new(10, 100), Span::new(15, 100)),
        ("effect-apply", Span::new(20, 90), Span::new(25, 90)),
        ("inner-root", Span::new(30, 80), Span::new(35, 80)),
    ];
    let chain = |span| providing_region_chain(&regions, functions, span);
    assert_eq!(chain(Span::new(16, 18)), Some(vec!["outer-root"]));
    assert_eq!(
        chain(Span::new(26, 28)),
        Some(vec!["outer-root", "effect-apply"])
    );
    // The inner root sits inside the detached apply and answers for what
    // it contains: the apply is not in its chain.
    assert_eq!(chain(Span::new(40, 50)), Some(vec!["inner-root"]));
    let context = |function: &&str| match *function {
        "effect-apply" => OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
        _ => OWNER_CONTEXT_OWNED,
    };
    assert!(root_owned_at(chain(Span::new(16, 18)).as_deref(), context));
    assert!(!root_owned_at(chain(Span::new(26, 28)).as_deref(), context));
    assert!(root_owned_at(chain(Span::new(40, 50)).as_deref(), context));
    assert!(!root_owned_at(None::<&[&str]>, context));
}

#[test]
fn root_owned_at_withholds_for_either_detaching_bit() {
    let chain = [0_u8];
    for (context, answered) in [
        (OWNER_CONTEXT_OWNED, true),
        (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED, true),
        (OWNER_CONTEXT_LEAF, false),
        (OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED, false),
        (0, true),
        (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED, false),
        (
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LATER_RUN_UNOWNED,
            false,
        ),
    ] {
        assert_eq!(
            root_owned_at(Some(&chain[..]), |_| context),
            answered,
            "context {context:#b}"
        );
    }
}
