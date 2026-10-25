use super::*;
use crate::{EntityFact, Location, SourceDigest, SourceHash, SymbolFact, v3::SymbolOp};
use serde::Deserialize;

#[test]
fn project_identity_matches_the_producers_absolute_clean_path() {
    let current = std::env::current_dir().unwrap();
    let dirty = current
        .join("nested")
        .join("..")
        .join("project")
        .join(".")
        .join("tsconfig.json");
    let normalized = normalize_project_id(dirty.to_string_lossy().into_owned()).unwrap();
    assert_eq!(
        normalized,
        current.join("project/tsconfig.json").to_string_lossy()
    );

    let relative = normalize_project_id("project/../tsconfig.json".into()).unwrap();
    assert_eq!(relative, current.join("tsconfig.json").to_string_lossy());
    assert!(normalize_project_id("  ".into()).is_err());
}

fn location(path: &str, start: u64) -> Location {
    Location {
        path: path.into(),
        start_byte: start,
        end_byte: start + 1,
    }
}

#[test]
fn shared_demand_groups_retain_the_callers_allocation() {
    let run: Arc<[EntityDemand]> = vec![EntityDemand {
        location: location("/p/a.ts", 1),
        symbol: true,
        ..EntityDemand::default()
    }]
    .into();
    let shared = DemandGroup::shared(&run).expect("non-empty shared run");
    let retained = shared.retained();

    assert!(Arc::ptr_eq(&run, &retained));
    assert_eq!(retained.as_ref(), shared.demands());
}

fn table_with_symbol(symbol: SymbolFact) -> FactTable {
    fact_table(Vec::new(), Vec::new(), vec![symbol], Vec::new())
}

fn fact_table(
    sources: Vec<SourceDigest>,
    entities: Vec<EntityFact>,
    symbols: Vec<SymbolFact>,
    files: Vec<crate::FileFact>,
) -> FactTable {
    FactTable::from_parts(
        v3::TYPE_FACTS_TABLE_SCHEMA_V22,
        1,
        "/p/tsconfig.json",
        sources,
        entities,
        symbols,
        files,
    )
}

fn delta_transition(
    base_generation: u64,
    target_generation: u64,
    paths: Vec<v3::PathOp>,
    symbols: Vec<SymbolOp>,
) -> WireTableTransition {
    WireTableTransition {
        mode: TransitionMode::Delta,
        table_schema: v3::TYPE_FACTS_TABLE_SCHEMA_V22,
        base_generation,
        target_generation,
        project_id: "/p/tsconfig.json".into(),
        base_state_token: format!("base-{base_generation}").into(),
        paths,
        symbols,
    }
}

fn reference_transition(id: &str, path: &str, references: Vec<Location>) -> WireTableTransition {
    delta_transition(
        1,
        2,
        Vec::new(),
        vec![SymbolOp::ReplaceReferencePath {
            id: id.into(),
            path: path.into(),
            references,
        }],
    )
}

fn response(generation: u64, state_token: &str, table_transition: Vec<u8>) -> Response {
    Response {
        schema: v3::TYPE_FACTS_SCHEMA_V1,
        request_id: 1,
        project_id: "/p/tsconfig.json".into(),
        generation,
        ok: true,
        table_transition,
        symbol_evidence: Vec::new(),
        reference_evidence: Vec::new(),
        changed_reference_symbols: Vec::new(),
        reference_changes_exact: false,
        state_token: state_token.into(),
        affected: Vec::new(),
        graph_kept: false,
        sources: Vec::new(),
        source_arena: String::new(),
        source_lengths: Vec::new(),
        timings: None,
        error: None,
        modules: Vec::new(),
        module_imports: Vec::new(),
        unknown_import_paths: Vec::new(),
        unmapped_roots: Vec::new(),
        client_decode_ns: 0,
        client_response_bytes: 0,
        client_request_send_ns: 0,
        client_request_bytes: 0,
        client_roundtrip_ns: 0,
    }
}

fn materialize_full(frame: &[u8], label: &str) -> FactTable {
    let transition = v3::decode_table_transition(frame)
        .unwrap_or_else(|error| panic!("{label}: decode full transition: {error}"));
    assert_eq!(
        transition.mode,
        TransitionMode::Full,
        "{label}: expected full transition"
    );
    materialize_full_transition(transition).0
}

#[test]
fn repeated_edits_to_one_path_keep_only_the_newest_replay_overlay() {
    // A long editing session sends the same handful of files over and over.
    // Retaining every version would grow the session by the full source text
    // of each edit; only the newest overlay per path changes what a replayed
    // generation produces.
    let mut session = Session {
        producer: Some(Producer::at("/nonexistent")),
        project_id: "/p/tsconfig.json".into(),
        generation: 1,
        restart_epoch: 0,
        connection: None,
        replay_batches: Vec::new(),
        replay_index: HashMap::new(),
        state_token: String::new(),
        retained_demands: HashMap::new(),
        retained_table: None,
        reference_tier: HashSet::new(),
        symbols_by_path: HashMap::new(),
        closure_index: None,
        last_closure_patched: false,
        pending_demand_paths: HashSet::new(),
        retained_reference_locations: HashMap::new(),
        reference_locations_valid: false,
        last_exchange_timings: None,
        last_update_timings: None,
        last_update_graph: None,
        last_table_changes: None,
        closed: false,
    };
    for version in 1..=32 {
        let changes = vec![FileChange {
            path: "/p/a.ts".into(),
            version,
            source: "x".repeat(1024).into_bytes(),
            deleted: false,
        }];
        session.supersede_replayed_overlays(&changes);
        session.replay_batches.push(changes);
    }

    // One batch per accepted update, because the producer advances a
    // generation per update and replay must land on the same generation.
    assert_eq!(session.replay_batches.len(), 32);
    let retained: usize = session.replay_batches.iter().map(Vec::len).sum();
    assert_eq!(
        retained, 1,
        "only the newest overlay for a path may be retained, found {retained}"
    );
    let newest = session
        .replay_batches
        .last()
        .and_then(|batch| batch.first())
        .expect("the newest batch keeps its overlay");
    assert_eq!(newest.version, 32);
}

/// A reference replacement touches only its own path's run and leaves the
/// surrounding path-sorted order intact.
#[test]
fn reference_transition_replaces_only_the_named_paths_run() {
    let mut table = table_with_symbol(SymbolFact {
        id: "shared".into(),
        alias_target: "".into(),
        declarations: Vec::new().into(),
        references: vec![
            location("a.ts", 1),
            location("b.ts", 1),
            location("c.ts", 1),
        ]
        .into(),
    });
    let transition = reference_transition(
        "shared",
        "b.ts",
        vec![location("b.ts", 4), location("b.ts", 9)],
    );
    let changes =
        apply_table_transition(&mut table, transition).expect("apply the reference transition");

    assert_eq!(table.generation(), 2);
    assert_eq!(
        table
            .symbol("shared")
            .expect("shared symbol")
            .references()
            .map(|reference| (reference.path.as_ref(), reference.start_byte))
            .collect::<Vec<_>>(),
        vec![("a.ts", 1), ("b.ts", 4), ("b.ts", 9), ("c.ts", 1)],
    );
    assert_eq!(
        changes,
        TableChanges {
            full: false,
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["shared".into()],
            file_paths: Vec::new(),
        }
    );
}

/// Rows keyed by path or id are replaced, removed, and re-sorted.
#[test]
fn keyed_rows_are_replaced_removed_and_reordered() {
    let mut table = fact_table(
        vec![
            SourceDigest {
                path: "a.ts".into(),
                sha256: SourceHash::of("a"),
            },
            SourceDigest {
                path: "b.ts".into(),
                sha256: SourceHash::of("b"),
            },
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let transition = delta_transition(
        1,
        2,
        vec![
            v3::PathOp {
                path: "a.ts".into(),
                source: SlotOp::Replace(SourceHash::of("a2")),
                entities: SlotOp::Unchanged,
                file: SlotOp::Unchanged,
            },
            v3::PathOp {
                path: "b.ts".into(),
                source: SlotOp::Remove,
                entities: SlotOp::Unchanged,
                file: SlotOp::Unchanged,
            },
        ],
        Vec::new(),
    );
    apply_table_transition(&mut table, transition).expect("apply the keyed transition");

    assert_eq!(table.source_count(), 1);
    let source = table.source("a.ts").expect("retained a.ts source");
    assert_eq!(source.path.as_ref(), "a.ts");
    assert_eq!(source.sha256, SourceHash::of("a2"));
}

#[test]
fn invalid_candidate_never_mutates_the_retained_table() {
    let retained = fact_table(
        vec![SourceDigest {
            path: "a.ts".into(),
            sha256: SourceHash::of("old"),
        }],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let transition = delta_transition(
        1,
        2,
        vec![v3::PathOp {
            path: "a.ts".into(),
            source: SlotOp::Replace(SourceHash::of("new")),
            entities: SlotOp::Unchanged,
            file: SlotOp::Unchanged,
        }],
        vec![SymbolOp::ReplaceReferencePath {
            id: "missing".into(),
            path: "a.ts".into(),
            references: vec![location("a.ts", 1)],
        }],
    );
    let mut candidate = retained.clone();
    let candidate_root = candidate.path_root_identity();
    assert!(matches!(
        apply_table_transition(&mut candidate, transition),
        Err(SessionError::InvalidResponse(_))
    ));
    assert_eq!(
        candidate.path_root_identity(),
        candidate_root,
        "a rejected transition copied storage before validation finished"
    );
    assert_eq!(candidate, retained);
    assert_eq!(
        retained.source("a.ts").expect("a.ts").sha256,
        SourceHash::of("old")
    );
    assert_eq!(retained.generation(), 1);
}

#[test]
fn sparse_transition_copies_only_the_touched_leaf() {
    let table = || {
        fact_table(
            (0..128)
                .map(|index| SourceDigest {
                    path: format!("file-{index:03}.ts").into(),
                    sha256: SourceHash::of("old"),
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    let transition = delta_transition(
        1,
        2,
        vec![v3::PathOp {
            path: "file-000.ts".into(),
            source: SlotOp::Replace(SourceHash::of("new")),
            entities: SlotOp::Unchanged,
            file: SlotOp::Unchanged,
        }],
        Vec::new(),
    );

    let mut shared = table();
    let held = shared.clone();
    let touched_leaf = shared.path_leaf_identity("file-000.ts");
    let untouched_leaf = shared.path_leaf_identity("file-127.ts");
    apply_table_transition(&mut shared, transition).unwrap();
    assert_ne!(shared.path_leaf_identity("file-000.ts"), touched_leaf);
    assert_eq!(shared.path_leaf_identity("file-127.ts"), untouched_leaf);
    assert_eq!(
        shared.source("file-000.ts").expect("new source").sha256,
        SourceHash::of("new")
    );
    assert_eq!(
        held.source("file-000.ts").expect("held source").sha256,
        SourceHash::of("old")
    );
}

#[test]
fn broad_transition_uses_one_canonical_merge() {
    const BROAD_OPERATION_COUNT: usize = 32;
    let sources = (0..64)
        .map(|index| {
            let path: Arc<str> = format!("file-{index:03}.ts").into();
            SourceDigest {
                path,
                sha256: SourceHash::of("old"),
            }
        })
        .collect::<Vec<_>>();
    let paths = (0..BROAD_OPERATION_COUNT)
        .map(|index| v3::PathOp {
            path: format!("file-{index:03}.ts").into(),
            source: SlotOp::Replace(SourceHash::of("new")),
            entities: SlotOp::Unchanged,
            file: SlotOp::Unchanged,
        })
        .collect();
    let mut table = fact_table(sources, Vec::new(), Vec::new(), Vec::new());
    let original_root = table.path_root_identity();

    apply_table_transition(&mut table, delta_transition(1, 2, paths, Vec::new()))
        .expect("apply the broad transition");

    assert_ne!(
        table.path_root_identity(),
        original_root,
        "the broad branch should build one merged index"
    );
    assert_eq!(table.source_count(), 64);
    let sources = table.sources().collect::<Vec<_>>();
    assert!(sources.windows(2).all(|pair| pair[0].path < pair[1].path));
    assert!(
        sources[..BROAD_OPERATION_COUNT]
            .iter()
            .all(|source| source.sha256 == SourceHash::of("new"))
    );
    assert!(
        sources[BROAD_OPERATION_COUNT..]
            .iter()
            .all(|source| source.sha256 == SourceHash::of("old"))
    );
}

#[test]
fn reuse_requires_the_retained_table_generation() {
    let retained = fact_table(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let valid = response(1, "successor", Vec::new());
    let mut valid_retained = Some(retained.clone());
    let (reused, changes) =
        prepare_analyze_response(&valid, "/p/tsconfig.json", 1, &mut valid_retained, "base")
            .expect("reuse the retained table");
    assert_eq!(reused, retained);
    assert!(changes.unchanged);

    let invalid = response(2, "successor", Vec::new());
    let mut invalid_retained = Some(retained.clone());
    assert!(matches!(
        prepare_analyze_response(
            &invalid,
            "/p/tsconfig.json",
            2,
            &mut invalid_retained,
            "base"
        ),
        Err(SessionError::InvalidResponse(_))
    ));
    assert_eq!(invalid_retained.as_ref(), Some(&retained));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransitionGoldenStep {
    label: String,
    base_token: String,
    #[serde(with = "serde_bytes")]
    base_transition: Vec<u8>,
    #[serde(with = "serde_bytes")]
    transition: Vec<u8>,
    #[serde(with = "serde_bytes")]
    expected_transition: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransitionGolden {
    steps: Vec<TransitionGoldenStep>,
}

/// The production Go encoder and this decoder/applier must assign exactly
/// the same meaning to full and delta transitions.
#[test]
fn applies_the_producers_transitions_exactly() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../benchmarks/typefacts/phase1/typefacts-v5-transition-golden.cbor");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let golden: TransitionGolden = crate::decode(&bytes).expect("decode the transition golden");
    assert_eq!(golden.steps.len(), 4, "fixture lost a transition");

    for step in &golden.steps {
        let mut actual = materialize_full(&step.base_transition, &step.label);
        let expected = materialize_full(&step.expected_transition, &step.label);
        let transition = v3::decode_table_transition(&step.transition)
            .unwrap_or_else(|error| panic!("{}: decode delta: {error}", step.label));
        assert_eq!(transition.mode, TransitionMode::Delta, "{}", step.label);
        assert_eq!(
            transition.base_state_token.as_ref(),
            step.base_token,
            "{}",
            step.label
        );
        assert_eq!(
            actual.generation(),
            transition.base_generation,
            "{}",
            step.label
        );
        apply_table_transition(&mut actual, transition)
            .unwrap_or_else(|error| panic!("{}: apply delta: {error}", step.label));
        assert_eq!(actual, expected, "{} produced the wrong table", step.label);
    }
}

/// Once the reader has failed what was pending it is gone, so a request
/// registered afterwards would wait forever. Registration must refuse it with
/// the reader's failure, which the session treats as a transport failure and
/// recovers from by restarting the producer.
#[test]
fn a_request_registered_after_the_reader_failed_is_refused() {
    let pending = PendingResponses::default();
    let (early, early_response) = mpsc::sync_channel(1);
    pending.lock().unwrap().register(1, early).unwrap();

    fail_pending(&pending, "producer closed its output".into());
    assert_eq!(
        early_response.recv().unwrap().unwrap_err(),
        "producer closed its output"
    );

    let (late, late_response) = mpsc::sync_channel(1);
    assert_eq!(
        pending.lock().unwrap().register(2, late),
        Err("producer closed its output".into())
    );
    assert!(
        late_response.recv().is_err(),
        "nobody holds the refused sender"
    );
}

/// A reader that unwinds never reaches its own failure path, so the guard it
/// holds must fail what is pending and refuse what comes after.
#[test]
fn a_reader_that_panics_still_fails_pending_requests() {
    let pending = PendingResponses::default();
    let (early, early_response) = mpsc::sync_channel(1);
    pending.lock().unwrap().register(1, early).unwrap();

    let reader_pending = Arc::clone(&pending);
    let reader = std::thread::spawn(move || {
        let _exit = ReaderExit(reader_pending);
        panic!("reader unwinds");
    });
    assert!(reader.join().is_err());

    assert_eq!(
        early_response.recv().unwrap().unwrap_err(),
        "producer response reader stopped"
    );
    let (late, _late_response) = mpsc::sync_channel(1);
    assert!(pending.lock().unwrap().register(2, late).is_err());
}
