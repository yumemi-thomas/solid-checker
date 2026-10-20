//! Native-only, non-evaluating application discovery (ADR 0270).
//!
//! Children, by section (each is `use super::*`; the public entry points are
//! re-exported from `inputs` and defined here):
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | `inputs` | Input paths, identities, digests and their validation. |
//! | `application` | Vite configuration, tsconfig and route inputs of one project directory. |
//! | `execution` | Execution scopes, call edges, constants and regions. |
//! | `html` | The restricted HTML tokenizer that selects client roots. |
//! | `resolution`, `references` | Local module resolution and the references a scope exactly reaches. |
//! | `graph_inputs` | The explicit static surface, empty-graph refusals and selection premises. |
//! | `discovery` | The discovered host index and the attachment of its inputs. |

use crate::BackendError;
use crate::host_config::{self, Config, EntryMode};
use sha2::{Digest, Sha256};
use solid_facts::ast::{
    AST_FACTS_SCHEMA, BindingShape, ExportKind, FunctionFact, FunctionKind, HostConstantIdentity,
    HostDefaultInitializer, HostExecutionSiteKind, ImportFact, ImportKind, ModuleHazardKind,
    ModuleLoadKind,
};
use solid_facts::core::Span;
use solid_facts::resolution::{ImportResolution, SpecifierAttestation};
use solid_facts::{FileFacts, ProjectFacts};
use solid_reactive_ir::hosts::{
    BrowserRootManifest, HostExecutionRegion, HostModule, HostScope, ProjectHostIndex,
};
use solid_reactive_ir::{ProgramBoundary, RuntimeEnvironment};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod inputs;
pub use inputs::*;
mod application;
use application::*;
mod execution;
use execution::*;
mod html;
use html::*;
mod resolution;
use resolution::*;
mod references;
use references::*;
mod graph_inputs;
use graph_inputs::*;
mod discovery;
use discovery::*;

/// Opt-in timings include refused paths; dropping the clock closes the stage.
pub(crate) struct HostStage(&'static str, Option<std::time::Instant>);

impl HostStage {
    pub(crate) fn new(stage: &'static str) -> Self {
        Self(
            stage,
            std::env::var_os("SOLID_CHECKER_TIMINGS").map(|_| std::time::Instant::now()),
        )
    }
}

impl Drop for HostStage {
    fn drop(&mut self) {
        if let Some(started) = self.1 {
            eprintln!(
                "{}",
                serde_json::json!({"hostStage": self.0, "elapsedNs": started.elapsed().as_nanos()})
            );
        }
    }
}

const CONFIGS: &[&str] = &[
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mts",
    "vite.config.mjs",
    "vite.config.cts",
    "vite.config.cjs",
];

/// Discovery refusal is explanation only, never diagnostic authority.
#[derive(Debug)]
struct DiscoveryRefusal {
    path: PathBuf,
    line: Option<usize>,
    reason: String,
}

impl DiscoveryRefusal {
    fn new(path: &Path, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            line: None,
            reason: reason.into(),
        }
    }

    fn at(path: &Path, source: &str, offset: usize, reason: impl Into<String>) -> Self {
        let mut refusal = Self::new(path, reason);
        refusal.line = Some(
            1 + source.as_bytes()[..offset.min(source.len())]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count(),
        );
        refusal
    }

    fn message(&self) -> String {
        let line = self
            .line
            .map_or_else(String::new, |line| format!(":{line}"));
        format!("{}{}: {}", self.path.display(), line, self.reason)
    }
}

trait Required<T> {
    fn required(self, path: &Path, reason: &str) -> Result<T, DiscoveryRefusal>;
}

impl<T> Required<T> for Option<T> {
    fn required(self, path: &Path, reason: &str) -> Result<T, DiscoveryRefusal> {
        self.ok_or_else(|| DiscoveryRefusal::new(path, reason))
    }
}

/// What host inference returns: the host that selected the browser execution
/// (`None` on every refusal and every declared target), and the run note.
///
/// `hosts` is the analysis's own input; hand it to
/// [`solid_reactive_ir::AnalysisInputs::with_hosts`].
#[derive(Debug)]
pub struct InferredHost {
    pub hosts: Option<ProjectHostIndex>,
    pub note: Option<String>,
}

impl InferredHost {
    fn without_host(note: Option<String>) -> Self {
        Self { hosts: None, note }
    }
}

/// Native run note. The caller must replay it through the existing note channel
/// on fresh analyses and cached daemon answers alike. Explicit runtime
/// selections bypass discovery.
pub fn inferred_project_host_with_note(
    directory: &Path,
    runtime: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
    facts: &ProjectFacts,
) -> Result<InferredHost, BackendError> {
    let conditions = runtime.selected_conditions();
    if runtime.target.is_some() {
        return Ok(InferredHost::without_host(None));
    }
    let refused = |reason: String| {
        Some(format!(
            "solid-checker: note: browser host not inferred: {reason}"
        ))
    };
    if !conditions.is_empty() {
        return Ok(InferredHost::without_host(refused(format!(
            "{}: explicit runtime conditions select {conditions:?}",
            facts.project_id
        ))));
    }
    if runtime.program_boundary == Some(ProgramBoundary::Open) {
        return Ok(InferredHost::without_host(refused(format!(
            "{}: explicit open program boundary",
            facts.project_id
        ))));
    }
    let inputs = match discovery_inputs(directory, facts, dialect) {
        Ok(inputs) => inputs,
        Err(reason) => return Ok(InferredHost::without_host(refused(reason.message()))),
    };
    let mut index = match discovered_index_with_inputs(&inputs, facts, dialect, false) {
        Ok(index) if index.is_empty() => {
            return Ok(InferredHost::without_host(refused(index.manifest.reason)));
        }
        Ok(index) => index,
        Err(reason) => return Ok(InferredHost::without_host(refused(reason.message()))),
    };
    // The provisional graph schedules manifest capture; it supplies no final
    // authority. Build the admitted graph only after that capture, inside the
    // replay below, and compare every observation with the complete generation.
    if let Err(reason) = attach_discovery_inputs(&mut index, &inputs, facts, dialect, true) {
        return Ok(InferredHost::without_host(refused(reason.message())));
    }
    if index.is_empty() {
        return Ok(InferredHost::without_host(refused(index.manifest.reason)));
    }
    // No argument/child/prop edge is installed by native syntax discovery.
    let selection = HostStage::new("callback-host-selection");
    index = index.with_callback_invocations(
        solid_reactive_ir::callback_host::browser_callback_invocations(facts, dialect),
    );
    drop(selection);
    validate_inputs(&index)?;
    let note = format!(
        "solid-checker: note: browser host inferred for {} scopes from roots {}",
        index.browser_scope_count(),
        index
            .manifest
            .roots
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(InferredHost {
        hosts: Some(index),
        note: Some(note),
    })
}

#[cfg(test)]
mod tests;
