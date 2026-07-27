//! Process-free Node-API/WASI entry point for browser and WebContainer hosts.

use std::path::Path;

use napi_derive::napi;
use serde::Deserialize;
use solid_facts_backend::SemanticDemandGroup;
use solid_facts_backend::{
    BackendError, SourceFile, TypeFactsProvider, analyze_project, build_project_native,
};
use typefacts::FactTable;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckRequest {
    project_id: String,
    generation: u64,
    sources: Vec<SourceFile>,
    type_facts: FactTable,
}

/// The host has already run TypeScript and hands the finished table in, so
/// there is no producer and no demand round trip: the one table answers the
/// single analysis this entry point performs.
struct InMemoryTypeFacts(Option<FactTable>);

impl TypeFactsProvider for InMemoryTypeFacts {
    fn semantic_grouped(
        &mut self,
        _groups: &[SemanticDemandGroup<'_>],
    ) -> Result<FactTable, BackendError> {
        self.0
            .take()
            .ok_or_else(|| BackendError::Process("TypeFacts response was already consumed".into()))
    }
}

/// Analyze an in-memory project without spawning native processes.
///
/// The host supplies the TypeFacts closure produced by the browser-side
/// TypeScript engine. The result is the same JSON snapshot emitted by the CLI.
#[napi]
pub fn check_sync(request_json: String) -> napi::Result<String> {
    check(&request_json).map_err(|error| napi::Error::from_reason(error.to_string()))
}

fn check(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: CheckRequest = serde_json::from_str(request_json)?;
    let mut typescript = InMemoryTypeFacts(Some(request.type_facts));
    let facts = build_project_native(
        request.project_id.clone(),
        request.generation,
        request.sources.clone(),
        &mut typescript,
    )?;
    let analysis = analyze_project(
        Path::new(&request.project_id),
        &request.sources,
        &facts,
        &[],
    )?;
    Ok(serde_json::to_string(&analysis.snapshot)?)
}
