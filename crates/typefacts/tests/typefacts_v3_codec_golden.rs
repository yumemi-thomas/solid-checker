//! Cross-language codec conformance.
//!
//! Both fixtures are produced by Go (see
//! internal/typefacts/protocolv3_golden_test.go) and decoded here through the
//! deterministic-CBOR validator. Re-encoding must reproduce the file byte for
//! byte, so any drift in field names, canonical map ordering, or optional-field
//! omission between the two implementations fails this test.

use std::{fs, path::PathBuf};

use typefacts::{
    decode, encode,
    v3::{Operation, Request, Response, TYPE_FACTS_SCHEMA_V3},
};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/phase1")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn request_golden_round_trips_identically() {
    let bytes = golden("typefacts-v3-request-golden.cbor");
    let request: Request = decode(&bytes).expect("decode Go request golden");

    assert_eq!(request.schema, TYPE_FACTS_SCHEMA_V3);
    assert_eq!(request.operation, Operation::Analyze);
    assert_eq!(request.request_id, 7);
    assert_eq!(request.state_token, "4");
    assert_eq!(request.changes.len(), 2);
    assert!(request.changes[1].deleted);
    let compact = request
        .compact_demands
        .as_ref()
        .expect("golden carries a compact demand snapshot");
    assert_eq!(compact.groups.len(), 2);
    assert_eq!(compact.strings[0], "");

    assert_eq!(encode(&request).expect("re-encode request"), bytes);
}

#[test]
fn response_golden_round_trips_identically() {
    let bytes = golden("typefacts-v3-response-golden.cbor");
    let response: Response = decode(&bytes).expect("decode Go response golden");

    assert_eq!(response.schema, TYPE_FACTS_SCHEMA_V3);
    assert!(response.ok);
    assert_eq!(response.table_mode, "full");
    assert_eq!(response.state_token, "5");
    assert!(!response.packed_table.is_empty());

    // The packed frame the response carries must expand through the same
    // decoder the live client uses.
    let table = typefacts::v3::decode_packed_fact_table(
        &response.packed_table,
        response.project_id.clone(),
    )
    .expect("expand the golden packed table");
    assert_eq!(table.project_id, "/p/tsconfig.json");
    assert!(!table.entities.is_empty());
    assert!(!table.symbols.is_empty());

    assert_eq!(encode(&response).expect("re-encode response"), bytes);
}
