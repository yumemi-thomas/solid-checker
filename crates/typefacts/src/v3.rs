use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    ArgumentMapping, ArgumentMappingReason, ArgumentMappingStatus, AsyncFunctionFact, CallKind,
    CallTargetSet, Callability, Declaration, DeclarationOwner, EntityFact, FileFact, Location,
    ParameterFact, ReferenceSpace, ResolvedCall, ResolvedCallValidity, ResolvedDeclaration,
    RuntimeValueDomain, SourceBinding, SourceCall, SourceFunction, SourceHash, SymbolFact,
    TypeDescriptor,
};

pub const TYPE_FACTS_SCHEMA_V5: u64 = 5;
pub const TYPE_FACTS_SCHEMA_V6: u64 = 6;
pub const TYPE_FACTS_SCHEMA_V7: u64 = 7;
pub const TYPE_FACTS_SCHEMA_V8: u64 = 8;
pub const TYPE_FACTS_SCHEMA_V9: u64 = 9;
pub(crate) const TYPE_FACTS_TABLE_SCHEMA_V3: u64 = 3;
pub(crate) const TYPE_FACTS_TABLE_SCHEMA_V4: u64 = 4;
pub(crate) const TYPE_FACTS_TABLE_SCHEMA_V5: u64 = 5;
pub(crate) const TYPE_FACTS_TABLE_SCHEMA_V6: u64 = 6;
pub const TYPE_FACTS_SCHEMA_SHA256: &str =
    "sha256:a4dfff25783d9dd99cf0d44e315a7c01e6c7d132965431ab5624a0975fd549a8";
pub const TYPE_FACTS_SCHEMA_V6_SHA256: &str =
    "sha256:adffdee1486dd009bb2599593e09edd4c48804678b4f23002f72e5693ffc606d";
pub const TYPE_FACTS_SCHEMA_V7_SHA256: &str =
    "sha256:6939a166249694edf3cf4fe1f81bd687f9b572d331988f2faaa6f2277047d352";
pub const TYPE_FACTS_SCHEMA_V8_SHA256: &str =
    "sha256:edbb15dc48793a12230a70305d2586da503f27f26ae5644c43b271661a30b1e1";
pub const TYPE_FACTS_SCHEMA_V9_SHA256: &str =
    "sha256:c6f4ffd342381a64ba6220785f7a71f688d15c9abee43bddd6d37c8d790c2e8d";
pub const TYPE_FACTS_HANDSHAKE_PROTOCOL: u64 = 1;
pub const TYPE_FACTS_BUILD_ID: &str = match option_env!("TYPEFACTS_BUILD_ID") {
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
    Symbols,
    Sources,
    Cancel,
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChange {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "serde_bytes")]
    pub source: Vec<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
    pub version: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub callability: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub runtime_value_domain: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reference_space: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub runtime_identity: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_demands: Option<CompactDemands>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cancel_request_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_demand_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbol_queries: Vec<SymbolQuery>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub release_analysis: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reference_changes: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolQuery {
    pub id: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub references: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub references_only: bool,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "serde_bytes")]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "serde_bytes")]
    pub table_transition: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbol_evidence: Vec<SymbolFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_evidence: Vec<SymbolFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_reference_symbols: Vec<Arc<str>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reference_changes_exact: bool,
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

// Compact v3 demand encoding.
//
// Cold analyze exchanges dominate boundary bytes because the plain wire
// shapes repeat CBOR field-name keys on every record and the absolute source
// path on every location. The compact demand snapshot carries one string
// dictionary per frame (index 0 is reserved for the empty string, which also
// encodes an absent optional string) and packs rows as varints. Both
// executables ship in build-ID lockstep, so no runtime negotiation exists.
// Element order mirrors the Go `toarray` structs in
// internal/typefacts/protocolv3_compact.go.
//
// An analyze response carries at most one opaque table-transition frame.

/// `[path-index, packed demand rows]`. Each byte string stores unsigned
/// LEB128 rows as `(flags << 1 | hasQuery, startDelta, length)`, followed by
/// `(queryPath, queryStart, queryLength)` when present. Starts are delta-coded
/// within the enclosing path group.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactDemandGroup(pub u64, #[serde(with = "serde_bytes")] pub Vec<u8>);

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
pub const DEMAND_FLAG_CALLABILITY: u64 = 1 << 6;
pub const DEMAND_FLAG_REFERENCE_SPACE: u64 = 1 << 7;
pub const DEMAND_FLAG_RUNTIME_IDENTITY: u64 = 1 << 8;
pub const DEMAND_FLAG_RUNTIME_VALUE_DOMAIN: u64 = 1 << 9;

fn push_uvarint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

// Optional-field and boolean flag bits inside the packed fact-table frame.
pub const BINDING_FLAG_ARRAY: u64 = 1 << 0;
pub const FUNCTION_FLAG_EXPORTED: u64 = 1 << 0;
pub const FUNCTION_FLAG_ASYNC: u64 = 1 << 1;
pub const FUNCTION_FLAG_ARROW: u64 = 1 << 2;
pub const ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC: u64 = 1 << 0;

const TABLE_TRANSITION_VERSION: u64 = 1;
const PACKED_COLLECTION_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransitionMode {
    Full,
    Delta,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SlotOp<T> {
    Unchanged,
    Replace(T),
    Remove,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathOp {
    pub path: Arc<str>,
    pub source: SlotOp<SourceHash>,
    pub entities: SlotOp<Vec<EntityFact>>,
    pub file: SlotOp<FileFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SymbolOp {
    Replace(SymbolFact),
    Remove {
        id: Arc<str>,
    },
    ReplaceReferencePath {
        id: Arc<str>,
        path: Arc<str>,
        references: Vec<Location>,
    },
}

impl SymbolOp {
    pub(crate) fn id(&self) -> &Arc<str> {
        match self {
            Self::Replace(symbol) => &symbol.id,
            Self::Remove { id } | Self::ReplaceReferencePath { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WireTableTransition {
    pub mode: TransitionMode,
    pub table_schema: u64,
    pub base_generation: u64,
    pub target_generation: u64,
    pub project_id: Arc<str>,
    pub base_state_token: Arc<str>,
    pub paths: Vec<PathOp>,
    pub symbols: Vec<SymbolOp>,
}

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
                if shift != 0 && byte == 0 {
                    return Err("packed table integer is not shortest-form encoded".into());
                }
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

    fn boolean(&mut self, label: &str) -> Result<bool, String> {
        match self.u64()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("packed {label} boolean has value {value}")),
        }
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

    fn string_index(&mut self, strings: &[Arc<str>], label: &str) -> Result<Arc<str>, String> {
        let index = usize::try_from(self.u64()?)
            .map_err(|_| format!("packed {label} string index overflows usize"))?;
        strings
            .get(index)
            .cloned()
            .ok_or_else(|| format!("packed {label} string index {index} is out of range"))
    }

    fn location(
        &mut self,
        strings: &[Arc<str>],
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

    fn locations(&mut self, strings: &[Arc<str>]) -> Result<Vec<Location>, String> {
        let count = self.count("locations")?;
        let mut locations = Vec::with_capacity(count);
        let mut state = PackedLocationState::default();
        for _ in 0..count {
            locations.push(self.location(strings, &mut state)?);
        }
        Ok(locations)
    }

    fn declarations(&mut self, strings: &[Arc<str>]) -> Result<Vec<Declaration>, String> {
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

    fn type_descriptor(&mut self, strings: &[Arc<str>]) -> Result<TypeDescriptor, String> {
        Ok(TypeDescriptor {
            text: self.string_index(strings, "type text")?,
            origin_module: self.string_index(strings, "origin module")?,
            alias_declarations: self.declarations(strings)?.into(),
        })
    }

    fn resolved_declaration(
        &mut self,
        strings: &[Arc<str>],
    ) -> Result<ResolvedDeclaration, String> {
        let symbol = self.string_index(strings, "resolved declaration symbol")?;
        let name = self.string_index(strings, "resolved declaration name")?;
        let kind = self.string_index(strings, "resolved declaration kind")?;
        let mut state = PackedLocationState::default();
        let location = self.location(strings, &mut state)?;
        let owner_count = self.count("declaration owners")?;
        let mut owners = Vec::with_capacity(owner_count);
        for _ in 0..owner_count {
            owners.push(DeclarationOwner {
                symbol: self.string_index(strings, "declaration owner symbol")?,
                name: self.string_index(strings, "declaration owner name")?,
                kind: self.string_index(strings, "declaration owner kind")?,
                location: self.location(strings, &mut state)?,
            });
        }
        Ok(ResolvedDeclaration {
            symbol,
            name,
            kind,
            location,
            owners: owners.into(),
            qualified_name: self.string_index(strings, "qualified declaration name")?,
            origin_module: self.string_index(strings, "declaration origin module")?,
            source_file: self.string_index(strings, "declaration source file")?,
            standard_library: self.boolean("standard library")?,
        })
    }

    fn resolved_call(
        &mut self,
        strings: &[Arc<str>],
        table_schema: u64,
    ) -> Result<ResolvedCall, String> {
        let target = self.string_index(strings, "resolved target")?;
        let return_type_text = self.string_index(strings, "return type")?;
        let validity = parse_resolved_call_validity(self.u64()?)?;
        let kind = parse_call_kind(self.u64()?)?;
        let declaration = if self.boolean("resolved declaration presence")? {
            Some(self.resolved_declaration(strings)?)
        } else {
            None
        };
        let targets =
            if table_schema >= TYPE_FACTS_TABLE_SCHEMA_V6 && self.boolean("target set presence")? {
                let exhaustive = self.boolean("target set exhaustiveness")?;
                let candidate_count = self.count("target candidates")?;
                if candidate_count == 0 {
                    return Err("packed resolved-call target set has no candidates".into());
                }
                let mut candidates = Vec::with_capacity(candidate_count);
                for _ in 0..candidate_count {
                    let candidate = self.resolved_declaration(strings)?;
                    if candidate.symbol.is_empty() {
                        return Err("packed target candidate has no symbol".into());
                    }
                    candidates.push(candidate);
                }
                Some(CallTargetSet {
                    exhaustive,
                    candidates: candidates.into(),
                })
            } else {
                None
            };
        let argument_count = self.count("argument mappings")?;
        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            let argument_index = self.u64()?;
            let status = parse_argument_mapping_status(self.u64()?)?;
            let unresolved = parse_argument_mapping_reason(status, self.u64()?)?;
            let parameter = if self.boolean("mapped parameter presence")? {
                let index = self.u64()?;
                let symbol = self.string_index(strings, "parameter symbol")?;
                let flags = self.u64()?;
                if flags & !15 != 0 {
                    return Err(format!("packed parameter has unknown flags {flags}"));
                }
                let declaration = if flags & 1 != 0 {
                    let name = self.string_index(strings, "parameter declaration name")?;
                    let kind = self.string_index(strings, "parameter declaration kind")?;
                    let mut state = PackedLocationState::default();
                    Some(Declaration {
                        name,
                        kind,
                        location: self.location(strings, &mut state)?,
                    })
                } else {
                    None
                };
                let callability = parse_callability(self.u64()?)?;
                let type_descriptor = if flags & 8 != 0 {
                    Some(self.type_descriptor(strings)?)
                } else {
                    None
                };
                Some(ParameterFact {
                    index,
                    symbol,
                    declaration,
                    rest: flags & 2 != 0,
                    optional: flags & 4 != 0,
                    callability,
                    type_descriptor,
                })
            } else {
                None
            };
            arguments.push(ArgumentMapping {
                argument_index,
                status,
                unresolved,
                parameter,
            });
        }
        Ok(ResolvedCall {
            target,
            return_type_text,
            validity,
            kind,
            declaration,
            targets,
            arguments: arguments.into(),
        })
    }

    fn source_call(&mut self, strings: &[Arc<str>]) -> Result<SourceCall, String> {
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

fn decode_packed_strings(cursor: &mut PackedCursor<'_>) -> Result<Vec<Arc<str>>, String> {
    let count = cursor.count("strings")?;
    let mut strings = Vec::with_capacity(count);
    // The prefix window for tag-0 strings. Hashed symbols (tag 1) do not
    // participate in prefix coding, so the window survives them untouched.
    let mut previous = Vec::<u8>::new();
    for _ in 0..count {
        let tag = cursor.u64()?;
        let value: Arc<str> = match tag {
            0 => {
                let prefix = usize::try_from(cursor.u64()?)
                    .map_err(|_| "packed string prefix overflows usize".to_owned())?;
                if prefix > previous.len() {
                    return Err("packed string prefix exceeds previous string".into());
                }
                let suffix_length = usize::try_from(cursor.u64()?)
                    .map_err(|_| "packed string length overflows usize".to_owned())?;
                previous.truncate(prefix);
                previous.extend_from_slice(cursor.raw(suffix_length)?);
                std::str::from_utf8(&previous)
                    .map_err(|_| "packed string is not UTF-8".to_owned())?
                    .into()
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
                value.into()
            }
            other => return Err(format!("packed string has unknown encoding tag {other}")),
        };
        strings.push(value);
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

/// Decodes one path's entity run: a count, then delta-coded starts, lengths,
/// and flagged optional fields, appended to `entities`.
fn decode_entity_run(
    cursor: &mut PackedCursor<'_>,
    strings: &[Arc<str>],
    path: &Arc<str>,
    table_schema: u64,
    entities: &mut Vec<EntityFact>,
) -> Result<(), String> {
    let count = cursor.count("entities")?;
    entities.reserve(count);
    let mut previous_start = 0;
    for _ in 0..count {
        let start = add_signed(previous_start, cursor.signed()?, "entity start")?;
        let end = start
            .checked_add(cursor.u64()?)
            .ok_or_else(|| "packed entity end overflow".to_owned())?;
        let symbol = cursor.string_index(strings, "entity symbol")?;
        let flags = cursor.u64()?;
        let known_flags = match table_schema {
            TYPE_FACTS_TABLE_SCHEMA_V5 | TYPE_FACTS_TABLE_SCHEMA_V6 => 127,
            TYPE_FACTS_TABLE_SCHEMA_V4 => 63,
            _ => 31,
        };
        if flags & !known_flags != 0 {
            return Err(format!("packed entity has unknown flags {flags}"));
        }
        let type_descriptor = if flags & 1 != 0 {
            Some(Arc::new(cursor.type_descriptor(strings)?))
        } else {
            None
        };
        let resolved_call = if flags & 2 != 0 {
            Some(Arc::new(cursor.resolved_call(strings, table_schema)?))
        } else {
            None
        };
        let callability = if flags & 4 != 0 {
            Some(parse_callability(cursor.u64()?)?)
        } else {
            None
        };
        let reference_space = if flags & 8 != 0 {
            Some(parse_reference_space(cursor.u64()?)?)
        } else {
            None
        };
        let runtime_identity = if flags & 16 != 0 {
            cursor.string_index(strings, "runtime identity")?
        } else {
            Arc::from("")
        };
        let runtime_value_domain = if flags & 32 != 0 {
            Some(parse_runtime_value_domain(cursor.u64()?)?)
        } else {
            None
        };
        let symbol_unresolved = flags & 64 != 0;
        if symbol_unresolved && !symbol.is_empty() {
            return Err("packed entity cannot be both resolved and unresolved".into());
        }
        entities.push(EntityFact {
            location: Location {
                path: path.clone(),
                start_byte: start,
                end_byte: end,
            },
            symbol,
            symbol_unresolved,
            type_descriptor,
            resolved_call,
            callability,
            runtime_value_domain,
            reference_space,
            runtime_identity,
        });
        previous_start = start;
    }
    Ok(())
}

fn parse_runtime_value_domain(value: u64) -> Result<RuntimeValueDomain, String> {
    if value & !15 != 0 {
        return Err(format!("unknown runtime value-domain bits {value}"));
    }
    Ok(RuntimeValueDomain {
        may_be_callable: value & 1 != 0,
        may_be_undefined: value & 2 != 0,
        may_be_other: value & 4 != 0,
        unknown: value & 8 != 0,
    })
}

fn parse_callability(value: u64) -> Result<Callability, String> {
    match value {
        0 => Ok(Callability::Callable),
        1 => Ok(Callability::NonCallable),
        2 => Ok(Callability::Mixed),
        3 => Ok(Callability::Unknown),
        _ => Err(format!("unknown callability tag {value}")),
    }
}

fn parse_reference_space(value: u64) -> Result<ReferenceSpace, String> {
    match value {
        0 => Ok(ReferenceSpace::Value),
        1 => Ok(ReferenceSpace::Type),
        2 => Ok(ReferenceSpace::Both),
        3 => Ok(ReferenceSpace::Neither),
        _ => Err(format!("unknown reference-space tag {value}")),
    }
}

fn parse_resolved_call_validity(value: u64) -> Result<ResolvedCallValidity, String> {
    match value {
        0 => Ok(ResolvedCallValidity::Valid),
        1 => Ok(ResolvedCallValidity::Recovery),
        2 => Ok(ResolvedCallValidity::Unresolved),
        _ => Err(format!("unknown resolved-call validity tag {value}")),
    }
}

fn parse_call_kind(value: u64) -> Result<CallKind, String> {
    match value {
        0 => Ok(CallKind::Unknown),
        1 => Ok(CallKind::Call),
        2 => Ok(CallKind::Construct),
        _ => Err(format!("unknown call-kind tag {value}")),
    }
}

fn parse_argument_mapping_status(value: u64) -> Result<ArgumentMappingStatus, String> {
    match value {
        0 => Ok(ArgumentMappingStatus::Resolved),
        1 => Ok(ArgumentMappingStatus::Unresolved),
        _ => Err(format!("unknown argument-mapping status tag {value}")),
    }
}

fn parse_argument_mapping_reason(
    status: ArgumentMappingStatus,
    value: u64,
) -> Result<Option<ArgumentMappingReason>, String> {
    let reason = match value {
        0 => Ok(None),
        1 => Ok(Some(ArgumentMappingReason::CallUnresolved)),
        2 => Ok(Some(ArgumentMappingReason::RecoverySignature)),
        3 => Ok(Some(ArgumentMappingReason::CompositeSignature)),
        4 => Ok(Some(ArgumentMappingReason::SpreadArgument)),
        5 => Ok(Some(ArgumentMappingReason::ParameterUnavailable)),
        _ => Err(format!("unknown argument-mapping reason tag {value}")),
    }?;
    match (status, reason) {
        (ArgumentMappingStatus::Resolved, Some(_)) | (ArgumentMappingStatus::Unresolved, None) => {
            Err("packed argument status and reason disagree".into())
        }
        _ => Ok(reason),
    }
}

/// Decodes one file's calls, bindings, functions, and async functions — the
/// body every frame writes after whichever path placement it uses.
fn decode_file_fact(
    cursor: &mut PackedCursor<'_>,
    strings: &[Arc<str>],
    path: Arc<str>,
) -> Result<FileFact, String> {
    let call_count = cursor.count("calls")?;
    let mut calls = Vec::with_capacity(call_count);
    for _ in 0..call_count {
        calls.push(cursor.source_call(strings)?);
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
            names: cursor.locations(strings)?,
            initializer: cursor.source_call(strings)?,
        });
    }
    let function_count = cursor.count("functions")?;
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        let mut state = PackedLocationState::default();
        let name = cursor.location(strings, &mut state)?;
        let body = cursor.location(strings, &mut state)?;
        let parameters = cursor.locations(strings)?;
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
        let expression = cursor.location(strings, &mut state)?;
        let symbol = cursor.string_index(strings, "async symbol")?;
        let target = cursor.string_index(strings, "async target")?;
        let flags = cursor.u64()?;
        if flags & !ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC != 0 {
            return Err(format!("packed async function has unknown flags {flags}"));
        }
        async_functions.push(AsyncFunctionFact {
            expression,
            symbol,
            target,
            can_return_async: flags & ASYNC_FUNCTION_FLAG_CAN_RETURN_ASYNC != 0,
            calls_after_await: cursor.locations(strings)?,
        });
    }
    Ok(FileFact {
        path,
        calls: calls.into(),
        bindings: bindings.into(),
        functions: functions.into(),
        async_functions: async_functions.into(),
    })
}

fn slot_tag(value: u64, label: &str) -> Result<u64, String> {
    if value <= 2 {
        Ok(value)
    } else {
        Err(format!(
            "table transition {label} has invalid operation {value}"
        ))
    }
}

fn locations_are_canonical(locations: &[Location]) -> bool {
    locations.windows(2).all(|pair| {
        (pair[0].path.as_ref(), pair[0].start_byte, pair[0].end_byte)
            <= (pair[1].path.as_ref(), pair[1].start_byte, pair[1].end_byte)
    })
}

fn entity_run_is_canonical(path: &str, entities: &[EntityFact]) -> bool {
    entities
        .iter()
        .all(|entity| entity.location.path.as_ref() == path)
        && entities.windows(2).all(|pair| {
            (pair[0].location.start_byte, pair[0].location.end_byte)
                <= (pair[1].location.start_byte, pair[1].location.end_byte)
        })
}

/// Decodes and validates one complete v5 Wire table transition.
///
/// The returned plan owns every replacement row but is not yet applied to the
/// retained table. This keeps malformed frames from partially mutating
/// published Session state.
pub(crate) fn decode_table_transition(input: &[u8]) -> Result<WireTableTransition, String> {
    let mut cursor = PackedCursor::new(input);
    let version = cursor.u64()?;
    if version != TABLE_TRANSITION_VERSION {
        return Err(format!("unsupported table transition version {version}"));
    }
    let mode = match cursor.u64()? {
        0 => TransitionMode::Full,
        1 => TransitionMode::Delta,
        value => return Err(format!("unsupported table transition mode {value}")),
    };
    let table_schema = cursor.u64()?;
    if table_schema != TYPE_FACTS_TABLE_SCHEMA_V3
        && table_schema != TYPE_FACTS_TABLE_SCHEMA_V4
        && table_schema != TYPE_FACTS_TABLE_SCHEMA_V5
        && table_schema != TYPE_FACTS_TABLE_SCHEMA_V6
    {
        return Err(format!("unsupported Wire table schema {table_schema}"));
    }
    let base_generation = cursor.u64()?;
    let target_generation = cursor.u64()?;
    if target_generation == 0 {
        return Err("table transition target generation is zero".into());
    }
    let strings = decode_packed_strings(&mut cursor)?;
    if strings.first().is_none_or(|value| !value.is_empty()) {
        return Err("table transition dictionary does not begin with the empty string".into());
    }
    let mut unique_strings = HashSet::with_capacity(strings.len());
    if strings
        .iter()
        .any(|value| !unique_strings.insert(value.as_ref()))
    {
        return Err("table transition dictionary contains a duplicate string".into());
    }
    let project_id = cursor.string_index(&strings, "project id")?;
    if project_id.is_empty() {
        return Err("table transition project id is empty".into());
    }
    let base_state_token = cursor.string_index(&strings, "base state token")?;
    match mode {
        TransitionMode::Full if base_generation != 0 || !base_state_token.is_empty() => {
            return Err("full table transition has a base identity".into());
        }
        TransitionMode::Delta if base_generation == 0 || base_state_token.is_empty() => {
            return Err("delta table transition has no base identity".into());
        }
        TransitionMode::Delta if target_generation < base_generation => {
            return Err("delta table transition target precedes its base".into());
        }
        _ => {}
    }

    let path_count = cursor.count("path operations")?;
    let mut paths = Vec::with_capacity(path_count);
    let mut previous_path: Option<Arc<str>> = None;
    for _ in 0..path_count {
        let path = cursor.string_index(&strings, "path operation")?;
        if path.is_empty() {
            return Err("table transition path is empty".into());
        }
        if previous_path
            .as_ref()
            .is_some_and(|previous| previous.as_ref() >= path.as_ref())
        {
            return Err("table transition paths are not strictly ordered".into());
        }
        previous_path = Some(path.clone());
        let flags = cursor.u64()?;
        if flags & !0x3f != 0 {
            return Err(format!("table transition path has unknown flags {flags}"));
        }
        let source_tag = slot_tag(flags & 3, "source")?;
        let entity_tag = slot_tag((flags >> 2) & 3, "entity")?;
        let file_tag = slot_tag((flags >> 4) & 3, "file")?;
        if source_tag == 0 && entity_tag == 0 && file_tag == 0 {
            return Err("table transition contains an empty path operation".into());
        }
        if mode == TransitionMode::Full && [source_tag, entity_tag, file_tag].contains(&2) {
            return Err("full table transition contains a remove operation".into());
        }

        let source = match source_tag {
            0 => SlotOp::Unchanged,
            1 => SlotOp::Replace(raw_digest(cursor.raw(32)?)?),
            2 => SlotOp::Remove,
            _ => unreachable!(),
        };
        let entities = match entity_tag {
            0 => SlotOp::Unchanged,
            1 => {
                let mut entities = Vec::new();
                decode_entity_run(&mut cursor, &strings, &path, table_schema, &mut entities)?;
                if entities.is_empty() {
                    return Err(format!(
                        "entity replacement for {path:?} is empty; use remove"
                    ));
                }
                if !entity_run_is_canonical(&path, &entities) {
                    return Err(format!("entity replacement for {path:?} is not canonical"));
                }
                SlotOp::Replace(entities)
            }
            2 => SlotOp::Remove,
            _ => unreachable!(),
        };
        let file = match file_tag {
            0 => SlotOp::Unchanged,
            1 => SlotOp::Replace(decode_file_fact(&mut cursor, &strings, path.clone())?),
            2 => SlotOp::Remove,
            _ => unreachable!(),
        };
        paths.push(PathOp {
            path,
            source,
            entities,
            file,
        });
    }

    let symbol_count = cursor.count("symbol operations")?;
    let mut symbols = Vec::with_capacity(symbol_count);
    let mut previous_symbol_key: Option<(Arc<str>, u64, Arc<str>)> = None;
    for _ in 0..symbol_count {
        let header = cursor.u64()?;
        let (id_index, tag) = match mode {
            TransitionMode::Full => (header, 0),
            TransitionMode::Delta => {
                let tag = header & 3;
                if tag == 3 {
                    return Err("table transition symbol has invalid tag 3".into());
                }
                (header >> 2, tag)
            }
        };
        let id_index = usize::try_from(id_index)
            .map_err(|_| "packed symbol id string index overflows usize".to_owned())?;
        let id = strings
            .get(id_index)
            .cloned()
            .ok_or_else(|| format!("packed symbol id string index {id_index} is out of range"))?;
        if id.is_empty() {
            return Err("table transition symbol id is empty".into());
        }
        if mode == TransitionMode::Full && tag != 0 {
            return Err("full table transition contains a non-replace symbol operation".into());
        }
        let (operation, reference_path) = match tag {
            0 => {
                let alias_target = cursor.string_index(&strings, "alias target")?;
                let declarations = cursor.declarations(&strings)?;
                let references = cursor.locations(&strings)?;
                if !locations_are_canonical(&references) {
                    return Err(format!(
                        "symbol replacement for {id:?} has non-canonical references"
                    ));
                }
                if !alias_target.is_empty() && !references.is_empty() {
                    return Err(format!(
                        "alias symbol replacement for {id:?} carries references"
                    ));
                }
                (
                    SymbolOp::Replace(SymbolFact {
                        id: id.clone(),
                        alias_target,
                        declarations: declarations.into(),
                        references: references.into(),
                    }),
                    Arc::from(""),
                )
            }
            1 => (SymbolOp::Remove { id: id.clone() }, Arc::from("")),
            2 => {
                let path = cursor.string_index(&strings, "symbol reference path")?;
                if path.is_empty() {
                    return Err("symbol reference path is empty".into());
                }
                let references = cursor.locations(&strings)?;
                if references
                    .iter()
                    .any(|reference| reference.path.as_ref() != path.as_ref())
                    || !locations_are_canonical(&references)
                {
                    return Err(format!(
                        "symbol reference replacement for {id:?} and {path:?} is not canonical"
                    ));
                }
                (
                    SymbolOp::ReplaceReferencePath {
                        id: id.clone(),
                        path: path.clone(),
                        references,
                    },
                    path,
                )
            }
            _ => unreachable!(),
        };
        let key = (id, tag, reference_path);
        if previous_symbol_key.as_ref().is_some_and(|previous| {
            (previous.0.as_ref(), previous.1, previous.2.as_ref())
                >= (key.0.as_ref(), key.1, key.2.as_ref())
        }) {
            return Err("table transition symbol operations are not strictly ordered".into());
        }
        previous_symbol_key = Some(key);
        symbols.push(operation);
    }

    if cursor.offset != input.len() {
        return Err("table transition has trailing bytes".into());
    }
    Ok(WireTableTransition {
        mode,
        table_schema,
        base_generation,
        target_generation,
        project_id,
        base_state_token,
        paths,
        symbols,
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

/// Converts a full demand snapshot into its compact form. Demands are
/// grouped by location path in input order.
pub fn compact_demands(demands: &[EntityDemand]) -> CompactDemands {
    let mut strings = StringTable::new();
    let mut groups: Vec<CompactDemandGroup> = Vec::new();
    let mut previous_start = 0;
    for demand in demands {
        let path = strings.intern(demand.location.path.as_ref());
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
        if demand.callability {
            flags |= DEMAND_FLAG_CALLABILITY;
        }
        if demand.reference_space {
            flags |= DEMAND_FLAG_REFERENCE_SPACE;
        }
        if demand.runtime_identity {
            flags |= DEMAND_FLAG_RUNTIME_IDENTITY;
        }
        if demand.runtime_value_domain {
            flags |= DEMAND_FLAG_RUNTIME_VALUE_DOMAIN;
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
            push_uvarint(&mut group.1, strings.intern(query.path.as_ref()));
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

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use crate::{
        ArgumentMappingReason, ArgumentMappingStatus, CallKind, Callability, ReferenceSpace,
        ResolvedCallValidity,
    };

    use super::{
        SlotOp, TYPE_FACTS_SCHEMA_SHA256, TYPE_FACTS_SCHEMA_V6_SHA256, TYPE_FACTS_SCHEMA_V7_SHA256,
        TYPE_FACTS_SCHEMA_V8_SHA256, TransitionMode, decode_table_transition,
        parse_argument_mapping_reason, parse_argument_mapping_status, parse_call_kind,
        parse_callability, parse_reference_space, parse_resolved_call_validity, push_uvarint,
    };

    fn push_test_string(frame: &mut Vec<u8>, value: &str) {
        push_uvarint(frame, 0);
        push_uvarint(frame, 0);
        push_uvarint(frame, value.len() as u64);
        frame.extend_from_slice(value.as_bytes());
    }

    fn empty_full_transition() -> Vec<u8> {
        let mut frame = Vec::new();
        for value in [1, 0, 3, 0, 1, 2] {
            push_uvarint(&mut frame, value);
        }
        push_test_string(&mut frame, "");
        push_test_string(&mut frame, "/p/tsconfig.json");
        // project, empty base token, no path operations, no symbol operations.
        for value in [1, 0, 0, 0] {
            push_uvarint(&mut frame, value);
        }
        frame
    }

    fn runtime_value_domain_transition(table_schema: u64, domain_bits: u64) -> Vec<u8> {
        let mut frame = Vec::new();
        for value in [1, 0, table_schema, 0, 1, 3] {
            push_uvarint(&mut frame, value);
        }
        push_test_string(&mut frame, "");
        push_test_string(&mut frame, "/p/tsconfig.json");
        push_test_string(&mut frame, "/p/a.ts");
        // Project, empty base token, one path; path index, entity-replace tag.
        for value in [1, 0, 1, 2, 4] {
            push_uvarint(&mut frame, value);
        }
        // One entity: start 0, length 1, no symbol, value-domain field.
        for value in [1, 0, 1, 0, 32, domain_bits, 0] {
            push_uvarint(&mut frame, value);
        }
        frame
    }

    fn unresolved_symbol_transition(table_schema: u64) -> Vec<u8> {
        let mut frame = Vec::new();
        for value in [1, 0, table_schema, 0, 1, 3] {
            push_uvarint(&mut frame, value);
        }
        push_test_string(&mut frame, "");
        push_test_string(&mut frame, "/p/tsconfig.json");
        push_test_string(&mut frame, "/p/a.ts");
        // Project, empty base token, one path; path index, entity-replace tag.
        for value in [1, 0, 1, 2, 4] {
            push_uvarint(&mut frame, value);
        }
        // One entity: start 0, length 1, no symbol, explicit-unresolved flag.
        for value in [1, 0, 1, 0, 64, 0] {
            push_uvarint(&mut frame, value);
        }
        frame
    }

    #[test]
    fn handshake_hash_matches_frozen_schema() {
        let actual = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v5.schema.json"))
        );
        assert_eq!(actual, TYPE_FACTS_SCHEMA_SHA256);
        let actual_v6 = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v6.schema.json"))
        );
        assert_eq!(actual_v6, TYPE_FACTS_SCHEMA_V6_SHA256);
        let actual_v7 = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v7.schema.json"))
        );
        assert_eq!(actual_v7, TYPE_FACTS_SCHEMA_V7_SHA256);
        let actual_v8 = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v8.schema.json"))
        );
        assert_eq!(actual_v8, TYPE_FACTS_SCHEMA_V8_SHA256);
    }

    #[test]
    fn full_transition_header_is_strict_and_self_identifying() {
        let valid = empty_full_transition();
        let transition = decode_table_transition(&valid).unwrap();
        assert_eq!(transition.mode, TransitionMode::Full);
        assert_eq!(transition.table_schema, 3);
        assert_eq!(transition.base_generation, 0);
        assert_eq!(transition.target_generation, 1);
        assert_eq!(transition.project_id.as_ref(), "/p/tsconfig.json");
        assert!(transition.base_state_token.is_empty());
        assert!(transition.paths.is_empty());
        assert!(transition.symbols.is_empty());

        assert!(decode_table_transition(&valid[..valid.len() - 1]).is_err());
        assert!(decode_table_transition(&[1, 0, 3, 0, 1]).is_err());
        let mut trailing = valid.to_vec();
        trailing.push(0);
        assert!(decode_table_transition(&trailing).is_err());

        let mut overlong_version = vec![0x81, 0];
        overlong_version.extend_from_slice(&valid[1..]);
        assert!(decode_table_transition(&overlong_version).is_err());
    }

    #[test]
    fn wire_table_v4_decodes_runtime_value_domains_and_v3_stays_frozen() {
        let transition = decode_table_transition(&runtime_value_domain_transition(4, 3)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert_eq!(
            entities[0].runtime_value_domain,
            Some(crate::RuntimeValueDomain {
                may_be_callable: true,
                may_be_undefined: true,
                may_be_other: false,
                unknown: false,
            })
        );
        assert!(decode_table_transition(&runtime_value_domain_transition(4, 16)).is_err());
        assert!(decode_table_transition(&runtime_value_domain_transition(3, 3)).is_err());
    }

    #[test]
    fn wire_table_v5_decodes_explicit_unresolved_symbols_and_v4_stays_frozen() {
        let transition = decode_table_transition(&unresolved_symbol_transition(5)).unwrap();
        let SlotOp::Replace(entities) = &transition.paths[0].entities else {
            panic!("entity row was not replaced");
        };
        assert!(entities[0].symbol.is_empty());
        assert!(entities[0].symbol_unresolved);
        assert!(decode_table_transition(&unresolved_symbol_transition(4)).is_err());
    }

    #[test]
    fn numeric_enum_tags_decode_the_dense_go_golden() {
        let response: super::Response = crate::decode(include_bytes!(
            "../../../benchmarks/phase1/typefacts-v3-response-golden.cbor"
        ))
        .expect("decode response golden");
        let transition =
            decode_table_transition(&response.table_transition).expect("decode transition");
        let entity = transition
            .paths
            .iter()
            .find_map(|path| match &path.entities {
                SlotOp::Replace(entities) => entities.first(),
                SlotOp::Unchanged | SlotOp::Remove => None,
            })
            .expect("golden entity");
        assert_eq!(entity.callability, Some(Callability::Callable));
        assert_eq!(entity.reference_space, Some(ReferenceSpace::Both));
        let call = entity.resolved_call.as_ref().expect("golden resolved call");
        assert_eq!(call.validity, ResolvedCallValidity::Valid);
        assert_eq!(call.kind, CallKind::Call);
        assert_eq!(call.arguments[0].status, ArgumentMappingStatus::Resolved);
        assert_eq!(call.arguments[0].unresolved, None);
        assert_eq!(
            call.arguments[0]
                .parameter
                .as_ref()
                .expect("golden parameter")
                .callability,
            Callability::Callable
        );
    }

    #[test]
    fn closed_wire_enum_tags_reject_unknown_and_inconsistent_values() {
        assert_eq!(parse_callability(0).unwrap(), Callability::Callable);
        assert_eq!(parse_callability(1).unwrap(), Callability::NonCallable);
        assert_eq!(parse_callability(2).unwrap(), Callability::Mixed);
        assert_eq!(parse_callability(3).unwrap(), Callability::Unknown);
        assert!(parse_callability(4).is_err());

        assert_eq!(parse_reference_space(0).unwrap(), ReferenceSpace::Value);
        assert_eq!(parse_reference_space(1).unwrap(), ReferenceSpace::Type);
        assert_eq!(parse_reference_space(2).unwrap(), ReferenceSpace::Both);
        assert_eq!(parse_reference_space(3).unwrap(), ReferenceSpace::Neither);
        assert!(parse_reference_space(4).is_err());

        assert_eq!(
            parse_resolved_call_validity(0).unwrap(),
            ResolvedCallValidity::Valid
        );
        assert_eq!(
            parse_resolved_call_validity(1).unwrap(),
            ResolvedCallValidity::Recovery
        );
        assert_eq!(
            parse_resolved_call_validity(2).unwrap(),
            ResolvedCallValidity::Unresolved
        );
        assert!(parse_resolved_call_validity(3).is_err());

        assert_eq!(parse_call_kind(0).unwrap(), CallKind::Unknown);
        assert_eq!(parse_call_kind(1).unwrap(), CallKind::Call);
        assert_eq!(parse_call_kind(2).unwrap(), CallKind::Construct);
        assert!(parse_call_kind(3).is_err());

        assert_eq!(
            parse_argument_mapping_status(0).unwrap(),
            ArgumentMappingStatus::Resolved
        );
        assert_eq!(
            parse_argument_mapping_status(1).unwrap(),
            ArgumentMappingStatus::Unresolved
        );
        assert!(parse_argument_mapping_status(2).is_err());
        assert_eq!(
            parse_argument_mapping_reason(ArgumentMappingStatus::Resolved, 0).unwrap(),
            None
        );
        assert_eq!(
            parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 1).unwrap(),
            Some(ArgumentMappingReason::CallUnresolved)
        );
        assert_eq!(
            parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 5).unwrap(),
            Some(ArgumentMappingReason::ParameterUnavailable)
        );
        assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Resolved, 1).is_err());
        assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 0).is_err());
        assert!(parse_argument_mapping_reason(ArgumentMappingStatus::Unresolved, 6).is_err());
    }
}
