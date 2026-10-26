//! Ports `protocolv3_test.go` and `protocolv3_compact_test.go`. The frozen
//! schema hash and the compact round trip are pinned by the client crate's own
//! tests (`typefacts_v3_codec_golden`, `producer::tests`); what is the
//! producer's is which operations its validator accepts.

use super::{backend_with, lifecycle, lifecycle_request, session_over};
use typefacts::producer::{expand_compact_demands, expand_compact_group};
use typefacts::v3::{
    CompactDemandGroup, CompactDemands, ModuleGraphRequest, Operation,
    TYPE_FACTS_HANDSHAKE_PROTOCOL,
};

#[test]
fn handshake_declares_the_operation_sets_protocol() {
    assert_eq!(
        TYPE_FACTS_HANDSHAKE_PROTOCOL, 86,
        "protocol 86 adds the declaredKeys demand and Wire table schema v23 (ADR 0436); \
         85 added the modules demand omitModules and the update answer's graphKept"
    );
}

#[test]
fn lifecycle_modules_is_a_valid_read_only_generation_operation() {
    let mut session = session_over(backend_with(&[]));
    let mut request = lifecycle_request(1, Operation::Modules, 1);
    request.module_graph = Some(ModuleGraphRequest {
        imports: true,
        import_paths: Vec::new(),
        packages: true,
        distinct_bare_imports: false,
        omit_modules: false,
    });
    let response = lifecycle(&mut session, request);
    // Valid: the failure is the backend's, not the validator's.
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("modules-failed")
    );
}

/// A producer that predates the modules operation rejects the request
/// outright rather than answering an empty graph. The handshake refuses such a
/// pair long before this, but the operation validator must not be the thing
/// that softens it if it ever does not.
#[test]
fn an_unknown_lifecycle_operation_is_refused() {
    let mut session = session_over(backend_with(&[]));
    let incoming = crate::wire::Incoming {
        request: lifecycle_request(1, Operation::Open, 1),
        bad_operation: Some("moduleGraph".to_owned()),
    };
    let response = session.lifecycle(&crate::context::Context::new(), &incoming);
    let error = response.error.expect("refused");
    assert_eq!(error.code, "invalid-request");
    assert_eq!(
        error.message,
        "unsupported lifecycle operation \"moduleGraph\""
    );
}

#[test]
fn lifecycle_sources_is_a_valid_read_only_generation_operation() {
    let mut session = session_over(backend_with(&[("/project/a.ts", "a")]));
    let response = lifecycle(&mut session, lifecycle_request(1, Operation::Sources, 1));
    assert!(response.ok, "{response:?}");
}

#[test]
fn compact_expansion_fails_closed_on_string_gaps() {
    let demands = CompactDemands {
        strings: vec![String::new()],
        groups: vec![CompactDemandGroup(9, Vec::new())],
    };
    assert!(expand_compact_demands(&demands).is_err());
    assert!(expand_compact_group(&demands.groups[0], &demands.strings).is_err());
}
