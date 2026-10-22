//! A backend with fixed answers, standing in for a compiler. It exists so the
//! session, service loop and process behaviour can be exercised live before
//! S3 provides a real program; the `solid-typefacts-fake` binary serves it.
//! It is not an authority on anything and is never used by `solid-typefacts`.
//!
//! Three modes, by constructor:
//!
//! * [`FakeBackend::default`]: no analysis (`analyze` reports the S1 failure),
//!   the S1 fixed answers for everything else.
//! * [`FakeBackend::scripted`]: `analyze` pops pre-built tables, so a test can
//!   hand the session any table sequence (and see what it asked for).
//! * [`FakeBackend::deriving`]: `analyze` builds a deterministic table from the
//!   demands and updates it has seen, with the exact transport manifest a
//!   compiler-backed closure would supply, so full, delta, and sparse
//!   transitions are reachable over a real process boundary.

use crate::backend::{
    AnalysisRequest, AnalysisStats, AnalyzedTable, Backend, BackendError, ClosureBackend,
    ModuleGraph, ModuleInventory, OpenBackend, SymbolEvidence, UpdateOutcome,
};
use crate::context::Context;
use crate::gocompat::clean;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use typefacts::producer::{
    EntityRows, ProducerTable, rust_owned_transport_manifest, source_digest,
};
use typefacts::v3::{EntityDemand, FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};
use typefacts::{
    Callability, Declaration, EntityFact, FileFact, Location, PrimitiveValueDomain, ReferenceSpace,
    SourceCall, SymbolClaim, SymbolFact,
};

#[derive(Default)]
pub struct FakeBackend {
    pub closed: Arc<Mutex<u32>>,
    pub fail_update: bool,
    /// Scripted mode: tables `analyze` answers with, in order.
    pub scripted_analyses: VecDeque<Result<AnalyzedTable, BackendError>>,
    /// Every `analyze` call's `force_full`, in order.
    pub analysis_calls: Arc<Mutex<Vec<bool>>>,
    /// Times `release_analysis_state` ran.
    pub released: Arc<Mutex<u32>>,
    /// What `symbol_evidence` answers in scripted mode.
    pub evidence: Vec<SymbolFact>,
    /// What `changed_references` answers in scripted mode.
    pub changed: Option<(Vec<Arc<str>>, bool)>,
    /// Deriving mode: withhold the transport manifest, as a closure does after
    /// broad invalidation, so a sparse transition is impossible.
    pub withhold_manifest: bool,
    /// Set by [`FakeBackend::deriving`].
    pub derived: Option<Derived>,
}

/// Deriving mode's memory.
#[derive(Default)]
pub struct Derived {
    demands: BTreeMap<(String, u64, u64), EntityDemand>,
    sources: BTreeMap<String, Vec<u8>>,
    changed_paths: HashSet<String>,
    previous: Option<ProducerTable>,
    state_id: u64,
}

impl FakeBackend {
    #[must_use]
    pub fn scripted(analyses: impl IntoIterator<Item = AnalyzedTable>) -> Self {
        Self {
            scripted_analyses: analyses.into_iter().map(Ok).collect(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn deriving() -> Self {
        Self {
            derived: Some(Derived::default()),
            ..Self::default()
        }
    }
}

impl ClosureBackend for FakeBackend {
    fn update(
        &mut self,
        _: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        if self.fail_update {
            return Err(BackendError("boom".to_owned()));
        }
        if let Some(derived) = &mut self.derived {
            for change in changes {
                let path = clean(&change.path);
                if change.deleted {
                    derived.sources.remove(&path);
                } else {
                    derived.sources.insert(path.clone(), change.source.clone());
                }
                derived.changed_paths.insert(path);
            }
        }
        Ok(UpdateOutcome {
            affected: changes.iter().map(|change| change.path.clone()).collect(),
        })
    }

    fn analyze(
        &mut self,
        _: &Context,
        request: &AnalysisRequest<'_>,
    ) -> Result<AnalyzedTable, BackendError> {
        self.analysis_calls.lock().unwrap().push(request.force_full);
        if let Some(derived) = &mut self.derived {
            return Ok(derived.analyze(request, self.withhold_manifest));
        }
        self.scripted_analyses.pop_front().unwrap_or_else(|| {
            Err(BackendError(
                "analyze is not implemented in this build (migration slice S2)".to_owned(),
            ))
        })
    }

    fn release_analysis_state(&mut self) {
        *self.released.lock().unwrap() += 1;
    }

    fn source_files(&mut self, _: &Context) -> Result<Vec<SourceFile>, BackendError> {
        Ok(vec![
            SourceFile {
                path: "/p/a.ts".into(),
                local: false,
                source: b"let a".to_vec(),
            },
            SourceFile {
                path: "/p/b.ts".into(),
                local: false,
                source: Vec::new(),
            },
        ])
    }

    fn close(&mut self) -> Result<(), BackendError> {
        *self.closed.lock().unwrap() += 1;
        Ok(())
    }
}

impl SymbolEvidence for FakeBackend {
    fn symbol_evidence(
        &mut self,
        _: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        let Some(derived) = &self.derived else {
            return Ok(self.evidence.clone());
        };
        let Some(table) = &derived.previous else {
            return Ok(Vec::new());
        };
        Ok(queries
            .iter()
            .filter_map(|query| table.symbol(&query.id))
            .map(|fact| SymbolFact {
                references: if queries.iter().any(|q| q.id == fact.id && q.references) {
                    fact.references.clone()
                } else {
                    Arc::from(Vec::new())
                },
                ..fact.clone()
            })
            .collect())
    }

    fn changed_references(&mut self, _: &Context) -> Result<(Vec<Arc<str>>, bool), BackendError> {
        if let Some(derived) = &self.derived {
            let mut ids: BTreeSet<Arc<str>> = BTreeSet::new();
            if let Some(table) = &derived.previous {
                for path in &derived.changed_paths {
                    for entity in table.entities_in(path) {
                        if !entity.symbol.is_empty() {
                            ids.insert(entity.symbol.clone());
                        }
                    }
                }
            }
            return Ok((ids.into_iter().collect(), true));
        }
        self.changed.clone().ok_or_else(|| {
            BackendError("symbols is not implemented in this build (migration slice S2)".to_owned())
        })
    }
}

impl ModuleGraph for FakeBackend {
    fn module_graph(
        &mut self,
        ctx: &Context,
        _: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        ctx.cancel();
        Err(BackendError("interrupted".to_owned()))
    }
}

impl Derived {
    fn analyze(&mut self, request: &AnalysisRequest<'_>, withhold_manifest: bool) -> AnalyzedTable {
        let wire = request.request;
        if wire.reset_state {
            self.demands.clear();
            self.changed_paths.clear();
            self.previous = None;
        }
        for path in &wire.removed_demand_paths {
            let path = clean(path);
            self.demands.retain(|(demanded, ..), _| *demanded != path);
        }
        for demand in &wire.demands {
            let key = (
                demand.location.path.to_string(),
                demand.location.start_byte,
                demand.location.end_byte,
            );
            self.demands.insert(key, demand.clone());
        }

        self.state_id += 1;
        let mut table = ProducerTable::new(request.generation, &clean(&wire.project_id));
        table.state_id = self.state_id;
        let paths: BTreeSet<&str> = self.demands.keys().map(|key| key.0.as_str()).collect();
        table.sources = paths
            .iter()
            .map(|path| {
                self.sources.get(*path).map_or_else(
                    || source_digest(path, path.as_bytes()),
                    |bytes| source_digest(path, bytes),
                )
            })
            .collect();
        let mut entities = Vec::new();
        let mut files: BTreeMap<&str, Vec<SourceCall>> = BTreeMap::new();
        let mut symbols: BTreeMap<Arc<str>, SymbolFact> = BTreeMap::new();
        for ((path, start, end), demand) in &self.demands {
            let location = Location {
                path: path.as_str().into(),
                start_byte: *start,
                end_byte: *end,
            };
            let id: Arc<str> = hashed_symbol(path, *start).into();
            entities.push(EntityFact {
                location: location.clone(),
                symbol: id.clone(),
                symbol_claim: SymbolClaim::Partial,
                type_descriptor: None,
                resolved_call: None,
                callability: demand.callability.then_some(Callability::Callable),
                constructability: None,
                runtime_binding_kind: None,
                runtime_value_domain: None,
                primitive_value_domain: PrimitiveValueDomain::default(),
                primitive_literal_candidates: None,
                call_result_domain: None,
                constant_value: None,
                array_shape: None,
                tuple_shape: None,
                library_types: None,
                reference_space: demand.reference_space.then_some(ReferenceSpace::Value),
                runtime_identity: "".into(),
            });
            let fact = symbols.entry(id.clone()).or_insert_with(|| SymbolFact {
                id: id.clone(),
                alias_target: "".into(),
                declarations: Arc::from(vec![Declaration {
                    name: "fake".into(),
                    kind: "Variable".into(),
                    location: location.clone(),
                }]),
                references: Arc::from(Vec::new()),
            });
            if demand.references {
                let mut references = fact.references.to_vec();
                references.push(location.clone());
                fact.references = references.into();
            }
            if demand.resolved_call {
                files.entry(path.as_str()).or_default().push(SourceCall {
                    location: location.clone(),
                    callee: location,
                    arguments: Vec::new(),
                    target: id,
                });
            }
        }
        table.entities = EntityRows::Flat(entities);
        table.symbols = symbols.into_values().collect();
        table.files = paths
            .iter()
            .map(|path| FileFact {
                path: (*path).into(),
                calls: files.remove(*path).unwrap_or_default().into(),
                bindings: Arc::from(Vec::new()),
                functions: Arc::from(Vec::new()),
                async_functions: Arc::from(Vec::new()),
            })
            .collect();
        if !withhold_manifest && !request.force_full {
            table.transport =
                rust_owned_transport_manifest(self.previous.as_ref(), Some(&self.changed_paths));
        }
        self.changed_paths.clear();
        self.previous = Some(table.clone());
        AnalyzedTable {
            table,
            stats: AnalysisStats {
                materialized: true,
                ..AnalysisStats::default()
            },
        }
    }
}

fn hashed_symbol(path: &str, start: u64) -> String {
    let mut digest = [0_u8; 12];
    for (half, seed) in [0xcbf2_9ce4_8422_2325_u64, 0x8422_2325_cbf2_9ce4]
        .into_iter()
        .enumerate()
    {
        let mut hash = seed;
        for byte in path.bytes().chain(start.to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        digest[half * 6..half * 6 + 6].copy_from_slice(&hash.to_le_bytes()[..6]);
    }
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("symbol:h:{hex}")
}

/// Opens a [`FakeBackend`] for any project. The process double derives its
/// tables from the demands it is sent; setting
/// `SOLID_TYPEFACTS_FAKE_NO_MANIFEST` withholds the transport manifest.
pub fn fake_opener() -> OpenBackend {
    Box::new(|_ctx, _project, _config| {
        let mut fake = FakeBackend::deriving();
        fake.withhold_manifest = std::env::var_os("SOLID_TYPEFACTS_FAKE_NO_MANIFEST").is_some();
        Ok(Box::new(fake) as Box<dyn Backend>)
    })
}
