//! Source discovery's project passes on kept state (S7a; S7 design §2).
//!
//! After every file's own discovery is merged (pass 0), ten passes refine the
//! source tables: withdrawn tuple candidates (1), typed sources (2), children
//! accessors (3), effect store parameters (4), setter/action aliases (5),
//! store views (6), prop roots (7), prop aliases (8), the written set (9) and
//! component props (10). Each pass is split in two:
//!
//! - **Rows** ([`super::post_rows`], [`super::props_reactivity`]): what the
//!   pass reads of one file (or one TypeScript path), as data. A row set is a
//!   function of its file's own facts, bound by the file being one of the
//!   build's `unchanged_paths`, and of what computing it recorded: its memo
//!   and section reads ([`crate::base::reads`]) and its other answers
//!   ([`super::trace`]: another file's facts, a lookup question, an earlier
//!   pass's table at a key). A row set is kept while every one of them
//!   answers as recorded (trace replay, study lemma 2); each recorded answer
//!   is asked once per build, and only the row sets that read a moved one
//!   recompute ([`ChunkSet::refresh`]).
//! - **The fold**, in file order, with the pass's own rule: last wins
//!   (passes 1, 2's typed sources, 4, 7), first wins (2's accessors and
//!   setters, 3), a union (9), and the alias passes' rounds (5, 6, 8).
//!   Grouping folds are kept K1 folds ([`KeyedFold`]): per symbol (pass 2),
//!   per rendered function (pass 10's uses), per candidate source (the
//!   alias indexes), the exports and the written set. The alias passes are
//!   solved from their sources over the kept candidate indexes: a Jacobi
//!   round's only rows that can add a key are those holding a key the round
//!   before added (5, 6), and pass 8's Gauss-Seidel sweeps fire each row at
//!   its earliest (sweep, position), so a priority queue in that order
//!   reproduces them exactly. Pass 10's forwarded-props fixed point replays
//!   over the kept rows (its unresolved sets depend on the order of rounds).
//!
//! A build whose files list a path twice recomputes every row set (the
//! folds are keyed by slot, so the answer is the same); so does one whose
//! domain (closed program, caller proof, runtime redirects) moved. In shadow
//! and verify-reused modes the passes also run on empty state and the two
//! answers are compared: a difference names the first differing table and
//! key, poisons the store, and the fresh answer is served.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::post_rows::{FileRows, TupleRow, Typed, TypedRow};
use super::trace::{self, Answers, Dep, StateKey};
use super::*;
use crate::base::FileId;
use crate::base::file_table::FileInterner;
use crate::base::fixpoint::{FileOrder, KeyedFold};
use crate::base::memo::derived::SolverId;
use crate::base::reads::ReadProof;

/// The kept post-pass state's slot in the memo store.
pub(super) const POST_PASSES_STATE: SolverId = SolverId(320);

/// One kept row set and what computing it read.
struct Chunk<T> {
    value: Arc<T>,
    proof: ReadProof,
    deps: Arc<[Dep]>,
}

/// The row sets of one pass, by file (or path) key, and for each recorded
/// answer the keys whose row set read it.
struct ChunkSet<T> {
    chunks: HashMap<FileId, Chunk<T>>,
    readers: HashMap<Dep, HashSet<FileId>>,
}

impl<T> Default for ChunkSet<T> {
    fn default() -> Self {
        Self {
            chunks: HashMap::default(),
            readers: HashMap::default(),
        }
    }
}

/// What [`ChunkSet::refresh`] did.
struct Refreshed {
    /// Keys whose row set was added, removed or changed.
    moved: Vec<FileId>,
    recomputed: usize,
    kept: usize,
}

impl<T: Eq + Send + Sync> ChunkSet<T> {
    fn clear(&mut self) {
        self.chunks.clear();
        self.readers.clear();
    }

    fn unread(&mut self, key: FileId, deps: &[Dep]) {
        for dep in deps {
            if let Some(readers) = self.readers.get_mut(dep) {
                readers.remove(&key);
                if readers.is_empty() {
                    self.readers.remove(dep);
                }
            }
        }
    }

    /// Brings the row sets to `items`, in order: a kept row set is reused
    /// when `reuse`, its own file passes `gate`, and everything it recorded
    /// answers as recorded; every other one is computed (in parallel) by
    /// `compute`, recording its reads, on behalf of `own(item)`.
    #[allow(clippy::too_many_arguments)]
    fn refresh<I: Sync>(
        &mut self,
        items: &[(FileId, I)],
        reuse: bool,
        gate: &(dyn Fn(&I) -> bool + Sync),
        answers: &Answers<'_, '_>,
        own: &(dyn Fn(&I) -> &str + Sync),
        compute: &(dyn Fn(&I) -> T + Sync),
    ) -> Refreshed {
        let lookup = answers.lookup;
        let current = items.iter().map(|(key, _)| *key).collect::<HashSet<_>>();
        let mut moved = Vec::new();
        let gone = self
            .chunks
            .keys()
            .filter(|key| !current.contains(key))
            .copied()
            .collect::<Vec<_>>();
        for key in gone {
            if let Some(chunk) = self.chunks.remove(&key) {
                self.unread(key, &chunk.deps);
            }
            moved.push(key);
        }
        let mut dirty = HashSet::<FileId>::default();
        if reuse {
            for (key, item) in items {
                if !self.chunks.contains_key(key) || !gate(item) {
                    dirty.insert(*key);
                }
            }
            for (dep, readers) in &self.readers {
                if !dep.holds(answers) {
                    dirty.extend(readers.iter().copied());
                }
            }
            for (key, chunk) in &self.chunks {
                if !dirty.contains(key) && !chunk.proof.holds(lookup) {
                    dirty.insert(*key);
                }
            }
        } else {
            dirty = current;
        }
        // A kept row set's memo and section reads are still this build's.
        for (key, _) in items {
            if !dirty.contains(key)
                && let Some(chunk) = self.chunks.get(key)
            {
                crate::base::reads::replay(&chunk.proof.reads);
            }
        }
        let todo = items
            .iter()
            .filter(|(key, _)| dirty.contains(key))
            .collect::<Vec<_>>();
        let recomputed = crate::pipeline::parallel_balanced_results(&todo, |(key, item)| {
            let ((value, deps), reads) =
                crate::base::reads::record(|| trace::record(own(item), || compute(item)));
            (*key, value, deps, reads)
        });
        let count = recomputed.len();
        for (key, value, deps, reads) in recomputed {
            let proof = ReadProof::new(reads, lookup);
            let same = match self.chunks.remove(&key) {
                Some(old) => {
                    self.unread(key, &old.deps);
                    *old.value == value
                }
                None => false,
            };
            if !same {
                moved.push(key);
            }
            for dep in &deps {
                self.readers.entry(dep.clone()).or_default().insert(key);
            }
            self.chunks.insert(
                key,
                Chunk {
                    value: Arc::new(value),
                    proof,
                    deps: Arc::from(deps),
                },
            );
        }
        moved.sort_unstable();
        moved.dedup();
        Refreshed {
            moved,
            recomputed: count,
            kept: items.len() - count,
        }
    }

    fn get(&self, key: FileId) -> Option<&Arc<T>> {
        self.chunks.get(&key).map(|chunk| &chunk.value)
    }
}

/// A position of a row: its file's slot key and the row's ordinal there.
type RowAt = (FileId, u32);

/// What the passes keep between builds.
#[derive(Default)]
pub(super) struct KeptPasses {
    /// The domain the state was built under ([`domain`]).
    domain: Option<u64>,
    /// Slot keys (a path; `path\0rank` for a path's later slots) and
    /// TypeScript paths.
    slot_keys: FileInterner,
    ts_keys: FileInterner,
    /// The TypeScript paths holding entities, as of the last build.
    ts_paths: Option<Vec<Arc<str>>>,
    rows: ChunkSet<FileRows>,
    tuples: ChunkSet<Vec<TupleRow>>,
    typed: ChunkSet<Vec<TypedRow>>,
    components: ChunkSet<Vec<ComponentRow>>,
    /// Pass 2's rows by symbol, in entity order.
    typed_fold: KeyedFold<SymbolId, TypedRow, Arc<Vec<TypedRow>>>,
    /// Pass 5's rows by source, pass 6's and 8's by candidate.
    alias_index: KeyedFold<SymbolId, RowAt, Arc<Vec<RowAt>>>,
    view_index: KeyedFold<SymbolId, RowAt, Arc<Vec<RowAt>>>,
    prop_index: KeyedFold<SymbolId, RowAt, Arc<Vec<RowAt>>>,
    /// Pass 9's written symbols and pass 10's exports.
    written: KeyedFold<SymbolId, (), ()>,
    exports: KeyedFold<SymbolId, (), ()>,
    /// Pass 10's uses: the JSX elements rendering each project function.
    uses: KeyedFold<(Arc<str>, Span), RowAt, Arc<Vec<RowAt>>>,
}

/// What the passes read that no row set records: a change empties the kept
/// state.
fn domain(ctx: &StageContext<'_>) -> u64 {
    let redirects = {
        let mut redirects = ctx
            .facts
            .runtime_symbol_redirects
            .iter()
            .collect::<Vec<_>>();
        redirects.sort_unstable();
        trace::fingerprint(&redirects)
    };
    trace::fingerprint(&(
        ctx.runtime.program_is_closed(),
        ctx.semantic_lookup.dialect.props_require_caller_proof(),
        redirects,
    ))
}

/// The analyzed files' slot keys, in file order, and whether a path is
/// listed twice.
fn slot_keys(interner: &mut FileInterner, files: &[FileFacts]) -> (Vec<FileId>, bool) {
    let mut ranks = HashMap::<&str, u32>::default();
    let mut duplicated = false;
    let keys = files
        .iter()
        .map(|file| {
            let rank = ranks.entry(file.path.as_str()).or_insert(0);
            let key = if *rank == 0 {
                interner.intern(file.path.as_str())
            } else {
                duplicated = true;
                interner.intern(&format!("{}\0{rank}", file.path.as_str()))
            };
            *rank += 1;
            key
        })
        .collect();
    (keys, duplicated)
}

/// The TypeScript paths holding entities, in table order: the last build's
/// patched by the paths whose entities the TypeScript delta names, or every
/// entity's path when there is no exact delta.
fn typescript_paths(
    kept: Option<Vec<Arc<str>>>,
    table: &solid_facts::TypeScriptTable,
    typescript_unchanged: bool,
    delta: Option<&SourceDiscoveryTypeScriptDelta>,
) -> Vec<Arc<str>> {
    match (kept, typescript_unchanged, delta) {
        (Some(kept), true, _) => kept,
        (Some(mut paths), false, Some(delta)) => {
            for path in &delta.entity_paths {
                let at = paths.binary_search_by(|candidate| (**candidate).cmp(path.as_str()));
                match (at, table.entities_for_path(path).is_empty()) {
                    (Ok(at), true) => {
                        paths.remove(at);
                    }
                    (Err(at), false) => paths.insert(at, Arc::from(path.as_str())),
                    _ => {}
                }
            }
            paths
        }
        _ => {
            let mut paths = Vec::<Arc<str>>::new();
            for entity in table.entities() {
                if paths.last().is_none_or(|last| {
                    !Arc::ptr_eq(last, &entity.location.path) && **last != *entity.location.path
                }) {
                    paths.push(Arc::clone(&entity.location.path));
                }
            }
            paths.sort_unstable();
            paths.dedup();
            paths
        }
    }
}

/// A row set's own file.
fn own_path<'a>(item: &'a (usize, &FileFacts)) -> &'a str {
    item.1.path.as_str()
}

/// A TypeScript path row set's own path.
fn typescript_path(path: &Arc<str>) -> &str {
    path
}

/// What a pass hands the next: the runtime sources, and pass 10's index.
pub(super) struct PassesOutput {
    pub(super) runtime_sources: HashSet<SymbolId>,
    pub(super) props_reactivity: PropsReactivityIndex,
    /// Row sets kept and recomputed, over every pass.
    pub(super) row_sets: [u64; 2],
}

/// The rows of every file in slot order.
fn rows_in_order(state: &KeptPasses, keys: &[FileId]) -> Vec<Arc<FileRows>> {
    keys.iter()
        .map(|key| {
            state
                .rows
                .get(*key)
                .cloned()
                .expect("every slot has rows after the refresh")
        })
        .collect()
}

/// Brings a kept fold to `order` from the moved keys' rows.
fn update_fold<K, C, V>(
    fold: &mut KeyedFold<K, C, V>,
    order: &FileOrder,
    moved: &[FileId],
    rows: &dyn Fn(FileId) -> Option<Vec<(K, C)>>,
    combine: &dyn Fn(&K, &[&C]) -> Option<V>,
) where
    K: Ord + Clone,
    C: Eq,
    V: Eq,
{
    fold.update(
        order.clone(),
        moved.iter().map(|key| (*key, rows(*key))),
        &|key, values| combine(key, values),
    );
}

fn ordered_list<K, C: Clone>(_: &K, values: &[&C]) -> Option<Arc<Vec<C>>> {
    Some(Arc::new(
        values.iter().map(|value| (*value).clone()).collect(),
    ))
}

fn present<K>(_: &K, _: &[&()]) -> Option<()> {
    Some(())
}

/// Runs the post-passes on `tables` (pass 0's merge) over `state`.
#[allow(clippy::too_many_lines)]
pub(super) fn run(
    state: &mut KeptPasses,
    ctx: &StageContext<'_>,
    tables: &mut SourceTables,
    unchanged_paths: &HashSet<String>,
    typescript_unchanged: bool,
    timer: &mut PassTimer,
    prop_roots_done: &mut dyn FnMut(),
) -> PassesOutput {
    let StageContext {
        facts,
        entities,
        semantic_lookup: lookup,
        source_declarations,
        ..
    } = *ctx;
    let domain = domain(ctx);
    if state.domain != Some(domain) {
        *state = KeptPasses {
            domain: Some(domain),
            ..KeptPasses::default()
        };
    }
    let (keys, duplicated) = slot_keys(&mut state.slot_keys, &facts.files);
    let order = FileOrder::new(&keys);
    let slots = keys
        .iter()
        .enumerate()
        .map(|(slot, key)| (*key, slot))
        .collect::<HashMap<_, _>>();
    // A dependency-source build overrides project entities with the
    // project's own program's answers, which no TypeScript delta names.
    let reuse = !duplicated && facts.dependency_source_roots.is_empty();
    let delta = ctx.typescript_indexes.source_discovery_delta.as_ref();
    let entity_rows_unchanged = |path: &str| {
        typescript_unchanged || delta.is_some_and(|delta| !delta.entity_paths.contains(path))
    };
    let changed_roots_index = std::sync::OnceLock::<HashSet<SymbolId>>::new();
    let changed_roots = |symbol: &SymbolId| {
        if typescript_unchanged {
            return false;
        }
        let Some(delta) = delta else {
            return true;
        };
        changed_roots_index
            .get_or_init(|| {
                let mut roots = HashSet::default();
                for path in &delta.entity_paths {
                    for entity in facts.typescript.entities_for_path(path) {
                        if let Some(root) = lookup.entities().get(&entity.location) {
                            roots.insert(root.clone());
                        }
                    }
                }
                roots
            })
            .contains(symbol)
    };
    let declared =
        |path: &str, name: &str| super::post_rows::declared_symbol(source_declarations, path, name);
    // A symbol's TypeScript row is the last build's unless the change set
    // names it (no change set: none is known to be).
    let changed_symbols = std::sync::OnceLock::<HashSet<&str>>::new();
    let symbol_unchanged = |symbol: &str| {
        let Some(changes) = facts
            .typescript_changes
            .as_ref()
            .filter(|_| facts.runtime_symbol_redirects.is_empty())
        else {
            return false;
        };
        changes.unchanged
            || !changed_symbols
                .get_or_init(|| changes.symbol_ids.iter().map(String::as_str).collect())
                .contains(symbol)
    };
    macro_rules! answers {
        ($state:expr) => {
            Answers {
                lookup,
                unchanged: unchanged_paths,
                declared: &declared,
                entity_rows_unchanged: &entity_rows_unchanged,
                changed_roots: &changed_roots,
                symbol_unchanged: &symbol_unchanged,
                state: $state,
            }
        };
    }
    let no_state = |_: &StateKey| None;
    let own_gate = |(_, file): &(usize, &FileFacts)| unchanged_paths.contains(file.path.as_str());
    let items = keys
        .iter()
        .zip(facts.files.iter().enumerate())
        .map(|(key, item)| (*key, item))
        .collect::<Vec<_>>();
    // Every file's rows that read no source table.
    let refreshed = state.rows.refresh(
        &items,
        reuse,
        &own_gate,
        &answers!(&no_state),
        &own_path,
        &|(slot, file)| super::post_rows::file_rows(ctx, *slot, file),
    );
    timer.count("rowsRecomputed", refreshed.recomputed);
    let mut row_sets = [0_u64; 2];
    let mut tally = |refreshed: &Refreshed| {
        row_sets[0] += refreshed.kept as u64;
        row_sets[1] += refreshed.recomputed as u64;
    };
    tally(&refreshed);
    let moved = refreshed.moved;
    {
        let chunks = &state.rows;
        let rows_of = |key: FileId| chunks.get(key).cloned();
        update_fold(
            &mut state.written,
            &order,
            &moved,
            &|key| {
                rows_of(key).map(|rows| {
                    rows.written
                        .iter()
                        .map(|symbol| (symbol.clone(), ()))
                        .collect()
                })
            },
            &present,
        );
        update_fold(
            &mut state.exports,
            &order,
            &moved,
            &|key| {
                rows_of(key).map(|rows| {
                    rows.exports
                        .iter()
                        .map(|symbol| (symbol.clone(), ()))
                        .collect()
                })
            },
            &present,
        );
        update_fold(
            &mut state.uses,
            &order,
            &moved,
            &|key| {
                rows_of(key).map(|rows| {
                    rows.renders
                        .iter()
                        .map(|(ordinal, path, span)| ((Arc::clone(path), *span), (key, *ordinal)))
                        .collect()
                })
            },
            &ordered_list,
        );
        let index = |select: fn(&FileRows) -> Vec<Vec<SymbolId>>| {
            move |key: FileId| {
                rows_of(key).map(|rows| {
                    let mut entries = Vec::new();
                    for (ordinal, candidates) in select(&rows).into_iter().enumerate() {
                        let ordinal = u32::try_from(ordinal).expect("fewer than 2^32 rows");
                        let mut seen = HashSet::default();
                        for candidate in candidates {
                            if seen.insert(candidate.clone()) {
                                entries.push((candidate, (key, ordinal)));
                            }
                        }
                    }
                    entries
                })
            }
        };
        update_fold(
            &mut state.alias_index,
            &order,
            &moved,
            &index(|rows| {
                rows.aliases
                    .iter()
                    .map(|row| vec![row.source.clone()])
                    .collect()
            }),
            &ordered_list,
        );
        update_fold(
            &mut state.view_index,
            &order,
            &moved,
            &index(|rows| {
                rows.views
                    .iter()
                    .map(|row| row.candidates.clone())
                    .collect()
            }),
            &ordered_list,
        );
        update_fold(
            &mut state.prop_index,
            &order,
            &moved,
            &index(|rows| {
                rows.prop_aliases
                    .iter()
                    .map(|row| row.candidates.clone())
                    .collect()
            }),
            &ordered_list,
        );
    }
    let rows = rows_in_order(state, &keys);
    timer.lap("rows");
    // Pass 0's runtime sources and kinds, which passes 1 and 2 read.
    let runtime0 = tables.accessors.keys().cloned().collect::<HashSet<_>>();
    // Pass 1: withdrawn tuple candidates, under the tuple-iteration veto.
    if lookup.tuple_iteration_may_be_mutated() {
        let refreshed = state.tuples.refresh(
            &items,
            reuse,
            &own_gate,
            &answers!(&no_state),
            &own_path,
            &|(_, file)| super::post_rows::tuple_rows(ctx, file),
        );
        timer.count("tuplesRecomputed", refreshed.recomputed);
        tally(&refreshed);
        for key in &keys {
            for row in state
                .tuples
                .get(*key)
                .map(|rows| rows.as_slice())
                .unwrap_or_default()
            {
                if row.unless_runtime && runtime0.contains(&row.symbol) {
                    continue;
                }
                tables
                    .typed_sources
                    .insert(row.symbol.clone(), row.source.clone());
            }
        }
    } else {
        // Rows not checked in a build would answer for a stale one.
        state.tuples.clear();
    }
    timer.lap("withdrawnTuples");
    // Pass 2: typed sources, by TypeScript path in table order.
    let typescript = typescript_paths(
        state.ts_paths.take().filter(|_| reuse),
        &facts.typescript,
        typescript_unchanged,
        delta,
    );
    let ts_reuse = reuse;
    let ts_items = typescript
        .iter()
        .map(|path| (state.ts_keys.intern(path), Arc::clone(path)))
        .collect::<Vec<_>>();
    let ts_order = FileOrder::new(&ts_items.iter().map(|(key, _)| *key).collect::<Vec<_>>());
    let semantic_unchanged =
        typescript_unchanged || delta.is_some_and(|delta| delta.semantic_symbol_ids.is_empty());
    let ts_gate = |path: &Arc<str>| {
        if lookup.file_by_path(path).is_some() {
            unchanged_paths.contains(&**path)
        } else {
            entity_rows_unchanged(path)
                && (typescript_unchanged
                    || delta.is_some_and(|delta| !delta.file_paths.contains(&**path)))
                && semantic_unchanged
        }
    };
    let literal_member_slots = crate::literal_member_slots::LiteralMemberSlots::new(facts, lookup);
    let kinds0 = &tables.source_kinds;
    let stage0 = |key: &StateKey| match key {
        StateKey::Runtime0(symbol) => Some(u64::from(runtime0.contains(symbol))),
        StateKey::Kind0(symbol) => Some(trace::kind_fingerprint(kinds0.get(symbol))),
        _ => None,
    };
    let refreshed = state.typed.refresh(
        &ts_items,
        ts_reuse,
        &ts_gate,
        &answers!(&stage0),
        &typescript_path,
        &|path| {
            super::post_rows::typed_rows(
                ctx,
                facts.typescript.entities_for_path(path),
                &runtime0,
                kinds0,
                &literal_member_slots,
            )
        },
    );
    timer.count("typedRecomputed", refreshed.recomputed);
    tally(&refreshed);
    state.ts_paths = Some(typescript.clone());
    {
        let chunks = &state.typed;
        update_fold(
            &mut state.typed_fold,
            &ts_order,
            &refreshed.moved,
            &|key| {
                chunks.get(key).map(|rows| {
                    rows.iter()
                        .map(|row| match row {
                            TypedRow::Source { symbol, .. } | TypedRow::Setter { symbol, .. } => {
                                (symbol.clone(), row.clone())
                            }
                        })
                        .collect()
                })
            },
            &ordered_list,
        );
    }
    for (symbol, rows) in state.typed_fold.values() {
        let mut accessor = tables.accessors.contains_key(symbol);
        for row in rows.iter() {
            match row {
                TypedRow::Source {
                    kind,
                    name,
                    declaration,
                    typed,
                    ..
                } => {
                    if accessor {
                        continue;
                    }
                    let source = |implementation_visible, held_body_unfollowed| TypedSource {
                        name: name.clone(),
                        declaration: declaration.clone(),
                        kind: *kind,
                        implementation_visible,
                        held_body_unfollowed,
                    };
                    match typed {
                        Typed::Source(source) => {
                            let entry = tables.accessors[source].clone();
                            tables.accessors.insert(symbol.clone(), entry);
                            tables.source_kinds.insert(symbol.clone(), *kind);
                            tables.source_phases.entry(symbol.clone()).or_insert(1);
                            accessor = true;
                        }
                        Typed::Visible => {
                            tables
                                .typed_sources
                                .insert(symbol.clone(), source(true, false));
                        }
                        Typed::HeldUnfollowed => {
                            tables
                                .typed_sources
                                .insert(symbol.clone(), source(false, true));
                        }
                        Typed::Unknown => {
                            tables
                                .typed_sources
                                .insert(symbol.clone(), source(false, false));
                        }
                    }
                }
                TypedRow::Setter {
                    name,
                    declaration,
                    kind,
                    owned_write,
                    ..
                } => {
                    tables.setters.entry(symbol.clone()).or_insert((
                        name.clone(),
                        declaration.clone(),
                        *owned_write,
                        *kind,
                    ));
                }
            }
        }
    }
    timer.lap("typedSources");
    // Pass 3: children accessors, first wins.
    let mut runtime_sources = runtime0;
    for rows in &rows {
        for (symbol, display, declaration) in &rows.children {
            runtime_sources.insert(symbol.clone());
            tables
                .accessors
                .entry(symbol.clone())
                .or_insert_with(|| (display.clone(), declaration.clone()));
        }
    }
    timer.lap("childrenAccessors");
    // Pass 4: effect store parameters, in call order; a later effect sees an
    // earlier one's parameter.
    for rows in &rows {
        for row in &rows.effects {
            if tables.source_kinds.get(&row.source) != Some(&ReactiveSourceKind::Store) {
                continue;
            }
            let entry = tables
                .accessors
                .get(&row.source)
                .cloned()
                .unwrap_or_else(|| (row.display.clone(), row.declaration.clone()));
            tables.accessors.insert(row.parameter.clone(), entry);
            tables
                .source_kinds
                .insert(row.parameter.clone(), ReactiveSourceKind::Store);
        }
    }
    timer.lap("effectStoreParameters");
    let position = |(key, ordinal): &RowAt| (slots[key], *ordinal);
    resolve_setter_action_aliases(
        &rows,
        state.alias_index.values(),
        &position,
        tables,
        &mut runtime_sources,
    );
    timer.lap("setterActionAliases");
    resolve_store_view_aliases(&rows, state.view_index.values(), &position, tables);
    timer.lap("storeViewAliases");
    // Pass 7: prop roots, last wins.
    for rows in &rows {
        for root in &rows.prop_roots {
            if root.uncertain {
                tables.uncertain_prop_sources.insert(root.symbol.clone());
            }
            tables.prop_sources.insert(
                root.symbol.clone(),
                (root.display.clone(), root.declaration.clone()),
            );
        }
    }
    timer.lap("propSources");
    prop_roots_done();
    propagate_prop_sources(&rows, state.prop_index.values(), &position, tables);
    timer.lap("propPropagation");
    // Pass 9: a runtime source stays only while no file writes it.
    let written = state.written.values();
    runtime_sources.retain(|symbol| !written.contains_key(symbol));
    timer.lap("writtenSet");
    // Pass 10: component props from their callers.
    let props_reactivity = if lookup.dialect.props_require_caller_proof() {
        let inputs = ComponentInputs {
            files: &facts.files,
            accessors: &tables.accessors,
            source_kinds: &tables.source_kinds,
            prop_sources: &tables.prop_sources,
            setters: &tables.setters,
            actions: &tables.actions,
            async_sources: &tables.async_sources,
            runtime_sources: &runtime_sources,
            uses: state.uses.values(),
            slots: &slots,
            exported: state.exports.values(),
            attached: std::sync::OnceLock::new(),
        };
        let final_state = |key: &StateKey| inputs.answer(key);
        let refreshed = state.components.refresh(
            &items,
            reuse,
            &own_gate,
            &answers!(&final_state),
            &own_path,
            &|(slot, file)| {
                let statuses = super::post_rows::file_statuses(lookup, *slot, file);
                component_rows(
                    ctx.runtime,
                    facts,
                    lookup,
                    entities,
                    &inputs,
                    statuses.as_deref().map(Vec::as_slice),
                    file,
                )
            },
        );
        timer.count("componentsRecomputed", refreshed.recomputed);
        tally(&refreshed);
        let components = keys
            .iter()
            .map(|key| {
                state
                    .components
                    .get(*key)
                    .cloned()
                    .expect("every slot has component rows after the refresh")
            })
            .collect::<Vec<_>>();
        props_reactivity_index(components.iter().flat_map(|rows| rows.iter()))
    } else {
        state.components.clear();
        PropsReactivityIndex::default()
    };
    timer.lap("componentProps");
    PassesOutput {
        runtime_sources,
        props_reactivity,
        row_sets,
    }
}

/// Pass 5: setter and action aliases (`const set = setCount`), in Jacobi
/// rounds: a round reads the tables as the round before left them, and its
/// last alias of a key wins. A binding can add a key only in the round after
/// its source got one (every name it could add was added then), so a round
/// visits just the bindings whose source the round before added, in
/// position order, and adds exactly what a round over every binding would.
///
/// ADR 0463: an exact alias (`AliasRow::exact`) of an accessor is that
/// accessor too, with every fact the source has: its kind, primitive, phase,
/// async provenance and options, origin, runtime standing, and whether
/// summaries and returns carry it.
fn resolve_setter_action_aliases(
    rows: &[Arc<FileRows>],
    index: &BTreeMap<SymbolId, Arc<Vec<RowAt>>>,
    position: &dyn Fn(&RowAt) -> (usize, u32),
    tables: &mut SourceTables,
    runtime_sources: &mut HashSet<SymbolId>,
) {
    let SourceTables {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        source_owned_write,
        async_sources,
        source_async_options,
        value_form_stores,
        returned_source_symbols,
        summary_source_symbols,
        ..
    } = tables;
    let mut frontier = setters
        .keys()
        .chain(actions.keys())
        .chain(accessors.keys())
        .filter(|symbol| index.contains_key(*symbol))
        .cloned()
        .collect::<BTreeSet<_>>();
    while !frontier.is_empty() {
        let touched = frontier
            .iter()
            .filter_map(|symbol| index.get(symbol))
            .flat_map(|rows| rows.iter().map(position))
            .collect::<BTreeSet<_>>();
        let mut setter_aliases = Vec::new();
        let mut action_aliases = Vec::new();
        // An alias of a setter was made by the same primitive, which an
        // owned-scope guard can depend on (`optimistic_*_setter_guarded`).
        let mut primitive_aliases = Vec::new();
        // (alias, source, the alias's accessor entry).
        let mut accessor_aliases = Vec::new();
        for (slot, ordinal) in touched {
            let row = &rows[slot].aliases[ordinal as usize];
            let setter = setters.get(&row.source).cloned();
            let action = actions.get(&row.source).cloned();
            let accessor = row
                .exact
                .then(|| accessors.get(&row.source).cloned())
                .flatten();
            if setter.is_none() && action.is_none() && accessor.is_none() {
                continue;
            }
            for (symbol, display) in &row.names {
                if let Some((_, source, owned_write, source_kind)) = &setter
                    && !setters.contains_key(symbol)
                {
                    if row.immutable
                        && let Some(primitive) = source_primitives.get(&row.source)
                    {
                        primitive_aliases.push((symbol.clone(), primitive.clone()));
                    }
                    setter_aliases.push((
                        symbol.clone(),
                        (display.clone(), source.clone(), *owned_write, *source_kind),
                    ));
                }
                if let Some((_, source)) = &action
                    && !actions.contains_key(symbol)
                {
                    action_aliases.push((symbol.clone(), (display.clone(), source.clone())));
                }
                // An alias the typed pass (ADR 0294) already proved to hold
                // this source has its entry; it still takes the source's facts.
                if let Some((_, declaration)) = &accessor
                    && accessors
                        .get(symbol)
                        .is_none_or(|(_, existing)| existing == declaration)
                {
                    accessor_aliases.push((
                        symbol.clone(),
                        row.source.clone(),
                        (display.clone(), declaration.clone()),
                    ));
                }
            }
        }
        frontier = setter_aliases
            .iter()
            .map(|(symbol, _)| symbol.clone())
            .chain(action_aliases.iter().map(|(symbol, _)| symbol.clone()))
            .collect();
        // Every per-source fact is read as the round began: a source's own
        // entries were all made before the round that added the source.
        let copies = accessor_aliases
            .iter()
            .map(|(symbol, source, _)| {
                (
                    symbol.clone(),
                    accessor_origins.get(source).cloned(),
                    source_kinds.get(source).copied(),
                    source_primitives.get(source).cloned(),
                    source_phases.get(source).copied(),
                    source_owned_write.get(source).copied(),
                    async_sources.contains(source),
                    source_async_options.get(source).cloned(),
                    value_form_stores.contains(source),
                    runtime_sources.contains(source),
                    (
                        returned_source_symbols.contains(source),
                        summary_source_symbols.contains(source),
                    ),
                )
            })
            .collect::<Vec<_>>();
        setters.extend(setter_aliases);
        actions.extend(action_aliases);
        for (symbol, primitive) in primitive_aliases {
            source_primitives.entry(symbol).or_insert(primitive);
        }
        // An alias joins the next round when it gained anything: its own
        // aliases then read what it holds now. Facts only grow, so the rounds
        // end.
        for (symbol, _, entry) in accessor_aliases {
            if !accessors.contains_key(&symbol) {
                accessors.insert(symbol.clone(), entry);
                frontier.insert(symbol);
            }
        }
        for (
            symbol,
            origin,
            kind,
            primitive,
            phase,
            owned,
            asynchronous,
            options,
            value_form,
            runtime,
            (returned, summary),
        ) in copies
        {
            let mut gained = false;
            if let Some(origin) = origin
                && !accessor_origins.contains_key(&symbol)
            {
                accessor_origins.insert(symbol.clone(), origin);
                gained = true;
            }
            if let Some(kind) = kind
                && !source_kinds.contains_key(&symbol)
            {
                source_kinds.insert(symbol.clone(), kind);
                gained = true;
            }
            if let Some(primitive) = primitive
                && !source_primitives.contains_key(&symbol)
            {
                source_primitives.insert(symbol.clone(), primitive);
                gained = true;
            }
            if let Some(phase) = phase
                && !source_phases.contains_key(&symbol)
            {
                source_phases.insert(symbol.clone(), phase);
                gained = true;
            }
            if let Some(owned) = owned
                && !source_owned_write.contains_key(&symbol)
            {
                source_owned_write.insert(symbol.clone(), owned);
                gained = true;
            }
            if asynchronous {
                gained |= async_sources.insert(symbol.clone());
            }
            if let Some(options) = options
                && !source_async_options.contains_key(&symbol)
            {
                source_async_options.insert(symbol.clone(), options);
                gained = true;
            }
            if value_form {
                gained |= value_form_stores.insert(symbol.clone());
            }
            if runtime {
                gained |= runtime_sources.insert(symbol.clone());
            }
            if returned {
                gained |= returned_source_symbols.insert(symbol.clone());
            }
            if summary {
                gained |= summary_source_symbols.insert(symbol.clone());
            }
            if gained {
                frontier.insert(symbol);
            }
        }
        if frontier.is_empty() {
            break;
        }
    }
}

/// Pass 6: store views (`const view = omit(store, "k")`), in Jacobi rounds
/// like pass 5: a binding's root is its first candidate that is a store
/// accessor as the round began, and it can add a key only in the round after
/// one of its candidates became one.
fn resolve_store_view_aliases(
    rows: &[Arc<FileRows>],
    index: &BTreeMap<SymbolId, Arc<Vec<RowAt>>>,
    position: &dyn Fn(&RowAt) -> (usize, u32),
    tables: &mut SourceTables,
) {
    let SourceTables {
        accessors,
        source_kinds,
        ..
    } = tables;
    // ADR 0323 (review round 10): an `omit` or `merge` view over a store
    // reads through to it (`viewSource(e)[key]`, `@solidjs/signals@2.0.0-rc.13`
    // `dist/prod/store/utils.js`), so a binding of one is a store alias, like
    // an effect apply's store parameter.
    let mut frontier = source_kinds
        .iter()
        .filter(|(symbol, kind)| {
            **kind == ReactiveSourceKind::Store
                && accessors.contains_key(*symbol)
                && index.contains_key(*symbol)
        })
        .map(|(symbol, _)| symbol.clone())
        .collect::<BTreeSet<_>>();
    while !frontier.is_empty() {
        let touched = frontier
            .iter()
            .filter_map(|symbol| index.get(symbol))
            .flat_map(|rows| rows.iter().map(position))
            .collect::<BTreeSet<_>>();
        let mut aliases = Vec::new();
        for (slot, ordinal) in touched {
            let row = &rows[slot].views[ordinal as usize];
            let Some(root) = row.candidates.iter().find(|symbol| {
                source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store)
                    && accessors.contains_key(*symbol)
            }) else {
                continue;
            };
            let Some((_, declaration)) = accessors.get(root).cloned() else {
                continue;
            };
            for (symbol, display) in &row.names {
                if !accessors.contains_key(symbol) {
                    aliases.push((symbol.clone(), (display.clone(), declaration.clone())));
                }
            }
        }
        if aliases.is_empty() {
            break;
        }
        frontier = aliases.iter().map(|(symbol, _)| symbol.clone()).collect();
        for (symbol, entry) in aliases {
            accessors.insert(symbol.clone(), entry);
            source_kinds.insert(symbol, ReactiveSourceKind::Store);
        }
    }
}

/// Pass 8: prop aliases and views (`const local = props`,
/// `const rest = omit(props, "a")`), in Gauss-Seidel sweeps over the
/// bindings in position order until one adds nothing: a binding adds its
/// absent names, from its first candidate that is a prop source when it is
/// visited. It adds at its first visit with such a candidate and never
/// again, so it fires at the earliest (sweep, position) after one of its
/// candidates became a prop source: in the same sweep when that happened at
/// an earlier position, the next one otherwise. A queue in that order visits
/// each binding once, at that time, seeing exactly the sources the sweeps
/// would have.
fn propagate_prop_sources(
    rows: &[Arc<FileRows>],
    index: &BTreeMap<SymbolId, Arc<Vec<RowAt>>>,
    position: &dyn Fn(&RowAt) -> (usize, u32),
    tables: &mut SourceTables,
) {
    let SourceTables {
        prop_sources,
        uncertain_prop_sources,
        ..
    } = tables;
    let mut queue = BinaryHeap::new();
    for symbol in prop_sources.keys() {
        for at in index.get(symbol).into_iter().flat_map(|rows| rows.iter()) {
            let (slot, ordinal) = position(at);
            queue.push(Reverse((1_u32, slot, ordinal)));
        }
    }
    let mut visited = HashSet::<(usize, u32)>::default();
    while let Some(Reverse((sweep, slot, ordinal))) = queue.pop() {
        if !visited.insert((slot, ordinal)) {
            continue;
        }
        let row = &rows[slot].prop_aliases[ordinal as usize];
        let Some((source, (_, declaration))) = row.candidates.iter().find_map(|symbol| {
            prop_sources
                .get(symbol)
                .map(|entry| (symbol.clone(), entry.clone()))
        }) else {
            continue;
        };
        let source_uncertain = uncertain_prop_sources.contains(&source);
        for (symbol, display) in &row.names {
            if prop_sources.contains_key(symbol) {
                continue;
            }
            if source_uncertain {
                uncertain_prop_sources.insert(symbol.clone());
            }
            prop_sources.insert(symbol.clone(), (display.clone(), declaration.clone()));
            for at in index.get(symbol).into_iter().flat_map(|rows| rows.iter()) {
                let next = position(at);
                let sweep = if next > (slot, ordinal) {
                    sweep
                } else {
                    sweep + 1
                };
                queue.push(Reverse((sweep, next.0, next.1)));
            }
        }
    }
}

/// The first table and key in which two runs of the passes differ, if any.
pub(super) fn first_difference(
    kept: &SourceTables,
    kept_output: &PassesOutput,
    fresh: &SourceTables,
    fresh_output: &PassesOutput,
) -> Option<String> {
    fn map<K: std::fmt::Debug + Eq + std::hash::Hash, V: std::fmt::Debug + PartialEq>(
        name: &str,
        kept: &HashMap<K, V>,
        fresh: &HashMap<K, V>,
    ) -> Option<String> {
        let mut differing = kept
            .iter()
            .filter(|(key, value)| fresh.get(*key) != Some(*value))
            .map(|(key, value)| format!("{name} {key:?}: {value:?} != {:?}", fresh.get(key)))
            .chain(
                fresh
                    .iter()
                    .filter(|(key, _)| !kept.contains_key(*key))
                    .map(|(key, value)| format!("{name} {key:?}: None != {value:?}")),
            )
            .collect::<Vec<_>>();
        differing.sort_unstable();
        differing.into_iter().next()
    }
    fn set<K: std::fmt::Debug + Eq + std::hash::Hash>(
        name: &str,
        kept: &HashSet<K>,
        fresh: &HashSet<K>,
    ) -> Option<String> {
        let mut differing = kept
            .symmetric_difference(fresh)
            .map(|key| format!("{name} {key:?}"))
            .collect::<Vec<_>>();
        differing.sort_unstable();
        differing.into_iter().next()
    }
    map("accessors", &kept.accessors, &fresh.accessors)
        .or_else(|| {
            map(
                "accessor_origins",
                &kept.accessor_origins,
                &fresh.accessor_origins,
            )
        })
        .or_else(|| map("setters", &kept.setters, &fresh.setters))
        .or_else(|| map("actions", &kept.actions, &fresh.actions))
        .or_else(|| map("source_kinds", &kept.source_kinds, &fresh.source_kinds))
        .or_else(|| {
            map(
                "source_primitives",
                &kept.source_primitives,
                &fresh.source_primitives,
            )
        })
        .or_else(|| map("source_phases", &kept.source_phases, &fresh.source_phases))
        .or_else(|| {
            set(
                "returned_source_symbols",
                &kept.returned_source_symbols,
                &fresh.returned_source_symbols,
            )
        })
        .or_else(|| {
            set(
                "summary_source_symbols",
                &kept.summary_source_symbols,
                &fresh.summary_source_symbols,
            )
        })
        .or_else(|| {
            map(
                "source_owned_write",
                &kept.source_owned_write,
                &fresh.source_owned_write,
            )
        })
        .or_else(|| set("async_sources", &kept.async_sources, &fresh.async_sources))
        .or_else(|| {
            map(
                "source_async_options",
                &kept.source_async_options,
                &fresh.source_async_options,
            )
        })
        .or_else(|| {
            set(
                "value_form_stores",
                &kept.value_form_stores,
                &fresh.value_form_stores,
            )
        })
        .or_else(|| map("typed_sources", &kept.typed_sources, &fresh.typed_sources))
        .or_else(|| map("prop_sources", &kept.prop_sources, &fresh.prop_sources))
        .or_else(|| {
            set(
                "uncertain_prop_sources",
                &kept.uncertain_prop_sources,
                &fresh.uncertain_prop_sources,
            )
        })
        .or_else(|| {
            set(
                "runtime_sources",
                &kept_output.runtime_sources,
                &fresh_output.runtime_sources,
            )
        })
        .or_else(|| {
            (kept_output.props_reactivity.caller_proof
                != fresh_output.props_reactivity.caller_proof)
                .then(|| "props_reactivity caller_proof".to_owned())
        })
        .or_else(|| {
            map(
                "props_reactivity",
                &kept_output.props_reactivity.by_declaration,
                &fresh_output.props_reactivity.by_declaration,
            )
        })
}
