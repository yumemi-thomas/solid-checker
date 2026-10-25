use super::*;

// Every retained row pays for every inline optional field, so the budget
// rises only after the field has been made as small as it can be. `TupleShape`
// cost 16 bytes with a `usize` length and 8 with a `u32` one — a tuple cannot
// have more slots than the source has bytes — and the `Option` rides the
// callability enum's niche. Boxing it would buy the other 8 bytes back at the
// price of an allocation per fact; it is three scalars, so it stays inline.
//
// `library_types` is the opposite call and shows the rule working: as an
// `Arc<[Arc<str>]>` it cost 16 bytes on every row to carry an absence, because
// a slice pointer is fat. Behind a thin `Arc`, like `resolved_call` and
// `type_descriptor` before it, it costs 8. Primitive literal candidates
// use the same thin-Arc representation; the one new pointer raises the
// deliberate retained-row ceiling by exactly 8 bytes. `declared_keys` (ADR
// 0394) is the same thin `Arc` and raises it by 8 more: it is demanded only at
// an intrinsic element's spread arguments, so a bare `Option` is the cheapest
// way to carry its absence on every other row.
#[test]
fn retained_entity_rows_keep_optional_evidence_bounded() {
    assert!(
        std::mem::size_of::<EntityFact>() <= 160,
        "EntityFact is {} bytes; optional evidence exceeded the bounded row budget",
        std::mem::size_of::<EntityFact>()
    );
}

#[test]
fn sidecar_request_fast_path_preserves_canonical_cbor() {
    let location = Location {
        path: "a.ts".into(),
        start_byte: 1,
        end_byte: 2,
    };
    let request = v3::Request {
        schema: v3::TYPE_FACTS_SCHEMA_V1,
        request_id: 7,
        operation: v3::Operation::Analyze,
        project_id: "project".into(),
        generation: 3,
        changes: vec![v3::FileChange {
            path: "a.ts".into(),
            version: 3,
            source: b"let a = 1".to_vec(),
            deleted: false,
        }],
        demands: vec![v3::EntityDemand {
            location,
            query_location: None,
            symbol: true,
            type_descriptor: true,
            resolved_call: false,
            references: true,
            r#async: false,
            structural_accessor: false,
            callability: false,
            constructability: false,
            runtime_value_domain: false,
            primitive_value_domain: false,
            primitive_literal_candidates: false,
            parameter_object_shape: false,
            call_result_domain: false,
            constant_value: false,
            array_shape: false,
            tuple_shape: false,
            library_types: false,
            reference_space: false,
            runtime_identity: false,
            declared_keys: false,
        }],
        compact_demands: Some(v3::CompactDemands {
            groups: vec![v3::CompactDemandGroup(1, vec![7, 1, 1, 1, 1, 1])],
            strings: vec![String::new(), "a.ts".into()],
        }),
        state_token: "9".into(),
        reset_state: false,
        removed_demand_paths: vec!["old.ts".into()],
        symbol_queries: Vec::new(),
        release_analysis: false,
        reference_changes: false,
        reference_paths: Vec::new(),
        cancel_request_id: 2,
        module_graph: None,
    };
    assert_eq!(
        encode_sidecar_request(&request).unwrap(),
        encode(&request).unwrap()
    );
}

#[test]
fn compact_demands_round_trip() {
    let location = |path: &str, start: u64, end: u64| Location {
        path: path.into(),
        start_byte: start,
        end_byte: end,
    };
    let demands = vec![
        v3::EntityDemand {
            location: location("a.ts", 1, 4),
            query_location: None,
            symbol: true,
            type_descriptor: false,
            resolved_call: false,
            references: true,
            r#async: false,
            structural_accessor: false,
            callability: false,
            constructability: false,
            runtime_value_domain: false,
            primitive_value_domain: false,
            primitive_literal_candidates: false,
            parameter_object_shape: false,
            call_result_domain: false,
            constant_value: false,
            array_shape: false,
            tuple_shape: false,
            library_types: false,
            reference_space: false,
            runtime_identity: false,
            declared_keys: false,
        },
        v3::EntityDemand {
            location: location("a.ts", 5, 9),
            query_location: Some(location("a.ts", 6, 8)),
            symbol: true,
            type_descriptor: true,
            resolved_call: true,
            references: false,
            r#async: true,
            structural_accessor: true,
            callability: true,
            constructability: true,
            runtime_value_domain: true,
            primitive_value_domain: true,
            primitive_literal_candidates: true,
            parameter_object_shape: false,
            call_result_domain: true,
            constant_value: true,
            array_shape: true,
            tuple_shape: true,
            library_types: true,
            reference_space: true,
            runtime_identity: true,
            declared_keys: false,
        },
        v3::EntityDemand {
            location: location("b.ts", 2, 8),
            query_location: None,
            symbol: false,
            type_descriptor: false,
            resolved_call: false,
            references: false,
            r#async: true,
            structural_accessor: false,
            callability: false,
            constructability: false,
            runtime_value_domain: false,
            primitive_value_domain: false,
            primitive_literal_candidates: false,
            parameter_object_shape: false,
            call_result_domain: false,
            constant_value: false,
            array_shape: false,
            tuple_shape: false,
            library_types: false,
            reference_space: false,
            runtime_identity: false,
            declared_keys: false,
        },
    ];
    let compact = v3::compact_demands(&demands);
    let decoded: v3::CompactDemands = decode(&encode(&compact).unwrap()).unwrap();
    assert_eq!(decoded, compact);
    assert_eq!(decoded.groups.len(), 2);
    assert_eq!(decoded.strings[0], "");
}

#[test]
fn frame_codec_round_trips_and_rejects_oversized_prefixes() {
    let mut framed = Vec::new();
    write_frame(&mut framed, b"payload").unwrap();
    assert_eq!(read_frame(&mut framed.as_slice()).unwrap(), b"payload");

    let oversized = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap().to_le_bytes();
    assert!(matches!(
        read_frame(&mut oversized.as_slice()),
        Err(TypeFactsError::MessageLimit { .. })
    ));
}

#[test]
fn rejects_non_deterministic_and_unsafe_cbor_before_typed_decode() {
    for (label, encoded) in [
        ("overlong integer", vec![0x18, 0x01]),
        ("indefinite array", vec![0x9f, 0xff]),
        (
            "duplicate map key",
            vec![0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02],
        ),
        (
            "non-canonical map order",
            vec![0xa2, 0x62, b'a', b'a', 0x01, 0x61, b'b', 0x02],
        ),
        ("tag", vec![0xc0, 0x01]),
        ("null", vec![0xf6]),
    ] {
        assert!(
            matches!(
                decode::<ciborium::Value>(&encoded),
                Err(TypeFactsError::DeterministicCbor(_))
            ),
            "{label} was accepted"
        );
    }

    let mut too_deep = vec![0x81; MAX_NESTING_DEPTH];
    too_deep.push(0x01);
    assert!(matches!(
        decode::<ciborium::Value>(&too_deep),
        Err(TypeFactsError::DeterministicCbor(_))
    ));
}
