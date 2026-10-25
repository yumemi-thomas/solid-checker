//! The producer's use of the shared codec: handshake bytes, request decoding
//! that tolerates what the Go decoder tolerates, and response encoding.

use ciborium::Value;
use typefacts::TypeFactsError;
use typefacts::v3::{
    Error as WireError, Handshake, Operation, Request, Response, TYPE_FACTS_BUILD_ID,
    TYPE_FACTS_HANDSHAKE_PROTOCOL, TYPE_FACTS_SCHEMA_SHA256,
};

/// The startup frame's payload: protocol version, schema hash and build id,
/// nothing derived from the project.
pub fn handshake_payload() -> Result<Vec<u8>, String> {
    typefacts::encode(&Handshake {
        protocol: TYPE_FACTS_HANDSHAKE_PROTOCOL,
        schema_hash: TYPE_FACTS_SCHEMA_SHA256.to_owned(),
        build_id: TYPE_FACTS_BUILD_ID.to_owned(),
    })
    .map_err(|error| error.to_string())
}

/// A decoded request frame.
///
/// Go decodes `operation` as a free string and rejects an unknown one during
/// validation, with an `invalid-request` response. The shared `Request` type
/// holds a closed enum, so an unknown or absent operation decodes with a
/// placeholder and `bad_operation` carries the original text; sessions must
/// check it before looking at `request.operation`.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    pub request: Request,
    pub bad_operation: Option<String>,
}

impl Incoming {
    /// The operation, or `None` when the frame named one this protocol lacks.
    #[must_use]
    pub fn operation(&self) -> Option<Operation> {
        self.bad_operation
            .is_none()
            .then_some(self.request.operation)
    }
}

const KNOWN_OPERATIONS: [&str; 8] = [
    "open", "update", "analyze", "symbols", "sources", "modules", "cancel", "close",
];

fn codec_error(error: &TypeFactsError) -> String {
    format!("decode deterministic CBOR: {error}")
}

/// Decodes one request payload. Missing fields take Go's zero values (and are
/// then refused by validation); unknown fields and wrong types are decode
/// errors, as in Go.
///
/// Go does not require deterministic CBOR, and the client does not always send
/// it: `EntityDemand`'s fields are declared in an order that is canonical for
/// most flag sets but not all (`structuralAccessor` precedes `callability`,
/// `referenceSpace`, ...), and the sidecar encoder serializes the struct as
/// declared. A request that is well-formed CBOR but not in core deterministic
/// key order is therefore accepted, as Go accepts it; only undecodable input
/// is an error.
pub fn decode_request(payload: &[u8]) -> Result<Incoming, String> {
    if let Ok(request) = typefacts::decode::<Request>(payload)
        .or_else(|_| typefacts::decode_trusted::<Request>(payload))
    {
        return Ok(Incoming {
            request,
            bad_operation: None,
        });
    }
    let mut value: Value = typefacts::decode(payload)
        .or_else(|_| typefacts::decode_trusted(payload))
        .map_err(|error| codec_error(&error))?;
    let Value::Map(entries) = &mut value else {
        return Err(codec_error(&TypeFactsError::Codec(
            "request is not a map".to_owned(),
        )));
    };
    let find = |entries: &[(Value, Value)], name: &str| {
        entries
            .iter()
            .position(|(key, _)| matches!(key, Value::Text(text) if text == name))
    };
    let mut bad_operation = None;
    match find(entries, "operation") {
        Some(index) => {
            if let Value::Text(text) = &entries[index].1
                && !KNOWN_OPERATIONS.contains(&text.as_str())
            {
                bad_operation = Some(text.clone());
                entries[index].1 = Value::Text("close".to_owned());
            }
        }
        None => {
            bad_operation = Some(String::new());
            entries.push((
                Value::Text("operation".to_owned()),
                Value::Text("close".to_owned()),
            ));
        }
    }
    for (name, zero) in [
        ("schema", Value::Integer(0.into())),
        ("projectId", Value::Text(String::new())),
        ("requestId", Value::Integer(0.into())),
        ("generation", Value::Integer(0.into())),
    ] {
        if find(entries, name).is_none() {
            entries.push((Value::Text(name.to_owned()), zero));
        }
    }
    let request = value
        .deserialized::<Request>()
        .map_err(|error| codec_error(&TypeFactsError::Codec(error.to_string())))?;
    Ok(Incoming {
        request,
        bad_operation,
    })
}

/// Encodes one response payload, with Go's wording for the size limit.
pub fn encode_response(response: &Response) -> Result<Vec<u8>, String> {
    typefacts::encode_direct(response).map_err(|error| {
        let inner = match error {
            TypeFactsError::MessageLimit { actual, limit } => {
                format!("encoded message is {actual} bytes, limit is {limit}")
            }
            other => other.to_string(),
        };
        format!("encode response: encode deterministic CBOR: {inner}")
    })
}

/// A response with the identity fields set and every payload field empty.
#[must_use]
pub fn blank_response(schema: u64, request_id: u64, project_id: &str, generation: u64) -> Response {
    Response {
        schema,
        request_id,
        project_id: project_id.to_owned(),
        generation,
        ok: false,
        table_transition: Vec::new(),
        symbol_evidence: Vec::new(),
        reference_evidence: Vec::new(),
        changed_reference_symbols: Vec::new(),
        reference_changes_exact: false,
        state_token: String::new(),
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

#[must_use]
pub fn error_response(mut response: Response, code: &str, message: impl Into<String>) -> Response {
    response.error = Some(WireError {
        code: code.to_owned(),
        message: message.into(),
    });
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_map(entries: Vec<(&str, Value)>) -> Vec<u8> {
        let mut value = Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| (Value::Text(key.to_owned()), value))
                .collect(),
        );
        // Sort like the codec so the frame is deterministic.
        let mut bytes = Vec::new();
        if let Value::Map(entries) = &mut value {
            entries.sort_by_key(|(key, _)| {
                let mut encoded = Vec::new();
                ciborium::into_writer(key, &mut encoded).unwrap();
                (encoded.len(), encoded)
            });
        }
        ciborium::into_writer(&value, &mut bytes).unwrap();
        bytes
    }

    fn text(value: &str) -> Value {
        Value::Text(value.to_owned())
    }

    fn int(value: u64) -> Value {
        Value::Integer(value.into())
    }

    #[test]
    fn unknown_and_missing_operations_decode_with_a_marker() {
        let unknown = decode_request(&request_map(vec![
            ("schema", int(1)),
            ("operation", text("explode")),
            ("projectId", text("/p")),
            ("requestId", int(4)),
            ("generation", int(1)),
        ]))
        .unwrap();
        assert_eq!(unknown.bad_operation.as_deref(), Some("explode"));
        assert_eq!(unknown.operation(), None);
        let missing = decode_request(&request_map(vec![("schema", int(1))])).unwrap();
        assert_eq!(missing.bad_operation.as_deref(), Some(""));
        assert_eq!(missing.request.request_id, 0);
    }

    #[test]
    fn unknown_fields_and_wrong_types_stay_decode_errors() {
        let extra = request_map(vec![("schema", int(1)), ("extra", int(1))]);
        assert!(decode_request(&extra).is_err());
        let wrong = request_map(vec![("schema", text("one"))]);
        assert!(decode_request(&wrong).is_err());
        assert!(decode_request(&[0xff]).is_err());
    }

    #[test]
    fn a_known_operation_has_no_marker() {
        let known = decode_request(&request_map(vec![
            ("schema", int(1)),
            ("operation", text("modules")),
            ("projectId", text("/p")),
            ("requestId", int(4)),
            ("generation", int(1)),
        ]))
        .unwrap();
        assert_eq!(known.operation(), Some(Operation::Modules));
    }
}

#[cfg(test)]
mod encode_tests {
    use super::*;
    use crate::model::blank_symbol;
    use std::sync::Arc;
    use typefacts::v3::{ServerTimings, SourceFile};
    use typefacts::{
        Declaration, Location, ModuleFact, ModuleFormat, ModuleImportFact, ModuleResolution,
        PackageIdentity, ProjectReferenceMapping, ResolverPackageId, SymbolFact,
    };

    fn location(path: &str, start: u64, end: u64) -> Location {
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: end,
        }
    }

    fn populated_modules(response: &mut Response) {
        response.modules = vec![
            ModuleFact {
                path: "/p/a.ts".into(),
                root: None,
                declaration_file: true,
                format: ModuleFormat::Esm,
                project_reference: Some(ProjectReferenceMapping {
                    source: "/p/a.ts".into(),
                    output_dts: "/p/out/a.d.ts".into(),
                }),
                redirect_targets: Arc::from(vec![Arc::<str>::from("/q/a.ts")]),
            },
            ModuleFact {
                path: "/p/b.ts".into(),
                root: None,
                declaration_file: false,
                format: ModuleFormat::Unknown,
                project_reference: None,
                redirect_targets: Arc::from(Vec::new()),
            },
        ];
        response.module_imports = vec![ModuleImportFact {
            specifier: location("/p/a.ts", 4, 9),
            text: "./b".into(),
            resolution: ModuleResolution::NodeModules,
            resolved_path: "/p/b.ts".into(),
            included_path: "/p/c.ts".into(),
            symlink_path: "/link/b.ts".into(),
            extension: ".ts".into(),
            ts_extension: true,
            paths_pattern: "@/*".into(),
            package: Some(PackageIdentity {
                manifest_path: "/p/package.json".into(),
                name: "p".into(),
                version: "1.0.0".into(),
            }),
            resolver_package: Some(ResolverPackageId {
                name: "p".into(),
                subpath: "index.d.ts".into(),
                version: "1.0.0".into(),
                peer_dependencies: "+react@19".into(),
            }),
        }];
        response.symbol_evidence = vec![SymbolFact {
            id: "sym:a".into(),
            alias_target: "sym:b".into(),
            declarations: Arc::from(vec![Declaration {
                name: "a".into(),
                kind: "variable".into(),
                location: location("/p/a.ts", 1, 2),
            }]),
            references: Arc::from(vec![location("/p/a.ts", 5, 6)]),
        }];
    }

    /// `Response` and the types it carries declare their fields in core
    /// deterministic key order, so the raw serialization already is the
    /// canonical encoding and `encode_direct` never falls back.
    #[test]
    fn direct_response_encoding_takes_the_direct_path() {
        let mut response = blank_response(1, 7, "/p/tsconfig.json", 3);
        response.ok = true;
        response.table_transition = vec![1, 2, 3];
        response.changed_reference_symbols = vec!["x".into()];
        response.reference_changes_exact = true;
        response.state_token = "t".to_owned();
        response.affected = vec!["/p/a.ts".to_owned()];
        response.sources = vec![SourceFile {
            path: "/p/a.ts".to_owned(),
            local: true,
            source: b"x".to_vec(),
        }];
        response.source_arena = "a".to_owned();
        response.source_lengths = vec![1];
        response.timings = Some(ServerTimings {
            request_decode_ns: 1,
            analyze_ns: 2,
            async_ns: 3,
            demand_ns: 4,
            assembly_ns: 5,
            sort_ns: 6,
            close_symbols_ns: 7,
            materialized: true,
            retained_files: 8,
            recomputed_files: 9,
            non_durable_files: 10,
        });
        response.error = Some(WireError {
            code: "c".to_owned(),
            message: "m".to_owned(),
        });
        response.unknown_import_paths = vec!["/p/x".into()];
        response.reference_evidence = vec![blank_symbol("r".into())];
        populated_modules(&mut response);
        let mut raw = Vec::new();
        ciborium::into_writer(&response, &mut raw).unwrap();
        assert_eq!(raw, typefacts::encode(&response).unwrap());
    }

    #[test]
    fn direct_response_encoding_equals_the_canonicalizing_encoder() {
        let mut response = blank_response(1, 7, "/p/tsconfig.json", 3);
        response.ok = true;
        response.table_transition = (0..70_000_u32).map(|n| (n % 251) as u8).collect();
        populated_modules(&mut response);
        response.symbol_evidence.push(blank_symbol("b".into()));
        response.reference_evidence = vec![blank_symbol("c".into())];
        response.changed_reference_symbols = vec!["x".into(), "yy".into()];
        response.reference_changes_exact = true;
        response.state_token = "token".to_owned();
        response.affected = vec!["/p/a.ts".to_owned()];
        response.sources = vec![SourceFile {
            path: "/p/a.ts".to_owned(),
            local: true,
            source: b"let a = 1;".to_vec(),
        }];
        response.source_arena = "arena".to_owned();
        response.source_lengths =
            vec![0, 23, 24, 255, 256, 65_535, 65_536, u64::from(u32::MAX) + 1];
        response.timings = Some(ServerTimings {
            analyze_ns: 5,
            materialized: true,
            retained_files: 300,
            ..ServerTimings::default()
        });
        response.unknown_import_paths = vec!["/p/missing".into()];
        let variants = [
            blank_response(1, 0, "", 0),
            response.clone(),
            error_response(blank_response(2, 9, "/p", 1), "invalid-request", "bad"),
            Response {
                error: Some(WireError {
                    code: "c".to_owned(),
                    message: "m".repeat(300),
                }),
                ..response
            },
        ];
        for variant in &variants {
            assert_eq!(
                typefacts::encode_direct(variant).unwrap(),
                typefacts::encode(variant).unwrap()
            );
            assert_eq!(
                encode_response(variant).unwrap(),
                typefacts::encode(variant).unwrap()
            );
        }
    }

    #[test]
    fn direct_encoding_falls_back_when_declaration_order_is_not_canonical() {
        let unsorted = Value::Map(vec![
            (Value::Text("bb".to_owned()), Value::Integer(1.into())),
            (Value::Text("a".to_owned()), Value::Integer(2.into())),
            (Value::Text("ab".to_owned()), Value::Bool(true)),
        ]);
        let direct = typefacts::encode_direct(&unsorted).unwrap();
        assert_eq!(direct, typefacts::encode(&unsorted).unwrap());
        assert!(typefacts::decode::<Value>(&direct).is_ok());
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use ciborium::Value;

    fn text(value: &str) -> Value {
        Value::Text(value.to_owned())
    }

    /// A `modules` request as the checker encodes it: `moduleGraph` keys in
    /// struct order (`imports`, `importPaths`, `packages`), not length-first.
    pub(crate) fn checker_modules_request() -> Vec<u8> {
        let graph = Value::Map(vec![
            (text("imports"), Value::Bool(true)),
            (text("importPaths"), Value::Array(vec![text("/p/a.ts")])),
            (text("packages"), Value::Bool(true)),
        ]);
        let request = Value::Map(vec![
            (text("schema"), Value::Integer(1.into())),
            (text("operation"), text("modules")),
            (text("projectId"), text("/p/tsconfig.json")),
            (text("requestId"), Value::Integer(2.into())),
            (text("generation"), Value::Integer(1.into())),
            (text("moduleGraph"), graph),
        ]);
        let mut bytes = Vec::new();
        ciborium::into_writer(&request, &mut bytes).unwrap();
        bytes
    }

    #[test]
    fn non_canonical_key_order_decodes_like_go() {
        let payload = checker_modules_request();
        assert!(
            typefacts::decode::<Value>(&payload).is_err(),
            "fixture must be non-canonical"
        );
        let incoming = decode_request(&payload).unwrap();
        assert_eq!(incoming.operation(), Some(Operation::Modules));
        let graph = incoming.request.module_graph.unwrap();
        assert!(graph.imports && graph.packages);
        assert_eq!(graph.import_paths, ["/p/a.ts"]);
    }

    #[test]
    fn unknown_operation_with_non_canonical_order_is_still_marked() {
        let request = Value::Map(vec![
            (text("schema"), Value::Integer(1.into())),
            (text("operation"), text("frobnicate")),
            (text("projectId"), text("/p")),
        ]);
        let mut bytes = Vec::new();
        ciborium::into_writer(&request, &mut bytes).unwrap();
        let incoming = decode_request(&bytes).unwrap();
        assert_eq!(incoming.bad_operation.as_deref(), Some("frobnicate"));
    }
}
