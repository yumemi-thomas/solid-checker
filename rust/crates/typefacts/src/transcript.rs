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

use std::collections::{BTreeSet, HashMap, HashSet};

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
        // A `symbols` answer packs its rows as a standalone symbol table.
        if operation == Operation::Symbols && response.ok && !response.table_transition.is_empty() {
            match symbol_table_rows(response) {
                Ok(rows) => {
                    out.insert("symbolEvidence".into(), rows);
                }
                Err(message) => {
                    out.insert("decodeError".into(), json!(message));
                }
            }
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

fn symbol_rows_of(table: &FactTable) -> Vec<Value> {
    table
        .symbols()
        .map(|symbol| {
            json!({
                "id": symbol.id(),
                "aliasTarget": symbol.alias_target(),
                "declarations": to_json(&symbol.declarations()),
                "references": symbol.references().map(to_json).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The rows of a `symbols` answer: a full transition holding only symbols.
pub fn symbol_table_rows(response: &Response) -> Result<Value, String> {
    let transition = v3::decode_table_transition(&response.table_transition)?;
    if transition.mode != TransitionMode::Full {
        return Err("symbol evidence transition is not a full table".into());
    }
    Ok(Value::Array(symbol_rows_of(&FactTable::materialize_full(
        transition,
    ))))
}

fn table_json(table: &FactTable) -> Value {
    json!({
        "schema": table.schema(),
        "sources": table.sources().map(to_json).collect::<Vec<_>>(),
        "entities": table.entities().map(to_json).collect::<Vec<_>>(),
        "files": table.files().map(to_json).collect::<Vec<_>>(),
        "symbols": symbol_rows_of(table),
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

/// One producer's retained table, as the client would hold it, reduced to
/// what the replay needs: the symbol of each entity by source location, every
/// durable id mentioned, and the token the next request must carry.
#[derive(Default)]
pub struct Tables {
    decoder: StreamDecoder,
    pub entities: HashMap<String, String>,
    pub ids: BTreeSet<String>,
    pub token: String,
}

impl Tables {
    pub fn observe(&mut self, operation: Operation, response: &Response) {
        let rendered = self.decoder.decode(operation, response);
        if operation == Operation::Analyze && response.ok {
            if let Some(table) = rendered.get("table") {
                collect_ids(table, &mut self.ids);
                self.entities = entity_map(table);
            }
            self.token.clone_from(&response.state_token);
        }
    }
}

/// The symbol of each entity row that has one, by `path:start-end`.
fn entity_map(table: &Value) -> HashMap<String, String> {
    table
        .get("entities")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(entity_symbol).collect())
        .unwrap_or_default()
}

/// `(path:start-end, symbol)` of an entity row that has a symbol.
fn entity_symbol(row: &Value) -> Option<(String, String)> {
    let location = row.get("location")?;
    let symbol = row.get("symbol")?.as_str()?;
    Some((
        format!(
            "{}:{}-{}",
            location.get("path")?.as_str()?,
            location.get("startByte")?,
            location.get("endByte")?
        ),
        symbol.to_owned(),
    ))
}

/// A recorded symbol id and the replaying producer's id for the entity at the
/// same location. An id with two candidates is not translated.
#[derive(Default)]
pub struct IdMap {
    map: HashMap<String, String>,
    ambiguous: HashSet<String>,
}

impl IdMap {
    pub fn reconcile(&mut self, recorded: &Tables, replayed: &Tables) {
        for (location, recorded_id) in &recorded.entities {
            if !recorded_id.starts_with("symbol:") || self.ambiguous.contains(recorded_id) {
                continue;
            }
            let Some(replayed_id) = replayed.entities.get(location) else {
                continue;
            };
            match self.map.get(recorded_id) {
                Some(known) if known != replayed_id => {
                    self.map.remove(recorded_id);
                    self.ambiguous.insert(recorded_id.clone());
                }
                Some(_) => {}
                None => {
                    self.map.insert(recorded_id.clone(), replayed_id.clone());
                }
            }
        }
    }

    /// Rewrites the generation-scoped ids of `request`'s symbol queries.
    pub fn translate(&self, request: &mut Request) -> u64 {
        let mut changed = 0;
        for query in &mut request.symbol_queries {
            if let Some(replayed) = self.map.get(query.id.as_ref())
                && replayed.as_str() != query.id.as_ref()
            {
                query.id = replayed.as_str().into();
                changed += 1;
            }
        }
        changed
    }
}

/// Every `symbol:h:` id (24 hex digits) inside `text`.
pub fn durable_ids(text: &str, into: &mut BTreeSet<String>) {
    const MARK: &str = "symbol:h:";
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        let tail = &rest[at + MARK.len()..];
        let digits = tail.bytes().take_while(u8::is_ascii_hexdigit).count();
        if digits >= 24 {
            into.insert(format!("{MARK}{}", &tail[..24]));
        }
        rest = &rest[at + MARK.len()..];
    }
}

pub fn collect_ids(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => durable_ids(text, into),
        Value::Array(items) => items.iter().for_each(|item| collect_ids(item, into)),
        Value::Object(map) => map.values().for_each(|item| collect_ids(item, into)),
        _ => {}
    }
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

    fn analyzed(symbol_of: &[(&str, u64, &str)]) -> Value {
        json!({
            "entities": symbol_of
                .iter()
                .map(|(path, start, symbol)| json!({
                    "location": {"path": path, "startByte": start, "endByte": start + 1},
                    "symbol": symbol,
                }))
                .collect::<Vec<_>>()
        })
    }

    fn tables(rows: &[(&str, u64, &str)]) -> Tables {
        Tables {
            entities: entity_map(&analyzed(rows)),
            ..Tables::default()
        }
    }

    #[test]
    fn durable_ids_are_found_inside_text_and_short_ones_are_ignored() {
        let mut ids = BTreeSet::new();
        durable_ids(
            "x symbol:h:0123456789abcdef01234567 and symbol:h:abc and symbol:3:4 \
             symbol:h:fedcba9876543210fedcba98tail",
            &mut ids,
        );
        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec![
                "symbol:h:0123456789abcdef01234567".to_owned(),
                "symbol:h:fedcba9876543210fedcba98".to_owned(),
            ]
        );
    }

    #[test]
    fn a_recorded_id_is_translated_to_the_replayed_ids_for_the_same_location() {
        let recorded = tables(&[
            ("/p/a.ts", 1, "symbol:1:12"),
            ("/p/a.ts", 5, "symbol:h:aaaaaaaaaaaaaaaaaaaaaaaa"),
        ]);
        let replayed = tables(&[
            ("/p/a.ts", 1, "symbol:1:6"),
            ("/p/a.ts", 5, "symbol:h:bbbbbbbbbbbbbbbbbbbbbbbb"),
        ]);
        let mut map = IdMap::default();
        map.reconcile(&recorded, &replayed);
        let mut request: Request = serde_json::from_value(json!({
            "schema": 1, "operation": "symbols", "projectId": "p", "requestId": 4, "generation": 1,
            "symbolQueries": [
                {"id": "symbol:1:12"},
                {"id": "symbol:h:aaaaaaaaaaaaaaaaaaaaaaaa"},
                {"id": "symbol:h:cccccccccccccccccccccccc"},
            ]
        }))
        .unwrap();
        assert_eq!(map.translate(&mut request), 2);
        let ids: Vec<&str> = request
            .symbol_queries
            .iter()
            .map(|q| q.id.as_ref())
            .collect();
        // An id nobody can place is sent as recorded.
        assert_eq!(
            ids,
            [
                "symbol:1:6",
                "symbol:h:bbbbbbbbbbbbbbbbbbbbbbbb",
                "symbol:h:cccccccccccccccccccccccc"
            ]
        );
    }

    #[test]
    fn an_id_with_two_candidates_is_not_translated_and_stays_that_way() {
        let recorded = tables(&[("/p/a.ts", 1, "symbol:1:12"), ("/p/a.ts", 9, "symbol:1:12")]);
        let mut map = IdMap::default();
        map.reconcile(
            &recorded,
            &tables(&[("/p/a.ts", 1, "symbol:1:6"), ("/p/a.ts", 9, "symbol:1:7")]),
        );
        // A later, agreeing table does not resurrect it.
        map.reconcile(
            &recorded,
            &tables(&[("/p/a.ts", 1, "symbol:1:6"), ("/p/a.ts", 9, "symbol:1:6")]),
        );
        let mut request: Request = serde_json::from_value(json!({
            "schema": 1, "operation": "symbols", "projectId": "p", "requestId": 4, "generation": 1,
            "symbolQueries": [{"id": "symbol:1:12"}]
        }))
        .unwrap();
        assert_eq!(map.translate(&mut request), 0);
        assert_eq!(request.symbol_queries[0].id.as_ref(), "symbol:1:12");
    }

    #[test]
    fn a_symbols_answer_decodes_its_packed_rows() {
        use crate::producer::{ProducerTable, TransitionEncoder, TransitionInput};
        let mut table = ProducerTable::new(1, "p");
        table.symbols = vec![SymbolFact {
            id: "symbol:h:0123456789abcdef01234567".into(),
            alias_target: "".into(),
            declarations: vec![crate::Declaration {
                name: "JSON".into(),
                kind: "variable".into(),
                location: crate::Location {
                    path: "bundled:/libs/lib.es5.d.ts".into(),
                    start_byte: 3,
                    end_byte: 7,
                },
            }]
            .into(),
            references: Vec::new().into(),
        }];
        let packed = TransitionEncoder::default()
            .encode(TransitionInput {
                project_id: "p",
                base_state_token: "",
                base: None,
                target: &table,
                sparse: false,
            })
            .unwrap();
        let response: Response = serde_json::from_value(json!({
            "schema": 1, "requestId": 4, "projectId": "p", "generation": 1, "ok": true
        }))
        .unwrap();
        let response = Response {
            table_transition: packed.bytes,
            ..response
        };
        let rendered = StreamDecoder::default().decode(Operation::Symbols, &response);
        let rows = rendered["symbolEvidence"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "symbol:h:0123456789abcdef01234567");
        assert_eq!(rows[0]["declarations"][0]["name"], "JSON");
    }
}
