//! The source-discovery stage: finds reactive sources, accessors,
//! setters, and dependency-source-backed facts per file, with per-file reuse.

use crate::cache::{
    CachedSourceDiscovery, CachedTypeScriptIndexes, SourceDiscoveryContribution,
    SourceDiscoveryIdentity, SourceDiscoveryTypeScriptDelta,
};
use crate::owners::{
    binding_returns_reactive_source, computation_is_async, containing_ast_function,
};
use crate::pipeline::{parallel_file_chunk_results, parallel_file_results, parallel_slice_results};
use crate::{
    BuildTimings, PrimitiveName, ReactiveSourceKind, ReturnSummary, RuntimeEnvironment,
    RuntimeRendering, jsx_primitive_name, known_primitive, location,
};

use crate::{HashMap, HashSet};
use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{SymbolId, symbol_id};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use crate::timings::{ReactiveIrStage, StageClock};
use solid_dialect::{Dialect, Primitive};
use solid_facts::core::{SourceHash, SourcePath};
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::{Declaration, Location, ResolvedCallValidity};

mod returned_sources;
use returned_sources::*;
mod async_server;
pub(crate) use async_server::*;
mod discover_file;
pub(crate) use discover_file::*;
mod discover_sources;
pub(crate) use discover_sources::*;
mod typed_sources;
pub(crate) use typed_sources::*;
mod props_reactivity;
pub(crate) use props_reactivity::*;

pub(crate) fn source_discovery_identity(
    file: &FileFacts,
    indexes: &ProjectIndexes<'_>,
) -> SourceDiscoveryIdentity {
    let mut symbol_ids = HashSet::<SymbolId>::default();
    for entity in indexes.entities_for_path(file.path.as_str()) {
        if !entity.symbol.is_empty() {
            symbol_ids.insert(symbol_id(entity.symbol.as_ref()));
        }
        if let Some(call) = &entity.resolved_call
            && call.validity == ResolvedCallValidity::Valid
            && !call.target.is_empty()
        {
            symbol_ids.insert(symbol_id(call.target.as_ref()));
        }
    }
    let mut pending = symbol_ids.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let Some(symbol) = indexes.symbols_by_id.get(id.as_str()) else {
            continue;
        };
        if !symbol.alias_target().is_empty() && symbol_ids.insert(symbol_id(symbol.alias_target()))
        {
            pending.push(symbol_id(symbol.alias_target()));
        }
    }
    let mut symbols = symbol_ids.into_iter().collect::<Vec<_>>();
    symbols.sort_unstable();
    SourceDiscoveryIdentity {
        source_hash: file.source_hash.clone(),
        symbols,
    }
}

pub(crate) fn source_discovery_identity_matches(
    cached: &SourceDiscoveryIdentity,
    path: &str,
    source_hash: &SourceHash,
    typescript_unchanged: bool,
    typescript_delta: Option<&SourceDiscoveryTypeScriptDelta>,
) -> bool {
    if &cached.source_hash != source_hash {
        return false;
    }
    if typescript_unchanged {
        return true;
    }
    if let Some(delta) = typescript_delta {
        if delta.entity_paths.contains(path) || delta.file_paths.contains(path) {
            return false;
        }
        if delta.semantic_symbol_ids.is_empty() {
            return true;
        }
        return cached
            .symbols
            .iter()
            .all(|symbol| !delta.semantic_symbol_ids.contains(symbol.as_str()));
    }
    false
}
