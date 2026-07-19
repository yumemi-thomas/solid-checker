use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use solid_facts_core::SourceHash;

use crate::{
    AsyncFunctionFact, Declaration, EntityFact, FactTable, FileFact, Location, ResolvedCall,
    SourceBinding, SourceCall, SourceDigest, SourceFunction, SymbolFact, TypeDescriptor,
};

pub const TYPE_FACTS_SCHEMA_V3: u64 = 3;
pub const TYPE_FACTS_SCHEMA_SHA256: &str =
    "sha256:6a35b7da27fa097f43cde6ea474e2d64b80823a468593e7044ffb25bb33f8e44";
pub const TYPE_FACTS_HANDSHAKE_PROTOCOL: u64 = 1;
pub const TYPE_FACTS_BUILD_ID: &str = match option_env!("SOLID_CHECK_BUILD_ID") {
    Some(value) => value,
    None => "dev",
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Handshake {
    pub protocol: u64,
    pub schema_hash: String,
    pub build_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Open,
    Update,
    Analyze,
    Sources,
    Cancel,
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChange {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source: Vec<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
    pub version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityDemand {
    #[serde(default, skip_serializing_if = "is_false")]
    pub r#async: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub symbol: bool,
    pub location: Location,
    #[serde(default, skip_serializing_if = "is_false")]
    pub references: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub resolved_call: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_location: Option<Location>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub type_descriptor: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub structural_accessor: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub schema: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<FileChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demands: Vec<EntityDemand>,
    pub operation: Operation,
    pub project_id: String,
    pub request_id: u64,
    pub generation: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reset_state: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state_token: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compiler_spans: Vec<Location>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_demands: Option<CompactDemands>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cancel_request_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structural_spans: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_demand_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityFileDelta {
    pub path: String,
    pub entities: Vec<EntityFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolReferenceFileDelta {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FactTableDelta {
    pub generation: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<SourceDigest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_source_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_files: Vec<EntityFileDelta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_entity_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<SymbolFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_symbol_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbol_reference_files: Vec<SymbolReferenceFileDelta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_file_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Error {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub local: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerTimings {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub request_decode_ns: u64,
    pub analyze_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub r#async_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub demand_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub assembly_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sort_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub close_symbols_ns: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub prepare_ns: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub materialized: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retained_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub recomputed_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub non_durable_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub schema: u64,
    pub request_id: u64,
    pub project_id: String,
    pub generation: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<FactTable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_table: Option<CompactFactTable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packed_table: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_delta: Option<FactTableDelta>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub table_mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state_token: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<SourceFile>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_arena: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_lengths: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<ServerTimings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
    #[serde(skip)]
    pub client_decode_ns: u64,
    #[serde(skip)]
    pub client_response_bytes: u64,
    #[serde(skip)]
    pub client_request_send_ns: u64,
    #[serde(skip)]
    pub client_request_bytes: u64,
    #[serde(skip)]
    pub client_roundtrip_ns: u64,
}

const fn is_zero(value: &u64) -> bool {
    *value == 0
}

const fn is_false(value: &bool) -> bool {
    !*value
}

// Compact v3 full-frame encoding.
//
// Cold analyze exchanges dominate boundary bytes because the plain wire
// shapes repeat CBOR field-name keys on every record and the absolute source
// path on every location. The compact forms carry one string dictionary per
// frame (index 0 is reserved for the empty string, which also encodes an
// absent optional string) and encode rows as fixed-arity arrays. They expand
// into exactly the plain shapes, so everything past the transport seam is
// unchanged. Both executables ship in build-ID lockstep, so no runtime
// negotiation exists. Tuple element order mirrors the Go `toarray` structs
// in internal/typefacts/protocolv3_compact.go.

/// `[path-index, startByte, endByte]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactLocation(pub u64, pub u64, pub u64);

/// `[path-index, packed demand rows]`. Each byte string stores unsigned
/// LEB128 rows as `(flags << 1 | hasQuery, startDelta, length)`, followed by
/// `(queryPath, queryStart, queryLength)` when present. Starts are delta-coded
/// within the enclosing path group.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactDemandGroup(pub u64, pub Vec<u8>);

/// The compact form of a full demand snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactDemands {
    pub groups: Vec<CompactDemandGroup>,
    pub strings: Vec<String>,
}

pub const DEMAND_FLAG_SYMBOL: u64 = 1 << 0;
pub const DEMAND_FLAG_REFERENCES: u64 = 1 << 1;
pub const DEMAND_FLAG_TYPE_DESCRIPTOR: u64 = 1 << 2;
pub const DEMAND_FLAG_RESOLVED_CALL: u64 = 1 << 3;
pub const DEMAND_FLAG_ASYNC: u64 = 1 << 4;
pub const DEMAND_FLAG_STRUCTURAL_ACCESSOR: u64 = 1 << 5;

fn push_uvarint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

/// `[name-index, kind-index, location]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactDeclaration(pub u64, pub u64, pub CompactLocation);

/// `[text-index, originModule-index, aliasDeclarations]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactTypeDescriptor(pub u64, pub u64, pub Vec<CompactDeclaration>);

/// `[target-index, returnTypeText-index]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactCall(pub u64, pub u64);

/// `[startByte, endByte, symbol-index, descriptor-or-empty, resolvedCall-or-empty]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactEntityFact(
    pub u64,
    pub u64,
    pub u64,
    pub Vec<CompactTypeDescriptor>,
    pub Vec<CompactCall>,
);

/// `[path-index, entity rows]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactEntityFile(pub u64, pub Vec<CompactEntityFact>);

/// `[id-index, aliasTarget-index, declarations, references]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactSymbolFact(
    pub u64,
    pub u64,
    pub Vec<CompactDeclaration>,
    pub Vec<CompactLocation>,
);

/// `[location, callee, arguments, target-index]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactSourceCall(
    pub CompactLocation,
    pub CompactLocation,
    pub Vec<CompactLocation>,
    pub u64,
);

/// `[flags, names, initializer]`; flag bit 0 is `array`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactSourceBinding(pub u64, pub Vec<CompactLocation>, pub CompactSourceCall);

pub const BINDING_FLAG_ARRAY: u64 = 1 << 0;

/// `[name, body, parameters, flags]`; flag bits are exported, async, arrow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactSourceFunction(
    pub CompactLocation,
    pub CompactLocation,
    pub Vec<CompactLocation>,
    pub u64,
);

pub const FUNCTION_FLAG_EXPORTED: u64 = 1 << 0;
pub const FUNCTION_FLAG_ASYNC: u64 = 1 << 1;
pub const FUNCTION_FLAG_ARROW: u64 = 1 << 2;

/// `[expression, symbol-index, target-index, flags, callsAfterAwait]`; flag
/// bit 0 is canReturnAsync.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactAsyncFunction(
    pub CompactLocation,
    pub u64,
    pub u64,
    pub u64,
    pub Vec<CompactLocation>,
);

pub const ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC: u64 = 1 << 0;

/// `[path-index, calls, bindings, functions, asyncFunctions]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactFileFact(
    pub u64,
    pub Vec<CompactSourceCall>,
    pub Vec<CompactSourceBinding>,
    pub Vec<CompactSourceFunction>,
    pub Vec<CompactAsyncFunction>,
);

/// `[path-index, sha256]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactSourceDigest(pub u64, pub SourceHash);

/// The compact form of a full v2-shaped fact table.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactFactTable {
    pub schema: u64,
    pub generation: u64,
    pub project_id: String,
    pub strings: Vec<String>,
    pub sources: Vec<CompactSourceDigest>,
    pub entity_files: Vec<CompactEntityFile>,
    pub symbols: Vec<CompactSymbolFact>,
    pub files: Vec<CompactFileFact>,
}

const PACKED_FACT_TABLE_VERSION: u64 = 2;
const PACKED_COLLECTION_LIMIT: usize = 1_000_000;

struct PackedCursor<'a> {
    input: &'a [u8],
    offset: usize,
}

#[derive(Default)]
struct PackedLocationState {
    path: usize,
    start: u64,
    valid: bool,
}

impl<'a> PackedCursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn u64(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        for shift in (0..=63).step_by(7) {
            let byte = *self
                .input
                .get(self.offset)
                .ok_or_else(|| "packed table is truncated".to_owned())?;
            self.offset += 1;
            if shift == 63 && byte > 1 {
                return Err("packed table integer overflow".into());
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("packed table integer overflow".into())
    }

    fn signed(&mut self) -> Result<i64, String> {
        let value = self.u64()?;
        Ok(((value >> 1) as i64) ^ -((value & 1) as i64))
    }

    fn count(&mut self, label: &str) -> Result<usize, String> {
        let count = usize::try_from(self.u64()?)
            .map_err(|_| format!("packed {label} count overflows usize"))?;
        if count > PACKED_COLLECTION_LIMIT {
            return Err(format!(
                "packed {label} count {count} exceeds {PACKED_COLLECTION_LIMIT}"
            ));
        }
        Ok(count)
    }

    fn raw(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| "packed table range overflow".to_owned())?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or_else(|| "packed table is truncated".to_owned())?;
        self.offset = end;
        Ok(bytes)
    }

    fn string_index(&mut self, strings: &[String], label: &str) -> Result<String, String> {
        let index = usize::try_from(self.u64()?)
            .map_err(|_| format!("packed {label} string index overflows usize"))?;
        strings
            .get(index)
            .cloned()
            .ok_or_else(|| format!("packed {label} string index {index} is out of range"))
    }

    fn location(
        &mut self,
        strings: &[String],
        state: &mut PackedLocationState,
    ) -> Result<Location, String> {
        let path_token = self.u64()?;
        let (path, start) = if path_token & 1 == 1 {
            if path_token != 1 || !state.valid {
                return Err("packed location has invalid repeated-path marker".into());
            }
            let start = add_signed(state.start, self.signed()?, "location start")?;
            (state.path, start)
        } else {
            let path = usize::try_from(path_token >> 1)
                .map_err(|_| "packed location path index overflows usize".to_owned())?;
            if path >= strings.len() {
                return Err(format!("packed location path index {path} is out of range"));
            }
            (path, self.u64()?)
        };
        let end = start
            .checked_add(self.u64()?)
            .ok_or_else(|| "packed location end overflow".to_owned())?;
        state.path = path;
        state.start = start;
        state.valid = true;
        Ok(Location {
            path: strings[path].clone(),
            start_byte: start,
            end_byte: end,
        })
    }

    fn locations(&mut self, strings: &[String]) -> Result<Vec<Location>, String> {
        let count = self.count("locations")?;
        let mut locations = Vec::with_capacity(count);
        let mut state = PackedLocationState::default();
        for _ in 0..count {
            locations.push(self.location(strings, &mut state)?);
        }
        Ok(locations)
    }

    fn declarations(&mut self, strings: &[String]) -> Result<Vec<Declaration>, String> {
        let count = self.count("declarations")?;
        let mut declarations = Vec::with_capacity(count);
        let mut state = PackedLocationState::default();
        for _ in 0..count {
            declarations.push(Declaration {
                name: self.string_index(strings, "declaration name")?,
                kind: self.string_index(strings, "declaration kind")?,
                location: self.location(strings, &mut state)?,
            });
        }
        Ok(declarations)
    }

    fn source_call(&mut self, strings: &[String]) -> Result<SourceCall, String> {
        let mut state = PackedLocationState::default();
        Ok(SourceCall {
            location: self.location(strings, &mut state)?,
            callee: self.location(strings, &mut state)?,
            arguments: self.locations(strings)?,
            target: self.string_index(strings, "source call target")?,
        })
    }
}

fn add_signed(base: u64, delta: i64, label: &str) -> Result<u64, String> {
    if delta >= 0 {
        base.checked_add(delta as u64)
    } else {
        base.checked_sub(delta.unsigned_abs())
    }
    .ok_or_else(|| format!("packed {label} delta overflow"))
}

fn decode_packed_strings(cursor: &mut PackedCursor<'_>) -> Result<Vec<String>, String> {
    let count = cursor.count("strings")?;
    let mut strings = Vec::with_capacity(count);
    let mut previous = Vec::<u8>::new();
    for _ in 0..count {
        let tag = cursor.u64()?;
        let (value, next_previous) = match tag {
            0 => {
                let prefix = usize::try_from(cursor.u64()?)
                    .map_err(|_| "packed string prefix overflows usize".to_owned())?;
                if prefix > previous.len() {
                    return Err("packed string prefix exceeds previous string".into());
                }
                let suffix_length = usize::try_from(cursor.u64()?)
                    .map_err(|_| "packed string length overflows usize".to_owned())?;
                let mut bytes = previous[..prefix].to_vec();
                bytes.extend_from_slice(cursor.raw(suffix_length)?);
                let value = String::from_utf8(bytes.clone())
                    .map_err(|_| "packed string is not UTF-8".to_owned())?;
                (value, Some(bytes))
            }
            1 => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let raw = cursor.raw(12)?;
                let mut value = String::with_capacity(33);
                value.push_str("symbol:h:");
                for byte in raw {
                    value.push(HEX[usize::from(byte >> 4)] as char);
                    value.push(HEX[usize::from(byte & 0x0f)] as char);
                }
                (value, None)
            }
            other => return Err(format!("packed string has unknown encoding tag {other}")),
        };
        strings.push(value);
        if let Some(bytes) = next_previous {
            previous = bytes;
        }
    }
    Ok(strings)
}

fn raw_digest(bytes: &[u8]) -> Result<SourceHash, String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if bytes.len() != 32 {
        return Err("packed source digest must be 32 bytes".into());
    }
    let mut value = String::with_capacity(71);
    value.push_str("sha256:");
    for byte in bytes {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    SourceHash::parse(value).map_err(|error| error.to_string())
}

/// Decodes the opaque v3 cold frame directly into the retained fact table.
/// No intermediate compact rows or second full-table expansion are created.
pub fn decode_packed_fact_table(input: &[u8], project_id: String) -> Result<FactTable, String> {
    let mut cursor = PackedCursor::new(input);
    let version = cursor.u64()?;
    if version != PACKED_FACT_TABLE_VERSION {
        return Err(format!("unsupported packed table version {version}"));
    }
    let schema = cursor.u64()?;
    let generation = cursor.u64()?;
    let strings = decode_packed_strings(&mut cursor)?;

    let source_count = cursor.count("sources")?;
    let mut sources = Vec::with_capacity(source_count);
    for _ in 0..source_count {
        sources.push(SourceDigest {
            path: cursor.string_index(&strings, "source path")?,
            sha256: raw_digest(cursor.raw(32)?)?,
        });
    }

    let entity_file_count = cursor.count("entity files")?;
    let mut entities = Vec::new();
    for _ in 0..entity_file_count {
        let path = cursor.string_index(&strings, "entity path")?;
        let count = cursor.count("entities")?;
        let mut previous_start = 0;
        for _ in 0..count {
            let start = add_signed(previous_start, cursor.signed()?, "entity start")?;
            let end = start
                .checked_add(cursor.u64()?)
                .ok_or_else(|| "packed entity end overflow".to_owned())?;
            let symbol = cursor.string_index(&strings, "entity symbol")?;
            let flags = cursor.u64()?;
            if flags & !3 != 0 {
                return Err(format!("packed entity has unknown flags {flags}"));
            }
            let type_descriptor = if flags & 1 != 0 {
                Some(TypeDescriptor {
                    text: cursor.string_index(&strings, "type text")?,
                    origin_module: cursor.string_index(&strings, "origin module")?,
                    alias_declarations: cursor.declarations(&strings)?,
                })
            } else {
                None
            };
            let resolved_call = if flags & 2 != 0 {
                Some(ResolvedCall {
                    target: cursor.string_index(&strings, "resolved target")?,
                    return_type_text: cursor.string_index(&strings, "return type")?,
                })
            } else {
                None
            };
            entities.push(EntityFact {
                location: Location {
                    path: path.clone(),
                    start_byte: start,
                    end_byte: end,
                },
                symbol,
                type_descriptor,
                resolved_call,
            });
            previous_start = start;
        }
    }

    let symbol_count = cursor.count("symbols")?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        symbols.push(SymbolFact {
            id: cursor.string_index(&strings, "symbol id")?,
            alias_target: cursor.string_index(&strings, "alias target")?,
            declarations: cursor.declarations(&strings)?,
            references: cursor.locations(&strings)?,
        });
    }

    let file_count = cursor.count("files")?;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let path = cursor.string_index(&strings, "file path")?;
        let call_count = cursor.count("calls")?;
        let mut calls = Vec::with_capacity(call_count);
        for _ in 0..call_count {
            calls.push(cursor.source_call(&strings)?);
        }
        let binding_count = cursor.count("bindings")?;
        let mut bindings = Vec::with_capacity(binding_count);
        for _ in 0..binding_count {
            let flags = cursor.u64()?;
            if flags & !BINDING_FLAG_ARRAY != 0 {
                return Err(format!("packed binding has unknown flags {flags}"));
            }
            bindings.push(SourceBinding {
                array: flags & BINDING_FLAG_ARRAY != 0,
                names: cursor.locations(&strings)?,
                initializer: cursor.source_call(&strings)?,
            });
        }
        let function_count = cursor.count("functions")?;
        let mut functions = Vec::with_capacity(function_count);
        for _ in 0..function_count {
            let mut state = PackedLocationState::default();
            let name = cursor.location(&strings, &mut state)?;
            let body = cursor.location(&strings, &mut state)?;
            let parameters = cursor.locations(&strings)?;
            let flags = cursor.u64()?;
            if flags & !(FUNCTION_FLAG_EXPORTED | FUNCTION_FLAG_ASYNC | FUNCTION_FLAG_ARROW) != 0 {
                return Err(format!("packed function has unknown flags {flags}"));
            }
            functions.push(SourceFunction {
                name,
                body,
                parameters,
                exported: flags & FUNCTION_FLAG_EXPORTED != 0,
                r#async: flags & FUNCTION_FLAG_ASYNC != 0,
                arrow: flags & FUNCTION_FLAG_ARROW != 0,
            });
        }
        let async_count = cursor.count("async functions")?;
        let mut async_functions = Vec::with_capacity(async_count);
        for _ in 0..async_count {
            let mut state = PackedLocationState::default();
            let expression = cursor.location(&strings, &mut state)?;
            let symbol = cursor.string_index(&strings, "async symbol")?;
            let target = cursor.string_index(&strings, "async target")?;
            let flags = cursor.u64()?;
            if flags & !ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC != 0 {
                return Err(format!("packed async function has unknown flags {flags}"));
            }
            async_functions.push(AsyncFunctionFact {
                expression,
                symbol,
                target,
                can_return_async: flags & ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC != 0,
                calls_after_await: cursor.locations(&strings)?,
            });
        }
        files.push(FileFact {
            path,
            calls,
            bindings,
            functions,
            async_functions,
        });
    }
    if cursor.offset != input.len() {
        return Err("packed table has trailing bytes".into());
    }
    Ok(FactTable {
        schema,
        generation,
        project_id,
        sources: sources.into(),
        entities: entities.into(),
        symbols: symbols.into(),
        files: files.into(),
    })
}

struct StringTable<'a> {
    indexes: HashMap<&'a str, u64>,
    values: Vec<String>,
}

impl<'a> StringTable<'a> {
    fn new() -> Self {
        Self {
            indexes: HashMap::from([("", 0)]),
            values: vec![String::new()],
        }
    }

    fn intern(&mut self, value: &'a str) -> u64 {
        if let Some(index) = self.indexes.get(value) {
            return *index;
        }
        let index = self.values.len() as u64;
        self.indexes.insert(value, index);
        self.values.push(value.to_owned());
        index
    }
}

fn lookup(strings: &[String], index: u64) -> Result<&str, String> {
    strings
        .get(usize::try_from(index).map_err(|_| "compact string index overflow".to_owned())?)
        .map(String::as_str)
        .ok_or_else(|| {
            format!(
                "compact string index {index} out of range ({} strings)",
                strings.len()
            )
        })
}

/// Converts a full demand snapshot into its compact form. Demands are
/// grouped by location path in input order.
pub fn compact_demands(demands: &[EntityDemand]) -> CompactDemands {
    let mut strings = StringTable::new();
    let mut groups: Vec<CompactDemandGroup> = Vec::new();
    let mut previous_start = 0;
    for demand in demands {
        let path = strings.intern(demand.location.path.as_str());
        if groups
            .last()
            .is_none_or(|group| group.0 != path || demand.location.start_byte < previous_start)
        {
            groups.push(CompactDemandGroup(path, Vec::new()));
            previous_start = 0;
        }
        let mut flags = 0;
        if demand.symbol {
            flags |= DEMAND_FLAG_SYMBOL;
        }
        if demand.references {
            flags |= DEMAND_FLAG_REFERENCES;
        }
        if demand.type_descriptor {
            flags |= DEMAND_FLAG_TYPE_DESCRIPTOR;
        }
        if demand.resolved_call {
            flags |= DEMAND_FLAG_RESOLVED_CALL;
        }
        if demand.r#async {
            flags |= DEMAND_FLAG_ASYNC;
        }
        if demand.structural_accessor {
            flags |= DEMAND_FLAG_STRUCTURAL_ACCESSOR;
        }
        let group = groups.last_mut().expect("group pushed above");
        let has_query = u64::from(demand.query_location.is_some());
        push_uvarint(&mut group.1, (flags << 1) | has_query);
        push_uvarint(
            &mut group.1,
            demand.location.start_byte.saturating_sub(previous_start),
        );
        push_uvarint(
            &mut group.1,
            demand
                .location
                .end_byte
                .saturating_sub(demand.location.start_byte),
        );
        previous_start = demand.location.start_byte;
        if let Some(query) = &demand.query_location {
            push_uvarint(&mut group.1, strings.intern(query.path.as_str()));
            push_uvarint(&mut group.1, query.start_byte);
            push_uvarint(
                &mut group.1,
                query.end_byte.saturating_sub(query.start_byte),
            );
        }
    }
    CompactDemands {
        groups,
        strings: strings.values,
    }
}

impl CompactFactTable {
    /// Expands the compact table into the plain full table. Every dictionary
    /// reference is bounds-checked; a gap fails the frame closed.
    pub fn expand(self) -> Result<FactTable, String> {
        let strings = &self.strings;
        let location = |l: &CompactLocation| -> Result<Location, String> {
            Ok(Location {
                path: lookup(strings, l.0)?.to_owned(),
                end_byte: l.2,
                start_byte: l.1,
            })
        };
        let locations = |list: &[CompactLocation]| -> Result<Vec<Location>, String> {
            list.iter().map(location).collect()
        };
        let declarations = |list: &[CompactDeclaration]| -> Result<Vec<Declaration>, String> {
            list.iter()
                .map(|d| {
                    Ok(Declaration {
                        name: lookup(strings, d.0)?.to_owned(),
                        kind: lookup(strings, d.1)?.to_owned(),
                        location: location(&d.2)?,
                    })
                })
                .collect()
        };
        let source_call = |c: &CompactSourceCall| -> Result<SourceCall, String> {
            Ok(SourceCall {
                location: location(&c.0)?,
                callee: location(&c.1)?,
                arguments: locations(&c.2)?,
                target: lookup(strings, c.3)?.to_owned(),
            })
        };

        let mut sources = Vec::with_capacity(self.sources.len());
        for digest in &self.sources {
            sources.push(SourceDigest {
                path: lookup(strings, digest.0)?.to_owned(),
                sha256: digest.1.clone(),
            });
        }
        let optional = |label: &str, len: usize| -> Result<(), String> {
            if len > 1 {
                return Err(format!("compact optional {label} has {len} elements"));
            }
            Ok(())
        };
        let mut entities = Vec::new();
        for group in &self.entity_files {
            let path = lookup(strings, group.0)?;
            for row in &group.1 {
                optional("typeDescriptor", row.3.len())?;
                optional("resolvedCall", row.4.len())?;
                let type_descriptor = row
                    .3
                    .first()
                    .map(|descriptor| {
                        Ok::<_, String>(TypeDescriptor {
                            text: lookup(strings, descriptor.0)?.to_owned(),
                            origin_module: lookup(strings, descriptor.1)?.to_owned(),
                            alias_declarations: declarations(&descriptor.2)?,
                        })
                    })
                    .transpose()?;
                let resolved_call = row
                    .4
                    .first()
                    .map(|call| {
                        Ok::<_, String>(ResolvedCall {
                            target: lookup(strings, call.0)?.to_owned(),
                            return_type_text: lookup(strings, call.1)?.to_owned(),
                        })
                    })
                    .transpose()?;
                entities.push(EntityFact {
                    location: Location {
                        path: path.to_owned(),
                        end_byte: row.1,
                        start_byte: row.0,
                    },
                    symbol: lookup(strings, row.2)?.to_owned(),
                    type_descriptor,
                    resolved_call,
                });
            }
        }
        let mut symbols = Vec::with_capacity(self.symbols.len());
        for symbol in &self.symbols {
            symbols.push(SymbolFact {
                id: lookup(strings, symbol.0)?.to_owned(),
                alias_target: lookup(strings, symbol.1)?.to_owned(),
                declarations: declarations(&symbol.2)?,
                references: locations(&symbol.3)?,
            });
        }
        let mut files = Vec::with_capacity(self.files.len());
        for file in &self.files {
            let calls = file.1.iter().map(source_call).collect::<Result<_, _>>()?;
            let bindings = file
                .2
                .iter()
                .map(|binding| {
                    Ok::<_, String>(SourceBinding {
                        array: binding.0 & BINDING_FLAG_ARRAY != 0,
                        names: locations(&binding.1)?,
                        initializer: source_call(&binding.2)?,
                    })
                })
                .collect::<Result<_, _>>()?;
            let functions = file
                .3
                .iter()
                .map(|function| {
                    Ok::<_, String>(SourceFunction {
                        name: location(&function.0)?,
                        body: location(&function.1)?,
                        parameters: locations(&function.2)?,
                        exported: function.3 & FUNCTION_FLAG_EXPORTED != 0,
                        r#async: function.3 & FUNCTION_FLAG_ASYNC != 0,
                        arrow: function.3 & FUNCTION_FLAG_ARROW != 0,
                    })
                })
                .collect::<Result<_, _>>()?;
            let async_functions = file
                .4
                .iter()
                .map(|function| {
                    Ok::<_, String>(AsyncFunctionFact {
                        expression: location(&function.0)?,
                        symbol: lookup(strings, function.1)?.to_owned(),
                        target: lookup(strings, function.2)?.to_owned(),
                        can_return_async: function.3 & ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC != 0,
                        calls_after_await: locations(&function.4)?,
                    })
                })
                .collect::<Result<_, _>>()?;
            files.push(FileFact {
                path: lookup(strings, file.0)?.to_owned(),
                calls,
                bindings,
                functions,
                async_functions,
            });
        }
        Ok(FactTable {
            schema: self.schema,
            generation: self.generation,
            project_id: self.project_id,
            sources: sources.into(),
            entities: entities.into(),
            symbols: symbols.into(),
            files: files.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{TYPE_FACTS_SCHEMA_SHA256, decode_packed_fact_table};

    #[test]
    fn handshake_hash_matches_frozen_schema() {
        let actual = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v2.schema.json"))
        );
        assert_eq!(actual, TYPE_FACTS_SCHEMA_SHA256);
    }

    #[test]
    fn packed_table_decoder_is_strict_and_direct() {
        // version, schema, generation, one prefix-coded empty string, then
        // four empty top-level collections.
        let valid = [2, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0];
        let table = decode_packed_fact_table(&valid, "/p/tsconfig.json".into()).unwrap();
        assert_eq!(table.schema, 2);
        assert_eq!(table.generation, 1);
        assert_eq!(table.project_id, "/p/tsconfig.json");
        assert!(table.sources.is_empty());
        assert!(table.entities.is_empty());
        assert!(table.symbols.is_empty());
        assert!(table.files.is_empty());

        assert!(decode_packed_fact_table(&valid[..valid.len() - 1], "/p".into()).is_err());
        assert!(decode_packed_fact_table(&[2, 2, 1], "/p".into()).is_err());
        let mut trailing = valid.to_vec();
        trailing.push(0);
        assert!(decode_packed_fact_table(&trailing, "/p".into()).is_err());
    }
}
