//! The compiler seam. Everything the session and the demand closure need from
//! a program sits behind these traits, so the session, closure, framing and
//! lifecycle code never name a compiler type (Go: `ClosureBackend`,
//! `symbolEvidenceBackend` and `ModuleGraphProvider` of `closure.go`).
//!
//! A backend answers per-run, per-demand, per-symbol and per-file questions.
//! It never builds a fact table: the demand closure (`closure`, `retained`)
//! owns demand sets, retained contributions, source digests, the transport
//! manifest and the table, and asks the backend only for what needs a
//! compiler. S3 implements the lifetime and graph methods over a tsrs
//! program; S4 implements `semantic_demand_runs` (Go: the tsgo adapter's
//! `SemanticDemandRuns`, `semantic_runs.go`) and its per-family helpers.
//!
//! Every path crossing the seam is clean (`filepath.Clean`). `source_files`
//! returns strictly path-ordered rows. `semantic_demand_runs` accepts runs
//! whose paths and every nested location are already clean and returns clean,
//! strictly ordered, unique dependency paths that exclude their owning run.
//! The closure checks returned evidence and rejects malformed rows, but
//! neither side normalizes paths again.
//!
//! A backend is created and used on the single worker thread and is dropped
//! there; it need not be `Send` (tsrs state is thread-local).

use crate::context::Context;
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SemanticScope, SymbolId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use typefacts::producer::source_digest;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};
use typefacts::{
    AsyncFunctionFact, Declaration, Location, ModuleFact, ModuleImportFact, SourceBinding,
    SourceCall, SourceDigest, SourceFunction, SymbolFact,
};

/// A backend failure. The session picks the wire code from the operation
/// (`update-failed`, `sources-failed`, `modules-failed`, `analysis-failed`),
/// and reports `analysis-cancelled` instead when the request was cancelled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendError(pub String);

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BackendError {}

impl From<String> for BackendError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// The text of Go's `ErrModuleGraphUnavailable`. There is deliberately no
/// approximation: an inventory that omits files the analysis read, presented
/// as the complete list, is the defect an attested closure exists to remove.
pub const ERR_MODULE_GRAPH_UNAVAILABLE: &str =
    "type facts backend reports no resolved module graph";

/// What a backend without the optional `Source*` capabilities answers.
pub const ERR_SOURCE_FACTS_UNAVAILABLE: &str = "type facts backend has no source facts";

/// The result of applying an `update`'s file changes (Go: `AffectedSet`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateOutcome {
    pub affected: Vec<String>,
}

/// The `modules` answer, flattened as the response carries it (Go:
/// `ModuleInventory`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleInventory {
    pub modules: Vec<ModuleFact>,
    pub imports: Vec<ModuleImportFact>,
    pub unknown_import_paths: Vec<Arc<str>>,
    /// Configured roots the program holds no file for although the loader's
    /// lookup finds one (`module_graph::root_positions`).
    pub unmapped_roots: Vec<Arc<str>>,
}

/// The live project surface the demand closure materializes from. Every method
/// is called by the closure; there are no optional capabilities and no
/// fallbacks beyond the two defaulted ones, because there is exactly one
/// implementation per build.
pub trait ClosureBackend {
    /// Every program source file with its bytes, strictly path-ordered (Go:
    /// `SourceFiles`). Backs the `sources` operation, and the digests the
    /// closure hashes into its table unless `source_digests` overrides them.
    fn source_files(&mut self, ctx: &Context) -> Result<Vec<SourceFile>, BackendError>;

    /// The source identity the closure's table retains instead of source
    /// bodies (Go: the optional `sourceDigestBackend.SourceDigests`, which
    /// production's tsgo adapter implements). The default hashes
    /// `source_files` after checking they are clean and strictly path-ordered,
    /// as Go's fallback does; the closure checks an override's rows the same
    /// way.
    fn source_digests(&mut self, ctx: &Context) -> Result<Vec<SourceDigest>, BackendError> {
        let sources = self.source_files(ctx)?;
        validate_canonical_source_files(&sources)?;
        Ok(sources
            .iter()
            .map(|source| source_digest(&source.path, &source.source))
            .collect())
    }

    /// Applies versioned overlay changes transactionally: on error the
    /// previous generation stays intact (Go: `Update`). The affected set is
    /// the normalized source paths the update invalidated.
    fn update(
        &mut self,
        ctx: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError>;

    /// Releases the program (Go: `Close`).
    fn close(&mut self) -> Result<(), BackendError>;

    /// Forgets every answer and identity claim derived from earlier queries:
    /// the source-fact memo, the reference contributions and the claims of
    /// symbol ids. A table is built from nothing but the program and the
    /// demands, so ids that two symbols contend for (a declaration hash, a
    /// transient copy of a member) go to whichever symbol the table's own
    /// demands reach first, however the earlier queries were ordered. The
    /// program itself and its checker are kept.
    fn discard_derived_state(&mut self) {}

    /// The alias target of one symbol, or `None` when it has none or is
    /// unknown (Go: `ResolveAlias`, `ErrNotFound`).
    fn resolve_alias(&mut self, ctx: &Context, id: &str) -> Result<Option<SymbolId>, BackendError>;

    /// The declarations of one symbol, or `None` when it is unknown (Go:
    /// `Declarations`, `ErrNotFound`).
    fn declarations(
        &mut self,
        ctx: &Context,
        id: &str,
    ) -> Result<Option<Vec<Declaration>>, BackendError>;

    /// Fully checks the demanded files, in the given (path) order, before a
    /// table answers any of their demands (ADR 0367). Inside a file every
    /// later answer is then the one tsc's source-order check computes (an
    /// enclosing call resolved before the calls in its arguments, contextual
    /// parameter types assigned), whatever order the demands, the batches and
    /// earlier requests came in. The default does nothing: a backend without
    /// a checker has no checker history.
    fn check_demanded_files(&mut self, ctx: &Context, paths: &[&str]) -> Result<(), BackendError> {
        let _ = (ctx, paths);
        Ok(())
    }

    /// Resolves canonically ordered per-file runs of demands (Go:
    /// `SemanticDemandRuns`, `tsgo/semantic_runs.go`). Results are
    /// index-aligned with `runs` and with each run's demands. `scope` carries
    /// the structural-accessor symbols from outside the subset, so a
    /// recomputed subset matches a whole-batch run.
    fn semantic_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        scope: &SemanticScope<'_>,
    ) -> Result<Vec<SemanticDemandRunResult>, BackendError>;

    /// The async-function facts relevant at the requested locations (Go:
    /// `AsyncFunctionsAt`). The closure cleans the returned expression paths.
    fn async_functions_at(
        &mut self,
        ctx: &Context,
        locations: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError>;

    /// The call expressions of one file in source order, each with its
    /// alias-resolved target (Go: the optional `CallDiscoverer`). Not every
    /// backend has them: the default says so rather than answer empty.
    fn source_calls(
        &mut self,
        _ctx: &Context,
        _path: &str,
    ) -> Result<Vec<SourceCall>, BackendError> {
        Err(BackendError(ERR_SOURCE_FACTS_UNAVAILABLE.to_owned()))
    }

    /// Variables initialized directly by a resolved call (Go:
    /// `BindingDiscoverer`).
    fn source_bindings(
        &mut self,
        _ctx: &Context,
        _path: &str,
    ) -> Result<Vec<SourceBinding>, BackendError> {
        Err(BackendError(ERR_SOURCE_FACTS_UNAVAILABLE.to_owned()))
    }

    /// Named block-bodied functions (Go: `FunctionDiscoverer`).
    fn source_functions(
        &mut self,
        _ctx: &Context,
        _path: &str,
    ) -> Result<Vec<SourceFunction>, BackendError> {
        Err(BackendError(ERR_SOURCE_FACTS_UNAVAILABLE.to_owned()))
    }

    /// Every async function and local alias of one file (Go:
    /// `AsyncFunctionDiscoverer`).
    fn source_async_functions(
        &mut self,
        _ctx: &Context,
        _path: &str,
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        Err(BackendError(ERR_SOURCE_FACTS_UNAVAILABLE.to_owned()))
    }

    /// Reference lists keyed by the requested id; an absent key means the
    /// same as an empty list (Go: `ReferencesBatch`).
    fn references_batch(
        &mut self,
        ctx: &Context,
        ids: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError>;

    /// The exact canonical-symbol keys whose reference lists moved since the
    /// preceding generation, and `false` after a first build or a broad
    /// invalidation: a live answer, not a missing capability. It must stay
    /// stable for the generation so retries and cancelled analyses do not
    /// advance hidden state (Go: `ChangedReferences`).
    fn changed_references(&mut self, ctx: &Context) -> Result<(Vec<SymbolId>, bool), BackendError>;

    /// Drops checker-expanded query state after a cold materialization
    /// (Go: `ReleaseAnalysisState`).
    fn release_analysis_state(&mut self) {}

    /// How much binder-level work was reused since the last call (ADR 0369).
    fn binder_retention(&mut self) -> crate::references::BinderRetention {
        crate::references::BinderRetention::default()
    }
}

/// Symbol-level evidence for the `symbols` operation and for `analyze`'s
/// piggybacked queries (Go: `symbolEvidenceBackend.SymbolEvidence`, the
/// production oracle seam of the active protocol).
pub trait SymbolEvidence: ClosureBackend {
    /// One batch holds the checker once. The default composes the per-symbol
    /// questions of [`ClosureBackend`], as Go's `resolveSymbolEvidence`
    /// fallback does to keep compiler-independent test adapters small.
    fn symbol_evidence(
        &mut self,
        ctx: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        let mut facts: Vec<SymbolFact> = Vec::with_capacity(queries.len());
        let mut reference_ids: Vec<SymbolId> = Vec::new();
        for query in queries {
            if let Some(reason) = ctx.err() {
                return Err(BackendError(reason.to_owned()));
            }
            let mut fact = crate::model::blank_symbol(Arc::clone(&query.id));
            if !query.references_only {
                if let Some(target) = self.resolve_alias(ctx, &query.id)? {
                    fact.alias_target = target;
                }
                if let Some(declarations) = self.declarations(ctx, &query.id)? {
                    fact.declarations = declarations.into();
                }
            }
            if query.references {
                reference_ids.push(Arc::clone(&query.id));
            }
            facts.push(fact);
        }
        let references = self.references_batch(ctx, &reference_ids)?;
        for (query, fact) in queries.iter().zip(&mut facts) {
            if query.references {
                // A duplicated id reads the same list for each query, as Go's
                // map lookup does.
                if let Some(list) = references.get(&query.id) {
                    fact.references = list.clone().into();
                }
            }
        }
        Ok(facts)
    }
}

/// The resolved module graph for the `modules` operation (Go:
/// `ModuleGraphProvider.ModuleGraph`). It is a compiler-resolution capability
/// with no compiler-independent approximation: a backend that cannot answer
/// must say so rather than return a partial graph.
pub trait ModuleGraph {
    fn module_graph(
        &mut self,
        _ctx: &Context,
        _demand: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        Err(BackendError(ERR_MODULE_GRAPH_UNAVAILABLE.to_owned()))
    }
}

pub trait Backend: ClosureBackend + SymbolEvidence + ModuleGraph {}

impl<T: ClosureBackend + SymbolEvidence + ModuleGraph> Backend for T {}

/// Source rows are clean, non-empty and strictly path-ordered (Go:
/// `validateCanonicalSourceFiles`).
pub fn validate_canonical_source_files(files: &[SourceFile]) -> Result<(), BackendError> {
    for (index, file) in files.iter().enumerate() {
        if file.path.is_empty() || crate::gocompat::clean(&file.path) != file.path {
            return Err(BackendError(format!(
                "source file path {} is not clean and non-empty",
                crate::gocompat::quote(&file.path)
            )));
        }
        if index != 0 && files[index - 1].path >= file.path {
            return Err(BackendError(format!(
                "source files are not strictly path-ordered at {}",
                crate::gocompat::quote(&file.path)
            )));
        }
    }
    Ok(())
}

/// Opens the program for a project: its absolute, cleaned tsconfig path and,
/// when the client passed `-project-config`, the bytes of that file.
pub type OpenBackend =
    Box<dyn FnOnce(&Context, &str, Option<&str>) -> Result<Box<dyn Backend>, String> + Send>;
