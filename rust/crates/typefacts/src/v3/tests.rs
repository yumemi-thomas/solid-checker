use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::{
    ArgumentMappingReason, ArgumentMappingStatus, CallKind, Callability, Constructability,
    PrimitiveLiteralCandidate, PrimitiveLiteralKind, ReferenceSpace, ResolvedCallValidity,
};

use super::{
    SlotOp, TYPE_FACTS_SCHEMA_SHA256, TYPE_FACTS_TABLE_SCHEMA_V3, TYPE_FACTS_TABLE_SCHEMA_V14,
    TYPE_FACTS_TABLE_SCHEMA_V15, TYPE_FACTS_TABLE_SCHEMA_V16, TYPE_FACTS_TABLE_SCHEMA_V17,
    TYPE_FACTS_TABLE_SCHEMA_V18, TYPE_FACTS_TABLE_SCHEMA_V19, TYPE_FACTS_TABLE_SCHEMA_V21,
    TransitionMode, decode_table_transition, parse_argument_mapping_reason,
    parse_argument_mapping_status, parse_call_kind, parse_callability, parse_constructability,
    parse_reference_space, parse_resolved_call_validity, push_uvarint,
};

fn push_test_string(frame: &mut Vec<u8>, value: &str) {
    push_uvarint(frame, 0);
    push_uvarint(frame, 0);
    push_uvarint(frame, value.len() as u64);
    frame.extend_from_slice(value.as_bytes());
}

fn empty_full_transition() -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, 3, 0, 1, 2] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    // project, empty base token, no path operations, no symbol operations.
    for value in [1, 0, 0, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn runtime_value_domain_transition(table_schema: u64, domain_bits: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    // Project, empty base token, one path; path index, entity-replace tag.
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, value-domain field.
    for value in [1, 0, 1, 0, 32, domain_bits, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn primitive_literal_candidates_transition(table_schema: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 4] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    push_test_string(&mut frame, "alpha");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity with string "alpha", number 2, and boolean true.
    for value in [1, 0, 1, 0, 16384, 3, 0, 3, 1, 2.0_f64.to_bits(), 2, 1, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn call_result_domain_transition(table_schema: u64, domain_bits: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, call-result field.
    for value in [1, 0, 1, 0, 128, domain_bits, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn array_shape_transition(table_schema: u64, tag: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, array-shape field.
    for value in [1, 0, 1, 0, 512, tag, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn library_types_transition(table_schema: u64, count: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 4] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    push_test_string(&mut frame, "Date");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, library-types field.
    let mut row = vec![1, 0, 1, 0, 2048, count];
    row.extend(std::iter::repeat_n(3, usize::try_from(count).unwrap()));
    row.push(0);
    for value in row {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn primitive_value_domain_transition(table_schema: u64, domain_bits: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, primitive-domain field.
    for value in [1, 0, 1, 0, 4096, domain_bits, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn constructability_transition(table_schema: u64, tag: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, constructability field.
    for value in [1, 0, 1, 0, 8192, tag, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn callability_transition(table_schema: u64, tag: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, callability field. The flag
    // bit and the row layout are identical at v14 and v15 — only the tag
    // space differs, which is exactly what the freeze below checks.
    for value in [1, 0, 1, 0, 4, tag, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn tuple_shape_transition(
    table_schema: u64,
    packed: u64,
    element_zero: u64,
    min_parameters: u64,
    exact_length_plus_one: u64,
) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, tuple-shape field. V13
    // appends exactLength + 1, reserving zero for absence.
    let mut row = vec![1, 0, 1, 0, 1024, packed, element_zero, min_parameters];
    if table_schema >= 13 {
        row.push(exact_length_plus_one);
    }
    row.push(0);
    for value in row {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn constant_value_transition(table_schema: u64, tag: u64, payload: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    let dictionary_count = if tag == 0 { 4 } else { 3 };
    for value in [1, 0, table_schema, 0, 1, dictionary_count] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    if tag == 0 {
        push_test_string(&mut frame, "value");
    }
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, constant-value field.
    for value in [1, 0, 1, 0, 256, tag, payload, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

fn unresolved_symbol_transition(table_schema: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    // Project, empty base token, one path; path index, entity-replace tag.
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, no symbol, explicit-unresolved flag.
    for value in [1, 0, 1, 0, 64, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

#[test]
fn handshake_hash_matches_frozen_schema() {
    let actual = format!(
        "sha256:{:x}",
        Sha256::digest(include_bytes!(
            "../../../../../schema/typefacts-v1.schema.json"
        ))
    );
    assert_eq!(actual, TYPE_FACTS_SCHEMA_SHA256);
}

#[test]
fn full_transition_header_is_strict_and_self_identifying() {
    let valid = empty_full_transition();
    let transition = decode_table_transition(&valid).unwrap();
    assert_eq!(transition.mode, TransitionMode::Full);
    assert_eq!(transition.table_schema, 3);
    assert_eq!(transition.base_generation, 0);
    assert_eq!(transition.target_generation, 1);
    assert_eq!(transition.project_id.as_ref(), "/p/tsconfig.json");
    assert!(transition.base_state_token.is_empty());
    assert!(transition.paths.is_empty());
    assert!(transition.symbols.is_empty());

    assert!(decode_table_transition(&valid[..valid.len() - 1]).is_err());
    assert!(decode_table_transition(&[1, 0, 3, 0, 1]).is_err());
    let mut trailing = valid.to_vec();
    trailing.push(0);
    assert!(decode_table_transition(&trailing).is_err());

    let mut overlong_version = vec![0x81, 0];
    overlong_version.extend_from_slice(&valid[1..]);
    assert!(decode_table_transition(&overlong_version).is_err());
}

#[test]
fn wire_table_v4_decodes_runtime_value_domains_and_v3_stays_frozen() {
    let transition = decode_table_transition(&runtime_value_domain_transition(4, 3)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        entities[0].runtime_value_domain,
        Some(crate::RuntimeValueDomain::new(true, true, false, false))
    );
    assert!(decode_table_transition(&runtime_value_domain_transition(4, 16)).is_err());
    assert!(decode_table_transition(&runtime_value_domain_transition(3, 3)).is_err());
}

#[test]
fn wire_table_v5_decodes_explicit_unresolved_symbols_and_v4_stays_frozen() {
    let transition = decode_table_transition(&unresolved_symbol_transition(5)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert!(entities[0].symbol.is_empty());
    assert!(entities[0].symbol_unresolved());
    assert!(decode_table_transition(&unresolved_symbol_transition(4)).is_err());
}

/// One path with one entity whose symbol is the string-table entry at
/// `symbol_index` (0 is the empty string, which means no symbol), carrying
/// the flag word `flags`.
fn symbol_flag_transition(table_schema: u64, symbol_index: u64, flags: u64) -> Vec<u8> {
    let mut frame = Vec::new();
    for value in [1, 0, table_schema, 0, 1, 3] {
        push_uvarint(&mut frame, value);
    }
    push_test_string(&mut frame, "");
    push_test_string(&mut frame, "/p/tsconfig.json");
    push_test_string(&mut frame, "/p/a.ts");
    for value in [1, 0, 1, 2, 4] {
        push_uvarint(&mut frame, value);
    }
    // One entity: start 0, length 1, the symbol, the flag word. Bit 6 has no
    // payload, so the row ends after it.
    for value in [1, 0, 1, symbol_index, flags, 0] {
        push_uvarint(&mut frame, value);
    }
    frame
}

#[test]
fn wire_table_v21_reads_bit_six_as_span_coverage_on_a_symbol() {
    // Bit 6 on a symbol-bearing row: the symbol covers the whole span.
    let covering = decode_table_transition(&symbol_flag_transition(21, 2, 64)).unwrap();
    let SlotOp::Replace(entities) = &covering.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(entities[0].symbol.as_ref(), "/p/a.ts");
    assert!(entities[0].symbol_covers_span());
    assert_eq!(entities[0].symbol_claim, crate::SymbolClaim::Covering);

    // No bit 6 on a symbol-bearing row: a token inside the span, not the value.
    let plain = decode_table_transition(&symbol_flag_transition(21, 2, 0)).unwrap();
    let SlotOp::Replace(entities) = &plain.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert!(!entities[0].symbol_covers_span());
    assert_eq!(entities[0].symbol_claim, crate::SymbolClaim::Partial);

    // Bit 6 on a symbol-less row is still the unresolved claim at every schema.
    let unresolved = decode_table_transition(&symbol_flag_transition(21, 0, 64)).unwrap();
    let SlotOp::Replace(entities) = &unresolved.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(entities[0].symbol_claim, crate::SymbolClaim::Unresolved);

    // Before v21, bit 6 on a symbol-bearing row is the contradiction it always
    // was.
    assert!(decode_table_transition(&symbol_flag_transition(20, 2, 64)).is_err());
}

#[test]
fn wire_table_v7_decodes_call_result_domains_and_v6_stays_frozen() {
    let transition = decode_table_transition(&call_result_domain_transition(7, 4)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        entities[0].call_result_domain,
        Some(crate::RuntimeValueDomain::new(false, false, true, false))
    );
    assert!(decode_table_transition(&call_result_domain_transition(7, 16)).is_err());
    assert!(decode_table_transition(&call_result_domain_transition(6, 4)).is_err());
}

#[test]
fn wire_table_v8_decodes_constant_values_and_v7_stays_frozen() {
    let string_transition = decode_table_transition(&constant_value_transition(8, 0, 3)).unwrap();
    let SlotOp::Replace(string_entities) = &string_transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        string_entities[0].constant_value,
        Some(crate::ConstantValue {
            kind: crate::ConstantValueKind::String,
            string: "value".into(),
            number: 0.0,
        })
    );

    let number_transition =
        decode_table_transition(&constant_value_transition(8, 1, 42.5_f64.to_bits())).unwrap();
    let SlotOp::Replace(number_entities) = &number_transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        number_entities[0].constant_value,
        Some(crate::ConstantValue {
            kind: crate::ConstantValueKind::Number,
            string: "".into(),
            number: 42.5,
        })
    );
    assert!(decode_table_transition(&constant_value_transition(7, 0, 3)).is_err());
    assert!(decode_table_transition(&constant_value_transition(8, 2, 0)).is_err());
    assert!(decode_table_transition(&constant_value_transition(8, 1, f64::NAN.to_bits())).is_err());
}

#[test]
fn wire_table_v9_decodes_array_shapes_and_v8_stays_frozen() {
    for (tag, expected) in [
        (0, crate::ArrayShape::Array),
        (1, crate::ArrayShape::NotArray),
        (2, crate::ArrayShape::Mixed),
        (3, crate::ArrayShape::Unknown),
    ] {
        let transition = decode_table_transition(&array_shape_transition(9, tag)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert_eq!(entities[0].array_shape, Some(expected));
    }
    // The field is v9-only, and an out-of-range tag is a decode error rather
    // than a silently dropped fact.
    assert!(decode_table_transition(&array_shape_transition(8, 0)).is_err());
    assert!(decode_table_transition(&array_shape_transition(9, 4)).is_err());
}

#[test]
fn wire_table_v11_decodes_tuple_shapes_and_v10_stays_frozen() {
    // packed = fixed_length << 1 | has_rest; element zero code 0 = callable.
    let transition = decode_table_transition(&tuple_shape_transition(11, 5, 0, 2, 0)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        entities[0].tuple_shape,
        Some(crate::TupleShape::try_new(2, true, Some(Callability::Callable), 2, None,).unwrap())
    );
    assert!(entities[0].tuple_shape.unwrap().element_zero_accepts(2));
    // The same callable slot, asked for one fewer argument than it requires.
    assert!(!entities[0].tuple_shape.unwrap().element_zero_accepts(1));

    let plain = decode_table_transition(&tuple_shape_transition(11, 4, 1, 0, 0)).unwrap();
    let SlotOp::Replace(plain_entities) = &plain.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        plain_entities[0].tuple_shape,
        Some(
            crate::TupleShape::try_new(2, false, Some(Callability::NonCallable), 0, None,).unwrap()
        )
    );
    assert!(
        !plain_entities[0]
            .tuple_shape
            .unwrap()
            .element_zero_accepts(2)
    );

    assert!(decode_table_transition(&tuple_shape_transition(10, 4, 0, 0, 0)).is_err());

    let exact = decode_table_transition(&tuple_shape_transition(13, 4, 0, 2, 3)).unwrap();
    let SlotOp::Replace(exact_entities) = &exact.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        exact_entities[0].tuple_shape.unwrap().exact_length(),
        Some(2)
    );
}

// Tuple shape's elementZero shares parse_callability (and so tag 4) with
// the top-level Callability field, and TupleShape's own packed
// representation carries UntypedCallable as arm 5 (see element_zero's
// match in lib.rs). Neither side had a regression test for the two
// meeting at the wire boundary until now.
#[test]
fn wire_table_v15_decodes_untyped_callable_tuple_element_zero() {
    // packed = 2 -> fixed_length 1, has_rest false.
    let transition = decode_table_transition(&tuple_shape_transition(15, 2, 4, 0, 0)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        entities[0].tuple_shape,
        Some(
            crate::TupleShape::try_new(1, false, Some(Callability::UntypedCallable), 0, None)
                .unwrap()
        )
    );
    assert_eq!(
        entities[0].tuple_shape.unwrap().element_zero(),
        Some(Callability::UntypedCallable)
    );
    // element_zero_accepts is deliberately false for this rung: it is
    // callable, but no argument count can be checked against it.
    assert!(!entities[0].tuple_shape.unwrap().element_zero_accepts(0));

    // The field's tag space is frozen exactly like the top-level one: a
    // v14 row never expressed tag 4 and refuses it rather than reading it
    // forward.
    assert!(decode_table_transition(&tuple_shape_transition(14, 2, 4, 0, 0)).is_err());
}

#[test]
fn wire_table_v12_decodes_library_type_sets_and_v11_stays_frozen() {
    let transition = decode_table_transition(&library_types_transition(12, 2)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    let names = entities[0].library_types.as_deref().unwrap();
    assert_eq!(names.len(), 2);
    assert_eq!(&*names[0], "Date");
    // An absent set stays None rather than an empty list, so "not demanded"
    // and "nothing from the standard library" remain distinguishable.
    assert!(decode_table_transition(&library_types_transition(11, 1)).is_err());
    assert!(decode_table_transition(&tuple_shape_transition(11, 4, 9, 0, 0)).is_err());
}

#[test]
fn wire_table_v13_decodes_primitive_value_domains_and_v12_stays_frozen() {
    // string | boolean | null | undefined
    let transition =
        decode_table_transition(&primitive_value_domain_transition(13, 1 | 4 | 32 | 64)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    let domain = entities[0]
        .primitive_value_domain
        .present()
        .expect("primitive domain is present");
    assert!(domain.may_be_string());
    assert!(domain.may_be_boolean());
    assert!(domain.may_be_null());
    assert!(domain.may_be_undefined());
    assert!(!domain.may_be_number());
    assert!(!domain.may_be_object());
    assert!(!domain.unknown());

    let finite = decode_table_transition(&primitive_value_domain_transition(13, 2 | 512)).unwrap();
    let SlotOp::Replace(finite_entities) = &finite.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert!(
        finite_entities[0]
            .primitive_value_domain
            .numbers_are_finite()
    );

    assert!(decode_table_transition(&primitive_value_domain_transition(12, 1)).is_err());
    assert!(decode_table_transition(&primitive_value_domain_transition(13, 1024)).is_err());
}

#[test]
fn wire_table_v14_decodes_constructability_and_v13_stays_frozen() {
    for (tag, expected) in [
        (0, Constructability::Constructable),
        (1, Constructability::NonConstructable),
        (2, Constructability::Mixed),
        (3, Constructability::Unknown),
    ] {
        let transition = decode_table_transition(&constructability_transition(14, tag)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert_eq!(entities[0].constructability, Some(expected));
        // The fact is independent: nothing else on the row is invented.
        assert_eq!(entities[0].callability, None);
    }

    // v13 froze at flag bit 12, so bit 13 is an unknown flag there.
    assert!(decode_table_transition(&constructability_transition(13, 0)).is_err());
    assert!(decode_table_transition(&constructability_transition(14, 4)).is_err());
}

#[test]
fn wire_table_v15_decodes_untyped_callable_and_v14_stays_frozen() {
    for (tag, expected) in [
        (0, Callability::Callable),
        (1, Callability::NonCallable),
        (2, Callability::Mixed),
        (3, Callability::Unknown),
        (4, Callability::UntypedCallable),
    ] {
        let transition = decode_table_transition(&callability_transition(15, tag)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert_eq!(entities[0].callability, Some(expected));
        assert_eq!(entities[0].constructability, None);
    }

    // v15 adds no flag bit and no field, so a v14 row decodes unchanged for
    // every tag v14 ever emitted — and refuses the one it never could.
    for (tag, expected) in [
        (0, Callability::Callable),
        (1, Callability::NonCallable),
        (2, Callability::Mixed),
        (3, Callability::Unknown),
    ] {
        let transition = decode_table_transition(&callability_transition(14, tag)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert_eq!(entities[0].callability, Some(expected));
    }
    assert!(decode_table_transition(&callability_transition(14, 4)).is_err());
    assert!(decode_table_transition(&callability_transition(13, 4)).is_err());
    assert!(decode_table_transition(&callability_transition(15, 5)).is_err());
}

#[test]
fn wire_table_v16_decodes_primitive_literal_candidates_and_v15_stays_frozen() {
    let transition = decode_table_transition(&primitive_literal_candidates_transition(16)).unwrap();
    let SlotOp::Replace(entities) = &transition.paths[0].entities else {
        panic!("entity row was not replaced");
    };
    assert_eq!(
        entities[0]
            .primitive_literal_candidates
            .as_deref()
            .unwrap()
            .as_slice(),
        [
            PrimitiveLiteralCandidate {
                kind: PrimitiveLiteralKind::String,
                string: Arc::from("alpha"),
                number: 0.0,
                boolean: false,
            },
            PrimitiveLiteralCandidate {
                kind: PrimitiveLiteralKind::Number,
                string: Arc::from(""),
                number: 2.0,
                boolean: false,
            },
            PrimitiveLiteralCandidate {
                kind: PrimitiveLiteralKind::Boolean,
                string: Arc::from(""),
                number: 0.0,
                boolean: true,
            },
        ]
    );
    assert!(decode_table_transition(&primitive_literal_candidates_transition(15)).is_err());
    assert_eq!(TYPE_FACTS_TABLE_SCHEMA_V16, 16);
    assert_eq!(TYPE_FACTS_TABLE_SCHEMA_V17, 17);
    assert_eq!(TYPE_FACTS_TABLE_SCHEMA_V18, 18);
    assert_eq!(TYPE_FACTS_TABLE_SCHEMA_V19, 19);
    assert_eq!(TYPE_FACTS_TABLE_SCHEMA_V21, 21);
}

#[test]
fn numeric_enum_tags_decode_the_dense_go_golden() {
    let response: super::Response = crate::decode(include_bytes!(
        "../../../../../benchmarks/typefacts/phase1/typefacts-v3-response-golden.cbor"
    ))
    .expect("decode response golden");
    let transition =
        decode_table_transition(&response.table_transition).expect("decode transition");
    let entity = transition
        .paths
        .iter()
        .find_map(|path| match &path.entities {
            SlotOp::Replace(entities) => entities.first(),
            SlotOp::Unchanged | SlotOp::Remove => None,
        })
        .expect("golden entity");
    assert_eq!(entity.callability, Some(Callability::Callable));
    assert_eq!(entity.reference_space, Some(ReferenceSpace::Both));
    let call = entity.resolved_call.as_ref().expect("golden resolved call");
    assert!(
        call.default_library_binding.is_none(),
        "frozen tables cannot supply new binding evidence"
    );
    assert_eq!(call.validity, ResolvedCallValidity::Valid);
    assert_eq!(call.kind, CallKind::Call);
    assert_eq!(call.arguments[0].status, ArgumentMappingStatus::Resolved);
    assert_eq!(call.arguments[0].unresolved, None);
    assert_eq!(
        call.arguments[0]
            .parameter
            .as_ref()
            .expect("golden parameter")
            .callability,
        Callability::Callable
    );
}

#[test]
fn closed_wire_enum_tags_reject_unknown_and_inconsistent_values() {
    for schema in [TYPE_FACTS_TABLE_SCHEMA_V14, TYPE_FACTS_TABLE_SCHEMA_V15] {
        assert_eq!(parse_callability(0, schema).unwrap(), Callability::Callable);
        assert_eq!(
            parse_callability(1, schema).unwrap(),
            Callability::NonCallable
        );
        assert_eq!(parse_callability(2, schema).unwrap(), Callability::Mixed);
        assert_eq!(parse_callability(3, schema).unwrap(), Callability::Unknown);
        assert!(parse_callability(5, schema).is_err());
    }
    // Tag 4 belongs to v15's vocabulary and to no earlier one. A frozen
    // schema refuses it instead of reading it forward.
    assert_eq!(
        parse_callability(4, TYPE_FACTS_TABLE_SCHEMA_V15).unwrap(),
        Callability::UntypedCallable
    );
    assert!(parse_callability(4, TYPE_FACTS_TABLE_SCHEMA_V14).is_err());
    assert!(parse_callability(4, TYPE_FACTS_TABLE_SCHEMA_V3).is_err());

    assert_eq!(
        parse_constructability(0).unwrap(),
        Constructability::Constructable
    );
    assert_eq!(
        parse_constructability(1).unwrap(),
        Constructability::NonConstructable
    );
    assert_eq!(parse_constructability(2).unwrap(), Constructability::Mixed);
    assert_eq!(
        parse_constructability(3).unwrap(),
        Constructability::Unknown
    );
    assert!(parse_constructability(4).is_err());

    assert_eq!(parse_reference_space(0).unwrap(), ReferenceSpace::Value);
    assert_eq!(parse_reference_space(1).unwrap(), ReferenceSpace::Type);
    assert_eq!(parse_reference_space(2).unwrap(), ReferenceSpace::Both);
    assert_eq!(parse_reference_space(3).unwrap(), ReferenceSpace::Neither);
    assert!(parse_reference_space(4).is_err());

    assert_eq!(
        parse_resolved_call_validity(0).unwrap(),
        ResolvedCallValidity::Valid
    );
    assert_eq!(
        parse_resolved_call_validity(1).unwrap(),
        ResolvedCallValidity::Recovery
    );
    assert_eq!(
        parse_resolved_call_validity(2).unwrap(),
        ResolvedCallValidity::Unresolved
    );
    assert!(parse_resolved_call_validity(3).is_err());

    assert_eq!(parse_call_kind(0).unwrap(), CallKind::Unknown);
    assert_eq!(parse_call_kind(1).unwrap(), CallKind::Call);
    assert_eq!(parse_call_kind(2).unwrap(), CallKind::Construct);
    assert!(parse_call_kind(3).is_err());

    assert_eq!(
        parse_argument_mapping_status(0).unwrap(),
        ArgumentMappingStatus::Resolved
    );
    assert_eq!(
        parse_argument_mapping_status(1).unwrap(),
        ArgumentMappingStatus::Unresolved
    );
    assert!(parse_argument_mapping_status(2).is_err());
    assert_eq!(
        parse_argument_mapping_reason(ArgumentMappingStatus::Resolved, 0).unwrap(),
        None
    );
    assert_eq!(
        parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 1).unwrap(),
        Some(ArgumentMappingReason::CallUnresolved)
    );
    assert_eq!(
        parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 5).unwrap(),
        Some(ArgumentMappingReason::ParameterUnavailable)
    );
    assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Resolved, 1).is_err());
    assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 0).is_err());
    assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 6).is_err());
}
