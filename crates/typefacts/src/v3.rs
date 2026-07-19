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

/// `[flags, startByte, endByte, query-location-or-empty]`. Optional row
/// elements encode as zero-or-one-element arrays because the deterministic
/// CBOR contract forbids null.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactDemand(pub u64, pub u64, pub u64, pub Vec<CompactLocation>);

/// `[path-index, demand rows]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactDemandGroup(pub u64, pub Vec<CompactDemand>);

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
    for demand in demands {
        let path = strings.intern(demand.location.path.as_str());
        if groups.last().is_none_or(|group| group.0 != path) {
            groups.push(CompactDemandGroup(path, Vec::new()));
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
        let query = demand
            .query_location
            .as_ref()
            .map(|query| {
                CompactLocation(
                    strings.intern(query.path.as_str()),
                    query.start_byte,
                    query.end_byte,
                )
            })
            .into_iter()
            .collect();
        let group = groups.last_mut().expect("group pushed above");
        group.1.push(CompactDemand(
            flags,
            demand.location.start_byte,
            demand.location.end_byte,
            query,
        ));
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
            sources,
            entities,
            symbols,
            files,
        })
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::TYPE_FACTS_SCHEMA_SHA256;

    #[test]
    fn handshake_hash_matches_frozen_schema() {
        let actual = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("../../../schema/typefacts-v2.schema.json"))
        );
        assert_eq!(actual, TYPE_FACTS_SCHEMA_SHA256);
    }
}
