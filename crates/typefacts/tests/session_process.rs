use std::{fs, path::PathBuf, process::Command, sync::OnceLock};

use typefacts::{
    AnalysisDemand, ArrayShape, CallKind, Callability, ConstantValue, ConstantValueKind,
    DemandGroup, Location, Producer, ReferenceSpace, ResolvedCallValidity, RuntimeValueDomain,
    Session,
    v3::{EntityDemand, FileChange},
};

/// The fact's reason for existing, end to end: an aliased tuple renders as its
/// alias, so no text test can see the tuple. The alias also lives in another
/// file, which makes the delta leg a test of the recorded dependency — an edit
/// to the alias must re-derive the shape rather than reuse a stale row.
#[test]
fn array_shape_follows_a_cross_file_alias_through_full_delta_and_reuse() {
    let root = std::env::temp_dir().join(format!("typefacts-array-shape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project = root.join("tsconfig.json");
    fs::write(
        &project,
        r#"{"compilerOptions":{"strict":true,"noEmit":true},"include":["*.ts"]}"#,
    )
    .unwrap();
    let types = root.join("types.ts");
    fs::write(
        &types,
        "export type Handlers = [(n: number) => void, number];\n",
    )
    .unwrap();
    let path = root.join("source.ts");
    let source = concat!(
        "import type { Handlers } from \"./types\";\n",
        "declare const pair: Handlers;\n",
        "export const used = pair;\n",
    );
    fs::write(&path, source).unwrap();
    let start = source.find("pair;").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: path.to_string_lossy().into_owned().into(),
            start_byte: start as u64,
            end_byte: (start + "pair".len()) as u64,
        },
        array_shape: true,
        ..EntityDemand::default()
    };
    let analysis = || AnalysisDemand {
        entities: vec![demand.clone()],
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let full = session.analyze(&analysis()).unwrap();
    assert_eq!(
        full.entities().next().unwrap().array_shape,
        Some(ArrayShape::Array)
    );
    let reused = session.analyze(&analysis()).unwrap();
    assert_eq!(reused.entities().next(), full.entities().next());
    assert!(session.take_last_table_changes().unwrap().unchanged);

    // The alias becomes a function type. Nothing in source.ts changed, so a
    // fact that did not record its dependency would answer Array forever.
    session
        .update([FileChange {
            path: types.to_string_lossy().into_owned(),
            source: b"export type Handlers = (n: number) => void;\n".to_vec(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let delta = session.analyze(&analysis()).unwrap();
    assert_eq!(
        delta.entities().next().unwrap().array_shape,
        Some(ArrayShape::NotArray)
    );

    session.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn constant_value_survives_full_delta_and_reuse_responses() {
    let root =
        std::env::temp_dir().join(format!("typefacts-constant-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project = root.join("tsconfig.json");
    fs::write(
        &project,
        r#"{"compilerOptions":{"strict":true,"noEmit":true},"include":["*.ts"]}"#,
    )
    .unwrap();
    let path = root.join("source.ts");
    let source = "export const value = \"a\" + \"b\";\n";
    fs::write(&path, source).unwrap();
    let start = source.find("\"a\" + \"b\"").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: path.to_string_lossy().into_owned().into(),
            start_byte: start as u64,
            end_byte: (start + "\"a\" + \"b\"".len()) as u64,
        },
        constant_value: true,
        ..EntityDemand::default()
    };
    let analysis = || AnalysisDemand {
        entities: vec![demand.clone()],
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let full = session.analyze(&analysis()).unwrap();
    assert_eq!(
        full.entities().next().unwrap().constant_value,
        Some(ConstantValue {
            kind: ConstantValueKind::String,
            string: "ab".into(),
            number: 0.0,
        })
    );
    let reused = session.analyze(&analysis()).unwrap();
    assert_eq!(reused.entities().next(), full.entities().next());
    assert!(session.take_last_table_changes().unwrap().unchanged);

    session
        .update([FileChange {
            path: path.to_string_lossy().into_owned(),
            source: b"export const value = \"c\" + \"d\";\n".to_vec(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let delta = session.analyze(&analysis()).unwrap();
    assert_eq!(
        delta.entities().next().unwrap().constant_value,
        Some(ConstantValue {
            kind: ConstantValueKind::String,
            string: "cd".into(),
            number: 0.0,
        })
    );

    session.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_unresolved_symbols_survive_the_process_seam() {
    let root = std::env::temp_dir().join(format!(
        "typefacts-unresolved-symbol-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project = root.join("tsconfig.json");
    fs::write(
        &project,
        r#"{"compilerOptions":{"strict":true,"noEmit":true},"include":["*.ts"]}"#,
    )
    .unwrap();
    let path = root.join("source.ts");
    let source = "export const present = 1;\nexport const missing = MissingName;\n";
    fs::write(&path, source).unwrap();
    let demand = |needle: &str, from: usize| {
        let start = source[from..].find(needle).unwrap() + from;
        EntityDemand {
            location: Location {
                path: path.to_string_lossy().into_owned().into(),
                start_byte: start as u64,
                end_byte: (start + needle.len()) as u64,
            },
            symbol: true,
            ..EntityDemand::default()
        }
    };
    let demands = vec![
        demand("present", source.find("present").unwrap()),
        demand("MissingName", 0),
    ];
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let facts = session
        .analyze(&AnalysisDemand { entities: demands })
        .unwrap();
    let entities = facts.entities().collect::<Vec<_>>();
    assert!(!entities[0].symbol.is_empty());
    assert!(!entities[0].symbol_unresolved);
    assert!(entities[1].symbol.is_empty());
    assert!(entities[1].symbol_unresolved);
    session.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exhaustive_call_target_sets_survive_full_delta_and_reuse_responses() {
    let project = repository_root()
        .join("internal/typefacts/testdata/call-targets/tsconfig.json")
        .canonicalize()
        .unwrap();
    let path = project.parent().unwrap().join("dispatch.ts");
    let source = fs::read_to_string(&path).unwrap();
    let start = source.find("dispatch(\"value\")").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: path.to_string_lossy().into_owned().into(),
            start_byte: start as u64,
            end_byte: (start + "dispatch(\"value\")".len()) as u64,
        },
        resolved_call: true,
        ..EntityDemand::default()
    };
    let analysis = || AnalysisDemand {
        entities: vec![demand.clone()],
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let full = session.analyze(&analysis()).unwrap();
    let entity = full.entities().next().unwrap();
    let call = entity.resolved_call.as_ref().unwrap();
    assert_eq!(call.validity, ResolvedCallValidity::Valid);
    assert!(call.declaration.is_none());
    let targets = call.targets.as_ref().expect("exhaustive target set");
    assert!(targets.exhaustive);
    let names = targets
        .candidates
        .iter()
        .map(|candidate| candidate.name.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(names, ["implA", "implB"]);
    assert!(
        targets
            .candidates
            .iter()
            .all(|candidate| !candidate.symbol.is_empty()
                && candidate.kind.as_ref() == "FunctionDeclaration")
    );

    let reused = session.analyze(&analysis()).unwrap();
    assert_eq!(reused.entities().next(), full.entities().next());
    assert!(session.take_last_table_changes().unwrap().unchanged);

    // Replacing one implementation with a structural function type keeps the
    // union composite but voids the exhaustiveness proof: no candidate set.
    session
        .update([FileChange {
            path: path.to_string_lossy().into_owned(),
            source: source
                .replace(
                    "const dispatch = cond ? implA : implB;",
                    "declare const external: { (value: string): \"x\" };\nconst dispatch = cond ? implA : external;",
                )
                .into_bytes(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let delta_demand = EntityDemand {
        location: Location {
            path: path.to_string_lossy().into_owned().into(),
            start_byte: (start
                + "declare const external: { (value: string): \"x\" };\n".len()
                + "const dispatch = cond ? implA : external;".len()
                - "const dispatch = cond ? implA : implB;".len()) as u64,
            end_byte: (start
                + "declare const external: { (value: string): \"x\" };\n".len()
                + "const dispatch = cond ? implA : external;".len()
                - "const dispatch = cond ? implA : implB;".len()
                + "dispatch(\"value\")".len()) as u64,
        },
        resolved_call: true,
        ..EntityDemand::default()
    };
    let delta = session
        .analyze(&AnalysisDemand {
            entities: vec![delta_demand],
        })
        .unwrap();
    let delta_call = delta
        .entities()
        .next()
        .unwrap()
        .resolved_call
        .as_ref()
        .cloned()
        .expect("composite call keeps a resolved-call fact");
    assert!(
        delta_call.targets.is_none(),
        "structural constituent must void the exhaustive candidate set: {:?}",
        delta_call.targets
    );
    session.close().unwrap();
}

#[test]
fn runtime_value_domain_survives_full_delta_and_reuse_responses() {
    let project = repository_root()
        .join("internal/typefacts/testdata/runtime-value-domain/tsconfig.json")
        .canonicalize()
        .unwrap();
    let path = project.parent().unwrap().join("domains.ts");
    let source = fs::read_to_string(&path).unwrap();
    let start = source.find("cleanupValue").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: path.to_string_lossy().into_owned().into(),
            start_byte: start as u64,
            end_byte: (start + "cleanupValue".len()) as u64,
        },
        runtime_value_domain: true,
        ..EntityDemand::default()
    };
    let analysis = || AnalysisDemand {
        entities: vec![demand.clone()],
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let full = session.analyze(&analysis()).unwrap();
    assert_eq!(
        full.entities().next().unwrap().runtime_value_domain,
        Some(RuntimeValueDomain {
            may_be_callable: true,
            may_be_undefined: true,
            may_be_other: false,
            unknown: false,
        })
    );

    let reused = session.analyze(&analysis()).unwrap();
    assert_eq!(reused.entities().next(), full.entities().next());
    assert!(session.take_last_table_changes().unwrap().unchanged);

    session
        .update([FileChange {
            path: path.to_string_lossy().into_owned(),
            source: b"export const cleanupValue = null as (() => void) | number;\n".to_vec(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let delta = session.analyze(&analysis()).unwrap();
    assert_eq!(
        delta.entities().next().unwrap().runtime_value_domain,
        Some(RuntimeValueDomain {
            may_be_callable: true,
            may_be_undefined: false,
            may_be_other: true,
            unknown: false,
        })
    );
    assert!(
        session
            .take_last_table_changes()
            .unwrap()
            .entity_paths
            .iter()
            .any(|changed| changed == path.to_string_lossy().as_ref())
    );

    let delta_reused = session.analyze(&analysis()).unwrap();
    assert_eq!(delta_reused.entities().next(), delta.entities().next());
    assert!(session.take_last_table_changes().unwrap().unchanged);
    session.close().unwrap();
}

#[test]
fn call_result_domain_survives_the_process_seam() {
    let project = repository_root()
        .join("internal/typefacts/testdata/call-result-domain/tsconfig.json")
        .canonicalize()
        .unwrap();
    let path = project.parent().unwrap().join("calls.ts");
    let source = fs::read_to_string(&path).unwrap();
    let demand = |needle: &str| {
        let start = source.rfind(needle).unwrap();
        EntityDemand {
            location: Location {
                path: path.to_string_lossy().into_owned().into(),
                start_byte: start as u64,
                end_byte: (start + needle.len()) as u64,
            },
            call_result_domain: true,
            ..EntityDemand::default()
        }
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let facts = session
        .analyze(&AnalysisDemand {
            entities: vec![
                demand("makeCount()"),
                demand("makeThunk()"),
                demand("make()"),
            ],
        })
        .unwrap();
    let domains = facts
        .entities()
        .map(|entity| entity.call_result_domain)
        .collect::<Vec<_>>();
    assert_eq!(
        domains,
        vec![
            Some(RuntimeValueDomain {
                may_be_callable: false,
                may_be_undefined: false,
                may_be_other: true,
                unknown: false,
            }),
            Some(RuntimeValueDomain {
                may_be_callable: true,
                may_be_undefined: false,
                may_be_other: false,
                unknown: false,
            }),
            Some(RuntimeValueDomain {
                may_be_callable: true,
                may_be_undefined: true,
                may_be_other: false,
                unknown: false,
            }),
        ]
    );
    let reused = session
        .analyze(&AnalysisDemand {
            entities: vec![demand("makeCount()")],
        })
        .unwrap();
    assert_eq!(
        reused.entities().next().unwrap().call_result_domain,
        domains[0]
    );
    session.close().unwrap();
}

#[test]
fn shared_transition_arena_matches_the_inline_process_adapter() {
    let project = project();
    let use_path = project.parent().unwrap().join("use.ts");
    let source = fs::read_to_string(&use_path).unwrap();
    let import_start = source.find("localCount").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: use_path.to_string_lossy().into_owned().into(),
            start_byte: import_start as u64,
            end_byte: (import_start + "localCount".len()) as u64,
        },
        symbol: true,
        references: true,
        ..EntityDemand::default()
    };
    let mut inline = Session::open(
        Producer::at(producer()).without_shared_transition_arena(),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let mut shared = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let expected = inline
        .analyze(&AnalysisDemand {
            entities: vec![demand.clone()],
        })
        .unwrap();
    let actual = shared
        .analyze(&AnalysisDemand {
            entities: vec![demand.clone()],
        })
        .unwrap();
    assert_eq!(
        actual.entities().collect::<Vec<_>>(),
        expected.entities().collect::<Vec<_>>()
    );

    let unrelated_path = project.parent().unwrap().join("unrelated.ts");
    let change = FileChange {
        path: unrelated_path.to_string_lossy().into_owned(),
        source: fs::read(&unrelated_path).unwrap(),
        deleted: false,
        version: 1,
    };
    inline.update([change.clone()]).unwrap();
    shared.update([change]).unwrap();
    let expected = inline
        .analyze(&AnalysisDemand {
            entities: vec![demand.clone()],
        })
        .unwrap();
    let actual = shared
        .analyze(&AnalysisDemand {
            entities: vec![demand],
        })
        .unwrap();
    assert_eq!(
        actual.entities().collect::<Vec<_>>(),
        expected.entities().collect::<Vec<_>>()
    );
    assert_eq!(
        shared.take_last_table_changes(),
        inline.take_last_table_changes()
    );
}

#[test]
fn rust_client_consumes_compiler_semantic_facts_across_retained_updates() {
    let project = project();
    let use_path = project.parent().unwrap().join("use.ts");
    let source = fs::read_to_string(&use_path).unwrap();
    let import_start = source.find("localCount").unwrap();
    let call_start = source.rfind("localCount()").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: use_path.to_string_lossy().into_owned().into(),
            start_byte: import_start as u64,
            end_byte: (import_start + "localCount".len()) as u64,
        },
        query_location: Some(Location {
            path: use_path.to_string_lossy().into_owned().into(),
            start_byte: call_start as u64,
            end_byte: (call_start + "localCount()".len()) as u64,
        }),
        symbol: true,
        resolved_call: true,
        callability: true,
        reference_space: true,
        runtime_identity: true,
        ..EntityDemand::default()
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let first = session
        .analyze(&AnalysisDemand {
            entities: vec![demand.clone()],
        })
        .unwrap();
    let entity = first.entities().next().expect("one demanded entity");
    // Callability classifies the complete query expression. `localCount()`
    // returns a number even though resolved-call selection finds its callee.
    assert_eq!(entity.callability, Some(Callability::NonCallable));
    assert_eq!(entity.reference_space, Some(ReferenceSpace::Value));
    assert!(entity.runtime_identity.starts_with("runtime:h:"));
    let resolved = entity.resolved_call.as_ref().unwrap();
    assert_eq!(resolved.validity, ResolvedCallValidity::Valid);
    assert_eq!(resolved.kind, CallKind::Call);
    let declaration = resolved
        .declaration
        .as_ref()
        .expect("valid call carries its selected declaration");
    assert_eq!(declaration.name.as_ref(), "count");
    assert!(!declaration.standard_library);
    assert!(resolved.arguments.is_empty());

    let unrelated_path = project.parent().unwrap().join("unrelated.ts");
    session
        .update([FileChange {
            path: unrelated_path.to_string_lossy().into_owned(),
            source: fs::read(&unrelated_path).unwrap(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let second = session
        .analyze(&AnalysisDemand {
            entities: vec![demand],
        })
        .unwrap();
    assert_eq!(second.entities().next(), first.entities().next());
    session.close().unwrap();
}

#[test]
fn rust_owns_alias_and_reference_closure() {
    let project = project();
    let use_path = project.parent().unwrap().join("use.ts");
    let source = fs::read_to_string(&use_path).unwrap();
    let import_start = source.find("localCount").unwrap();
    let demand = EntityDemand {
        location: Location {
            path: use_path.to_string_lossy().into_owned().into(),
            start_byte: import_start as u64,
            end_byte: (import_start + "localCount".len()) as u64,
        },
        symbol: true,
        references: true,
        ..EntityDemand::default()
    };
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let first = session
        .analyze(&AnalysisDemand {
            entities: vec![demand.clone()],
        })
        .unwrap();
    let alias_id = first.entities().next().unwrap().symbol.as_ref();
    let alias = first
        .symbol(alias_id)
        .expect("Rust closed the demanded alias");
    assert!(!alias.alias_target().is_empty());
    let canonical = first
        .symbol(alias.alias_target())
        .expect("Rust followed the alias target");
    assert!(!canonical.references().collect::<Vec<_>>().is_empty());

    let unrelated_path = project.parent().unwrap().join("unrelated.ts");
    session
        .update([FileChange {
            path: unrelated_path.to_string_lossy().into_owned(),
            source: fs::read(&unrelated_path).unwrap(),
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let second = session
        .analyze(&AnalysisDemand {
            entities: vec![demand],
        })
        .unwrap();
    assert_eq!(
        second
            .symbol(alias_id)
            .expect("alias survives unrelated update")
            .alias_target(),
        alias.alias_target()
    );
    assert!(
        session
            .take_last_table_changes()
            .expect("incremental changes")
            .symbol_ids
            .is_empty()
    );
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn producer() -> PathBuf {
    static PRODUCER: OnceLock<PathBuf> = OnceLock::new();
    PRODUCER
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("TYPEFACTS_TEST_BIN") {
                return PathBuf::from(path);
            }
            let output = repository_root()
                .join("target/typefacts-test")
                .join(if cfg!(windows) {
                    "solid-typefacts.exe"
                } else {
                    "solid-typefacts"
                });
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            let ldflags = format!("-X main.buildID={}", typefacts::v3::TYPE_FACTS_BUILD_ID);
            let status = Command::new("go")
                .current_dir(repository_root())
                .args(["build", "-ldflags", &ldflags, "-o"])
                .arg(&output)
                .arg("./cmd/solid-typefacts")
                .status()
                .expect("run go build for the session process test");
            assert!(status.success(), "build the Type Facts test producer");
            output
        })
        .clone()
}

fn project() -> PathBuf {
    repository_root()
        .join("internal/typefacts/testdata/aliased-import/tsconfig.json")
        .canonicalize()
        .unwrap()
}

#[test]
fn public_session_owns_the_retained_process_lifecycle() {
    let producer = producer();
    assert!(
        producer.is_file(),
        "build the test producer at {} or set TYPEFACTS_TEST_BIN",
        producer.display()
    );
    let project = project();

    let mut session = Session::open(
        Producer::at(producer),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let sources = session.configured_sources().unwrap();
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("consumer.ts"))
    );

    let first = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(first.generation(), 1);
    assert_eq!(first.project_id(), project.to_string_lossy());
    let timings = session.take_last_exchange_timings().unwrap();
    assert!(!timings.roundtrip.is_zero());
    assert!(timings.response_bytes > 0);
    assert!(timings.server_materialized);
    assert!(session.take_last_table_changes().is_some());

    let changed_path = project.parent().unwrap().join("unrelated.ts");
    let changed_source = fs::read(&changed_path).unwrap();
    session
        .update([FileChange {
            path: changed_path.to_string_lossy().into_owned(),
            source: changed_source,
            deleted: false,
            version: 1,
        }])
        .unwrap();
    let second = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(second.generation(), 2);
    assert!(session.take_last_exchange_timings().is_some());
    assert!(session.take_last_table_changes().is_some());
    session.close().unwrap();
}

#[cfg(unix)]
#[test]
fn analyze_restarts_the_producer_and_replays_updates_after_a_crash() {
    let directory =
        std::env::temp_dir().join(format!("typefacts-session-crash-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let pid_path = directory.join("producer.pid");
    let wrapper = directory.join("producer");
    // The wrapper reports the pid the session is talking to, then becomes the
    // real producer, so killing that pid kills the session's own process.
    fs::write(
        &wrapper,
        format!(
            "printf '%s' \"$$\" > '{}'\nexec '{}' \"$@\"\n",
            pid_path.display(),
            producer().display()
        ),
    )
    .unwrap();

    let project = project();
    // The shell is handed the wrapper to *read* rather than being made
    // executable for the kernel to exec. A sibling test spawning a producer
    // concurrently forks a copy of this file's still-open write descriptor, and
    // until that child reaches its own exec the kernel refuses to exec a file
    // that is open for writing (ETXTBSY). Reading a script has no such rule.
    let mut session = Session::open(
        Producer::at("/bin/sh").with_arg(&wrapper),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    let changed_path = project.parent().unwrap().join("unrelated.ts");
    session
        .update([FileChange {
            path: changed_path.to_string_lossy().into_owned(),
            source: fs::read(&changed_path).unwrap(),
            deleted: false,
            version: 1,
        }])
        .unwrap();

    let pid = fs::read_to_string(&pid_path).unwrap();
    assert!(
        Command::new("kill")
            .args(["-9", &pid])
            .status()
            .unwrap()
            .success()
    );
    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation(), 2);
    session.close().unwrap();

    fs::remove_file(wrapper).unwrap();
    fs::remove_file(pid_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

/// Builds `count` demand runs, one per path, each with one symbol demand.
fn synthetic_groups(base: &str, count: usize) -> Vec<Vec<EntityDemand>> {
    (0..count)
        .map(|index| {
            vec![EntityDemand {
                location: Location {
                    path: format!("{base}/file{index:04}.ts").into(),
                    start_byte: 0,
                    end_byte: 1,
                },
                symbol: true,
                ..EntityDemand::default()
            }]
        })
        .collect()
}

fn borrow(runs: &[Vec<EntityDemand>]) -> Vec<DemandGroup<'_>> {
    runs.iter()
        .map(|run| DemandGroup::new(run).expect("non-empty run"))
        .collect()
}

/// The grouped interface must transmit work proportional to what changed, not to
/// how much the caller is watching. Request size is the observable: the producer
/// only ever receives the demands the session chose to send.
#[test]
fn grouped_analysis_transmits_only_what_changed() {
    const GROUPS: usize = 1_000;
    let project = project();
    let base = project.parent().unwrap().to_string_lossy().into_owned();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let runs = synthetic_groups(&base, GROUPS);
    let groups = borrow(&runs);

    // Cold: the complete demand set crosses the wire.
    session.analyze_groups(&groups).unwrap();
    let cold = session.take_last_exchange_timings().unwrap().request_bytes;

    // Unchanged: an empty demand delta.
    session.analyze_groups(&groups).unwrap();
    let unchanged = session.take_last_exchange_timings().unwrap().request_bytes;
    assert!(
        unchanged * 20 < cold,
        "an unchanged demand set still sent {unchanged} of {cold} bytes; the delta should be empty"
    );

    // One of a thousand groups changes.
    let mut edited = runs.clone();
    edited[500][0].references = true;
    let mut changed_groups = borrow(&runs);
    changed_groups[500] = DemandGroup::new(&edited[500]).unwrap();
    session.analyze_groups(&changed_groups).unwrap();
    let one_changed = session.take_last_exchange_timings().unwrap().request_bytes;
    assert!(
        one_changed > unchanged,
        "changing a group sent no more than an unchanged analysis ({one_changed} vs {unchanged})"
    );
    assert!(
        one_changed * 20 < cold,
        "changing 1 of {GROUPS} groups sent {one_changed} of {cold} bytes; only the changed group should travel"
    );

    // Dropping a group reports exactly that path.
    let fewer = borrow(&runs[..GROUPS - 1]);
    session.analyze_groups(&fewer).unwrap();
    let removed = session.take_last_exchange_timings().unwrap().request_bytes;
    assert!(
        removed * 20 < cold,
        "removing one group sent {removed} of {cold} bytes; only the removed path should travel"
    );

    eprintln!(
        "grouped request bytes: cold={cold} unchanged={unchanged} one_changed={one_changed} removed={removed}"
    );
    session.close().unwrap();
}

/// The flat interface is a compatibility wrapper, so it must agree with the
/// grouped one fact for fact rather than being a second implementation.
#[test]
fn grouped_and_flat_analysis_agree() {
    let project = project();
    let base = project.parent().unwrap().to_string_lossy().into_owned();
    let runs = synthetic_groups(&base, 8);
    let flat = AnalysisDemand {
        entities: runs.iter().flatten().cloned().collect(),
    };

    let open = || {
        Session::open(
            Producer::at(producer()),
            project.to_string_lossy(),
            Vec::new(),
        )
        .unwrap()
    };

    let mut grouped_session = open();
    let grouped_table = grouped_session.analyze_groups(&borrow(&runs)).unwrap();
    grouped_session.close().unwrap();

    let mut flat_session = open();
    let flat_table = flat_session.analyze(&flat).unwrap();
    flat_session.close().unwrap();

    assert_eq!(
        grouped_table, flat_table,
        "the flat wrapper and the grouped interface produced different tables"
    );
}

/// A group whose demands point outside its own file would corrupt retained state
/// silently, so it is rejected rather than accepted and mis-keyed.
#[test]
fn a_group_carrying_a_foreign_location_is_rejected() {
    let project = project();
    let base = project.parent().unwrap().to_string_lossy().into_owned();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let mixed = vec![
        EntityDemand {
            location: Location {
                path: format!("{base}/here.ts").into(),
                start_byte: 0,
                end_byte: 1,
            },
            symbol: true,
            ..EntityDemand::default()
        },
        EntityDemand {
            location: Location {
                path: format!("{base}/elsewhere.ts").into(),
                start_byte: 0,
                end_byte: 1,
            },
            symbol: true,
            ..EntityDemand::default()
        },
    ];
    let error = session
        .analyze_groups(&[DemandGroup::new(&mixed).unwrap()])
        .expect_err("a group with a foreign location must be rejected");
    assert!(
        error.to_string().contains("elsewhere.ts"),
        "the rejection should name the offending location, got: {error}"
    );

    // Duplicated paths would silently overwrite one another in retained state.
    let duplicate = synthetic_groups(&base, 1);
    let repeated = [
        DemandGroup::new(&duplicate[0]).unwrap(),
        DemandGroup::new(&duplicate[0]).unwrap(),
    ];
    let error = session
        .analyze_groups(&repeated)
        .expect_err("a repeated path must be rejected");
    assert!(error.to_string().contains("twice"), "got: {error}");

    session.close().unwrap();
}

fn touch_source(project: &std::path::Path, version: u64) -> FileChange {
    let path = project.parent().unwrap().join("unrelated.ts");
    FileChange {
        path: path.to_string_lossy().into_owned(),
        source: fs::read(&path).unwrap(),
        deleted: false,
        version,
    }
}

/// The overlap must not cost correctness: the caller's value comes back, the
/// generation advances exactly once, and analysis sees the new generation.
#[test]
fn update_during_returns_the_work_and_advances_one_generation() {
    let project = project();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    session.analyze(&AnalysisDemand::default()).unwrap();

    let carried = session
        .update_during([touch_source(&project, 1)], || "local work result")
        .unwrap();
    assert_eq!(carried, "local work result");
    assert_eq!(session.generation(), 2);
    // The wait is reported separately so a caller can see whether it overlapped.
    assert!(session.take_last_update_timings().is_some());

    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(
        facts.generation(),
        2,
        "analysis must see the acknowledged generation"
    );
    session.close().unwrap();
}

/// A caller whose local work fails must still leave the session synchronised:
/// the update was already sent, so abandoning it would desync the generation.
#[test]
fn update_during_finishes_the_update_when_work_fails() {
    let project = project();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    session.analyze(&AnalysisDemand::default()).unwrap();

    let outcome: Result<(), &str> = session
        .update_during([touch_source(&project, 1)], || Err("local analysis failed"))
        .unwrap();
    assert_eq!(outcome, Err("local analysis failed"));
    assert_eq!(
        session.generation(),
        2,
        "a failed caller must not cost the session its acknowledgement"
    );
    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation(), 2);
    session.close().unwrap();
}

/// Same invariant under the harshest early exit. A panic that unwound past the
/// wait would leave the session one generation behind the producer, and every
/// later request would fail the generation check.
#[test]
fn update_during_finishes_the_update_when_work_panics() {
    let project = project();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    session.analyze(&AnalysisDemand::default()).unwrap();

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: () = session
            .update_during([touch_source(&project, 1)], || {
                panic!("local work exploded")
            })
            .unwrap();
    }));
    std::panic::set_hook(previous);
    assert!(
        panicked.is_err(),
        "the caller's panic must still reach the caller"
    );

    assert_eq!(
        session.generation(),
        2,
        "the acknowledgement must land even when the caller panics"
    );
    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation(), 2, "the session must still be usable");
    session.close().unwrap();
}

/// A producer that dies after the update is written but before it answers is the
/// case pipelining newly exposes: the session must restart, replay, re-send this
/// update exactly once, and land on one new generation.
#[cfg(unix)]
#[test]
fn update_during_recovers_when_the_producer_dies_before_acknowledging() {
    use std::os::unix::fs::PermissionsExt;

    let directory =
        std::env::temp_dir().join(format!("typefacts-update-crash-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let marker = directory.join("crash-before-update");
    let wrapper = directory.join("producer");
    // The producer consumes the marker and exits on the first update it sees, so
    // the replacement it is restarted as runs normally.
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nSOLID_TYPEFACTS_CRASH_BEFORE_UPDATE='{}' exec '{}' \"$@\"\n",
            marker.display(),
            producer().display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(&marker, b"crash").unwrap();

    let project = project();
    let mut session = Session::open(
        Producer::at(&wrapper),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();

    let carried = session
        .update_during([touch_source(&project, 1)], || 7_u32)
        .expect("the session must recover from a producer that died mid-update");
    assert_eq!(
        carried, 7,
        "the caller's work is unaffected by the recovery"
    );
    assert_eq!(
        session.generation(),
        2,
        "the replayed update must advance exactly one generation, not zero or two"
    );

    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation(), 2);
    session.close().unwrap();

    // Non-vacuity: the producer consumes the marker as it dies, so a surviving
    // marker would mean this test never exercised the recovery path at all.
    assert!(
        !marker.exists(),
        "the producer never consumed the crash marker, so no mid-update failure occurred"
    );
    fs::remove_file(wrapper).unwrap();
    fs::remove_dir(directory).unwrap();
}

/// Cancellation targets the active analysis. It must not be able to strand a
/// sent update: by the time analyze exists to cancel, the update is acknowledged.
#[test]
fn cancellation_cannot_strand_a_sent_update() {
    let project = project();
    let mut session = Session::open(
        Producer::at(producer()),
        project.to_string_lossy(),
        Vec::new(),
    )
    .unwrap();
    session.analyze(&AnalysisDemand::default()).unwrap();
    let cancellation = session.cancellation_handle().unwrap();

    // Cancelling from inside the caller's work cannot reach the update: only an
    // analysis is cancellable, and none is in flight here.
    session
        .update_during([touch_source(&project, 1)], || {
            cancellation.cancel_active().unwrap()
        })
        .unwrap();
    assert_eq!(
        session.generation(),
        2,
        "a cancellation during the caller's work must not abandon the update"
    );
    let facts = session.analyze(&AnalysisDemand::default()).unwrap();
    assert_eq!(facts.generation(), 2);
    session.close().unwrap();
}
