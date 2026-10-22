//! The compiler seam. Everything the session needs from a program sits behind
//! these traits, so the session, framing and lifecycle code never name a
//! compiler type (mirroring Go's `ClosureBackend`, `symbolEvidenceBackend` and
//! `ModuleGraphProvider`). S1 ships only [`StubBackend`]; the demand closure
//! (`analyze`) and its backend methods arrive with S2, the tsrs project with S3.
//!
//! A backend is created and used on the single worker thread and is dropped
//! there; it need not be `Send` (tsrs state is thread-local).

use crate::context::Context;
use std::fmt;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};
use typefacts::{ModuleFact, ModuleImportFact, SymbolFact};

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

/// The result of applying an `update`'s file changes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateOutcome {
    pub affected: Vec<String>,
}

/// The `modules` answer, flattened as the response carries it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleInventory {
    pub modules: Vec<ModuleFact>,
    pub imports: Vec<ModuleImportFact>,
    pub unknown_import_paths: Vec<std::sync::Arc<str>>,
}

/// The retained program: edits, source bytes, lifetime.
pub trait ClosureBackend {
    /// Applies versioned overlay changes transactionally: on error the
    /// previous generation stays intact.
    fn update(
        &mut self,
        ctx: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError>;

    /// Every program source file, in the order the client should see them.
    fn source_files(&mut self, ctx: &Context) -> Result<Vec<SourceFile>, BackendError>;

    fn close(&mut self) -> Result<(), BackendError>;
}

/// Symbol-level evidence for the `symbols` operation.
pub trait SymbolEvidence {
    fn symbol_evidence(
        &mut self,
        ctx: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError>;
}

/// The resolved module graph for the `modules` operation.
pub trait ModuleGraph {
    fn module_graph(
        &mut self,
        ctx: &Context,
        demand: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError>;
}

pub trait Backend: ClosureBackend + SymbolEvidence + ModuleGraph {}

impl<T: ClosureBackend + SymbolEvidence + ModuleGraph> Backend for T {}

/// Opens the program for a project: its absolute, cleaned tsconfig path and,
/// when the client passed `-project-config`, the bytes of that file.
pub type OpenBackend =
    Box<dyn FnOnce(&Context, &str, Option<&str>) -> Result<Box<dyn Backend>, String> + Send>;

/// S1's opener. There is no compiler yet, so no project can be served. It
/// reproduces the one failure Go reports without a compiler (a tsconfig that
/// cannot be read, TS5083) and otherwise says the compiler is missing.
pub fn stub_opener() -> OpenBackend {
    Box::new(|_ctx, project_id, config_source| {
        if config_source.is_none() && std::fs::metadata(project_id).is_err() {
            return Err(format!(
                "parse tsconfig: TS5083: Cannot read file '{project_id}'."
            ));
        }
        Err(
            "no compiler backend in this build (solid-typefacts-rs stub, migration slice S1)"
                .to_owned(),
        )
    })
}
