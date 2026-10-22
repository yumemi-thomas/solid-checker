//! Decoding of recorded producer transcripts for the Type Facts differential
//! harness (`scripts/typefacts-differential.mjs`).
//!
//! A transcript is two byte streams in the process protocol's framing (u32 LE
//! length, deterministic CBOR): the requests a client wrote and the responses
//! (first frame: the startup handshake) a producer wrote. This module turns a
//! response stream into plain JSON using the client's own decoder and retained
//! table, so the harness compares exactly the rows a checker would see and
//! never re-implements the wire format. It lives behind the `transcript`
//! feature: the checker does not link it.

use std::collections::HashMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    FactTable, SymbolFact, decode_trusted,
    v3::{
        self, Handshake, Operation, Request, Response, TYPE_FACTS_TABLE_SCHEMA_V22, TransitionMode,
    },
};

/// Splits a stream into frame payloads. A truncated final frame is an error.
pub fn split_frames(mut bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    let mut frames = Vec::new();
    while !bytes.is_empty() {
        let (prefix, rest) = bytes
            .split_at_checked(4)
            .ok_or("truncated frame length prefix")?;
        let length = u32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
        let (payload, rest) = rest
            .split_at_checked(length)
            .ok_or("truncated frame payload")?;
        frames.push(payload);
        bytes = rest;
    }
    Ok(frames)
}

pub fn decode_request(payload: &[u8]) -> Result<Request, String> {
    decode_trusted(payload).map_err(|error| error.to_string())
}

pub fn decode_response(payload: &[u8]) -> Result<Response, String> {
    decode_trusted(payload).map_err(|error| error.to_string())
}

pub fn decode_handshake(payload: &[u8]) -> Result<Handshake, String> {
    decode_trusted(payload).map_err(|error| error.to_string())
}

/// Applies one producer's analyze responses in order, as the client does, and
/// renders each response as JSON.
#[derive(Default)]
pub struct StreamDecoder {
    table: Option<FactTable>,
    state_token: String,
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn symbol_rows(symbols: &[SymbolFact]) -> Value {
    Value::Array(symbols.iter().map(to_json).collect())
}

impl StreamDecoder {
    /// Renders `response`, advancing the retained table on analyze answers.
    pub fn decode(&mut self, operation: Operation, response: &Response) -> Value {
        let mut out = serde_json::Map::new();
        out.insert("ok".into(), json!(response.ok));
        out.insert("generation".into(), json!(response.generation));
        if let Some(error) = &response.error {
            out.insert(
                "error".into(),
                json!({ "code": error.code, "message": error.message }),
            );
        }
        if !response.state_token.is_empty() {
            out.insert("stateToken".into(), json!(response.state_token));
        }
        if !response.affected.is_empty() {
            out.insert("affected".into(), json!(response.affected));
        }
        if !response.sources.is_empty() || !response.source_arena.is_empty() {
            out.insert("sourceArena".into(), json!(response.source_arena));
            out.insert(
                "sources".into(),
                Value::Array(
                    response
                        .sources
                        .iter()
                        .map(|source| {
                            json!({
                                "path": source.path,
                                "local": source.local,
                                "length": source.source.len(),
                                "sha256": format!("{:x}", Sha256::digest(&source.source)),
                            })
                        })
                        .collect(),
                ),
            );
        }
        if !response.modules.is_empty() {
            out.insert("modules".into(), to_json(&response.modules));
        }
        if !response.module_imports.is_empty() {
            out.insert("moduleImports".into(), to_json(&response.module_imports));
        }
        if !response.unknown_import_paths.is_empty() {
            out.insert(
                "unknownImportPaths".into(),
                to_json(&response.unknown_import_paths),
            );
        }
        if !response.symbol_evidence.is_empty() {
            out.insert(
                "symbolEvidence".into(),
                symbol_rows(&response.symbol_evidence),
            );
        }
        if !response.reference_evidence.is_empty() {
            out.insert(
                "referenceEvidence".into(),
                symbol_rows(&response.reference_evidence),
            );
        }
        if !response.changed_reference_symbols.is_empty() {
            out.insert(
                "changedReferenceSymbols".into(),
                to_json(&response.changed_reference_symbols),
            );
        }
        if response.reference_changes_exact {
            out.insert("referenceChangesExact".into(), json!(true));
        }
        if response.ok && operation == Operation::Analyze {
            match self.apply(response) {
                Ok(table) => {
                    out.insert("table".into(), table);
                }
                Err(message) => {
                    out.insert("decodeError".into(), json!(message));
                }
            }
            self.state_token.clone_from(&response.state_token);
        }
        Value::Object(out)
    }

    fn apply(&mut self, response: &Response) -> Result<Value, String> {
        if response.table_transition.is_empty() {
            let table = self
                .table
                .as_ref()
                .ok_or("reuse response has no retained table")?;
            return Ok(table_json(table));
        }
        let transition = v3::decode_table_transition(&response.table_transition)?;
        if transition.project_id.as_ref() != response.project_id.as_str()
            || transition.target_generation != response.generation
            || transition.table_schema != TYPE_FACTS_TABLE_SCHEMA_V22
        {
            return Err("table transition identity does not match response".into());
        }
        let next = match transition.mode {
            TransitionMode::Full => FactTable::materialize_full(transition),
            TransitionMode::Delta => {
                let base = self
                    .table
                    .as_ref()
                    .ok_or("delta transition has no retained table")?;
                if base.generation() != transition.base_generation
                    || self.state_token != transition.base_state_token.as_ref()
                {
                    return Err(
                        "delta transition base identity does not match retained state".into(),
                    );
                }
                base.validate_delta(&transition)?;
                base.apply_delta(transition)
            }
        };
        let rendered = table_json(&next);
        self.table = Some(next);
        Ok(rendered)
    }
}

fn table_json(table: &FactTable) -> Value {
    json!({
        "schema": table.schema(),
        "sources": table.sources().map(to_json).collect::<Vec<_>>(),
        "entities": table.entities().map(to_json).collect::<Vec<_>>(),
        "files": table.files().map(to_json).collect::<Vec<_>>(),
        "symbols": table.symbols().map(|symbol| json!({
            "id": symbol.id(),
            "aliasTarget": symbol.alias_target(),
            "declarations": to_json(&symbol.declarations()),
            "references": symbol.references().map(to_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// One request/response pairing of a transcript.
pub struct Exchange {
    pub request: Request,
    pub response: Option<Response>,
}

/// Pairs each recorded request with the response bearing its id.
pub fn pair(requests: &[&[u8]], responses: &[&[u8]]) -> Result<Vec<Exchange>, String> {
    let mut by_id: HashMap<u64, Response> = HashMap::new();
    for (index, payload) in responses.iter().enumerate() {
        let response = decode_response(payload).map_err(|e| format!("response {index}: {e}"))?;
        by_id.insert(response.request_id, response);
    }
    requests
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            let request = decode_request(payload).map_err(|e| format!("request {index}: {e}"))?;
            let response = by_id.remove(&request.request_id);
            Ok(Exchange { request, response })
        })
        .collect()
}

/// Decodes a whole transcript to JSON: the handshake and one entry per request.
pub fn decode_transcript(requests: &[u8], responses: &[u8]) -> Result<Value, String> {
    let request_frames = split_frames(requests)?;
    let response_frames = split_frames(responses)?;
    let (handshake, rest) = response_frames
        .split_first()
        .ok_or("response stream has no handshake frame")?;
    let handshake = decode_handshake(handshake)?;
    let mut decoder = StreamDecoder::default();
    let mut exchanges = Vec::new();
    for (index, exchange) in pair(&request_frames, rest)?.into_iter().enumerate() {
        let operation = exchange.request.operation;
        let response = exchange
            .response
            .as_ref()
            .map(|response| decoder.decode(operation, response));
        exchanges.push(json!({
            "index": index,
            "operation": to_json(&operation),
            "requestId": exchange.request.request_id,
            "generation": exchange.request.generation,
            "response": response,
        }));
    }
    Ok(json!({
        "handshake": to_json(&handshake),
        "exchanges": exchanges,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode, write_frame};

    fn frames(payloads: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        for payload in payloads {
            write_frame(&mut out, payload).unwrap();
        }
        out
    }

    fn response(value: Value) -> Vec<u8> {
        let response: Response = serde_json::from_value(value).unwrap();
        encode(&response).unwrap()
    }

    fn request(operation: &str, id: u64) -> Vec<u8> {
        let request: Request = serde_json::from_value(
            json!({"schema": 1, "operation": operation, "projectId": "p", "requestId": id, "generation": 1}),
        )
        .unwrap();
        encode(&request).unwrap()
    }

    #[test]
    fn frames_split_and_truncation_is_refused() {
        let stream = frames(&[vec![1, 2, 3], vec![], vec![9]]);
        assert_eq!(
            split_frames(&stream).unwrap(),
            vec![&[1_u8, 2, 3][..], &[][..], &[9][..]]
        );
        assert!(split_frames(&stream[..stream.len() - 1]).is_err());
        assert!(split_frames(&[1, 0]).is_err());
    }

    #[test]
    fn a_transcript_decodes_to_one_entry_per_request() {
        let handshake = encode(&Handshake {
            protocol: v3::TYPE_FACTS_HANDSHAKE_PROTOCOL,
            schema_hash: v3::TYPE_FACTS_SCHEMA_SHA256.to_owned(),
            build_id: "dev".to_owned(),
        })
        .unwrap();
        let requests = frames(&[
            request("open", 1),
            request("analyze", 2),
            request("cancel", 3),
        ]);
        let responses = frames(&[
            handshake,
            response(
                json!({"schema": 1, "requestId": 1, "projectId": "p", "generation": 1, "ok": true}),
            ),
            response(json!({
                "schema": 1, "requestId": 2, "projectId": "p", "generation": 1, "ok": false,
                "error": {"code": "analysis-failed", "message": "no"}
            })),
        ]);
        let value = decode_transcript(&requests, &responses).unwrap();
        assert_eq!(
            value["handshake"]["protocol"],
            v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        );
        let exchanges = value["exchanges"].as_array().unwrap();
        assert_eq!(exchanges.len(), 3);
        assert_eq!(exchanges[0]["operation"], "open");
        assert_eq!(exchanges[0]["response"]["ok"], true);
        assert_eq!(exchanges[1]["response"]["error"]["code"], "analysis-failed");
        // A request the producer never answered keeps a null response.
        assert!(exchanges[2]["response"].is_null());
    }

    #[test]
    fn an_analyze_answer_without_a_table_is_a_decode_error_not_a_panic() {
        let response: Response = serde_json::from_value(
            json!({"schema": 1, "requestId": 2, "projectId": "p", "generation": 1, "ok": true}),
        )
        .unwrap();
        let rendered = StreamDecoder::default().decode(Operation::Analyze, &response);
        assert!(
            rendered["decodeError"]
                .as_str()
                .unwrap()
                .contains("retained table")
        );
    }
}
