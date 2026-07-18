//! Rust client model for the frozen TypeFacts v2 closure protocol.
//!
//! This package contains checker-derived facts only. Structural discovery is
//! owned by `solid-ast-facts`; no regex or TypeScript AST shape is reproduced.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use solid_facts_core::{Generation, SourceHash};
use std::io::{Read, Write};
use thiserror::Error;

pub mod v3;

pub const TYPE_FACTS_SCHEMA: u64 = 2;
pub const EXPANSION_RULESET: u64 = 1;
pub const MAX_MESSAGE_BYTES: usize = 64 << 20;
pub const MAX_NESTING_DEPTH: usize = 32;
pub const MAX_COLLECTION_LENGTH: usize = 1_000_000;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Location {
    pub path: String,
    pub end_byte: u64,
    pub start_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Declaration {
    pub name: String,
    pub kind: String,
    pub location: Location,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedCall {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub return_type_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeDescriptor {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub origin_module: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alias_declarations: Vec<Declaration>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityFact {
    pub location: Location,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<TypeDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_call: Option<ResolvedCall>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolFact {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alias_target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declarations: Vec<Declaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCall {
    pub location: Location,
    pub callee: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<Location>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceBinding {
    #[serde(default, skip_serializing_if = "is_false")]
    pub array: bool,
    pub names: Vec<Location>,
    pub initializer: SourceCall,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFunction {
    pub name: Location,
    pub body: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Location>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub exported: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub r#async: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub arrow: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AsyncFunctionFact {
    pub expression: Location,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_return_async: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls_after_await: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileFact {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<SourceCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<SourceBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<SourceFunction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub async_functions: Vec<AsyncFunctionFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDigest {
    pub path: String,
    pub sha256: SourceHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FactTable {
    pub schema: u64,
    pub generation: u64,
    pub project_id: String,
    pub sources: Vec<SourceDigest>,
    pub entities: Vec<EntityFact>,
    pub symbols: Vec<SymbolFact>,
    pub files: Vec<FileFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureRequest {
    pub schema: u64,
    pub project_id: String,
    pub generation: u64,
    pub ruleset_version: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compiler_spans: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureResponse {
    pub schema: u64,
    pub project_id: String,
    pub generation: u64,
    pub table: FactTable,
}

#[derive(Debug, Error)]
pub enum TypeFactsError {
    #[error("message is {actual} bytes, limit is {limit}")]
    MessageLimit { actual: usize, limit: usize },
    #[error("CBOR codec error: {0}")]
    Codec(String),
    #[error("invalid deterministic CBOR: {0}")]
    DeterministicCbor(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported TypeFacts schema {0}")]
    Schema(u64),
    #[error("unsupported expansion ruleset {0}")]
    Ruleset(u64),
    #[error("project identity is empty")]
    ProjectIdentity,
    #[error("generation identity is invalid")]
    Generation,
    #[error("response identity does not match request")]
    IdentityMismatch,
    #[error("compiler spans are not in canonical order")]
    CompilerSpanOrder,
    #[error("source digests are not in canonical order")]
    SourceOrder,
    #[error("symbol {0} is an alias but also carries references")]
    AliasReferences(String),
    #[error("invalid {category} location {path}:{start}..{end}")]
    InvalidLocation {
        category: &'static str,
        path: String,
        start: u64,
        end: u64,
    },
}

impl ClosureRequest {
    pub fn new(
        project_id: impl Into<String>,
        generation: Generation,
        mut compiler_spans: Vec<Location>,
    ) -> Result<Self, TypeFactsError> {
        compiler_spans.sort_by(location_cmp);
        compiler_spans.dedup();
        let request = Self {
            schema: TYPE_FACTS_SCHEMA,
            project_id: project_id.into(),
            generation: generation.get(),
            ruleset_version: EXPANSION_RULESET,
            compiler_spans,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), TypeFactsError> {
        if self.schema != TYPE_FACTS_SCHEMA {
            return Err(TypeFactsError::Schema(self.schema));
        }
        if self.ruleset_version != EXPANSION_RULESET {
            return Err(TypeFactsError::Ruleset(self.ruleset_version));
        }
        if self.project_id.is_empty() {
            return Err(TypeFactsError::ProjectIdentity);
        }
        if self.generation == 0 {
            return Err(TypeFactsError::Generation);
        }
        validate_locations("compiler span", &self.compiler_spans)?;
        if !self
            .compiler_spans
            .windows(2)
            .all(|pair| location_cmp(&pair[0], &pair[1]).is_le())
        {
            return Err(TypeFactsError::CompilerSpanOrder);
        }
        Ok(())
    }
}

impl ClosureResponse {
    pub fn validate_for(&self, request: &ClosureRequest) -> Result<(), TypeFactsError> {
        request.validate()?;
        if self.schema != TYPE_FACTS_SCHEMA || self.table.schema != TYPE_FACTS_SCHEMA {
            return Err(TypeFactsError::Schema(self.schema));
        }
        if self.project_id != request.project_id
            || self.generation != request.generation
            || self.table.project_id != request.project_id
            || self.table.generation != request.generation
        {
            return Err(TypeFactsError::IdentityMismatch);
        }
        if !self
            .table
            .sources
            .windows(2)
            .all(|pair| pair[0].path <= pair[1].path)
        {
            return Err(TypeFactsError::SourceOrder);
        }
        for symbol in &self.table.symbols {
            if !symbol.alias_target.is_empty() && !symbol.references.is_empty() {
                return Err(TypeFactsError::AliasReferences(symbol.id.clone()));
            }
        }
        validate_table_locations(&self.table)
    }
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, TypeFactsError> {
    let mut intermediate = Vec::new();
    ciborium::into_writer(value, &mut intermediate)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    let mut value: ciborium::Value = ciborium::from_reader(intermediate.as_slice())
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    canonicalize(&mut value)?;
    let mut encoded = Vec::new();
    ciborium::into_writer(&value, &mut encoded)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    enforce_limit(encoded.len())?;
    Ok(encoded)
}

/// Encodes a request for the already authenticated local v3 sidecar.
///
/// The v3 request fields are declared in deterministic CBOR key order, so
/// serializing the struct directly preserves the wire contract without the
/// generic value round trip used by [`encode`].
pub fn encode_sidecar_request(value: &v3::Request) -> Result<Vec<u8>, TypeFactsError> {
    let mut encoded = Vec::new();
    ciborium::into_writer(value, &mut encoded)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    enforce_limit(encoded.len())?;
    Ok(encoded)
}

pub fn decode<T: DeserializeOwned>(encoded: &[u8]) -> Result<T, TypeFactsError> {
    enforce_limit(encoded.len())?;
    validate_deterministic_cbor(encoded)?;
    ciborium::from_reader(encoded).map_err(|error| TypeFactsError::Codec(error.to_string()))
}

/// Decodes a frame from the already authenticated local v3 sidecar.
///
/// Frozen protocol fixtures and untrusted inputs must continue to use
/// [`decode`], which verifies deterministic CBOR before deserializing.
pub fn decode_trusted<T: DeserializeOwned>(encoded: &[u8]) -> Result<T, TypeFactsError> {
    enforce_limit(encoded.len())?;
    ciborium::from_reader(encoded).map_err(|error| TypeFactsError::Codec(error.to_string()))
}

pub struct FramedTransport<S> {
    stream: S,
}

impl<S> FramedTransport<S> {
    #[must_use]
    pub const fn new(stream: S) -> Self {
        Self { stream }
    }

    #[must_use]
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Read + Write> FramedTransport<S> {
    pub fn send<Request: Serialize>(&mut self, request: &Request) -> Result<(), TypeFactsError> {
        let encoded = encode(request)?;
        let length = u32::try_from(encoded.len()).map_err(|_| TypeFactsError::MessageLimit {
            actual: encoded.len(),
            limit: u32::MAX as usize,
        })?;
        self.stream.write_all(&length.to_le_bytes())?;
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive<Response: DeserializeOwned>(&mut self) -> Result<Response, TypeFactsError> {
        let mut prefix = [0_u8; 4];
        self.stream.read_exact(&mut prefix)?;
        let response_len = u32::from_le_bytes(prefix) as usize;
        enforce_limit(response_len)?;
        let mut response = vec![0_u8; response_len];
        self.stream.read_exact(&mut response)?;
        decode(&response)
    }

    pub fn exchange<Request: Serialize, Response: DeserializeOwned>(
        &mut self,
        request: &Request,
    ) -> Result<Response, TypeFactsError> {
        self.send(request)?;
        self.receive()
    }

    pub fn closure(&mut self, request: &ClosureRequest) -> Result<ClosureResponse, TypeFactsError> {
        request.validate()?;
        let response: ClosureResponse = self.exchange(request)?;
        response.validate_for(request)?;
        Ok(response)
    }
}

fn validate_table_locations(table: &FactTable) -> Result<(), TypeFactsError> {
    for entity in &table.entities {
        validate_location("entity", &entity.location)?;
    }
    for symbol in &table.symbols {
        for declaration in &symbol.declarations {
            validate_location("declaration", &declaration.location)?;
        }
        validate_locations("symbol reference", &symbol.references)?;
    }
    for file in &table.files {
        for call in &file.calls {
            validate_location("source call", &call.location)?;
            validate_location("source callee", &call.callee)?;
            validate_locations("source argument", &call.arguments)?;
        }
        for binding in &file.bindings {
            for name in &binding.names {
                validate_optional_location(name)?;
            }
        }
        for function in &file.functions {
            validate_optional_location(&function.name)?;
            validate_optional_location(&function.body)?;
            validate_locations("function parameter", &function.parameters)?;
        }
        for function in &file.async_functions {
            validate_location("async expression", &function.expression)?;
            validate_locations("call after await", &function.calls_after_await)?;
        }
    }
    Ok(())
}

fn validate_optional_location(location: &Location) -> Result<(), TypeFactsError> {
    if location.path.is_empty() && location.start_byte == 0 && location.end_byte == 0 {
        Ok(())
    } else {
        validate_location("optional function", location)
    }
}

fn validate_locations(
    category: &'static str,
    locations: &[Location],
) -> Result<(), TypeFactsError> {
    for location in locations {
        validate_location(category, location)?;
    }
    Ok(())
}

fn validate_location(category: &'static str, location: &Location) -> Result<(), TypeFactsError> {
    if location.path.is_empty() || location.start_byte > location.end_byte {
        return Err(TypeFactsError::InvalidLocation {
            category,
            path: location.path.clone(),
            start: location.start_byte,
            end: location.end_byte,
        });
    }
    Ok(())
}

fn location_cmp(left: &Location, right: &Location) -> std::cmp::Ordering {
    (&left.path, left.start_byte, left.end_byte).cmp(&(
        &right.path,
        right.start_byte,
        right.end_byte,
    ))
}

fn canonicalize(value: &mut ciborium::Value) -> Result<(), TypeFactsError> {
    match value {
        ciborium::Value::Array(values) => {
            for value in values {
                canonicalize(value)?;
            }
        }
        ciborium::Value::Map(entries) => {
            for (key, value) in entries.iter_mut() {
                canonicalize(key)?;
                canonicalize(value)?;
            }
            let mut keyed = entries
                .drain(..)
                .map(|entry| {
                    let mut encoded_key = Vec::new();
                    ciborium::into_writer(&entry.0, &mut encoded_key)
                        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
                    Ok((encoded_key, entry))
                })
                .collect::<Result<Vec<_>, TypeFactsError>>()?;
            keyed.sort_by(|left, right| {
                left.0
                    .len()
                    .cmp(&right.0.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            entries.extend(keyed.into_iter().map(|(_, entry)| entry));
        }
        ciborium::Value::Tag(_, value) => canonicalize(value)?,
        _ => {}
    }
    Ok(())
}

fn enforce_limit(length: usize) -> Result<(), TypeFactsError> {
    if length > MAX_MESSAGE_BYTES {
        return Err(TypeFactsError::MessageLimit {
            actual: length,
            limit: MAX_MESSAGE_BYTES,
        });
    }
    Ok(())
}

fn validate_deterministic_cbor(encoded: &[u8]) -> Result<(), TypeFactsError> {
    let end = validate_cbor_item(encoded, 0, 1)?;
    if end != encoded.len() {
        return Err(TypeFactsError::DeterministicCbor(
            "trailing bytes after top-level item".into(),
        ));
    }
    Ok(())
}

fn validate_cbor_item(encoded: &[u8], start: usize, depth: usize) -> Result<usize, TypeFactsError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(TypeFactsError::DeterministicCbor(format!(
            "nesting depth exceeds {MAX_NESTING_DEPTH}"
        )));
    }
    let initial = *encoded
        .get(start)
        .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated item".into()))?;
    let major = initial >> 5;
    let additional = initial & 0x1f;
    let (argument, mut cursor) = decode_cbor_argument(encoded, start + 1, additional)?;
    match major {
        0 | 1 => Ok(cursor),
        2 | 3 => {
            let length = usize::try_from(argument).map_err(|_| {
                TypeFactsError::DeterministicCbor("string length overflows usize".into())
            })?;
            let end = cursor.checked_add(length).ok_or_else(|| {
                TypeFactsError::DeterministicCbor("string length overflow".into())
            })?;
            let bytes = encoded
                .get(cursor..end)
                .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated string".into()))?;
            if major == 3 {
                std::str::from_utf8(bytes).map_err(|error| {
                    TypeFactsError::DeterministicCbor(format!(
                        "text string at byte {cursor} (length {length}) is not UTF-8: {error}"
                    ))
                })?;
            }
            Ok(end)
        }
        4 => {
            let length = collection_length(argument)?;
            for _ in 0..length {
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
            }
            Ok(cursor)
        }
        5 => {
            let length = collection_length(argument)?;
            let mut previous_key: Option<&[u8]> = None;
            for _ in 0..length {
                let key_start = cursor;
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
                let key = &encoded[key_start..cursor];
                if let Some(previous) = previous_key {
                    let ordering = previous
                        .len()
                        .cmp(&key.len())
                        .then_with(|| previous.cmp(key));
                    if !ordering.is_lt() {
                        return Err(TypeFactsError::DeterministicCbor(
                            if previous == key {
                                "duplicate map key"
                            } else {
                                "map keys are not in core deterministic order"
                            }
                            .into(),
                        ));
                    }
                }
                previous_key = Some(key);
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
            }
            Ok(cursor)
        }
        6 => Err(TypeFactsError::DeterministicCbor(
            "CBOR tags are forbidden".into(),
        )),
        7 if matches!(additional, 20 | 21) => Ok(cursor),
        7 => Err(TypeFactsError::DeterministicCbor(
            "only boolean simple values are permitted".into(),
        )),
        _ => Err(TypeFactsError::DeterministicCbor(format!(
            "unsupported CBOR major type {major}"
        ))),
    }
}

fn decode_cbor_argument(
    encoded: &[u8],
    cursor: usize,
    additional: u8,
) -> Result<(u64, usize), TypeFactsError> {
    let (argument, width) = match additional {
        value @ 0..=23 => (u64::from(value), 0),
        24 => (
            u64::from(*encoded.get(cursor).ok_or_else(|| {
                TypeFactsError::DeterministicCbor("truncated uint8 argument".into())
            })?),
            1,
        ),
        25 => (
            u64::from(u16::from_be_bytes(read_cbor_bytes(encoded, cursor)?)),
            2,
        ),
        26 => (
            u64::from(u32::from_be_bytes(read_cbor_bytes(encoded, cursor)?)),
            4,
        ),
        27 => (u64::from_be_bytes(read_cbor_bytes(encoded, cursor)?), 8),
        31 => {
            return Err(TypeFactsError::DeterministicCbor(
                "indefinite-length items are forbidden".into(),
            ));
        }
        value => {
            return Err(TypeFactsError::DeterministicCbor(format!(
                "reserved additional information {value}"
            )));
        }
    };
    let shortest = match width {
        0 => true,
        1 => argument >= 24,
        2 => argument > u64::from(u8::MAX),
        4 => argument > u64::from(u16::MAX),
        8 => argument > u64::from(u32::MAX),
        _ => unreachable!(),
    };
    if !shortest {
        return Err(TypeFactsError::DeterministicCbor(
            "integer or length is not shortest-form encoded".into(),
        ));
    }
    Ok((argument, cursor + width))
}

fn read_cbor_bytes<const N: usize>(
    encoded: &[u8],
    cursor: usize,
) -> Result<[u8; N], TypeFactsError> {
    encoded
        .get(cursor..cursor + N)
        .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated argument".into()))?
        .try_into()
        .map_err(|_| TypeFactsError::DeterministicCbor("invalid argument width".into()))
}

fn collection_length(argument: u64) -> Result<usize, TypeFactsError> {
    let length = usize::try_from(argument).map_err(|_| {
        TypeFactsError::DeterministicCbor("collection length overflows usize".into())
    })?;
    if length > MAX_COLLECTION_LENGTH {
        return Err(TypeFactsError::DeterministicCbor(format!(
            "collection length {length} exceeds {MAX_COLLECTION_LENGTH}"
        )));
    }
    Ok(length)
}

const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_canonicalized() {
        let generation = Generation::new(3).unwrap();
        let request = ClosureRequest::new(
            "project",
            generation,
            vec![
                Location {
                    path: "b.ts".into(),
                    start_byte: 2,
                    end_byte: 3,
                },
                Location {
                    path: "a.ts".into(),
                    start_byte: 4,
                    end_byte: 5,
                },
            ],
        )
        .unwrap();
        assert_eq!(request.compiler_spans[0].path, "a.ts");
        assert_eq!(
            decode::<ClosureRequest>(&encode(&request).unwrap()).unwrap(),
            request
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
            schema: v3::TYPE_FACTS_SCHEMA_V3,
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
            structural_spans: vec![location.clone()],
            compiler_spans: vec![location.clone()],
            demands: vec![v3::EntityDemand {
                location,
                query_location: None,
                symbol: true,
                type_descriptor: true,
                resolved_call: false,
                references: true,
                r#async: false,
                structural_accessor: false,
            }],
            state_token: "9".into(),
            reset_state: false,
            removed_demand_paths: vec!["old.ts".into()],
            cancel_request_id: 2,
        };
        assert_eq!(
            encode_sidecar_request(&request).unwrap(),
            encode(&request).unwrap()
        );
    }

    #[test]
    fn rejects_alias_reference_storage() {
        let request = ClosureRequest::new("project", Generation::new(1).unwrap(), vec![]).unwrap();
        let response = ClosureResponse {
            schema: 2,
            project_id: "project".into(),
            generation: 1,
            table: FactTable {
                schema: 2,
                generation: 1,
                project_id: "project".into(),
                sources: vec![],
                entities: vec![],
                symbols: vec![SymbolFact {
                    id: "s1".into(),
                    alias_target: "s2".into(),
                    declarations: vec![],
                    references: vec![Location {
                        path: "a.ts".into(),
                        start_byte: 0,
                        end_byte: 1,
                    }],
                }],
                files: vec![],
            },
        };
        assert!(matches!(
            response.validate_for(&request),
            Err(TypeFactsError::AliasReferences(_))
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
}
