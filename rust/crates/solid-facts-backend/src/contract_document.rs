//! Normalized on-disk representation of package contracts.
//!
//! Reactive analysis consumes an expanded `PackageContract`. This adapter is
//! the only module that knows the compact document shape: shared summaries,
//! export groups, and entrypoint aliases.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use solid_reactive_ir::{
    ContractArtifacts, ContractEntrypoint, ContractEvidence, ContractExport, ContractPackage,
    PackageContract,
};

use crate::BackendError;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractDocument {
    schema_version: u32,
    package: ContractPackage,
    #[serde(default)]
    compiler_facts_protocol: u32,
    #[serde(default, skip_serializing_if = "empty_artifacts")]
    artifacts: ContractArtifacts,
    summaries: BTreeMap<String, ContractExport>,
    entrypoints: BTreeMap<String, DocumentEntrypoint>,
    evidence: ContractEvidence,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentEntrypoint {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    exports: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    same_as: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conditions: Vec<String>,
}

fn empty_artifacts(artifacts: &ContractArtifacts) -> bool {
    artifacts.declaration.is_none() && artifacts.implementation.is_none()
}

pub(crate) fn decode(data: &[u8]) -> Result<PackageContract, BackendError> {
    let document: ContractDocument = serde_json::from_slice(data)?;
    expand(document)
}

pub fn encode(contract: &PackageContract, pretty: bool) -> Result<Vec<u8>, BackendError> {
    contract
        .validate()
        .map_err(|error| BackendError::Contract(format!("invalid package contract: {error}")))?;
    let document = normalize(contract)?;
    if pretty {
        Ok(serde_json::to_vec_pretty(&document)?)
    } else {
        Ok(serde_json::to_vec(&document)?)
    }
}

fn normalize(contract: &PackageContract) -> Result<ContractDocument, BackendError> {
    let mut unique = BTreeMap::<String, ContractExport>::new();
    for entrypoint in contract.entrypoints.values() {
        for summary in entrypoint.exports.values() {
            unique
                .entry(serde_json::to_string(summary)?)
                .or_insert_with(|| summary.clone());
        }
    }

    let mut counters = HashMap::<String, usize>::new();
    let mut summary_ids = HashMap::<String, String>::new();
    let mut summaries = BTreeMap::new();
    for (canonical, summary) in unique {
        let plain = summary.reactive_reads.is_empty()
            && summary.returns.is_none()
            && summary.callbacks.is_empty()
            && summary.async_behavior.is_empty();
        let id = if plain {
            summary.kind.clone()
        } else {
            let counter = counters.entry(summary.kind.clone()).or_default();
            *counter += 1;
            format!("{}-{}", summary.kind, counter)
        };
        summary_ids.insert(canonical, id.clone());
        summaries.insert(id, summary);
    }

    let mut entrypoints = BTreeMap::new();
    let mut surfaces = HashMap::<String, String>::new();
    for (name, entrypoint) in &contract.entrypoints {
        let mut exports = BTreeMap::<String, Vec<String>>::new();
        for (export, summary) in &entrypoint.exports {
            let canonical = serde_json::to_string(summary)?;
            let id = summary_ids.get(&canonical).ok_or_else(|| {
                BackendError::Contract(format!(
                    "normalization lost summary for entrypoint {name} export {export}"
                ))
            })?;
            exports.entry(id.clone()).or_default().push(export.clone());
        }
        let surface = serde_json::to_string(&exports)?;
        let same_as = surfaces.get(&surface).cloned().unwrap_or_default();
        if same_as.is_empty() {
            surfaces.insert(surface, name.clone());
        }
        entrypoints.insert(
            name.clone(),
            DocumentEntrypoint {
                exports: if same_as.is_empty() {
                    exports
                } else {
                    Default::default()
                },
                same_as,
                conditions: entrypoint.conditions.clone(),
            },
        );
    }

    Ok(ContractDocument {
        schema_version: contract.schema_version,
        package: contract.package.clone(),
        compiler_facts_protocol: contract.compiler_facts_protocol,
        artifacts: contract.artifacts.clone(),
        summaries,
        entrypoints,
        evidence: contract.evidence.clone(),
    })
}

fn expand(document: ContractDocument) -> Result<PackageContract, BackendError> {
    if document.summaries.is_empty() || document.entrypoints.is_empty() {
        return Err(BackendError::Contract(
            "normalized package contract requires summaries and entrypoints".into(),
        ));
    }
    if document.summaries.keys().any(|id| id.is_empty()) {
        return Err(BackendError::Contract(
            "package contract summary identifiers must be nonempty".into(),
        ));
    }

    let mut expanded = BTreeMap::<String, BTreeMap<String, ContractExport>>::new();
    let mut visiting = HashSet::new();
    let mut used_summaries = HashSet::new();
    for name in document.entrypoints.keys() {
        expand_entrypoint(
            name,
            &document.entrypoints,
            &document.summaries,
            &mut expanded,
            &mut visiting,
            &mut used_summaries,
        )?;
    }
    if used_summaries.len() != document.summaries.len() {
        let unused = document
            .summaries
            .keys()
            .filter(|id| !used_summaries.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        return Err(BackendError::Contract(format!(
            "package contract has unused summaries: {}",
            unused.join(", ")
        )));
    }

    let entrypoints = document
        .entrypoints
        .into_iter()
        .map(|(name, entrypoint)| {
            let exports = expanded.remove(&name).unwrap_or_default();
            (
                name,
                ContractEntrypoint {
                    exports,
                    conditions: entrypoint.conditions,
                },
            )
        })
        .collect();
    let contract = PackageContract {
        schema_version: document.schema_version,
        package: document.package,
        compiler_facts_protocol: document.compiler_facts_protocol,
        artifacts: document.artifacts,
        entrypoints,
        evidence: document.evidence,
        contract_hash: String::new(),
        source_path: String::new(),
    };
    contract
        .validate()
        .map_err(|error| BackendError::Contract(format!("invalid package contract: {error}")))?;
    Ok(contract)
}

fn expand_entrypoint(
    name: &str,
    entrypoints: &BTreeMap<String, DocumentEntrypoint>,
    summaries: &BTreeMap<String, ContractExport>,
    expanded: &mut BTreeMap<String, BTreeMap<String, ContractExport>>,
    visiting: &mut HashSet<String>,
    used_summaries: &mut HashSet<String>,
) -> Result<BTreeMap<String, ContractExport>, BackendError> {
    if let Some(exports) = expanded.get(name) {
        return Ok(exports.clone());
    }
    let entrypoint = entrypoints.get(name).ok_or_else(|| {
        BackendError::Contract(format!(
            "entrypoint alias targets missing entrypoint {name:?}"
        ))
    })?;
    let grouped = !entrypoint.exports.is_empty();
    let aliased = !entrypoint.same_as.is_empty();
    if grouped == aliased {
        return Err(BackendError::Contract(format!(
            "entrypoint {name:?} must declare exactly one of exports or sameAs"
        )));
    }
    if !visiting.insert(name.to_owned()) {
        return Err(BackendError::Contract(format!(
            "entrypoint alias cycle includes {name:?}"
        )));
    }

    let exports = if aliased {
        expand_entrypoint(
            &entrypoint.same_as,
            entrypoints,
            summaries,
            expanded,
            visiting,
            used_summaries,
        )?
    } else {
        let mut exports = BTreeMap::new();
        for (summary_id, names) in &entrypoint.exports {
            let summary = summaries.get(summary_id).ok_or_else(|| {
                BackendError::Contract(format!(
                    "entrypoint {name:?} references missing summary {summary_id:?}"
                ))
            })?;
            if names.is_empty() {
                return Err(BackendError::Contract(format!(
                    "entrypoint {name:?} summary group {summary_id:?} is empty"
                )));
            }
            used_summaries.insert(summary_id.clone());
            for export in names {
                if export.is_empty() || exports.insert(export.clone(), summary.clone()).is_some() {
                    return Err(BackendError::Contract(format!(
                        "entrypoint {name:?} has an empty or duplicate export {export:?}"
                    )));
                }
            }
        }
        exports
    };
    visiting.remove(name);
    expanded.insert(name.to_owned(), exports.clone());
    Ok(exports)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> PackageContract {
        let function = ContractExport {
            kind: "function".into(),
            ..ContractExport::default()
        };
        let value = ContractExport {
            kind: "value".into(),
            ..ContractExport::default()
        };
        PackageContract {
            schema_version: 1,
            package: ContractPackage {
                name: "example".into(),
                version: "1.0.0".into(),
                integrity: String::new(),
            },
            compiler_facts_protocol: 1,
            artifacts: ContractArtifacts::default(),
            entrypoints: BTreeMap::from([
                (
                    ".".into(),
                    ContractEntrypoint {
                        exports: BTreeMap::from([
                            ("Component".into(), function.clone()),
                            ("version".into(), value),
                        ]),
                        conditions: vec!["import".into()],
                    },
                ),
                (
                    "./client".into(),
                    ContractEntrypoint {
                        exports: BTreeMap::from([("Component".into(), function)]),
                        conditions: vec![],
                    },
                ),
            ]),
            evidence: ContractEvidence {
                kind: "inferred".into(),
                generator: "test".into(),
            },
            contract_hash: String::new(),
            source_path: String::new(),
        }
    }

    #[test]
    fn normalized_document_round_trips_the_expanded_interface() {
        let original = contract();
        let encoded = encode(&original, true).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.package.name, original.package.name);
        assert_eq!(decoded.entrypoints, original.entrypoints);
        let text = String::from_utf8(encoded).unwrap();
        assert!(text.contains("\"summaries\""));
        assert!(text.contains("\"function\": ["));
        assert!(!text.contains("\"Component\": {"));
    }

    #[test]
    fn entrypoint_aliases_expand_without_leaking_into_analysis() {
        let mut original = contract();
        original.entrypoints.get_mut("./client").unwrap().exports =
            original.entrypoints["."].exports.clone();
        let encoded = encode(&original, false).unwrap();
        let text = String::from_utf8(encoded.clone()).unwrap();
        assert!(text.contains("\"sameAs\":\".\""));
        assert_eq!(decode(&encoded).unwrap().entrypoints, original.entrypoints);
    }

    #[test]
    fn malformed_groups_and_alias_cycles_fail_closed() {
        let duplicate = br#"{
          "schemaVersion":1,
          "package":{"name":"example","version":"1.0.0"},
          "compilerFactsProtocol":1,
          "summaries":{"function":{"kind":"function"}},
          "entrypoints":{".":{"exports":{"function":["x","x"]}}},
          "evidence":{"kind":"inferred"}
        }"#;
        assert!(decode(duplicate).is_err());
        let cycle = br#"{
          "schemaVersion":1,
          "package":{"name":"example","version":"1.0.0"},
          "compilerFactsProtocol":1,
          "summaries":{"function":{"kind":"function"}},
          "entrypoints":{".":{"sameAs":"./x"},"./x":{"sameAs":"."}},
          "evidence":{"kind":"inferred"}
        }"#;
        assert!(decode(cycle).is_err());
    }
}
