//! The producer's own decode and encode paths reproduce the frozen Go goldens
//! byte for byte. The v5 table-transition golden needs the transition encoder
//! and is S2's acceptance test.

use solid_typefacts::wire::{decode_request, encode_response};
use std::path::PathBuf;
use typefacts::v3::{Operation, Response};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/typefacts/phase1")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn request_round_trip(name: &str) -> Operation {
    let bytes = golden(name);
    let incoming = decode_request(&bytes).expect("producer decodes the Go request golden");
    assert_eq!(incoming.bad_operation, None);
    assert_eq!(
        typefacts::encode(&incoming.request).unwrap(),
        bytes,
        "{name}"
    );
    incoming.request.operation
}

fn response_round_trip(name: &str) {
    let bytes = golden(name);
    let response: Response = typefacts::decode(&bytes).unwrap();
    assert_eq!(encode_response(&response).unwrap(), bytes, "{name}");
}

#[test]
fn v3_request_golden() {
    assert_eq!(
        request_round_trip("typefacts-v3-request-golden.cbor"),
        Operation::Analyze
    );
}

#[test]
fn v3_response_golden() {
    response_round_trip("typefacts-v3-response-golden.cbor");
}

#[test]
fn module_graph_request_golden() {
    assert_eq!(
        request_round_trip("typefacts-module-graph-request-golden.cbor"),
        Operation::Modules
    );
}

#[test]
fn module_graph_response_golden() {
    response_round_trip("typefacts-module-graph-response-golden.cbor");
}
