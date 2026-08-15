//! Rust client model for the TypeFacts v3 lifecycle protocol.
//!
//! This package contains checker-derived facts only. Structural discovery is
//! owned by `solid-ast-facts`; no regex or TypeScript AST shape is reproduced.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;
use thiserror::Error;

mod retained_table;
mod session;
mod shared_transition_arena;
pub mod v3;

pub use retained_table::{FactTable, Symbol};
pub use session::{
    AnalysisDemand, Cancellation, DemandGroup, ExchangeTimings, Producer, Session, SessionError,
    TableChanges, UpdateTimings,
};

pub const MAX_MESSAGE_BYTES: usize = 64 << 20;
pub const MAX_NESTING_DEPTH: usize = 32;
pub const MAX_COLLECTION_LENGTH: usize = 1_000_000;
pub const SHA256_PREFIX: &str = "sha256:";

// Fact rows keep heap data behind `Arc`, so persistent retained-table leaves
// can share unchanged values between generations. The wire shape is unchanged;
// `Arc<str>` and `Arc<[T]>` serialize exactly as the string and list they hold.

/// serde `skip_serializing_if` helper for `Arc<[T]>` fields.
fn is_empty_slice<T>(values: &[T]) -> bool {
    values.is_empty()
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceHash(Arc<str>);

impl SourceHash {
    #[must_use]
    pub fn of(source: &str) -> Self {
        Self(format!("{SHA256_PREFIX}{:x}", Sha256::digest(source.as_bytes())).into())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, TypeFactsError> {
        let value = value.into();
        let digest = value
            .strip_prefix(SHA256_PREFIX)
            .ok_or_else(|| TypeFactsError::SourceHash(value.clone()))?;
        if digest.len() != 64
            || !digest
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(TypeFactsError::SourceHash(value));
        }
        Ok(Self(value.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Location {
    pub path: Arc<str>,
    pub end_byte: u64,
    pub start_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Declaration {
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Callability {
    Callable,
    NonCallable,
    Mixed,
    Unknown,
}

/// Checker-derived runtime value classes for one demanded expression.
///
/// `unknown` means the checker could not provide a closed domain. In that
/// case the three `may_be_*` fields conservatively describe all categories
/// that remain possible. The all-false value is the known empty `never`
/// domain, so absence is represented by `Option<RuntimeValueDomain>` on an
/// entity rather than by this struct's zero value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeValueDomain {
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_callable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_undefined: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_other: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unknown: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceSpace {
    Value,
    Type,
    Both,
    Neither,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolvedCallValidity {
    Valid,
    Recovery,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CallKind {
    Unknown,
    Call,
    Construct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentMappingStatus {
    Resolved,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentMappingReason {
    CallUnresolved,
    RecoverySignature,
    CompositeSignature,
    SpreadArgument,
    ParameterUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclarationOwner {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedDeclaration {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub owners: Arc<[DeclarationOwner]>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub qualified_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub origin_module: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub source_file: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub standard_library: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterFact {
    pub index: u64,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    pub callability: Callability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<TypeDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgumentMapping {
    pub argument_index: u64,
    pub status: ArgumentMappingStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<ArgumentMappingReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<ParameterFact>,
}

/// A finite set of exact callable declarations for one composite call.
///
/// `exhaustive` is an explicit compiler proof that `candidates` cover every
/// call signature of the callee's apparent type. A set without that proof
/// must never be treated as the complete runtime dispatch set; the producer
/// only emits proven sets, and consumers must still check the bit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallTargetSet {
    #[serde(default, skip_serializing_if = "is_false")]
    pub exhaustive: bool,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub candidates: Arc<[ResolvedDeclaration]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedCall {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub return_type_text: Arc<str>,
    pub validity: ResolvedCallValidity,
    pub kind: CallKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ResolvedDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<CallTargetSet>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub arguments: Arc<[ArgumentMapping]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeDescriptor {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub text: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub origin_module: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub alias_declarations: Arc<[Declaration]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityFact {
    pub location: Location,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub symbol_unresolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<Arc<TypeDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_call: Option<Arc<ResolvedCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callability: Option<Callability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_value_domain: Option<RuntimeValueDomain>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_space: Option<ReferenceSpace>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub runtime_identity: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolFact {
    pub id: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub alias_target: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub declarations: Arc<[Declaration]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub references: Arc<[Location]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCall {
    pub location: Location,
    pub callee: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<Location>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
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
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_return_async: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls_after_await: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileFact {
    pub path: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub calls: Arc<[SourceCall]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub bindings: Arc<[SourceBinding]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub functions: Arc<[SourceFunction]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub async_functions: Arc<[AsyncFunctionFact]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDigest {
    pub path: Arc<str>,
    pub sha256: SourceHash,
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
    #[error("source hash is not canonical sha256: {0:?}")]
    SourceHash(String),
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

/// Write one length-prefixed payload using the TypeFacts u32-LE frame codec.
pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), TypeFactsError> {
    enforce_limit(payload.len())?;
    let length = u32::try_from(payload.len()).map_err(|_| TypeFactsError::MessageLimit {
        actual: payload.len(),
        limit: u32::MAX as usize,
    })?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read one length-prefixed payload using the TypeFacts u32-LE frame codec.
pub fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>, TypeFactsError> {
    let mut prefix = [0_u8; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_le_bytes(prefix) as usize;
    enforce_limit(length)?;
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
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
    fn retained_entity_rows_keep_large_optional_evidence_indirect() {
        assert!(
            std::mem::size_of::<EntityFact>() <= 96,
            "EntityFact is {} bytes; large optional evidence must not inflate every retained row",
            std::mem::size_of::<EntityFact>()
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
            schema: v3::TYPE_FACTS_SCHEMA_V5,
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
            demands: vec![v3::EntityDemand {
                location,
                query_location: None,
                symbol: true,
                type_descriptor: true,
                resolved_call: false,
                references: true,
                r#async: false,
                structural_accessor: false,
                callability: false,
                runtime_value_domain: false,
                reference_space: false,
                runtime_identity: false,
            }],
            compact_demands: Some(v3::CompactDemands {
                groups: vec![v3::CompactDemandGroup(1, vec![7, 1, 1, 1, 1, 1])],
                strings: vec![String::new(), "a.ts".into()],
            }),
            state_token: "9".into(),
            reset_state: false,
            removed_demand_paths: vec!["old.ts".into()],
            symbol_queries: Vec::new(),
            release_analysis: false,
            reference_changes: false,
            reference_paths: Vec::new(),
            cancel_request_id: 2,
        };
        assert_eq!(
            encode_sidecar_request(&request).unwrap(),
            encode(&request).unwrap()
        );
    }

    #[test]
    fn compact_demands_round_trip() {
        let location = |path: &str, start: u64, end: u64| Location {
            path: path.into(),
            start_byte: start,
            end_byte: end,
        };
        let demands = vec![
            v3::EntityDemand {
                location: location("a.ts", 1, 4),
                query_location: None,
                symbol: true,
                type_descriptor: false,
                resolved_call: false,
                references: true,
                r#async: false,
                structural_accessor: false,
                callability: false,
                runtime_value_domain: false,
                reference_space: false,
                runtime_identity: false,
            },
            v3::EntityDemand {
                location: location("a.ts", 5, 9),
                query_location: Some(location("a.ts", 6, 8)),
                symbol: true,
                type_descriptor: true,
                resolved_call: true,
                references: false,
                r#async: true,
                structural_accessor: true,
                callability: true,
                runtime_value_domain: true,
                reference_space: true,
                runtime_identity: true,
            },
            v3::EntityDemand {
                location: location("b.ts", 2, 8),
                query_location: None,
                symbol: false,
                type_descriptor: false,
                resolved_call: false,
                references: false,
                r#async: true,
                structural_accessor: false,
                callability: false,
                runtime_value_domain: false,
                reference_space: false,
                runtime_identity: false,
            },
        ];
        let compact = v3::compact_demands(&demands);
        let decoded: v3::CompactDemands = decode(&encode(&compact).unwrap()).unwrap();
        assert_eq!(decoded, compact);
        assert_eq!(decoded.groups.len(), 2);
        assert_eq!(decoded.strings[0], "");
    }

    #[test]
    fn frame_codec_round_trips_and_rejects_oversized_prefixes() {
        let mut framed = Vec::new();
        write_frame(&mut framed, b"payload").unwrap();
        assert_eq!(read_frame(&mut framed.as_slice()).unwrap(), b"payload");

        let oversized = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap().to_le_bytes();
        assert!(matches!(
            read_frame(&mut oversized.as_slice()),
            Err(TypeFactsError::MessageLimit { .. })
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
