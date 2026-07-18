use serde::{Deserialize, Serialize};

use crate::{EntityFact, FactTable, FileFact, Location, SourceDigest, SymbolFact};

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
