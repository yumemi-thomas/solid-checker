use std::{fs, path::PathBuf};

use solid_ts_facts::{ClosureRequest, ClosureResponse, decode, encode};

fn golden(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("benchmarks/phase1").join(name)).expect("read checked-in golden")
}

#[test]
fn request_golden_round_trips_identically() {
    let bytes = golden("typefacts-v2-request-golden.cbor");
    let request: ClosureRequest = decode(&bytes).expect("decode Go request golden");
    request.validate().expect("validate Go request golden");
    assert_eq!(encode(&request).expect("encode request"), bytes);
}

#[test]
fn response_golden_round_trips_identically() {
    let request: ClosureRequest =
        decode(&golden("typefacts-v2-request-golden.cbor")).expect("decode request");
    let bytes = golden("typefacts-v2-golden.cbor");
    let response: ClosureResponse = decode(&bytes).expect("decode Go response golden");
    response
        .validate_for(&request)
        .expect("validate Go response golden");
    assert_eq!(encode(&response).expect("encode response"), bytes);
}
