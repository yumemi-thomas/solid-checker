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
/// errors, as in Go. Input must be deterministic CBOR, which Go does not
/// require but every client sends.
pub fn decode_request(payload: &[u8]) -> Result<Incoming, String> {
    if let Ok(request) = typefacts::decode::<Request>(payload) {
        return Ok(Incoming {
            request,
            bad_operation: None,
        });
    }
    let mut value: Value = typefacts::decode(payload).map_err(|error| codec_error(&error))?;
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
    typefacts::encode(response).map_err(|error| {
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
        sources: Vec::new(),
        source_arena: String::new(),
        source_lengths: Vec::new(),
        timings: None,
        error: None,
        modules: Vec::new(),
        module_imports: Vec::new(),
        unknown_import_paths: Vec::new(),
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
