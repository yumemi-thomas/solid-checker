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

// ---------------------------------------------------------------------------
// The kept owner graph (S7f): random fragments and random edits.

mod kept_graph {
    use super::super::owner_state::{HandOuts, OwnerGraph, PathId};
    use super::super::{
        CachedOwnerFile, OWNER_CONTEXT_LEAF, OWNER_CONTEXT_LEAF_OWNED, OWNER_CONTEXT_OWNED,
        OWNER_CONTEXT_PROVEN_UNOWNED, OWNER_CONTEXT_UNOWNED, OwnerEdgeKind, OwnerNode, OwnerSource,
        OwnerTarget, SymbolicOwnerEdge,
    };
    use crate::HashSet;
    use crate::identity::SymbolId;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{SourceHash, Span};
    use std::sync::Arc;

    /// Mulberry32: a failing run replays from its seed.
    struct Random(u32);

    impl Random {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_add(0x6d2b_79f5);
            let mut t = self.0;
            t = (t ^ (t >> 15)).wrapping_mul(t | 1);
            t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
            let unit = f64::from(t ^ (t >> 14)) / 4_294_967_296.0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let index = (unit * n as f64) as usize;
            index.min(n.saturating_sub(1))
        }
    }

    const PATHS: [&str; 5] = ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"];

    const KINDS: [OwnerEdgeKind; 11] = [
        OwnerEdgeKind::Preserve,
        OwnerEdgeKind::MayResumeUnowned,
        OwnerEdgeKind::Owned,
        OwnerEdgeKind::Unowned,
        OwnerEdgeKind::Conditional,
        OwnerEdgeKind::InheritsFirstRun,
        OwnerEdgeKind::QueuedLeaf,
        OwnerEdgeKind::RefApplied,
        OwnerEdgeKind::MayBeUnowned,
        OwnerEdgeKind::EagerCall,
        OwnerEdgeKind::Preserve,
    ];

    fn span(index: usize) -> Span {
        let start = u32::try_from(index * 10).unwrap();
        Span::new(start, start + 9)
    }

    fn fragment(random: &mut Random, path: &str, leaf: bool, namespaces: bool) -> CachedOwnerFile {
        let seeds = [
            0,
            OWNER_CONTEXT_OWNED,
            OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_PROVEN_UNOWNED,
            OWNER_CONTEXT_UNOWNED,
            OWNER_CONTEXT_LEAF,
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
            0,
            0,
        ];
        let count = 1 + random.below(6);
        let nodes = (0..count)
            .map(|index| OwnerNode {
                path: path.to_owned(),
                span: span(index),
                body: span(index),
                symbol: (random.below(3) != 0)
                    .then(|| SymbolId::from(format!("s{}", random.below(7)))),
                exported: random.below(4) == 0,
                generator: random.below(5) == 0,
                component: random.below(5) == 0,
                component_uncertain: false,
                seed_context: seeds[random.below(seeds.len())],
                namespace_candidate: namespaces && random.below(2) == 0,
            })
            .collect::<Vec<_>>();
        let edges = (0..random.below(10))
            .map(|_| {
                let source = (random.below(4) != 0).then(|| {
                    let other = random.below(3) == 0;
                    OwnerSource {
                        path: if other {
                            PATHS[random.below(PATHS.len())].to_owned()
                        } else {
                            path.to_owned()
                        },
                        span: span(random.below(count + 1)),
                    }
                });
                let target = match random.below(3) {
                    0 => OwnerTarget::Symbol(SymbolId::from(format!("s{}", random.below(7)))),
                    1 => OwnerTarget::LocalSpan(span(random.below(count + 1))),
                    _ => OwnerTarget::Function(
                        PATHS[random.below(PATHS.len())].to_owned(),
                        span(random.below(7)),
                    ),
                };
                let mut kind = KINDS[random.below(KINDS.len())];
                if leaf && random.below(6) == 0 {
                    kind = OwnerEdgeKind::Leaf;
                }
                SymbolicOwnerEdge {
                    source,
                    target,
                    kind,
                }
            })
            .collect();
        CachedOwnerFile {
            source_hash: SourceHash::of(path),
            proof: crate::base::reads::ReadProof::default(),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(path),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
            nodes,
            edges,
            requirements: vec![],
        }
    }

    fn table(graph: &OwnerGraph) -> std::collections::BTreeMap<(String, Span), u8> {
        graph.context_table()
    }

    /// Runs `steps` random edits against one kept graph and, after each,
    /// compares it with a graph built from the same fragments at once.
    fn run(seed: u32, steps: usize, leaf: bool, namespaces: bool) {
        if std::env::var_os("OWNER_PROPERTY_TRACE").is_some() {
            eprintln!("seed {seed}");
        }
        let mut random = Random(seed);
        let named = |path: &str, span: Span| (path.len() + span.start as usize).is_multiple_of(3);
        let escapes = |path: &str| path.starts_with('c') || path.starts_with('d');
        let hand_outs = HandOuts {
            named: &named,
            escapes_unnamed: &escapes,
        };
        let mut kept = OwnerGraph::default();
        let mut held = std::collections::BTreeMap::<&str, CachedOwnerFile>::new();
        for step in 0..steps {
            for _ in 0..1 + random.below(3) {
                let path = PATHS[random.below(PATHS.len())];
                match random.below(6) {
                    0 if held.contains_key(path) => {
                        held.remove(path);
                        if let Some(id) = kept.path_id(path) {
                            kept.remove_fragment(id);
                        }
                    }
                    1 if held.contains_key(path) => {
                        // A recomputed fragment of equal content.
                        let same = held[path].nodes.clone();
                        let edges = held[path].edges.clone();
                        let mut again = fragment(&mut random, path, leaf, namespaces);
                        again.nodes = same;
                        again.edges = edges;
                        kept.set_fragment(path, again.clone());
                        held.insert(path, again);
                    }
                    _ => {
                        let next = fragment(&mut random, path, leaf, namespaces);
                        kept.set_fragment(path, next.clone());
                        held.insert(path, next);
                    }
                }
            }
            let order = |graph: &mut OwnerGraph| {
                held.keys()
                    .map(|path| graph.intern(path))
                    .collect::<Vec<PathId>>()
            };
            let kept_order = order(&mut kept);
            kept.settle(&kept_order, &hand_outs);
            let mut fresh = OwnerGraph::default();
            for (path, fragment) in &held {
                fresh.set_fragment(path, fragment.clone());
            }
            let fresh_order = order(&mut fresh);
            fresh.settle(&fresh_order, &hand_outs);
            assert_eq!(
                table(&kept),
                table(&fresh),
                "seed {seed} step {step}: the kept graph differs from a fresh one"
            );
        }
    }

    /// Seeds per property: `OWNER_PROPERTY_SEEDS` widens a local run.
    fn seeds(default: u32) -> u32 {
        std::env::var("OWNER_PROPERTY_SEEDS")
            .ok()
            .and_then(|seeds| seeds.parse().ok())
            .unwrap_or(default)
    }

    #[test]
    fn a_kept_graph_edited_at_random_equals_a_fresh_one() {
        for seed in 1..=seeds(60) {
            run(seed, 25, false, false);
        }
    }

    #[test]
    fn a_kept_graph_with_tracked_effect_edges_equals_a_fresh_one() {
        for seed in 1_000_000..=1_000_000 + seeds(30) {
            run(seed, 20, true, false);
        }
    }

    #[test]
    fn a_kept_graph_with_namespace_hand_outs_equals_a_fresh_one() {
        for seed in 2_000_000..=2_000_000 + seeds(40) {
            run(seed, 20, false, true);
        }
    }

    #[test]
    fn the_resume_bit_a_proven_source_leaves_behind_is_the_only_history() {
        // The worklist's history: the source is {OWNED} when the edge first
        // fires and gains the proof later.
        let mut graph = OwnerGraph::default();
        let node = |span: Span, seed: u8| OwnerNode {
            path: "a.ts".into(),
            span,
            body: span,
            symbol: None,
            exported: false,
            generator: false,
            component: false,
            component_uncertain: false,
            seed_context: seed,
            namespace_candidate: false,
        };
        let edge = |from: usize, to: usize, kind| SymbolicOwnerEdge {
            source: Some(OwnerSource {
                path: "a.ts".into(),
                span: span(from),
            }),
            target: OwnerTarget::LocalSpan(span(to)),
            kind,
        };
        let mut fragment = fragment(&mut Random(1), "a.ts", false, false);
        fragment.nodes = vec![
            node(span(0), OWNER_CONTEXT_OWNED),
            node(span(1), 0),
            node(span(2), 0),
        ];
        // 1 --unowned--> 0 gives node 0 the proof after node 0's first visit.
        fragment.edges = vec![
            edge(0, 2, OwnerEdgeKind::MayResumeUnowned),
            edge(1, 0, OwnerEdgeKind::Unowned),
        ];
        graph.set_fragment("a.ts", fragment);
        let order = [graph.intern("a.ts")];
        let _ = (HashSet::<u8>::default(), OWNER_CONTEXT_LEAF_OWNED);
        graph.settle(
            &order,
            &HandOuts {
                named: &|_, _| false,
                escapes_unnamed: &|_| false,
            },
        );
        let table = graph.context_table();
        // Node 1 is unreached, so its edge never fires: node 0 stays owned
        // and node 2 holds the unowned and resume bits.
        assert_eq!(
            table[&("a.ts".to_owned(), span(2))],
            OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED | super::OWNER_CONTEXT_MAY_RESUME_UNOWNED
        );
    }
}
