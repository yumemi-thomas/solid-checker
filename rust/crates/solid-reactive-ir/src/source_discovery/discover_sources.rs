//! Project-wide source discovery (`discover_sources`).

use super::*;

/// The source tables discovery fills, owned together so the passes that read
/// and extend them can each take just the tables they touch.
#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct SourceTables {
    pub(super) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(super) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(super) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(super) source_phases: HashMap<SymbolId, u8>,
    pub(super) returned_source_symbols: HashSet<SymbolId>,
    pub(super) summary_source_symbols: HashSet<SymbolId>,
    pub(super) source_owned_write: HashMap<SymbolId, bool>,
    pub(super) async_sources: HashSet<SymbolId>,
    pub(super) source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    pub(super) value_form_stores: HashSet<SymbolId>,
    pub(super) typed_sources: HashMap<SymbolId, TypedSource>,
    pub(super) prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) uncertain_prop_sources: HashSet<SymbolId>,
}

impl SourceTables {
    fn merge_target(&mut self) -> SourceDiscoveryMergeTarget<'_> {
        SourceDiscoveryMergeTarget {
            accessors: &mut self.accessors,
            accessor_origins: &mut self.accessor_origins,
            setters: &mut self.setters,
            actions: &mut self.actions,
            source_kinds: &mut self.source_kinds,
            source_primitives: &mut self.source_primitives,
            source_phases: &mut self.source_phases,
            returned_source_symbols: &mut self.returned_source_symbols,
            summary_source_symbols: &mut self.summary_source_symbols,
            source_owned_write: &mut self.source_owned_write,
            async_sources: &mut self.async_sources,
            source_async_options: &mut self.source_async_options,
            value_form_stores: &mut self.value_form_stores,
        }
    }
}

pub(crate) fn discover_sources(
    ctx: &StageContext<'_>,
    source_discovery_cache: Option<&mut HashMap<SourcePath, SourceDiscoveryIdentity>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    emit_timings: bool,
) -> SourceDiscovery {
    let mut clock = StageClock::new(emit_timings);
    // The kept post-pass state leaves the store until this build has brought
    // it up to date: a build that stops on the way drops it, and the next one
    // starts empty instead of reusing rows the per-file caches moved past.
    let store = ctx.semantic_lookup.memo_store();
    let mut kept = store.derived().solver::<super::post_passes::KeptPasses, _>(
        super::post_passes::POST_PASSES_STATE,
        |kept, _| std::mem::take(kept),
    );
    let mut tables = SourceTables::default();
    let unchanged_paths = merge_file_source_contributions(
        ctx,
        source_discovery_cache,
        typescript_unchanged,
        build_timings,
        &mut tables,
    );
    clock.finish(build_timings, ReactiveIrStage::SourceDiscovery);
    let discovery = {
        let mut timer = PassTimer::start();
        let fresh_tables = store.check_solvers().then(|| tables.clone());
        let mut output = super::post_passes::run(
            &mut kept,
            ctx,
            &mut tables,
            &unchanged_paths,
            typescript_unchanged,
            &mut timer,
            &mut || clock.finish(build_timings, ReactiveIrStage::TypedAccessorsAndPropRoots),
        );
        if let Some(mut fresh) = fresh_tables {
            // Shadow and verify-reused modes: the same passes on empty state.
            let fresh_output = super::post_passes::run(
                &mut super::post_passes::KeptPasses::default(),
                ctx,
                &mut fresh,
                &unchanged_paths,
                typescript_unchanged,
                &mut PassTimer::off(),
                &mut || {},
            );
            if let Some(difference) =
                super::post_passes::first_difference(&tables, &output, &fresh, &fresh_output)
            {
                store.poison(&format!(
                    "IR source-discovery verify mismatch: {difference}: the kept post-passes differ from a fresh run"
                ));
                tables = fresh;
                output = fresh_output;
                kept = super::post_passes::KeptPasses::default();
            }
        }
        timer.emit();
        let super::post_passes::PassesOutput {
            runtime_sources: _,
            props_reactivity,
            row_sets,
        } = output;
        build_timings.source_post_pass_row_sets = row_sets;
        let SourceTables {
            accessors,
            accessor_origins,
            setters,
            actions,
            source_kinds,
            source_primitives,
            source_phases,
            returned_source_symbols,
            summary_source_symbols,
            source_owned_write,
            async_sources,
            source_async_options,
            value_form_stores,
            typed_sources,
            prop_sources,
            uncertain_prop_sources,
        } = tables;
        clock.finish(
            build_timings,
            ReactiveIrStage::PropPropagationAndControlFlow,
        );
        SourceDiscovery {
            accessors,
            accessor_origins,
            setters,
            actions,
            source_kinds,
            source_primitives,
            source_phases,
            returned_source_symbols,
            summary_source_symbols,
            source_owned_write,
            async_sources,
            source_async_options,
            value_form_stores,
            typed_sources,
            prop_sources,
            uncertain_prop_sources,
            props_reactivity,
            unchanged_paths,
        }
    };
    store.derived().solver::<super::post_passes::KeptPasses, _>(
        super::post_passes::POST_PASSES_STATE,
        |slot, _| *slot = kept,
    );
    discovery
}

/// The post-passes' own times under `SOLID_CHECKER_TIMINGS`, one
/// `reactiveIrSourcePasses` line per build: the two stage lines they sit in
/// do not split them.
pub(super) struct PassTimer {
    started: Option<std::time::Instant>,
    lap: Option<std::time::Instant>,
    passes: Vec<(&'static str, u64)>,
    counts: Vec<(&'static str, u64)>,
}

impl PassTimer {
    fn start() -> Self {
        let started = std::env::var_os("SOLID_CHECKER_TIMINGS").map(|_| std::time::Instant::now());
        Self {
            started,
            lap: started,
            passes: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// A timer that records nothing.
    fn off() -> Self {
        Self {
            started: None,
            lap: None,
            passes: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// Records a count beside the times.
    pub(super) fn count(&mut self, name: &'static str, count: usize) {
        if self.started.is_some() {
            self.counts
                .push((name, u64::try_from(count).unwrap_or(u64::MAX)));
        }
    }

    fn nanos(duration: std::time::Duration) -> u64 {
        u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
    }

    /// Ends the pass `pass` (a pass named twice adds up).
    pub(super) fn lap(&mut self, pass: &'static str) {
        let Some(lap) = self.lap else {
            return;
        };
        let now = std::time::Instant::now();
        let ns = Self::nanos(now - lap);
        match self.passes.iter_mut().find(|(name, _)| *name == pass) {
            Some((_, total)) => *total += ns,
            None => self.passes.push((pass, ns)),
        }
        self.lap = Some(now);
    }

    fn emit(self) {
        let Some(started) = self.started else {
            return;
        };
        let mut line = serde_json::Map::new();
        line.insert("totalNs".into(), Self::nanos(started.elapsed()).into());
        for (pass, ns) in self.passes {
            line.insert(format!("{pass}Ns"), ns.into());
        }
        for (name, count) in self.counts {
            line.insert(name.into(), count.into());
        }
        eprintln!("{}", serde_json::json!({ "reactiveIrSourcePasses": line }));
    }
}

/// Every file's contribution, merged in file order, with the build's
/// `unchanged_paths`: the files whose identity (source, TypeFacts rows and
/// the symbols they name) is the last build's.
fn merge_file_source_contributions(
    ctx: &StageContext<'_>,
    identities: Option<&mut HashMap<SourcePath, SourceDiscoveryIdentity>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    tables: &mut SourceTables,
) -> HashSet<String> {
    let StageContext {
        facts,
        project_indexes,
        typescript_indexes,
        entities,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    let mut aggregate = SourceDiscoveryAggregate::default();
    for contribution in &parallel_file_results(&facts.files, |file| {
        discover_file_sources(
            semantic_lookup,
            file,
            project_indexes
                .ast_files_by_path
                .get(file.path.as_str())
                .expect("project index contains every source file"),
            entities,
            symbol_names,
        )
    }) {
        aggregate.merge(contribution);
    }
    aggregate.append_to(tables.merge_target());
    let mut unchanged_paths = HashSet::<String>::default();
    if let Some(identities) = identities {
        let current_paths = facts
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        identities.retain(|path, _| current_paths.contains(path.as_str()));
        let mut identity_misses = 0;
        for file in &facts.files {
            let Some(cached) = identities.get(file.path.as_str()) else {
                continue;
            };
            if source_discovery_identity_matches(
                cached,
                file.path.as_str(),
                &file.source_hash,
                typescript_unchanged,
                typescript_indexes.source_discovery_delta.as_ref(),
            ) {
                unchanged_paths.insert(file.path.to_string());
            } else {
                identity_misses += 1;
            }
        }
        build_timings.source_discovery_identity_misses = identity_misses;
        // An unchanged file keeps the identity it was matched by.
        for file in &facts.files {
            if !unchanged_paths.contains(file.path.as_str()) {
                identities.insert(
                    file.path.clone(),
                    source_discovery_identity(file, project_indexes),
                );
            }
        }
    }
    build_timings.source_discovery_unchanged_files =
        u64::try_from(unchanged_paths.len()).unwrap_or(u64::MAX);
    unchanged_paths
}
