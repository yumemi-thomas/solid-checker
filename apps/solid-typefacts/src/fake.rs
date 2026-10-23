//! A thin compiler stand-in. It answers the per-run, per-demand and
//! per-symbol questions of [`crate::backend::ClosureBackend`] with fixed,
//! deterministic facts, and nothing else: the demand closure, retained
//! contributions, source digests, the transport manifest and the table are the
//! real ones (`closure`, `retained`, `session`). It exists so the session,
//! service loop and process behaviour can be exercised before S3 provides a
//! real program; the `solid-typefacts-fake` binary serves it. It is not an
//! authority on anything and is never used by `solid-typefacts`.
//!
//! It is the port of Go's `transportOnlyBackend` test double
//! (`semantic_demand_test.go`) plus the per-test variations Go builds by
//! embedding it, as fields:
//!
//! * a symbol's id encodes its location (`symbol:h:<path>:<start>:<end>`), so
//!   the fake answers `declarations` and `references_batch` with no state
//!   while the ids still count as durable (ADR 0001);
//! * `changed_references` reports an exact, empty delta unless told otherwise.

use crate::backend::{
    Backend, BackendError, ClosureBackend, ModuleGraph, ModuleInventory, OpenBackend,
    SymbolEvidence, UpdateOutcome,
};
use crate::context::Context;
use crate::gocompat::clean;
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SymbolId, blank_entity};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use typefacts::producer::source_digest;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile};
use typefacts::{AsyncFunctionFact, Declaration, Location, SourceDigest, SymbolClaim};

pub struct FakeBackend {
    /// The program's files by path; `update` edits them.
    pub sources: BTreeMap<String, Vec<u8>>,
    /// Counts of `close` and `release_analysis_state` calls.
    pub closed: Arc<Mutex<u32>>,
    pub released: Arc<Mutex<u32>>,
    pub fail_update: bool,
    /// Applies `update`'s changes to the program's files. Go's double does not:
    /// its `Update` reports an empty affected set and leaves the sources.
    pub apply_updates: bool,
    /// Calls `semantic_demand_runs` and `references_batch` have seen.
    pub semantic_calls: Arc<Mutex<u32>>,
    pub reference_calls: Arc<Mutex<u32>>,
    /// Every id `references_batch` was asked for, per call.
    pub reference_requests: Arc<Mutex<Vec<Vec<SymbolId>>>>,
    /// Makes `semantic_demand_runs` fail with this message.
    pub fail_semantic: Option<String>,
    /// Makes only the next `semantic_demand_runs` call fail with this message.
    pub fail_semantic_once: Option<String>,
    /// Per-location symbol ids replacing the minted ones, so tests can make
    /// two files' demands reach one symbol.
    pub symbol_overrides: HashMap<Location, SymbolId>,
    /// Whether results claim every identity durable (Go: `Durable`).
    pub durable: bool,
    /// Alias edges; `next_aliases` replaces them at the next `update`.
    pub aliases: HashMap<SymbolId, SymbolId>,
    pub next_aliases: Option<HashMap<SymbolId, SymbolId>>,
    /// Async facts `async_functions_at` answers with when one of the requested
    /// locations is in the fact's file.
    pub async_facts: Vec<AsyncFunctionFact>,
    /// Answer `async_functions_at` with every fact, whatever was requested.
    pub async_ignore_locations: bool,
    /// The `changed_references` answer.
    pub changed: (Vec<SymbolId>, bool),
    /// Answers `module_graph`; absent, the fake has no resolved module graph
    /// and says so.
    pub module_inventory: Option<ModuleInventory>,
    /// Without an inventory, cancel the request before failing, as the S1 fake
    /// did, so the session's cancellation mapping can be observed.
    pub interrupt_modules: bool,
    pub module_demands: Arc<Mutex<Vec<ModuleGraphRequest>>>,
    /// Returned by `source_files` verbatim instead of the program's files.
    pub raw_sources: Option<Vec<SourceFile>>,
    /// Digest-only mode: `source_digests` returns these and `source_files`
    /// fails, so any code that retains raw sources is caught.
    pub digests: Option<Vec<SourceDigest>>,
    /// Cancels this context after the next `references_batch` has answered. A
    /// shared handle, so a test can arm it after the backend moved into a
    /// session.
    pub cancel_after_references: Arc<Mutex<Option<Context>>>,
    /// Makes `references_batch` fail once the analysis was released.
    pub strict_release: bool,
    /// Set by the first `references_batch`.
    pub reference_before_release: Arc<Mutex<bool>>,
    /// Set by `release_analysis_state`.
    pub is_released: bool,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::with_sources([("/p/a.ts", "let a"), ("/p/b.ts", "")])
    }
}

/// The id the fake mints for a location.
#[must_use]
pub fn symbol_id(location: &Location) -> SymbolId {
    format!(
        "symbol:h:{}:{}:{}",
        location.path, location.start_byte, location.end_byte
    )
    .into()
}

/// The location a fake id encodes. Split from the right: the encoded path may
/// itself contain separators.
#[must_use]
pub fn symbol_location(id: &str) -> Option<Location> {
    let rest = id.strip_prefix("symbol:h:")?;
    let (head, end) = rest.rsplit_once(':')?;
    let (path, start) = head.rsplit_once(':')?;
    Some(Location {
        path: path.into(),
        start_byte: start.parse().ok()?,
        end_byte: end.parse().ok()?,
    })
}

impl FakeBackend {
    #[must_use]
    pub fn with_sources<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            sources: files
                .into_iter()
                .map(|(path, source)| (path.to_owned(), source.as_bytes().to_vec()))
                .collect(),
            closed: Arc::default(),
            released: Arc::default(),
            fail_update: false,
            apply_updates: false,
            semantic_calls: Arc::default(),
            reference_calls: Arc::default(),
            reference_requests: Arc::default(),
            fail_semantic: None,
            fail_semantic_once: None,
            symbol_overrides: HashMap::new(),
            durable: true,
            aliases: HashMap::new(),
            next_aliases: None,
            async_facts: Vec::new(),
            async_ignore_locations: false,
            changed: (Vec::new(), true),
            module_inventory: None,
            interrupt_modules: false,
            module_demands: Arc::default(),
            raw_sources: None,
            digests: None,
            cancel_after_references: Arc::default(),
            strict_release: false,
            reference_before_release: Arc::default(),
            is_released: false,
        }
    }
}

impl ClosureBackend for FakeBackend {
    fn source_files(&mut self, _: &Context) -> Result<Vec<SourceFile>, BackendError> {
        if self.digests.is_some() {
            return Err(BackendError(
                "semantic materialization retained raw sources".to_owned(),
            ));
        }
        if let Some(raw) = &self.raw_sources {
            return Ok(raw.clone());
        }
        Ok(self
            .sources
            .iter()
            .map(|(path, source)| SourceFile {
                path: path.clone(),
                local: false,
                source: source.clone(),
            })
            .collect())
    }

    fn source_digests(&mut self, ctx: &Context) -> Result<Vec<SourceDigest>, BackendError> {
        if let Some(digests) = &self.digests {
            return Ok(digests.clone());
        }
        let sources = self.source_files(ctx)?;
        crate::backend::validate_canonical_source_files(&sources)?;
        Ok(sources
            .iter()
            .map(|source| source_digest(&source.path, &source.source))
            .collect())
    }

    fn update(
        &mut self,
        _: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        if self.fail_update {
            return Err(BackendError("boom".to_owned()));
        }
        if let Some(next) = self.next_aliases.take() {
            self.aliases = next;
        }
        if self.apply_updates {
            for change in changes {
                let path = clean(&change.path);
                if change.deleted {
                    self.sources.remove(&path);
                } else {
                    self.sources.insert(path, change.source.clone());
                }
            }
        }
        Ok(UpdateOutcome {
            affected: changes.iter().map(|change| change.path.clone()).collect(),
        })
    }

    fn close(&mut self) -> Result<(), BackendError> {
        *self.closed.lock().unwrap() += 1;
        Ok(())
    }

    fn resolve_alias(&mut self, _: &Context, id: &str) -> Result<Option<SymbolId>, BackendError> {
        Ok(self.aliases.get(id).cloned())
    }

    fn declarations(
        &mut self,
        _: &Context,
        id: &str,
    ) -> Result<Option<Vec<Declaration>>, BackendError> {
        // Only declaration-backed identities are eligible for cross-generation
        // reuse, so a double that answered "unknown" here would silently
        // disable the symbol-fact memo it is meant to exercise.
        Ok(symbol_location(id).map(|location| {
            vec![Declaration {
                name: "value".into(),
                kind: "variable".into(),
                location,
            }]
        }))
    }

    fn semantic_demand_runs(
        &mut self,
        _: &Context,
        runs: &[SemanticDemandRun<'_>],
    ) -> Result<Vec<SemanticDemandRunResult>, BackendError> {
        *self.semantic_calls.lock().unwrap() += 1;
        if let Some(message) = self.fail_semantic_once.take() {
            return Err(BackendError(message));
        }
        if let Some(message) = &self.fail_semantic {
            return Err(BackendError(message.clone()));
        }
        let results: Vec<SemanticDemandRunResult> = runs
            .iter()
            .map(|run| {
                let mut result = SemanticDemandRunResult {
                    durable: self.durable,
                    ..SemanticDemandRunResult::default()
                };
                for demand in run.demands {
                    let mut location = demand.location.clone();
                    location.path = clean(&location.path).into();
                    let mut entity = blank_entity(location.clone());
                    if demand.symbol {
                        entity.symbol = self
                            .symbol_overrides
                            .get(&location)
                            .cloned()
                            .unwrap_or_else(|| symbol_id(&location));
                        entity.symbol_claim = SymbolClaim::Partial;
                    }
                    result.entities.push(entity);
                }
                result
            })
            .collect();
        Ok(results)
    }

    fn async_functions_at(
        &mut self,
        _: &Context,
        locations: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        Ok(self
            .async_facts
            .iter()
            .filter(|fact| {
                self.async_ignore_locations
                    || locations
                        .iter()
                        .any(|location| location.path == fact.expression.path)
            })
            .cloned()
            .collect())
    }

    fn references_batch(
        &mut self,
        _: &Context,
        ids: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
        if self.strict_release && self.is_released {
            return Err(BackendError(
                "reference evidence requested after analysis release".to_owned(),
            ));
        }
        *self.reference_before_release.lock().unwrap() = true;
        *self.reference_calls.lock().unwrap() += 1;
        self.reference_requests.lock().unwrap().push(ids.to_vec());
        let references = ids
            .iter()
            .filter_map(|id| symbol_location(id).map(|location| (id.clone(), vec![location])))
            .collect();
        if let Some(context) = self.cancel_after_references.lock().unwrap().take() {
            context.cancel();
        }
        Ok(references)
    }

    fn changed_references(&mut self, _: &Context) -> Result<(Vec<SymbolId>, bool), BackendError> {
        Ok(self.changed.clone())
    }

    fn release_analysis_state(&mut self) {
        self.is_released = true;
        *self.released.lock().unwrap() += 1;
    }
}

impl FakeBackend {
    fn module_graph_unavailable(&self) -> Result<ModuleInventory, BackendError> {
        Err(BackendError(
            crate::backend::ERR_MODULE_GRAPH_UNAVAILABLE.to_owned(),
        ))
    }
}

impl SymbolEvidence for FakeBackend {}

impl ModuleGraph for FakeBackend {
    fn module_graph(
        &mut self,
        ctx: &Context,
        demand: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        self.module_demands.lock().unwrap().push(demand.clone());
        if let Some(inventory) = &self.module_inventory {
            return Ok(inventory.clone());
        }
        if self.interrupt_modules {
            ctx.cancel();
            return Err(BackendError("interrupted".to_owned()));
        }
        self.module_graph_unavailable()
    }
}

/// Opens a [`FakeBackend`] for any project.
pub fn fake_opener() -> OpenBackend {
    Box::new(|_ctx, _project, _config| Ok(Box::new(FakeBackend::default()) as Box<dyn Backend>))
}
