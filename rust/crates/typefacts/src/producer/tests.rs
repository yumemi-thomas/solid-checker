//! Ports of the Go producer's transition tests (`wire_transition_test.go`,
//! `wire_transition_golden_test.go`, `wire_transition_codec_test.go`,
//! `transport_manifest_test.go`, `protocolv3_golden_test.go`) plus
//! encoder/decoder agreement over every field of every row.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::manifest::clean_path;
use super::plan::{SymbolBody, plan_exact_symbol_delta};
use super::*;
use crate::v3::{self, Response, TransitionMode};
use crate::{
    ArgumentMapping, ArgumentMappingReason, ArgumentMappingStatus, ArrayShape, AsyncFunctionFact,
    CallKind, CallTargetSet, Callability, ConstantValue, ConstantValueKind, Constructability,
    ConstructionWitness, Declaration, DeclarationOwner, EntityFact, FileFact, Location,
    ObjectConstructionProperty, ObjectConstructionShape, ParameterFact, PrimitiveLiteralCandidate,
    PrimitiveLiteralKind, PrimitiveValueDomain, ReferenceSpace, ResolvedCall, ResolvedCallValidity,
    ResolvedDeclaration, RuntimeBindingKind, RuntimeValueDomain, SourceBinding, SourceCall,
    SourceFunction, SymbolClaim, SymbolFact, TupleShape, TypeDescriptor,
};

const V3: u64 = v3::TYPE_FACTS_TABLE_SCHEMA_V3;

fn location(path: &str, start: u64, end: u64) -> Location {
    Location {
        path: path.into(),
        start_byte: start,
        end_byte: end,
    }
}

fn entity(path: &str, start: u64, symbol: &str) -> EntityFact {
    EntityFact {
        location: location(path, start, start + 1),
        symbol: symbol.into(),
        symbol_claim: if symbol.is_empty() {
            SymbolClaim::Absent
        } else {
            SymbolClaim::Partial
        },
        type_descriptor: None,
        resolved_call: None,
        callability: None,
        constructability: None,
        runtime_binding_kind: None,
        runtime_value_domain: None,
        primitive_value_domain: PrimitiveValueDomain::default(),
        primitive_literal_candidates: None,
        call_result_domain: None,
        constant_value: None,
        array_shape: None,
        tuple_shape: None,
        library_types: None,
        reference_space: None,
        runtime_identity: "".into(),
        declared_keys: None,
    }
}

fn symbol(id: &str) -> SymbolFact {
    SymbolFact {
        id: id.into(),
        alias_target: "".into(),
        declarations: Arc::from(Vec::new()),
        references: Arc::from(Vec::new()),
    }
}

fn file(path: &str) -> FileFact {
    FileFact {
        path: path.into(),
        calls: Arc::from(Vec::new()),
        bindings: Arc::from(Vec::new()),
        functions: Arc::from(Vec::new()),
        async_functions: Arc::from(Vec::new()),
    }
}

fn call_to(target: &str) -> SourceCall {
    SourceCall {
        location: Location::default(),
        callee: Location::default(),
        arguments: Vec::new(),
        target: target.into(),
    }
}

fn table(generation: u64, state_id: u64) -> ProducerTable {
    let mut table = ProducerTable::new(generation, "/p/tsconfig.json");
    table.state_id = state_id;
    table
}

// -- Go: goldenWireTable ----------------------------------------------------

fn golden_wire_table() -> ProducerTable {
    let path = "/p/a.tsx";
    let descriptor = Arc::new(TypeDescriptor {
        text: "Accessor<number>".into(),
        origin_module: "solid-js".into(),
        alias_declarations: Arc::from(vec![Declaration {
            name: "Accessor".into(),
            kind: "TypeAlias".into(),
            location: location("/p/solid-js.d.ts", 10, 30),
        }]),
    });
    let call = Arc::new(ResolvedCall {
        target: "symbol:h:1".into(),
        return_type_text: "() => number".into(),
        assertion_signature: false,
        never_returns: false,
        validity: ResolvedCallValidity::Valid,
        kind: CallKind::Call,
        declaration: Some(ResolvedDeclaration {
            symbol: "symbol:h:1".into(),
            name: "count".into(),
            kind: "FunctionDeclaration".into(),
            location: location(path, 1, 4),
            owners: Arc::from(vec![DeclarationOwner {
                symbol: "symbol:h:4".into(),
                name: "Counter".into(),
                kind: "InterfaceDeclaration".into(),
                location: location(path, 0, 1),
            }]),
            qualified_name: "Counter.count".into(),
            origin_module: "".into(),
            source_file: path.into(),
            standard_library: false,
        }),
        default_library_binding: None,
        targets: None,
        arguments: Arc::from(vec![ArgumentMapping {
            argument_index: 0,
            status: ArgumentMappingStatus::Resolved,
            unresolved: None,
            parameter: Some(ParameterFact {
                index: 0,
                symbol: "symbol:h:5".into(),
                declaration: Some(Declaration {
                    name: "callback".into(),
                    kind: "declaration".into(),
                    location: location(path, 5, 13),
                }),
                rest: false,
                optional: false,
                callability: Callability::Callable,
                type_descriptor: Some((*descriptor).clone()),
                object_shape: None,
            }),
        }]),
    });
    let mut table = ProducerTable::new(3, "/p/tsconfig.json");
    table.sources = vec![source_digest(path, b"export const value = 1\n")];
    let mut row = entity(path, 2, "symbol:h:2");
    row.location.end_byte = 8;
    row.type_descriptor = Some(descriptor);
    row.resolved_call = Some(call);
    row.callability = Some(Callability::Callable);
    row.reference_space = Some(ReferenceSpace::Both);
    row.runtime_identity = "runtime:h:1".into();
    table.entities = EntityRows::Flat(vec![row]);
    table.symbols = vec![
        SymbolFact {
            declarations: Arc::from(vec![Declaration {
                name: "count".into(),
                kind: "Variable".into(),
                location: location(path, 1, 4),
            }]),
            references: Arc::from(vec![location(path, 2, 8)]),
            ..symbol("symbol:h:1")
        },
        SymbolFact {
            alias_target: "symbol:h:1".into(),
            ..symbol("symbol:h:3")
        },
    ];
    table.files = vec![FileFact {
        calls: Arc::from(vec![SourceCall {
            location: location(path, 2, 8),
            callee: location(path, 2, 7),
            arguments: vec![location(path, 7, 8)],
            target: "symbol:h:1".into(),
        }]),
        bindings: Arc::from(vec![SourceBinding {
            array: true,
            names: vec![location(path, 0, 1)],
            initializer: SourceCall {
                location: location(path, 2, 8),
                callee: location(path, 2, 7),
                arguments: Vec::new(),
                target: "".into(),
            },
        }]),
        functions: Arc::from(vec![SourceFunction {
            name: location(path, 20, 25),
            body: location(path, 26, 40),
            parameters: vec![location(path, 21, 22)],
            exported: true,
            r#async: false,
            arrow: true,
        }]),
        async_functions: Arc::from(vec![AsyncFunctionFact {
            expression: location(path, 26, 40),
            symbol: "symbol:h:2".into(),
            target: "symbol:h:1".into(),
            can_return_async: true,
            calls_after_await: vec![location(path, 30, 34)],
        }]),
        ..file(path)
    }];
    table
}

fn full(encoder: &TransitionEncoder, table: &ProducerTable) -> Vec<u8> {
    encoder
        .encode(TransitionInput {
            project_id: &table.project_id,
            base_state_token: "",
            base: None,
            target: table,
            sparse: false,
        })
        .unwrap()
        .bytes
}

fn delta(
    encoder: &TransitionEncoder,
    base: &ProducerTable,
    target: &ProducerTable,
    token: &str,
) -> EncodedTransition {
    encoder
        .encode(TransitionInput {
            project_id: &target.project_id,
            base_state_token: token,
            base: Some(base),
            target,
            sparse: false,
        })
        .unwrap()
}

fn phase1(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../benchmarks/typefacts/phase1")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

// -- the v5 transition golden -----------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenStep {
    label: String,
    base_token: String,
    #[serde(with = "serde_bytes")]
    base_transition: Vec<u8>,
    #[serde(with = "serde_bytes")]
    transition: Vec<u8>,
    #[serde(with = "serde_bytes")]
    expected_transition: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct Golden {
    steps: Vec<GoldenStep>,
}

fn paths(names: &[&str]) -> HashSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn exact_closure(symbols: &[&str]) -> ChangedClosure {
    ChangedClosure {
        reference_changes_exact: true,
        changed_symbols: symbols.iter().map(|id| Arc::<str>::from(*id)).collect(),
    }
}

/// Go: `transitionGoldenSteps`, driving the same edit, deletion, demand shrink,
/// and per-path reference replacement through this encoder.
fn transition_golden_steps() -> Vec<GoldenStep> {
    let encoder = TransitionEncoder::new(V3);
    let make = |generation: u64,
                state_id: u64,
                sources: &[(&str, &[u8])],
                entities: Vec<EntityFact>,
                symbols: Vec<SymbolFact>,
                files: Vec<FileFact>| {
        let mut table = table(generation, state_id);
        table.sources = sources
            .iter()
            .map(|(path, source)| source_digest(path, source))
            .collect();
        table.entities = EntityRows::Flat(entities);
        table.symbols = symbols;
        table.files = files;
        table
    };
    let base = make(
        1,
        1,
        &[("a.ts", b"a"), ("b.ts", b"b")],
        vec![entity("a.ts", 1, "a"), entity("b.ts", 1, "b")],
        vec![symbol("a"), symbol("b")],
        vec![file("a.ts"), file("b.ts")],
    );
    let a2_file = FileFact {
        calls: Arc::from(vec![call_to("a2")]),
        ..file("a.ts")
    };
    let mut edit = make(
        2,
        2,
        &[("a.ts", b"a2"), ("b.ts", b"b")],
        vec![entity("a.ts", 3, "a2"), entity("b.ts", 1, "b")],
        vec![symbol("a2"), symbol("b")],
        vec![a2_file.clone(), file("b.ts")],
    );
    let mut deleted = make(
        3,
        3,
        &[("a.ts", b"a2")],
        vec![entity("a.ts", 3, "a2")],
        vec![symbol("a2")],
        vec![a2_file.clone()],
    );
    let mut shrunk = make(
        4,
        4,
        &[("a.ts", b"a2")],
        Vec::new(),
        Vec::new(),
        vec![a2_file],
    );
    let exact = exact_closure(&["a", "a2", "b"]);
    edit.transport = transport_manifest(Some(&base), &edit, Some(&exact), &paths(&["a.ts"]));
    deleted.transport = transport_manifest(Some(&edit), &deleted, Some(&exact), &paths(&["b.ts"]));
    shrunk.transport = transport_manifest(Some(&deleted), &shrunk, Some(&exact), &paths(&["a.ts"]));

    let mut steps = Vec::new();
    let mut previous = &base;
    for (index, (label, next)) in [
        ("edit", &edit),
        ("delete", &deleted),
        ("demand-shrink", &shrunk),
    ]
    .into_iter()
    .enumerate()
    {
        let token = ["base-1", "base-2", "base-3"][index];
        steps.push(GoldenStep {
            label: label.to_owned(),
            base_token: token.to_owned(),
            base_transition: full(&encoder, previous),
            transition: delta(&encoder, previous, next, token).bytes,
            expected_transition: full(&encoder, next),
        });
        previous = next;
    }

    // The reference-file delta splices one path's references inside an
    // already path-sorted list.
    let reference = |path: &str, start: u64| location(path, start, start + 1);
    let mut shared_base = table(1, 5);
    shared_base.symbols = vec![SymbolFact {
        references: Arc::from(vec![reference("a.ts", 1), reference("b.ts", 1)]),
        ..symbol("shared")
    }];
    let mut shared_edit = table(2, 6);
    shared_edit.symbols = vec![SymbolFact {
        references: Arc::from(vec![
            reference("a.ts", 3),
            reference("a.ts", 5),
            reference("b.ts", 1),
        ]),
        ..symbol("shared")
    }];
    shared_edit.transport = transport_manifest(
        Some(&shared_base),
        &shared_edit,
        Some(&exact_closure(&["shared"])),
        &paths(&["a.ts"]),
    );
    steps.push(GoldenStep {
        label: "symbol-reference-file".to_owned(),
        base_token: "base-reference".to_owned(),
        base_transition: full(&encoder, &shared_base),
        transition: delta(&encoder, &shared_base, &shared_edit, "base-reference").bytes,
        expected_transition: full(&encoder, &shared_edit),
    });
    steps
}

#[test]
fn v5_transition_golden_matches_producer_output_byte_for_byte() {
    let encoded = crate::encode(&Golden {
        steps: transition_golden_steps(),
    })
    .unwrap();
    let golden = phase1("typefacts-v5-transition-golden.cbor");
    assert_eq!(
        encoded.len(),
        golden.len(),
        "producer emits {} bytes, fixture has {}",
        encoded.len(),
        golden.len()
    );
    assert!(encoded == golden, "transition golden is stale");
}

#[test]
fn every_golden_transition_survives_decode_then_encode() {
    let golden: Golden = crate::decode(&phase1("typefacts-v5-transition-golden.cbor")).unwrap();
    assert_eq!(golden.steps.len(), 4);
    for step in &golden.steps {
        for (name, frame) in [
            ("base", &step.base_transition),
            ("delta", &step.transition),
            ("expected", &step.expected_transition),
        ] {
            let again = reencode_table_transition(frame)
                .unwrap_or_else(|error| panic!("{} {name}: {error}", step.label));
            assert!(
                &again == frame,
                "{} {name} changed on re-encode",
                step.label
            );
        }
    }
}

/// The v3 response golden embeds the transition of `goldenWireTable`. The file
/// was last regenerated when Wire table schema 7 was current and Go's test
/// only re-encodes the file's own bytes, so it never noticed the schema moving
/// on. Rebuilding the table at the schema the file names reproduces it, and the
/// current schema's frame is a different (longer) one.
#[test]
fn v3_response_golden_carries_the_producers_table_transition() {
    let golden = phase1("typefacts-v3-response-golden.cbor");
    let response: Response = crate::decode(&golden).unwrap();
    let embedded = decoded(&response.table_transition);
    assert_eq!(embedded.table_schema, v3::TYPE_FACTS_TABLE_SCHEMA_V7);
    let at_its_schema = full(
        &TransitionEncoder::new(embedded.table_schema),
        &golden_wire_table(),
    );
    assert!(
        at_its_schema == response.table_transition,
        "response golden transition drifted"
    );
    let again = reencode_table_transition(&response.table_transition).unwrap();
    assert!(again == response.table_transition);
    let latest = full(&TransitionEncoder::default(), &golden_wire_table());
    assert_ne!(latest, response.table_transition);
    assert!(reencode_table_transition(&latest).unwrap() == latest);
}

// -- Go: wire_transition_test.go --------------------------------------------

#[derive(Default)]
struct ArenaForTest {
    rows: Vec<u8>,
    prefix: Vec<u8>,
    chunks: usize,
}

impl TransitionSink for ArenaForTest {
    fn append_rows(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        self.rows.extend_from_slice(chunk);
        self.chunks += 1;
        Ok(())
    }

    fn finish(&mut self, prefix: &[u8]) -> std::io::Result<()> {
        self.prefix.extend_from_slice(prefix);
        Ok(())
    }
}

#[test]
fn streamed_transition_is_byte_identical() {
    let table = golden_wire_table();
    let encoder = TransitionEncoder::default();
    let expected = full(&encoder, &table);
    let mut arena = ArenaForTest::default();
    let actual = encoder
        .encode_into(
            TransitionInput {
                project_id: &table.project_id,
                base_state_token: "",
                base: None,
                target: &table,
                sparse: false,
            },
            &mut arena,
        )
        .unwrap();
    assert!(
        actual.bytes.is_empty(),
        "streamed transition retained bytes"
    );
    let streamed = [arena.prefix.as_slice(), arena.rows.as_slice()].concat();
    assert!(streamed == expected, "streamed differs from owned");
}

#[test]
fn a_transition_larger_than_one_chunk_streams_in_bounded_chunks() {
    let mut big = table(1, 1);
    let mut entities = Vec::new();
    for index in 0..12_000_u64 {
        let mut row = entity("/p/big.ts", index * 3, &format!("symbol:big:{index:06}"));
        row.location.end_byte = index * 3 + 2;
        row.runtime_identity = format!("runtime:{index}").into();
        entities.push(row);
    }
    big.entities = EntityRows::Flat(entities);
    let encoder = TransitionEncoder::default();
    let expected = full(&encoder, &big);
    let mut arena = ArenaForTest::default();
    encoder
        .encode_into(
            TransitionInput {
                project_id: &big.project_id,
                base_state_token: "",
                base: None,
                target: &big,
                sparse: false,
            },
            &mut arena,
        )
        .unwrap();
    assert!(arena.chunks > 1, "rows fit one chunk: {}", arena.rows.len());
    assert_eq!(
        [arena.prefix, arena.rows].concat(),
        expected,
        "chunked rows differ from the owned frame"
    );
}

#[test]
fn a_failing_sink_surfaces_as_a_stream_error() {
    struct Refuse;
    impl TransitionSink for Refuse {
        fn append_rows(&mut self, _: &[u8]) -> std::io::Result<()> {
            Err(std::io::Error::other("disk full"))
        }
        fn finish(&mut self, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut big = table(1, 1);
    big.entities = EntityRows::Flat(
        (0..12_000_u64)
            .map(|index| entity("/p/big.ts", index * 3, &format!("symbol:big:{index:06}")))
            .collect(),
    );
    let error = TransitionEncoder::default()
        .encode_into(
            TransitionInput {
                project_id: &big.project_id,
                base_state_token: "",
                base: None,
                target: &big,
                sparse: false,
            },
            &mut Refuse,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("stream transition rows"),
        "{error}"
    );
}

#[test]
fn library_binding_changes_rows_but_frozen_schemas_omit_it() {
    let mut table = golden_wire_table();
    let before = table.entities_vec()[0].resolved_call.clone().unwrap();
    let mut after = (*before).clone();
    after.default_library_binding = Some(Arc::new(ResolvedDeclaration {
        symbol: "symbol:h:library".into(),
        name: "setTimeout".into(),
        kind: "FunctionDeclaration".into(),
        location: location("bundled:/libs/lib.dom.d.ts", 1, 11),
        owners: Arc::from(Vec::new()),
        qualified_name: "setTimeout".into(),
        origin_module: "".into(),
        source_file: "bundled:/libs/lib.dom.d.ts".into(),
        standard_library: true,
    }));
    assert_ne!(
        *before, after,
        "binding changes must invalidate an entity row"
    );
    let mut encode = |schema: u64, call: &Arc<ResolvedCall>| {
        let EntityRows::Flat(rows) = &mut table.entities else {
            unreachable!()
        };
        rows[0].resolved_call = Some(call.clone());
        full(&TransitionEncoder::new(schema), &table)
    };
    let after = Arc::new(after);
    assert!(
        encode(v3::TYPE_FACTS_TABLE_SCHEMA_V19, &before)
            == encode(v3::TYPE_FACTS_TABLE_SCHEMA_V19, &after),
        "v19 must remain frozen"
    );
    assert!(
        encode(v3::TYPE_FACTS_TABLE_SCHEMA_V20, &before)
            != encode(v3::TYPE_FACTS_TABLE_SCHEMA_V20, &after),
        "v20 lost the library binding"
    );
}

#[test]
fn declared_keys_round_trip_on_v23_and_are_omitted_on_frozen_schemas() {
    let mut table = state(1, 1, "a", b"a");
    let names: Arc<Vec<Arc<str>>> = Arc::new(vec!["class".into(), "style".into()]);
    let EntityRows::Flat(rows) = &mut table.entities else {
        unreachable!()
    };
    rows[0].declared_keys = Some(names.clone());
    let keys_of = |schema: u64| {
        let plan = decoded(&full(&TransitionEncoder::new(schema), &table));
        let op = plan.paths.iter().find(|op| &*op.path == "a.ts").unwrap();
        match &op.entities {
            v3::SlotOp::Replace(entities) => entities[0].declared_keys.clone(),
            _ => panic!("a full transition replaces every path's entities"),
        }
    };
    assert_eq!(keys_of(v3::TYPE_FACTS_TABLE_SCHEMA_V23), Some(names));
    assert_eq!(
        keys_of(v3::TYPE_FACTS_TABLE_SCHEMA_V22),
        None,
        "v22 is frozen and must not carry the field"
    );
}

impl ProducerTable {
    fn entities_vec(&self) -> &[EntityFact] {
        match &self.entities {
            EntityRows::Flat(rows) => rows,
            EntityRows::Runs(_) => unreachable!("tests build flat tables"),
        }
    }
}

#[test]
fn borrowed_entity_runs_encode_byte_identically() {
    let mut flat = golden_wire_table();
    let mut second = entity("/p/b.tsx", 4, "symbol:h:7");
    second.callability = Some(Callability::Mixed);
    let EntityRows::Flat(rows) = &mut flat.entities else {
        unreachable!()
    };
    rows.push(second);
    let encoder = TransitionEncoder::default();
    let expected = full(&encoder, &flat);
    let mut borrowed = flat.clone();
    borrowed.entities = flat.entities.clone().into_runs();
    assert!(matches!(&borrowed.entities, EntityRows::Runs(runs) if runs.len() == 2));
    assert_eq!(full(&encoder, &borrowed), expected);
    assert_eq!(borrowed.entities.len(), 2);
}

/// What `decodeTransitionEnvelopeForTest` reads: the header and the first
/// counts, by hand rather than through the decoder under test.
struct Envelope {
    version: u64,
    mode: u64,
    schema: u64,
    base: u64,
    target: u64,
    project_id: String,
    base_state_token: String,
    path_operations: u64,
    symbol_operations: u64,
}

fn read_envelope(mut frame: &[u8]) -> Envelope {
    fn read(frame: &mut &[u8]) -> u64 {
        let mut value = 0_u64;
        let mut shift = 0;
        loop {
            let byte = frame[0];
            *frame = &frame[1..];
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }
    let frame = &mut frame;
    let (version, mode, schema, base, target) = (
        read(frame),
        read(frame),
        read(frame),
        read(frame),
        read(frame),
    );
    let count = read(frame) as usize;
    let mut dictionary: Vec<String> = Vec::new();
    let mut previous = String::new();
    for _ in 0..count {
        match read(frame) {
            0 => {
                let prefix = read(frame) as usize;
                let suffix = read(frame) as usize;
                let value = format!(
                    "{}{}",
                    &previous[..prefix],
                    std::str::from_utf8(&frame[..suffix]).unwrap()
                );
                *frame = &frame[suffix..];
                dictionary.push(value.clone());
                previous = value;
            }
            1 => {
                let digest: String = frame[..12].iter().map(|b| format!("{b:02x}")).collect();
                *frame = &frame[12..];
                dictionary.push(format!("symbol:h:{digest}"));
            }
            tag => panic!("dictionary tag {tag}"),
        }
    }
    let project_id = dictionary[read(frame) as usize].clone();
    let base_state_token = dictionary[read(frame) as usize].clone();
    let path_operations = read(frame);
    let symbol_operations = if path_operations == 0 { read(frame) } else { 0 };
    Envelope {
        version,
        mode,
        schema,
        base,
        target,
        project_id,
        base_state_token,
        path_operations,
        symbol_operations,
    }
}

#[test]
fn transition_carries_identity_and_distinguishes_reuse_from_advancement() {
    let table = golden_wire_table();
    let encoder = TransitionEncoder::default();
    let first = full(&encoder, &table);
    assert!(first == full(&encoder, &table), "full is not deterministic");
    let envelope = read_envelope(&first);
    assert_eq!(envelope.version, 1);
    assert_eq!(envelope.mode, 0);
    assert_eq!(envelope.schema, v3::TYPE_FACTS_TABLE_SCHEMA_V23);
    assert_eq!(envelope.base, 0);
    assert_eq!(envelope.target, table.generation);
    assert_eq!(&*table.project_id, envelope.project_id);
    assert_eq!(envelope.base_state_token, "");

    let reuse = delta(&encoder, &table, &table, "state-3");
    assert!(reuse.bytes.is_empty());
    assert_eq!((reuse.path_operations, reuse.symbol_operations), (0, 0));
    assert_eq!(reuse.mode, TransitionMode::Delta);

    let mut advanced = table.clone();
    advanced.generation += 1;
    let advanced_delta = delta(&encoder, &table, &advanced, "state-3");
    let envelope = read_envelope(&advanced_delta.bytes);
    assert_eq!(envelope.mode, 1);
    assert_eq!(envelope.base, table.generation);
    assert_eq!(envelope.target, advanced.generation);
    assert_eq!(&*table.project_id, envelope.project_id);
    assert_eq!(envelope.base_state_token, "state-3");
    assert_eq!(envelope.path_operations, 0);
    assert_eq!(envelope.symbol_operations, 0);
}

fn encode_error(table: &ProducerTable) -> String {
    TransitionEncoder::default()
        .encode(TransitionInput {
            project_id: &table.project_id,
            base_state_token: "",
            base: None,
            target: table,
            sparse: false,
        })
        .unwrap_err()
        .to_string()
}

#[test]
fn rejects_contradictory_symbol_resolution() {
    let mut table = golden_wire_table();
    let EntityRows::Flat(rows) = &mut table.entities else {
        unreachable!()
    };
    rows[0].symbol_claim = SymbolClaim::Unresolved;
    assert!(encode_error(&table).contains("cannot be both resolved and unresolved"));
}

/// A span-covering claim without a symbol has no value to cover, so the writer
/// refuses it rather than encoding a flag the reader would reject.
#[test]
fn rejects_span_coverage_without_a_symbol() {
    let mut table = golden_wire_table();
    let EntityRows::Flat(rows) = &mut table.entities else {
        unreachable!()
    };
    rows[0].symbol = "".into();
    rows[0].symbol_claim = SymbolClaim::Covering;
    assert!(encode_error(&table).contains("covers its span without a symbol"));
}

#[test]
fn rejects_locations_that_end_before_they_start() {
    let mut table = golden_wire_table();
    let EntityRows::Flat(rows) = &mut table.entities else {
        unreachable!()
    };
    rows[0].location.end_byte = 0;
    assert!(encode_error(&table).contains("precedes start byte"));
}

#[test]
fn rejects_non_finite_numbers_the_decoder_would_refuse() {
    let mut table = golden_wire_table();
    let set = |table: &mut ProducerTable, edit: &dyn Fn(&mut EntityFact)| {
        let EntityRows::Flat(rows) = &mut table.entities else {
            unreachable!()
        };
        edit(&mut rows[0]);
    };
    set(&mut table, &|row| {
        row.constant_value = Some(ConstantValue {
            kind: ConstantValueKind::Number,
            string: "".into(),
            number: f64::NAN,
        });
    });
    assert!(encode_error(&table).contains("NaN"));
    set(&mut table, &|row| {
        row.constant_value = None;
        row.primitive_literal_candidates = Some(Arc::new(vec![PrimitiveLiteralCandidate {
            kind: PrimitiveLiteralKind::Number,
            string: "".into(),
            number: f64::INFINITY,
            boolean: false,
        }]));
    });
    assert!(encode_error(&table).contains("non-finite"));
}

#[test]
fn input_validation_matches_go() {
    let encoder = TransitionEncoder::default();
    let target = golden_wire_table();
    let base = golden_wire_table();
    let input = |base: Option<&'static ProducerTable>, token| TransitionInput {
        project_id: "/p/tsconfig.json",
        base_state_token: token,
        base,
        target: Box::leak(Box::new(target.clone())),
        sparse: false,
    };
    let base: &'static ProducerTable = Box::leak(Box::new(base));
    let message = |input| encoder.encode(input).unwrap_err().to_string();
    assert!(
        message(input(None, "token")).contains("full wire transition carries a base state token")
    );
    assert!(
        message(input(Some(base), "")).contains("delta wire transition has no base state token")
    );
    let mut empty_project = input(None, "");
    empty_project.project_id = "";
    assert!(message(empty_project).contains("project ID is empty"));
    let mut late = base.clone();
    late.generation = 9;
    let late: &'static ProducerTable = Box::leak(Box::new(late));
    assert!(message(input(Some(late), "t")).contains("precedes base generation"));
    let mut wrong_schema = target.clone();
    wrong_schema.schema = 2;
    assert!(encode_error(&wrong_schema).contains("target schema = 2"));
    let mut zero = target.clone();
    zero.generation = 0;
    assert!(encode_error(&zero).contains("generation is zero"));
    assert!(
        TransitionEncoder::new(10)
            .encode(input(None, ""))
            .unwrap_err()
            .to_string()
            .contains("unsupported Wire table schema 10")
    );
}

// untypedCallable is tag 4, and tag 4 exists only from Wire table schema v15.
// Every frozen schema before it must keep emitting a tag its own decoder
// accepts, and the substitute must be unknown: nonCallable would turn "callable,
// no signature to read" into a claim that the value cannot be called at all.
#[test]
fn untyped_callable_degrades_to_unknown_on_frozen_table_schemas() {
    use super::rows::callability_code;
    for schema in [
        v3::TYPE_FACTS_TABLE_SCHEMA_V3,
        v3::TYPE_FACTS_TABLE_SCHEMA_V13,
        v3::TYPE_FACTS_TABLE_SCHEMA_V14,
    ] {
        assert_eq!(
            callability_code(Callability::UntypedCallable, schema),
            callability_code(Callability::Unknown, schema),
            "schema {schema}"
        );
    }
    assert_eq!(
        callability_code(
            Callability::UntypedCallable,
            v3::TYPE_FACTS_TABLE_SCHEMA_V15
        ),
        4
    );
    // The four frozen tags keep their codes at every schema, v15 included.
    for schema in [
        v3::TYPE_FACTS_TABLE_SCHEMA_V3,
        v3::TYPE_FACTS_TABLE_SCHEMA_V14,
        v3::TYPE_FACTS_TABLE_SCHEMA_V15,
    ] {
        for (value, want) in [
            (Callability::Callable, 0),
            (Callability::NonCallable, 1),
            (Callability::Mixed, 2),
            (Callability::Unknown, 3),
        ] {
            assert_eq!(callability_code(value, schema), want, "schema {schema}");
        }
    }
}

// -- Go: wire_transition_codec_test.go --------------------------------------

#[test]
fn wire_symbol_name_escapes_typescript_internal_names() {
    let got = wire_symbol_name_bytes(b"\xfeindex");
    assert_eq!(got, "__index");
}

#[test]
fn wire_symbol_name_repairs_unexpected_invalid_utf8() {
    assert_eq!(
        wire_symbol_name_bytes(b"before\xffafter"),
        "before\u{FFFD}after"
    );
    // Each run of invalid bytes becomes one replacement, as ToValidUTF8 does.
    assert_eq!(wire_symbol_name_bytes(b"a\xff\xfe\xffb"), "a\u{FFFD}b");
    assert_eq!(wire_symbol_name_bytes(b"a\xe2\x82"), "a\u{FFFD}");
    assert_eq!(wire_symbol_name_bytes("caf\u{e9}".as_bytes()), "caf\u{e9}");
    assert_eq!(wire_symbol_name_bytes(b"\xfe\xff"), "__\u{FFFD}");
}

// -- Go: transport_manifest_test.go -----------------------------------------

#[test]
fn transport_manifest_keeps_broad_exact_path_evidence() {
    let mut previous = table(1, 1);
    previous.schema = 1;
    let next = table(2, 2);
    let changed: HashSet<String> = (0..65).map(|index| format!("file-{index:02}.ts")).collect();
    let manifest = transport_manifest(Some(&previous), &next, Some(&exact_closure(&[])), &changed)
        .expect("broad exact path evidence was discarded");
    assert!(manifest.exact);
    assert_eq!(manifest.source_paths.len(), changed.len());
    assert_eq!(manifest.entity_paths.len(), changed.len());
    assert_eq!(manifest.file_paths.len(), changed.len());
}

#[test]
fn a_manifest_needs_a_base_and_proven_exact_references() {
    let previous = table(1, 1);
    let next = table(2, 2);
    let changed = paths(&["a.ts"]);
    assert!(transport_manifest(None, &next, Some(&exact_closure(&[])), &changed).is_none());
    assert!(transport_manifest(Some(&previous), &next, None, &changed).is_none());
    let inexact = ChangedClosure::default();
    assert!(transport_manifest(Some(&previous), &next, Some(&inexact), &changed).is_none());
    assert!(rust_owned_transport_manifest(None, Some(&changed)).is_none());
    assert!(rust_owned_transport_manifest(Some(&previous), None).is_none());
    let owned = rust_owned_transport_manifest(Some(&previous), Some(&changed)).unwrap();
    assert!(owned.exact && owned.symbol_ids.is_empty());
    assert_eq!((owned.base_generation, owned.base_state_id), (1, 1));
}

#[test]
fn the_manifest_closes_over_alias_targets_in_both_tables() {
    let mut previous = table(1, 1);
    previous.symbols = vec![
        SymbolFact {
            alias_target: "old-target".into(),
            ..symbol("alias")
        },
        symbol("old-target"),
    ];
    let mut next = table(2, 2);
    next.symbols = vec![
        SymbolFact {
            alias_target: "new-target".into(),
            ..symbol("alias")
        },
        symbol("new-target"),
    ];
    let manifest = transport_manifest(
        Some(&previous),
        &next,
        Some(&exact_closure(&["alias"])),
        &HashSet::new(),
    )
    .unwrap();
    let ids: HashSet<&str> = manifest.symbol_ids.iter().map(AsRef::as_ref).collect();
    assert_eq!(ids, HashSet::from(["alias", "old-target", "new-target"]));
}

#[test]
fn path_symbols_come_from_entities_calls_and_async_functions() {
    let mut retained = table(1, 1);
    let mut row = entity("src/a.ts", 1, "sym-entity");
    row.resolved_call = Some(Arc::new(ResolvedCall {
        target: "sym-call".into(),
        return_type_text: "".into(),
        assertion_signature: false,
        never_returns: false,
        validity: ResolvedCallValidity::Valid,
        kind: CallKind::Call,
        declaration: None,
        default_library_binding: None,
        targets: None,
        arguments: Arc::from(Vec::new()),
    }));
    retained.entities = EntityRows::Flat(vec![row, entity("src/b.ts", 1, "sym-other")]);
    retained.files = vec![FileFact {
        async_functions: Arc::from(vec![AsyncFunctionFact {
            expression: Location::default(),
            symbol: "sym-async".into(),
            target: "sym-async-target".into(),
            can_return_async: false,
            calls_after_await: Vec::new(),
        }]),
        ..file("src/a.ts")
    }];
    retained.symbols = vec![SymbolFact {
        alias_target: "sym-aliased".into(),
        ..symbol("sym-entity")
    }];
    let candidates = retained_symbol_candidates(Some(&retained), &paths(&["./src//a.ts"]));
    let ids: HashSet<&str> = candidates.iter().map(AsRef::as_ref).collect();
    assert_eq!(
        ids,
        HashSet::from([
            "sym-entity",
            "sym-call",
            "sym-async",
            "sym-async-target",
            "sym-aliased"
        ])
    );
    assert!(retained_symbol_candidates(None, &paths(&["src/a.ts"])).is_empty());

    // A retained table keeps only its compact view, and that view wins.
    let mut compact = table(1, 1);
    compact.path_symbols = Some(
        [(
            Arc::<str>::from("src/a.ts"),
            vec![Arc::<str>::from("only-this")],
        )]
        .into_iter()
        .collect(),
    );
    compact.entities = retained.entities.clone();
    let candidates = retained_symbol_candidates(Some(&compact), &paths(&["src/a.ts"]));
    assert_eq!(candidates.len(), 1);
}

#[test]
fn clean_path_matches_go() {
    for (input, want) in [
        ("", "."),
        ("a.ts", "a.ts"),
        ("./a//b/", "a/b"),
        ("/a/../b", "/b"),
        ("/../a", "/a"),
        ("../../a", "../../a"),
        ("a/..", "."),
        ("/", "/"),
    ] {
        assert_eq!(clean_path(input), want, "clean({input:?})");
    }
}

#[test]
fn exact_symbol_plan_derives_reference_operations_from_canonical_runs() {
    let reference = |path: &str, start: u64| location(path, start, start + 1);
    let mut base = table(1, 1);
    base.symbols = vec![SymbolFact {
        references: Arc::from(vec![
            reference("a.ts", 1),
            reference("b.ts", 1),
            reference("c.ts", 1),
        ]),
        ..symbol("shared")
    }];
    let mut target = table(2, 2);
    target.symbols = vec![SymbolFact {
        references: Arc::from(vec![
            reference("a.ts", 1),
            reference("b.ts", 2),
            reference("d.ts", 1),
        ]),
        ..symbol("shared")
    }];
    let manifest = TransportChanges {
        symbol_ids: HashSet::from([Arc::<str>::from("shared")]),
        exact: true,
        ..TransportChanges::default()
    };
    let operations = plan_exact_symbol_delta(&base, &target, &manifest);
    assert_eq!(operations.len(), 3, "{operations:?}");
    for (operation, want) in operations.iter().zip(["b.ts", "c.ts", "d.ts"]) {
        match operation.body {
            SymbolBody::ReplaceReferencePath { path, .. } => assert_eq!(path, want),
            other => panic!("operation {other:?}, want reference path {want:?}"),
        }
    }
    let SymbolBody::ReplaceReferencePath { references, .. } = operations[1].body else {
        unreachable!()
    };
    assert!(
        references.is_empty(),
        "removed c.ts run did not encode an empty replacement"
    );
}

// -- planning ---------------------------------------------------------------

fn state(generation: u64, state_id: u64, a_symbol: &str, source: &[u8]) -> ProducerTable {
    let mut table = table(generation, state_id);
    table.sources = vec![source_digest("a.ts", source), source_digest("b.ts", b"b")];
    table.entities = EntityRows::Flat(vec![entity("a.ts", 1, a_symbol), entity("b.ts", 1, "b")]);
    table.symbols = vec![symbol(a_symbol), symbol("b")];
    table.files = vec![file("a.ts"), file("b.ts")];
    table
}

fn decoded(frame: &[u8]) -> v3::WireTableTransition {
    v3::decode_table_transition(frame).unwrap()
}

#[test]
fn an_exact_manifest_limits_the_delta_to_changed_candidates() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    let mut edit = state(2, 2, "a2", b"a2");
    edit.transport = transport_manifest(
        Some(&base),
        &edit,
        Some(&exact_closure(&["a", "a2"])),
        &paths(&["a.ts"]),
    );
    let result = delta(&encoder, &base, &edit, "t1");
    assert_eq!(result.mode, TransitionMode::Delta);
    let plan = decoded(&result.bytes);
    assert_eq!(plan.paths.len(), 1);
    assert_eq!(&*plan.paths[0].path, "a.ts");
    // a removed, a2 added; b is outside the manifest and absent.
    let ids: Vec<&str> = plan.symbols.iter().map(|op| op.id().as_ref()).collect();
    assert_eq!(ids, ["a", "a2"]);
}

#[test]
fn a_manifest_for_another_base_falls_back_to_a_full_diff() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    let mut edit = state(2, 2, "a2", b"a2");
    // Claims a base state the encoder was not given: untrusted, so ignored.
    edit.transport = Some(TransportChanges {
        base_generation: 1,
        base_state_id: 99,
        source_paths: paths(&["b.ts"]),
        entity_paths: paths(&["b.ts"]),
        file_paths: paths(&["b.ts"]),
        symbol_ids: HashSet::new(),
        exact: true,
    });
    let plan = decoded(&delta(&encoder, &base, &edit, "t1").bytes);
    assert_eq!(plan.paths.len(), 1);
    assert_eq!(&*plan.paths[0].path, "a.ts");
    let ids: Vec<&str> = plan.symbols.iter().map(|op| op.id().as_ref()).collect();
    assert_eq!(ids, ["a", "a2"]);
}

#[test]
fn the_fallback_diff_covers_additions_removals_and_unchanged_rows() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    let mut target = state(2, 2, "a", b"a");
    target.sources.remove(1);
    target.files.remove(1);
    target.entities = EntityRows::Flat(vec![entity("a.ts", 1, "a"), entity("c.ts", 1, "c")]);
    target.sources.push(source_digest("c.ts", b"c"));
    target.files.push(file("c.ts"));
    target.symbols = vec![symbol("a"), symbol("c")];
    let plan = decoded(&delta(&encoder, &base, &target, "t1").bytes);
    let operations: Vec<(&str, bool, bool, bool)> = plan
        .paths
        .iter()
        .map(|op| {
            (
                op.path.as_ref(),
                matches!(op.source, v3::SlotOp::Remove),
                matches!(op.entities, v3::SlotOp::Remove),
                matches!(op.file, v3::SlotOp::Remove),
            )
        })
        .collect();
    assert_eq!(
        operations,
        [("b.ts", true, true, true), ("c.ts", false, false, false)]
    );
    assert!(matches!(plan.paths[1].source, v3::SlotOp::Replace(_)));
    assert!(matches!(plan.paths[1].entities, v3::SlotOp::Replace(_)));
    assert!(matches!(plan.paths[1].file, v3::SlotOp::Replace(_)));
    let symbol_ops: Vec<(&str, bool)> = plan
        .symbols
        .iter()
        .map(|op| (op.id().as_ref(), matches!(op, v3::SymbolOp::Remove { .. })))
        .collect();
    assert_eq!(symbol_ops, [("b", true), ("c", false)]);
}

#[test]
fn a_sparse_delta_upserts_and_removes_exactly_the_manifest_paths() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    let mut target = state(2, 2, "a", b"a2");
    target.sources.remove(1);
    target.entities = EntityRows::Flat(vec![entity("a.ts", 1, "a")]);
    target.files.remove(1);
    target.transport =
        rust_owned_transport_manifest(Some(&base), Some(&paths(&["a.ts", "b.ts", "gone.ts"])));
    let sparse = encoder
        .encode(TransitionInput {
            project_id: &target.project_id,
            base_state_token: "t1",
            base: Some(&base),
            target: &target,
            sparse: true,
        })
        .unwrap();
    let plan = decoded(&sparse.bytes);
    let tags = |op: &v3::PathOp| {
        (
            op.path.to_string(),
            match op.source {
                v3::SlotOp::Unchanged => 'u',
                v3::SlotOp::Replace(_) => 'r',
                v3::SlotOp::Remove => 'x',
            },
            match op.entities {
                v3::SlotOp::Unchanged => 'u',
                v3::SlotOp::Replace(_) => 'r',
                v3::SlotOp::Remove => 'x',
            },
            match op.file {
                v3::SlotOp::Unchanged => 'u',
                v3::SlotOp::Replace(_) => 'r',
                v3::SlotOp::Remove => 'x',
            },
        )
    };
    let operations: Vec<_> = plan.paths.iter().map(tags).collect();
    assert_eq!(
        operations,
        [
            ("a.ts".to_owned(), 'r', 'r', 'r'),
            // Unconditional: removed even though the producer holds no base rows.
            ("b.ts".to_owned(), 'x', 'x', 'x'),
            ("gone.ts".to_owned(), 'x', 'x', 'x'),
        ]
    );
    assert!(plan.symbols.is_empty());
}

#[test]
fn a_sparse_delta_without_an_exact_manifest_is_unavailable() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    for transport in [None, Some(TransportChanges::default())] {
        let mut target = state(2, 2, "a", b"a");
        target.transport = transport;
        let error = encoder
            .encode(TransitionInput {
                project_id: &target.project_id,
                base_state_token: "t1",
                base: Some(&base),
                target: &target,
                sparse: true,
            })
            .unwrap_err();
        assert!(matches!(error, EncodeError::SparseUnavailable), "{error}");
    }
    // A full transition ignores the sparse request.
    let target = state(2, 2, "a", b"a");
    let result = encoder
        .encode(TransitionInput {
            project_id: &target.project_id,
            base_state_token: "",
            base: None,
            target: &target,
            sparse: true,
        })
        .unwrap();
    assert_eq!(result.mode, TransitionMode::Full);
}

#[test]
fn a_symbols_only_table_packs_as_a_full_transition() {
    // The symbols operation: evidence packed into a table with no path rows.
    let mut evidence = table(7, 0);
    evidence.symbols = vec![
        SymbolFact {
            references: Arc::from(vec![location("/p/a.ts", 1, 2)]),
            ..symbol("symbol:h:aaaaaaaaaaaaaaaaaaaaaaaa")
        },
        symbol("symbol:h:bbbbbbbbbbbbbbbbbbbbbbbb"),
    ];
    let frame = full(&TransitionEncoder::default(), &evidence);
    let plan = decoded(&frame);
    assert_eq!(plan.mode, TransitionMode::Full);
    assert_eq!(plan.target_generation, 7);
    assert!(plan.paths.is_empty());
    assert_eq!(plan.symbols.len(), 2);
    assert!(reencode_table_transition(&frame).unwrap() == frame);
}

// -- encoder/decoder agreement ----------------------------------------------

fn dense_entity() -> EntityFact {
    let declaration = |name: &str, start: u64| Declaration {
        name: name.into(),
        kind: "FunctionDeclaration".into(),
        location: location("/p/lib.d.ts", start, start + 4),
    };
    let resolved = |symbol: &str| ResolvedDeclaration {
        symbol: symbol.into(),
        name: "target".into(),
        kind: "MethodSignature".into(),
        location: location("/p/lib.d.ts", 40, 50),
        owners: Arc::from(vec![
            DeclarationOwner {
                symbol: "symbol:h:0123456789abcdef01234567".into(),
                name: "Owner".into(),
                kind: "InterfaceDeclaration".into(),
                location: location("/p/lib.d.ts", 0, 100),
            },
            DeclarationOwner {
                symbol: "".into(),
                name: "".into(),
                kind: "Namespace".into(),
                location: location("/p/other.d.ts", 3, 9),
            },
        ]),
        qualified_name: "Owner.target".into(),
        origin_module: "lib".into(),
        source_file: "/p/lib.d.ts".into(),
        standard_library: true,
    };
    let descriptor = TypeDescriptor {
        text: "Promise<void>".into(),
        origin_module: "typescript/lib".into(),
        alias_declarations: Arc::from(vec![declaration("Promise", 1), declaration("Alias", 9)]),
    };
    let mut row = entity("/p/a.ts", 5, "symbol:h:0123456789abcdef01234568");
    row.location.end_byte = 25;
    row.symbol_claim = SymbolClaim::Covering;
    row.type_descriptor = Some(Arc::new(descriptor.clone()));
    row.resolved_call = Some(Arc::new(ResolvedCall {
        target: "symbol:h:0123456789abcdef01234569".into(),
        return_type_text: "void".into(),
        assertion_signature: true,
        never_returns: true,
        validity: ResolvedCallValidity::Recovery,
        kind: CallKind::Construct,
        declaration: Some(resolved("symbol:h:0123456789abcdef0123456a")),
        default_library_binding: Some(Arc::new(resolved("symbol:h:0123456789abcdef0123456b"))),
        targets: Some(CallTargetSet {
            exhaustive: true,
            candidates: Arc::from(vec![
                resolved("symbol:h:0123456789abcdef0123456c"),
                resolved("symbol:h:0123456789abcdef0123456d"),
            ]),
        }),
        arguments: Arc::from(vec![
            ArgumentMapping {
                argument_index: 0,
                status: ArgumentMappingStatus::Resolved,
                unresolved: None,
                parameter: Some(ParameterFact {
                    index: 0,
                    symbol: "param".into(),
                    declaration: Some(declaration("p", 70)),
                    rest: true,
                    optional: true,
                    callability: Callability::UntypedCallable,
                    type_descriptor: Some(descriptor),
                    object_shape: Some(ObjectConstructionShape {
                        required_properties: Arc::from(vec![
                            ObjectConstructionProperty {
                                name: "items".into(),
                                witness: ConstructionWitness::EmptyArray,
                            },
                            ObjectConstructionProperty {
                                name: "map".into(),
                                witness: ConstructionWitness::EmptyObject,
                            },
                            ObjectConstructionProperty {
                                name: "other".into(),
                                witness: ConstructionWitness::Unknown,
                            },
                        ]),
                    }),
                }),
            },
            ArgumentMapping {
                argument_index: 2,
                status: ArgumentMappingStatus::Unresolved,
                unresolved: Some(ArgumentMappingReason::SpreadArgument),
                parameter: None,
            },
        ]),
    }));
    row.callability = Some(Callability::Mixed);
    row.constructability = Some(Constructability::Mixed);
    row.runtime_binding_kind = Some(RuntimeBindingKind::Open);
    row.runtime_value_domain = Some(RuntimeValueDomain::new(true, false, true, false));
    row.primitive_value_domain = PrimitiveValueDomain::new(
        true, true, false, false, false, true, false, false, true, false,
    );
    row.primitive_literal_candidates = Some(Arc::new(vec![
        PrimitiveLiteralCandidate {
            kind: PrimitiveLiteralKind::String,
            string: "on".into(),
            number: 0.0,
            boolean: false,
        },
        PrimitiveLiteralCandidate {
            kind: PrimitiveLiteralKind::Number,
            string: "".into(),
            number: -1.5,
            boolean: false,
        },
        PrimitiveLiteralCandidate {
            kind: PrimitiveLiteralKind::Boolean,
            string: "".into(),
            number: 0.0,
            boolean: true,
        },
    ]));
    row.call_result_domain = Some(RuntimeValueDomain::new(false, true, false, true));
    row.constant_value = Some(ConstantValue {
        kind: ConstantValueKind::Number,
        string: "".into(),
        number: 1.5,
    });
    row.array_shape = Some(ArrayShape::Mixed);
    row.tuple_shape =
        Some(TupleShape::try_new(2, true, Some(Callability::Callable), 1, Some(3)).unwrap());
    row.library_types = Some(Arc::new(vec!["Map".into(), "Set".into()]));
    row.reference_space = Some(ReferenceSpace::Both);
    row.runtime_identity = "runtime:h:7".into();
    row
}

fn dense_table() -> ProducerTable {
    let mut table = golden_wire_table();
    let mut early = table.entities_vec()[0].clone();
    early.location.start_byte = 2;
    let mut second = dense_entity();
    second.location.path = "/p/a.tsx".into();
    let mut third = dense_entity();
    third.location.path = "/p/a.tsx".into();
    third.location.start_byte = 9;
    third.symbol = "".into();
    third.symbol_claim = SymbolClaim::Unresolved;
    third.constant_value = Some(ConstantValue {
        kind: ConstantValueKind::String,
        string: "text".into(),
        number: 0.0,
    });
    let mut empty_domain = entity("/p/b.ts", 2, "x");
    empty_domain.primitive_value_domain = PrimitiveValueDomain::empty();
    let mut unknown_domain = entity("/p/c.ts", 2, "y");
    unknown_domain.primitive_value_domain = PrimitiveValueDomain::new(
        false, false, false, false, false, false, false, false, false, true,
    );
    second.location.start_byte = 5;
    table.entities = EntityRows::Flat(vec![early, second, third, empty_domain, unknown_domain]);
    table.sources.push(source_digest("/p/b.ts", b"b"));
    table.sources.push(source_digest("/p/c.ts", b"c"));
    table
}

#[test]
fn the_decoder_reads_back_every_row_the_encoder_writes() {
    let table = dense_table();
    let frame = full(&TransitionEncoder::default(), &table);
    let plan = decoded(&frame);
    assert_eq!(plan.mode, TransitionMode::Full);
    assert_eq!(plan.table_schema, v3::TYPE_FACTS_TABLE_SCHEMA_V23);
    assert_eq!(&*plan.project_id, "/p/tsconfig.json");

    let mut want = table.entities_vec().iter().collect::<Vec<_>>();
    let mut got = Vec::new();
    for operation in &plan.paths {
        if let v3::SlotOp::Replace(rows) = &operation.entities {
            got.extend(rows.iter());
        }
    }
    assert_eq!(got.len(), want.len());
    for (got, want) in got.iter().zip(want.drain(..)) {
        assert_eq!(*got, want, "entity at {:?}", want.location);
    }
    let files: Vec<&FileFact> = plan
        .paths
        .iter()
        .filter_map(|operation| match &operation.file {
            v3::SlotOp::Replace(file) => Some(file),
            _ => None,
        })
        .collect();
    assert_eq!(files, table.files.iter().collect::<Vec<_>>());
    let symbols: Vec<&SymbolFact> = plan
        .symbols
        .iter()
        .map(|operation| match operation {
            v3::SymbolOp::Replace(fact) => fact,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(symbols, table.symbols.iter().collect::<Vec<_>>());
}

#[test]
fn every_schema_round_trips_its_own_frames() {
    let table = dense_table();
    for schema in v3::SUPPORTED_TABLE_SCHEMAS {
        // The dense table carries rows that older schemas cannot express; the
        // encoder drops them there (as Go does), and what it keeps must
        // decode and re-encode to the same bytes.
        let mut table = table.clone();
        if schema < v3::TYPE_FACTS_TABLE_SCHEMA_V6 {
            let EntityRows::Flat(rows) = &mut table.entities else {
                unreachable!()
            };
            for row in rows {
                if let Some(call) = &mut row.resolved_call {
                    let mut plain = (**call).clone();
                    plain.targets = None;
                    *call = Arc::new(plain);
                }
            }
        }
        if schema < v3::TYPE_FACTS_TABLE_SCHEMA_V5 {
            let EntityRows::Flat(rows) = &mut table.entities else {
                unreachable!()
            };
            for row in rows {
                row.symbol_claim = SymbolClaim::Partial;
                if row.symbol.is_empty() {
                    row.symbol_claim = SymbolClaim::Absent;
                }
            }
        }
        let frame = full(&TransitionEncoder::new(schema), &table);
        match v3::decode_table_transition(&frame) {
            Ok(plan) => assert_eq!(plan.table_schema, schema),
            Err(error) => {
                // Old schemas have a smaller flag space than the dense rows
                // use; the decoder refusing them is the schema gate working.
                assert!(
                    schema < v3::TYPE_FACTS_TABLE_SCHEMA_V18,
                    "schema {schema}: {error}"
                );
                continue;
            }
        }
        let again = reencode_table_transition(&frame).unwrap();
        assert!(again == frame, "schema {schema} changed on re-encode");
    }
}

#[test]
fn deltas_of_every_shape_round_trip() {
    let encoder = TransitionEncoder::default();
    let base = state(1, 1, "a", b"a");
    let mut edit = state(2, 2, "a2", b"a2");
    edit.symbols[0].references = Arc::from(vec![location("a.ts", 4, 5), location("b.ts", 1, 2)]);
    edit.transport = transport_manifest(
        Some(&base),
        &edit,
        Some(&exact_closure(&["a", "a2", "b"])),
        &paths(&["a.ts"]),
    );
    let exact = delta(&encoder, &base, &edit, "t1");
    let mut fallback_target = edit.clone();
    fallback_target.transport = None;
    let fallback = delta(&encoder, &base, &fallback_target, "t1");
    let mut sparse_target = edit.clone();
    sparse_target.transport = rust_owned_transport_manifest(Some(&base), Some(&paths(&["a.ts"])));
    let sparse = encoder
        .encode(TransitionInput {
            project_id: &sparse_target.project_id,
            base_state_token: "t1",
            base: Some(&base),
            target: &sparse_target,
            sparse: true,
        })
        .unwrap();
    for (name, result) in [("exact", exact), ("fallback", fallback), ("sparse", sparse)] {
        assert_eq!(result.mode, TransitionMode::Delta, "{name}");
        let again = reencode_table_transition(&result.bytes).unwrap();
        assert!(again == result.bytes, "{name} changed on re-encode");
    }
}

#[test]
fn hashed_symbols_pack_to_twelve_bytes_and_other_strings_prefix_code() {
    let mut table = table(1, 1);
    table.symbols = vec![
        symbol("symbol:h:00112233445566778899aabb"),
        symbol("symbol:h:short"),
        symbol("symbol:h:00112233445566778899aabbcc"),
        symbol("symbol:h:00112233445566778899aabz"),
    ];
    table.symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let frame = full(&TransitionEncoder::default(), &table);
    let hashed_entries = frame
        .windows(14)
        .filter(|w| w[0] == 1 && w[1] == 0x00)
        .count();
    assert!(hashed_entries >= 1);
    let plan = decoded(&frame);
    let ids: Vec<&str> = plan.symbols.iter().map(|op| op.id().as_ref()).collect();
    let mut want: Vec<&str> = table.symbols.iter().map(|fact| fact.id.as_ref()).collect();
    want.sort_unstable();
    assert_eq!(ids, want);
    assert!(reencode_table_transition(&frame).unwrap() == frame);
}

// -- Go: protocolv3_compact_test.go -----------------------------------------

#[test]
fn compact_demands_round_trip() {
    use crate::v3::{CompactDemands, EntityDemand, compact_demands};
    let demand = |path: &str, start: u64, end: u64| EntityDemand {
        location: location(path, start, end),
        ..EntityDemand::default()
    };
    let mut every_flag = demand("/p/b.tsx", 9, 12);
    every_flag.query_location = Some(location("/p/other.tsx", 0, 2));
    every_flag.symbol = true;
    every_flag.references = true;
    every_flag.type_descriptor = true;
    every_flag.resolved_call = true;
    every_flag.callability = true;
    every_flag.constructability = true;
    every_flag.runtime_value_domain = true;
    every_flag.primitive_value_domain = true;
    every_flag.primitive_literal_candidates = true;
    every_flag.parameter_object_shape = true;
    every_flag.call_result_domain = true;
    every_flag.constant_value = true;
    every_flag.array_shape = true;
    every_flag.tuple_shape = true;
    every_flag.library_types = true;
    every_flag.reference_space = true;
    every_flag.runtime_identity = true;
    let mut first = demand("/p/a.tsx", 1, 4);
    first.symbol = true;
    first.references = true;
    let mut second = demand("/p/a.tsx", 5, 9);
    second.query_location = Some(location("/p/a.tsx", 6, 8));
    second.symbol = true;
    second.type_descriptor = true;
    second.resolved_call = true;
    let mut third = demand("/p/b.tsx", 2, 8);
    third.r#async = true;
    third.structural_accessor = true;
    let demands = vec![first, second, third, every_flag];
    let compact = compact_demands(&demands);
    // Each path's rows go through the same CBOR the request carries.
    let carried: CompactDemands = crate::decode(&crate::encode(&compact).unwrap()).unwrap();
    assert_eq!(expand_compact_demands(&carried).unwrap(), demands);
}

#[test]
fn compact_expansion_fails_closed_on_string_gaps_and_bad_varints() {
    use crate::v3::{CompactDemandGroup, CompactDemands};
    let gap = CompactDemands {
        strings: vec![String::new()],
        groups: vec![CompactDemandGroup(9, Vec::new())],
    };
    assert!(
        expand_compact_demands(&gap)
            .unwrap_err()
            .contains("out of range")
    );
    let truncated = CompactDemands {
        strings: vec![String::new(), "/p/a.ts".into()],
        groups: vec![CompactDemandGroup(1, vec![0x80])],
    };
    assert!(
        expand_compact_demands(&truncated)
            .unwrap_err()
            .contains("truncated")
    );
    let overflow = CompactDemands {
        strings: vec![String::new(), "/p/a.ts".into()],
        groups: vec![CompactDemandGroup(1, vec![0xff; 10])],
    };
    assert!(
        expand_compact_demands(&overflow)
            .unwrap_err()
            .contains("overflowing")
    );
    let wrapped = CompactDemands {
        strings: vec![String::new(), "/p/a.ts".into()],
        groups: vec![CompactDemandGroup(
            1,
            [
                vec![0x00],
                vec![0xff; 9].into_iter().chain([0x01]).collect(),
                vec![0x02],
            ]
            .concat(),
        )],
    };
    assert!(
        expand_compact_demands(&wrapped)
            .unwrap_err()
            .contains("location overflow")
    );
}

#[test]
fn compact_groups_expand_alone_and_filter_by_flag() {
    use crate::v3::{
        CompactDemandGroup, CompactDemands, DEMAND_FLAG_ASYNC, DEMAND_FLAG_REFERENCES,
        EntityDemand, compact_demands,
    };
    let demand = |path: &str, start: u64, edit: &dyn Fn(&mut EntityDemand)| {
        let mut demand = EntityDemand {
            location: location(path, start, start + 2),
            symbol: true,
            ..EntityDemand::default()
        };
        edit(&mut demand);
        demand
    };
    let demands = vec![
        demand("/p/a.ts", 1, &|_| {}),
        demand("/p/a.ts", 5, &|demand| {
            demand.r#async = true;
            demand.query_location = Some(location("/p/other.ts", 0, 3));
        }),
        demand("/p/b.ts", 2, &|demand| demand.r#async = true),
    ];
    let compact = compact_demands(&demands);
    assert_eq!(compact.groups.len(), 2);

    // One group expands on its own, shape-checked first.
    let first = expand_compact_group(&compact.groups[0], &compact.strings).unwrap();
    assert_eq!(first, demands[..2]);
    assert_eq!(compact_demand_shape(&compact.groups).unwrap(), (3, 1));
    assert_eq!(
        compact_demand_shape(std::slice::from_ref(&compact.groups[1])).unwrap(),
        (1, 0)
    );

    // Only the rows carrying the flag are materialized.
    let mut asynchronous = Vec::new();
    for group in &compact.groups {
        append_compact_demands_with_flag(
            &mut asynchronous,
            group,
            &compact.strings,
            DEMAND_FLAG_ASYNC,
        )
        .unwrap();
    }
    assert_eq!(asynchronous, [demands[1].clone(), demands[2].clone()]);
    let mut references = Vec::new();
    append_compact_demands_with_flag(
        &mut references,
        &compact.groups[0],
        &compact.strings,
        DEMAND_FLAG_REFERENCES,
    )
    .unwrap();
    assert!(references.is_empty());

    // A malformed group is refused whole, before any row is returned, and a
    // filtered walk still validates the rows it skips.
    let strings = vec![String::new(), "/p/a.ts".into()];
    let truncated = CompactDemandGroup(1, vec![0x80]);
    assert!(
        expand_compact_group(&truncated, &strings)
            .unwrap_err()
            .contains("truncated")
    );
    let mut sink = Vec::new();
    assert!(
        append_compact_demands_with_flag(&mut sink, &truncated, &strings, DEMAND_FLAG_ASYNC)
            .is_err()
    );
    // The path index is checked before the rows are read.
    let bad_path = CompactDemands {
        strings,
        groups: vec![CompactDemandGroup(7, vec![0x80])],
    };
    assert!(
        expand_compact_group(&bad_path.groups[0], &bad_path.strings)
            .unwrap_err()
            .contains("truncated"),
        "a group's shape is checked before its path"
    );
    assert!(
        append_compact_demands_with_flag(
            &mut Vec::new(),
            &bad_path.groups[0],
            &bad_path.strings,
            DEMAND_FLAG_ASYNC
        )
        .unwrap_err()
        .contains("out of range")
    );
}
