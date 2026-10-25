//! L2: derived answers memoized per key, with recorded reads (P5 study §3.2
//! L2, §3.3, §3.4 item 4).
//!
//! A memo table ([`Memo`]) maps a key to a value computed by a deterministic
//! function whose only access to the world is a [`Ctx`]. Each read through it
//! is recorded with the entry, once, as a compact [`QueryId`]:
//! - an input of the [`Schema`] (for a build: one section of one file, an L1
//!   index key, a file's slots, a build constant), see [`build`]; its changes
//!   are pushed by [`Store::begin`], so it carries no fingerprint;
//! - another memo's answer, `ctx.memo::<T>(key)`, fingerprinted by the
//!   revision in which that answer last changed.
//!
//! The store keeps the reverse map `query -> readers`. Before a build reads
//! anything, [`Store::begin`] receives the inputs whose answers changed and
//! marks their readers dirty, and every transitive reader maybe-dirty. An
//! answer is then brought up to date when asked:
//! - clean: reused as is;
//! - maybe-dirty: its memo reads are brought up to date first, in recorded
//!   order; if each answers with the revision recorded, the entry is clean
//!   again without running (early cutoff);
//! - dirty, or a memo read that changed: recomputed. A recomputed value equal
//!   to the old one keeps its revision, so its readers do not rerun.
//!
//! **Soundness** (study §3.3). Determinism plus complete recording gives the
//! trace-replay lemma: a computation whose recorded reads all answer as
//! recorded returns the recorded value. Invalidation is complete: an input
//! read changes only when the schema reports it changed (pushed to its
//! readers as dirty), and a memo read changes only when that memo was
//! recomputed to a different value (seen by the revision comparison). The
//! read graph is acyclic because only context-free, top-level answers are
//! memoized: a computation that would read its own key is a [`Cycle`], which
//! fails closed (the table's [`Memo::on_cycle`] answer, a report, and the
//! store dropped before the next build).
//!
//! **Canonical answers.** Keys and values are [`Canonical`]: sorted or
//! ordered data only. A `HashMap` or `HashSet`, whose iteration order is its
//! insertion history, does not implement it, so a table answering one does
//! not compile.
//!
//! **Verify-reused mode** (`SOLID_CHECKER_IR_VERIFY_REUSED`, [`build`]):
//! every answer reused from an earlier build is recomputed once per build
//! and compared. A difference names the table and key whose record missed a
//! read, is reported like a shadow mismatch, and the fresh value is served.

pub(crate) mod build;
pub(crate) mod derived;
mod locks;
pub(crate) mod probes;
mod serial_digest;
#[cfg(test)]
mod tests;

use locks::{Mutex, RwLock};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use crate::{HashMap, HashSet};

/// A build of the store: answers are stamped with the revision in which
/// they last changed.
pub(crate) type Revision = u32;

/// A recorded query: an input, or one memo table's key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct QueryId(pub(crate) u32);

/// Which memo table of a schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct TableId(pub(crate) u16);

/// A value that can cross a [`Ctx`]: equal values are equal bytes in equal
/// order. Implemented for ordered containers and plain data only; a hash map
/// or set is not canonical (its order is its insertion history).
pub(crate) trait Canonical: Clone + Eq + Debug + Send + Sync + 'static {}

macro_rules! canonical {
    ($($ty:ty),* $(,)?) => { $(impl Canonical for $ty {})* };
}
canonical!(
    (),
    bool,
    u8,
    u16,
    u32,
    u64,
    usize,
    i32,
    i64,
    String,
    std::sync::Arc<str>,
    solid_facts::core::Span,
    super::FileId,
    crate::SymbolId,
    solid_dialect::Primitive,
);
impl<T: Canonical> Canonical for Option<T> {}
impl<T: Canonical> Canonical for Vec<T> {}
impl<T: Canonical> Canonical for Box<T> {}
/// Shared, compared by value: a large answer is cloned for each reader.
impl<T: Canonical> Canonical for std::sync::Arc<T> {}
impl<T: Canonical + Ord> Canonical for BTreeSet<T> {}
impl<K: Canonical + Ord, V: Canonical> Canonical for BTreeMap<K, V> {}
impl<A: Canonical, B: Canonical> Canonical for (A, B) {}
impl<A: Canonical, B: Canonical, C: Canonical> Canonical for (A, B, C) {}
impl<A: Canonical, B: Canonical, C: Canonical, D: Canonical> Canonical for (A, B, C, D) {}

/// The world a family of memo tables reads: its recorded inputs, its tables,
/// and the per-build world their computations read through a [`Ctx`].
pub(crate) trait Schema: Sized + 'static {
    /// An input query: what a computation reads besides other memos.
    type Input: Copy + Eq + Hash + Ord + Debug + Send + Sync + 'static;
    type Tables: TableSet<Self>;
    type World<'w>: World<Self>;
}

/// One build's world. An input read records no fingerprint: invalidation
/// is pushed by [`Store::begin`] (and [`Store::invalidate`]).
pub(crate) trait World<S: Schema>: Sync {}

/// A schema's memo tables, by id.
pub(crate) trait TableSet<S: Schema>: Default + Send + Sync {
    fn table(&self, id: TableId) -> &dyn ErasedTable<S>;
    fn tables(&self) -> Vec<&dyn ErasedTable<S>>;
}

/// A memo table: a deterministic, context-free function of its key.
pub(crate) trait Memo<S: Schema>: Sized + 'static {
    type Key: Canonical + Hash;
    type Value: Canonical;
    const NAME: &'static str;
    const ID: TableId;
    fn table(tables: &S::Tables) -> &Table<S, Self>;
    /// The answer for `key`. Every read of the world goes through `ctx`.
    fn compute(ctx: &Ctx<'_, '_, S>, key: &Self::Key) -> Result<Self::Value, Cycle>;
    /// The fail-closed answer for a key whose computation reads itself: the
    /// answer that proves nothing.
    fn on_cycle(key: &Self::Key) -> Self::Value;
    /// Inputs the key itself names (its own file): recorded with every
    /// entry, so reading them through the context is free.
    fn implied(_key: &Self::Key) -> Vec<S::Input> {
        Vec::new()
    }
}

/// A computation read its own key, directly or through other memos. The
/// path names each table and key on the way, innermost last.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Cycle {
    pub(crate) path: Vec<String>,
}

const CLEAN: u8 = 0;
const MAYBE_DIRTY: u8 = 1;
const DIRTY: u8 = 2;

struct Entry<V> {
    value: V,
    state: AtomicU8,
    /// The revision in which `value` last changed: what a reader records.
    changed_at: Revision,
    /// The revision in which `value` was last computed or found unchanged by
    /// a recomputation.
    computed_at: Revision,
    /// The last revision that asked for the entry (sweep, reuse counting).
    used_at: AtomicU32,
    /// The last revision whose verify-reused check covered the entry.
    reuse_checked_at: AtomicU32,
}

/// A table's interned keys (both ways) and its computed entries.
struct TableData<M: Memo<S>, S: Schema> {
    ids: HashMap<M::Key, QueryId>,
    keys: BTreeMap<QueryId, M::Key>,
    entries: BTreeMap<QueryId, Entry<M::Value>>,
}

impl<M: Memo<S>, S: Schema> Default for TableData<M, S> {
    fn default() -> Self {
        Self {
            ids: HashMap::default(),
            keys: BTreeMap::new(),
            entries: BTreeMap::new(),
        }
    }
}

/// How one table's answers were found in one build. A key asked several
/// times in a build counts once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TableStats {
    /// Never computed before (a new key, or the store was empty).
    pub(crate) fresh: u64,
    /// Clean from an earlier build: reused untouched.
    pub(crate) reused: u64,
    /// Maybe-dirty, and every memo read answered as recorded: reused.
    pub(crate) verified: u64,
    /// Recomputed to the value it had (early cutoff).
    pub(crate) unchanged: u64,
    /// Recomputed to a new value.
    pub(crate) changed: u64,
    pub(crate) cycles: u64,
    /// Verify-reused mode: reused answers that a fresh computation contradicts.
    pub(crate) mismatches: u64,
    /// Entries at the end of the build.
    pub(crate) entries: u64,
    /// Recorded reads of those entries.
    pub(crate) reads: u64,
}

#[derive(Default)]
struct AtomicStats {
    fresh: AtomicU64,
    reused: AtomicU64,
    verified: AtomicU64,
    unchanged: AtomicU64,
    changed: AtomicU64,
    cycles: AtomicU64,
    mismatches: AtomicU64,
}

impl AtomicStats {
    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn take(&self) -> TableStats {
        let take = |counter: &AtomicU64| counter.swap(0, Ordering::Relaxed);
        TableStats {
            fresh: take(&self.fresh),
            reused: take(&self.reused),
            verified: take(&self.verified),
            unchanged: take(&self.unchanged),
            changed: take(&self.changed),
            cycles: take(&self.cycles),
            mismatches: take(&self.mismatches),
            entries: 0,
            reads: 0,
        }
    }
}

/// A table's entries are split by key hash into this many shards, so the
/// workers of a parallel stage asking one table rarely wait on each other.
/// A key's query id is allocated in its shard ([`Graph::allocate_in`]): the
/// id names the shard too.
const TABLE_SHARDS: usize = 16;

/// The entries of one memo table.
pub(crate) struct Table<S: Schema, M: Memo<S>> {
    /// On the heap: a schema has dozens of tables, and a store built on a
    /// small-stack thread (the idle check's) must not carry their shards.
    data: Box<[RwLock<TableData<M, S>>; TABLE_SHARDS]>,
    stats: AtomicStats,
}

impl<S: Schema, M: Memo<S>> Default for Table<S, M> {
    fn default() -> Self {
        Self {
            data: Box::new(std::array::from_fn(|_| RwLock::default())),
            stats: AtomicStats::default(),
        }
    }
}

impl<S: Schema, M: Memo<S>> Table<S, M> {
    /// The shard holding `key`'s id and entry.
    fn key_shard(key: &M::Key) -> usize {
        use std::hash::Hasher;
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        // The high bits: FxHash's low bits are weak for small integer keys.
        (hasher.finish() >> 60) as usize % TABLE_SHARDS
    }

    /// The shard of an entry by its query id.
    fn shard(&self, query: QueryId) -> &RwLock<TableData<M, S>> {
        &self.data[query.0 as usize % TABLE_SHARDS]
    }
}

/// What the store needs of a table without knowing its key and value types.
pub(crate) trait ErasedTable<S: Schema>: Send + Sync {
    fn name(&self) -> &'static str;
    /// The table's id ([`Memo::ID`]).
    fn id(&self) -> TableId;
    /// Brings the entry `query` up to date, as a read from `parent` would,
    /// and answers the revision its value last changed in; `None` when the
    /// entry is gone.
    fn ensure(
        &self,
        store: &Store<S>,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
    ) -> Result<Option<Revision>, Cycle>;
    /// The revision a clean entry's value last changed in, without bringing
    /// anything up to date or cloning the value; the entry counts as asked
    /// for in `revision`. `None` when the entry is gone or not clean.
    fn clean_changed_at(&self, query: QueryId, revision: Revision) -> Option<Revision>;
    /// Raises the entry's state to `state` (dirty is never lowered).
    fn raise(&self, query: QueryId, state: u8);
    /// The entries asked for in `revision`.
    fn used(&self, revision: Revision) -> Vec<QueryId>;
    /// Drops every entry and key not in `live`, returning the dropped ids.
    fn sweep(&self, live: &HashSet<QueryId>) -> Vec<QueryId>;
    fn describe(&self, query: QueryId) -> String;
    fn take_stats(&self) -> TableStats;
    fn len(&self) -> usize;
    fn clear(&self);
}

impl<S: Schema, M: Memo<S>> ErasedTable<S> for Table<S, M> {
    fn name(&self) -> &'static str {
        M::NAME
    }

    fn id(&self) -> TableId {
        M::ID
    }

    fn ensure(
        &self,
        store: &Store<S>,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
    ) -> Result<Option<Revision>, Cycle> {
        let key = {
            let data = self.shard(query).read().expect("memo table lock");
            if !data.entries.contains_key(&query) {
                return Ok(None);
            }
            match data.keys.get(&query) {
                Some(key) => key.clone(),
                None => return Ok(None),
            }
        };
        store
            .get::<M>(parent, &key)
            .map(|answer| Some(answer.changed_at))
    }

    fn clean_changed_at(&self, query: QueryId, revision: Revision) -> Option<Revision> {
        let data = self.shard(query).read().expect("memo table lock");
        let entry = data.entries.get(&query)?;
        if entry.state.load(Ordering::Acquire) != CLEAN {
            return None;
        }
        if entry.used_at.swap(revision, Ordering::AcqRel) != revision
            && entry.computed_at != revision
        {
            AtomicStats::bump(&self.stats.reused);
        }
        Some(entry.changed_at)
    }

    fn raise(&self, query: QueryId, state: u8) {
        let data = self.shard(query).read().expect("memo table lock");
        if let Some(entry) = data.entries.get(&query) {
            entry.state.fetch_max(state, Ordering::Relaxed);
        }
    }

    fn used(&self, revision: Revision) -> Vec<QueryId> {
        let mut used = Vec::new();
        for shard in &self.data {
            let data = shard.read().expect("memo table lock");
            used.extend(
                data.entries
                    .iter()
                    .filter(|(_, entry)| entry.used_at.load(Ordering::Relaxed) == revision)
                    .map(|(query, _)| *query),
            );
        }
        used.sort_unstable();
        used
    }

    fn sweep(&self, live: &HashSet<QueryId>) -> Vec<QueryId> {
        let mut dropped = Vec::new();
        for shard in &self.data {
            let mut data = shard.write().expect("memo table lock");
            let TableData { ids, keys, entries } = &mut *data;
            entries.retain(|query, _| live.contains(query));
            keys.retain(|query, key| {
                let keep = entries.contains_key(query);
                if !keep {
                    ids.remove(key);
                    dropped.push(*query);
                }
                keep
            });
        }
        dropped.sort_unstable();
        dropped
    }

    fn describe(&self, query: QueryId) -> String {
        let data = self.shard(query).read().expect("memo table lock");
        data.keys.get(&query).map_or_else(
            || format!("{} {query:?}", M::NAME),
            |key| format!("{} {key:?}", M::NAME),
        )
    }

    fn take_stats(&self) -> TableStats {
        let mut stats = self.stats.take();
        stats.entries = self.len() as u64;
        stats
    }

    fn len(&self) -> usize {
        self.data
            .iter()
            .map(|shard| shard.read().expect("memo table lock").entries.len())
            .sum()
    }

    fn clear(&self) {
        for shard in &self.data {
            *shard.write().expect("memo table lock") = TableData::default();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeKind<I> {
    Input(I),
    Memo(TableId),
}

struct Node<I> {
    kind: NodeKind<I>,
    /// A memo's recorded reads, in reading order, each with its answer's
    /// fingerprint: for a memo read, the revision the answer last changed
    /// in; an input read records [`INPUT_READ`], since its changes are
    /// pushed by [`Store::begin`] and never compared.
    reads: Vec<(QueryId, u64)>,
    /// Sorted, without repeats. Exact once [`Graph::flush`] has applied the
    /// pending edges.
    readers: Vec<QueryId>,
}

impl<I> Node<I> {
    fn new(kind: NodeKind<I>) -> Self {
        Self {
            kind,
            reads: Vec::new(),
            readers: Vec::new(),
        }
    }
}

/// The fingerprint an input read records: no revision is this large, so a
/// recorded read is told an input read without looking its node up.
const INPUT_READ: u64 = u64::MAX;

/// One shard of the read graph: the nodes whose id is the shard modulo
/// [`TABLE_SHARDS`] (a memo key's id names its table shard, an input's its
/// [`input_shard`]), so moving the records in and applying reverse edges run
/// shard by shard on several workers.
struct GraphShard<I> {
    nodes: HashMap<QueryId, Node<I>>,
    /// Nodes that lost a reader or were allocated since the last
    /// [`Store::end`]: the only inputs it can find unread.
    unread: Vec<QueryId>,
    /// Reverse-edge changes [`Graph::record`] has not applied yet, in record
    /// order: each reader with the reads it dropped. Only [`Store::mark`] and
    /// [`Store::end`] read `readers`, and both flush first, so the reverse
    /// map they see is the one recording it eagerly would have built.
    pending: Vec<(QueryId, Vec<(QueryId, u64)>)>,
}

impl<I> Default for GraphShard<I> {
    fn default() -> Self {
        Self {
            nodes: HashMap::default(),
            unread: Vec::new(),
            pending: Vec::new(),
        }
    }
}

/// The read graph: every recorded query's reads and readers. Installs do
/// not take its lock: they intern their inputs in [`Interned`] and queue
/// their reads in [`Store::records`], which [`Store::synced_graph`] moves
/// here before anything reads the graph ([`Store::mark`], [`Store::end`]).
/// A verification ([`Store::reads_unchanged`]) reads it without moving
/// them: it verifies only a maybe-dirty entry, which a mark raised after
/// moving every record made before it, and an entry recorded since is
/// clean.
struct Graph<I> {
    shards: Box<[GraphShard<I>; TABLE_SHARDS]>,
}

impl<I> Default for Graph<I> {
    fn default() -> Self {
        Self {
            shards: Box::new(std::array::from_fn(|_| GraphShard::default())),
        }
    }
}

fn graph_shard(query: QueryId) -> usize {
    query.0 as usize % TABLE_SHARDS
}

impl<I> Graph<I> {
    fn node(&self, query: &QueryId) -> Option<&Node<I>> {
        self.shards[graph_shard(*query)].nodes.get(query)
    }

    fn remove(&mut self, query: &QueryId) -> Option<Node<I>> {
        self.shards[graph_shard(*query)].nodes.remove(query)
    }

    fn nodes(&self) -> impl Iterator<Item = &Node<I>> {
        self.shards.iter().flat_map(|shard| shard.nodes.values())
    }
}

impl<I: Copy + Eq + Hash + Send> GraphShard<I> {
    /// Replaces `query`'s recorded reads; the reverse map follows at the
    /// next [`Graph::flush`].
    fn record(&mut self, query: QueryId, reads: Vec<(QueryId, u64)>) {
        let previous = self
            .nodes
            .get_mut(&query)
            .map(|node| std::mem::replace(&mut node.reads, reads))
            .unwrap_or_default();
        self.pending.push((query, previous));
    }

    /// This shard's pending changes as reverse edges, by the shard of the
    /// node read: each reader's dropped reads (lost) and its reads as they
    /// are now (gained).
    fn pending_edges(&mut self) -> [(Vec<u64>, Vec<u64>); TABLE_SHARDS] {
        let mut edges: [(Vec<u64>, Vec<u64>); TABLE_SHARDS] =
            std::array::from_fn(|_| (Vec::new(), Vec::new()));
        let pending = std::mem::take(&mut self.pending);
        let mut readers = Vec::with_capacity(pending.len());
        for (query, previous) in pending {
            for (read, _) in previous {
                edges[graph_shard(read)].0.push(edge(read, query));
            }
            readers.push(query);
        }
        readers.sort_unstable();
        readers.dedup();
        for query in readers {
            if let Some(node) = self.nodes.get(&query) {
                for (read, _) in &node.reads {
                    edges[graph_shard(*read)].1.push(edge(*read, query));
                }
            }
        }
        edges
    }

    /// Removes the `lost` reverse edges and adds the `gained` ones (each
    /// [`edge`]-packed, of nodes in this shard; a reader in both keeps the
    /// edge), node by node. A node that lost a reader becomes an unread
    /// candidate.
    fn apply_edges(&mut self, mut lost: Vec<u64>, mut gained: Vec<u64>) {
        lost.sort_unstable();
        lost.dedup();
        gained.sort_unstable();
        gained.dedup();
        let (mut l, mut g) = (0, 0);
        let mut lost_readers = Vec::new();
        let mut gained_readers = Vec::new();
        while l < lost.len() || g < gained.len() {
            let read = match (lost.get(l), gained.get(g)) {
                (Some(a), Some(b)) => edge_read(*a).min(edge_read(*b)),
                (Some(a), None) => edge_read(*a),
                (None, Some(b)) => edge_read(*b),
                (None, None) => unreachable!("one side has edges"),
            };
            lost_readers.clear();
            gained_readers.clear();
            while let Some(packed) = lost.get(l).filter(|packed| edge_read(**packed) == read) {
                lost_readers.push(edge_reader(*packed));
                l += 1;
            }
            while let Some(packed) = gained.get(g).filter(|packed| edge_read(**packed) == read) {
                gained_readers.push(edge_reader(*packed));
                g += 1;
            }
            let Some(node) = self.nodes.get_mut(&read) else {
                continue;
            };
            if !lost_readers.is_empty() {
                node.readers
                    .retain(|reader| lost_readers.binary_search(reader).is_err());
                self.unread.push(read);
            }
            merge_sorted(&mut node.readers, &gained_readers);
        }
    }
}

impl<I: Copy + Eq + Hash + Send> Graph<I> {
    /// Applies the pending reverse-edge changes. Each reader ends with its
    /// reads as they are now and loses every read it dropped since the last
    /// flush: the map recording eagerly, in record order, builds (a reader
    /// recorded twice drops each earlier set and ends with the last; a
    /// reader's records are all in its own shard). The edges are applied
    /// grouped by the node read, sorted, so each node is looked up and
    /// merged once, shard by shard. Nodes are removed only by `Store::end`
    /// and `Store::clear`, which flush or drop the pending changes first, so
    /// a read present at its record is present here.
    fn flush(&mut self) {
        if self.shards.iter().all(|shard| shard.pending.is_empty()) {
            return;
        }
        let edges = each_shard(self.shards.iter_mut().collect(), |shard| {
            shard.pending_edges()
        });
        let mut by_read: [(Vec<u64>, Vec<u64>); TABLE_SHARDS] =
            std::array::from_fn(|_| (Vec::new(), Vec::new()));
        for source in edges {
            for ((lost, gained), (all_lost, all_gained)) in source.into_iter().zip(&mut by_read) {
                all_lost.extend(lost);
                all_gained.extend(gained);
            }
        }
        self.apply_edges(by_read);
    }

    /// Applies reverse-edge changes already split by the shard of the node
    /// read ([`GraphShard::apply_edges`]).
    fn apply_edges(&mut self, by_read: [(Vec<u64>, Vec<u64>); TABLE_SHARDS]) {
        each_shard(
            self.shards.iter_mut().zip(by_read).collect(),
            |(shard, (lost, gained))| shard.apply_edges(lost, gained),
        );
    }
}

/// Runs `work` on each item (one per graph shard), on several analysis
/// workers when there are; the sequential arm is the wasm32-wasip1 one.
#[allow(clippy::disallowed_methods)]
fn each_shard<T: Send, R: Send>(items: Vec<T>, work: impl Fn(T) -> R + Sync) -> Vec<R> {
    let workers = crate::pipeline::available_analysis_workers().min(items.len());
    if workers <= 1 {
        return items.into_iter().map(work).collect();
    }
    let chunk = items.len().div_ceil(workers);
    let mut groups = Vec::new();
    let mut items = items.into_iter();
    loop {
        let group = items.by_ref().take(chunk).collect::<Vec<_>>();
        if group.is_empty() {
            break;
        }
        groups.push(group);
    }
    let work = &work;
    std::thread::scope(|scope| {
        let handles = groups
            .into_iter()
            .map(|group| scope.spawn(move || group.into_iter().map(work).collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("memo graph worker panicked"))
            .collect()
    })
}

/// A reverse edge `read -> reader`, packed so that sorting groups the
/// edges by the node read.
fn edge(read: QueryId, reader: QueryId) -> u64 {
    (u64::from(read.0) << 32) | u64::from(reader.0)
}

fn edge_read(edge: u64) -> QueryId {
    QueryId((edge >> 32) as u32)
}

fn edge_reader(edge: u64) -> QueryId {
    QueryId(edge as u32)
}

/// Adds the sorted, distinct `gained` to the sorted, distinct `readers`.
fn merge_sorted(readers: &mut Vec<QueryId>, gained: &[QueryId]) {
    if gained.is_empty() {
        return;
    }
    if readers.last().is_none_or(|last| *last < gained[0]) {
        readers.extend_from_slice(gained);
        return;
    }
    let mut merged = Vec::with_capacity(readers.len() + gained.len());
    let (mut i, mut j) = (0, 0);
    while i < readers.len() && j < gained.len() {
        match readers[i].cmp(&gained[j]) {
            std::cmp::Ordering::Less => {
                merged.push(readers[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                merged.push(gained[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                merged.push(readers[i]);
                i += 1;
                j += 1;
            }
        }
    }
    merged.extend_from_slice(&readers[i..]);
    merged.extend_from_slice(&gained[j..]);
    *readers = merged;
}

/// One shard of the interned inputs: an install interns the inputs it
/// read here, under this shard's lock only.
struct Interned<I> {
    ids: HashMap<I, QueryId>,
    /// Interned since the graph last took them: their nodes are created
    /// then ([`Store::synced_graph`]).
    fresh: Vec<(QueryId, I)>,
}

impl<I> Default for Interned<I> {
    fn default() -> Self {
        Self {
            ids: HashMap::default(),
            fresh: Vec::new(),
        }
    }
}

/// An install's reads, queued for the graph: the query, its table, and its
/// reads with their fingerprints, in reading order.
type Record = (QueryId, TableId, Vec<(QueryId, u64)>);

/// The shard of an input's interned id.
fn input_shard<I: Hash>(input: &I) -> usize {
    use std::hash::Hasher;
    let mut hasher = rustc_hash::FxHasher::default();
    input.hash(&mut hasher);
    (hasher.finish() >> 60) as usize % TABLE_SHARDS
}

/// One recorded read, before its input is interned.
enum Read<I> {
    Input(I),
    Memo(QueryId, Revision),
}

/// How many reads a computation records before its repeated-input check
/// keeps a set ([`Ctx::record_input`]).
const SCANNED_READS: usize = 16;

/// The computation stack of one thread: the entries being computed or
/// verified, innermost first, for cycle detection.
struct Frame<'c> {
    current: Option<(QueryId, TableId)>,
    parent: Option<&'c Frame<'c>>,
}

impl Frame<'_> {
    fn contains(&self, query: QueryId) -> bool {
        let mut frame = Some(self);
        while let Some(current) = frame {
            if current.current.is_some_and(|(id, _)| id == query) {
                return true;
            }
            frame = current.parent;
        }
        false
    }
}

/// The only way a memo computation reads the world. Every read is recorded
/// into the entry being computed.
pub(crate) struct Ctx<'c, 'w, S: Schema> {
    world: &'c S::World<'w>,
    store: &'c Store<S>,
    frame: Frame<'c>,
    reads: RefCell<Vec<Read<S::Input>>>,
    /// The inputs already in `reads`: a repeated read adds nothing, since an
    /// install keeps only each read's first record.
    recorded: RefCell<HashSet<S::Input>>,
    /// Inputs the current key names itself: reading them is free.
    implied: Vec<S::Input>,
}

impl<'c, 'w, S: Schema> Ctx<'c, 'w, S> {
    /// A context for a top-level read (outside every memo): its reads are
    /// not recorded anywhere.
    pub(crate) fn root(world: &'c S::World<'w>, store: &'c Store<S>) -> Self {
        Self {
            world,
            store,
            frame: Frame {
                current: None,
                parent: None,
            },
            reads: RefCell::new(Vec::new()),
            recorded: RefCell::new(HashSet::default()),
            implied: Vec::new(),
        }
    }

    fn child<'d>(&'d self, current: (QueryId, TableId), implied: Vec<S::Input>) -> Ctx<'d, 'w, S> {
        Ctx {
            world: self.world,
            store: self.store,
            frame: Frame {
                current: Some(current),
                parent: Some(&self.frame),
            },
            reads: RefCell::new(Vec::new()),
            recorded: RefCell::new(HashSet::default()),
            implied,
        }
    }

    /// The world, for the schema's recorded accessors ([`build`]); never
    /// handed to memo code.
    fn world(&self) -> &'c S::World<'w> {
        self.world
    }

    /// Records a read of `input`. A computation's first few reads are
    /// checked for a repeat by scanning them; past [`SCANNED_READS`], a set
    /// of the inputs read answers.
    fn record_input(&self, input: S::Input) {
        if self.frame.current.is_none() || self.implied.contains(&input) {
            return;
        }
        let mut reads = self.reads.borrow_mut();
        let mut recorded = self.recorded.borrow_mut();
        if recorded.is_empty() {
            if reads
                .iter()
                .any(|read| matches!(read, Read::Input(read) if *read == input))
            {
                return;
            }
            if reads.len() < SCANNED_READS {
                reads.push(Read::Input(input));
                return;
            }
            recorded.extend(reads.iter().filter_map(|read| match read {
                Read::Input(input) => Some(*input),
                Read::Memo(..) => None,
            }));
            // A computation with only memo reads so far: the set must not
            // stay empty, or the scan would answer again.
            recorded.insert(input);
            reads.push(Read::Input(input));
            return;
        }
        if recorded.insert(input) {
            reads.push(Read::Input(input));
        }
    }

    /// Another memo's answer for `key`, recorded.
    pub(crate) fn memo<M: Memo<S>>(&self, key: &M::Key) -> Result<M::Value, Cycle> {
        let answer = self.store.get::<M>(self, key)?;
        if self.frame.current.is_some() {
            self.reads
                .borrow_mut()
                .push(Read::Memo(answer.query, answer.changed_at));
        }
        Ok(answer.value)
    }

    fn take_reads(&self) -> Vec<Read<S::Input>> {
        std::mem::take(&mut *self.reads.borrow_mut())
    }
}

/// A computed value with the reads it made and the inputs its key implies.
type Computed<V, I> = (V, Vec<Read<I>>, Vec<I>);

struct Answer<V> {
    value: V,
    query: QueryId,
    changed_at: Revision,
}

/// What [`Store::begin`] marked.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Invalidation {
    pub(crate) changed_inputs: usize,
    pub(crate) dirty: usize,
    pub(crate) maybe_dirty: usize,
}

/// A schema's memo tables, their read graph, and the current revision.
pub(crate) struct Store<S: Schema> {
    tables: S::Tables,
    graph: RwLock<Graph<S::Input>>,
    /// The next query id's sequence number ([`Self::allocate`]).
    next: AtomicU32,
    /// The interned inputs, split by hash ([`input_shard`]).
    inputs: Box<[Mutex<Interned<S::Input>>; TABLE_SHARDS]>,
    /// Installs' reads not yet in the graph, split like the tables' shards.
    records: Box<[Mutex<Vec<Record>>; TABLE_SHARDS]>,
    revision: AtomicU32,
    /// A cycle or a verify-reused mismatch: the next [`Self::begin`] starts
    /// from an empty store.
    poisoned: AtomicBool,
    verify_reused: bool,
    report: fn(&str),
}

impl<S: Schema> Store<S> {
    /// An empty store. `report` receives each cycle and verify-reused
    /// mismatch (a shadow report in a build).
    pub(crate) fn new(verify_reused: bool, report: fn(&str)) -> Self {
        Self {
            tables: S::Tables::default(),
            graph: RwLock::default(),
            next: AtomicU32::new(0),
            inputs: Box::new(std::array::from_fn(|_| Mutex::default())),
            records: Box::new(std::array::from_fn(|_| Mutex::default())),
            revision: AtomicU32::new(0),
            poisoned: AtomicBool::new(false),
            verify_reused,
            report,
        }
    }

    pub(crate) fn revision(&self) -> Revision {
        self.revision.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn tables(&self) -> &S::Tables {
        &self.tables
    }

    /// A query of `table`, named for a diagnostic.
    pub(crate) fn describe(&self, table: TableId, query: QueryId) -> String {
        self.tables.table(table).describe(query)
    }

    /// Reports `message` and poisons the store: the next build starts empty.
    pub(crate) fn poison(&self, message: &str) {
        self.poisoned.store(true, Ordering::Release);
        (self.report)(message);
    }

    /// Whether a cycle or mismatch since the last [`Self::begin`] means the
    /// store must not be trusted again.
    pub(crate) fn poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire)
    }

    /// Starts a build in which the answers of `changed` differ from the last
    /// build's: their readers become dirty, and every transitive reader
    /// maybe-dirty. A poisoned store is emptied first. Must run before any
    /// read of the build, on one thread.
    pub(crate) fn begin(&self, changed: &[S::Input]) -> Invalidation {
        if self.poisoned.swap(false, Ordering::AcqRel) {
            self.clear();
        }
        self.revision.fetch_add(1, Ordering::AcqRel);
        self.mark(changed)
    }

    /// Marks the readers of `changed` inside a build, after [`Self::begin`]:
    /// a project pass published new answers for derived inputs
    /// ([`derived`]). Exact only if no reader of them was asked yet in this
    /// build, which [`derived`] enforces.
    pub(crate) fn invalidate(&self, changed: &[S::Input]) -> Invalidation {
        self.mark(changed)
    }

    fn mark(&self, changed: &[S::Input]) -> Invalidation {
        // An input is interned when an install records it, so one no
        // reader records, in the graph or queued, marks nothing.
        let found = changed
            .iter()
            .filter_map(|input| self.interned(input))
            .collect::<Vec<_>>();
        if found.is_empty() {
            return Invalidation::default();
        }
        let mut graph = self.synced_graph();
        graph.flush();
        let mut dirty = BTreeSet::new();
        for query in &found {
            if let Some(node) = graph.node(query) {
                dirty.extend(node.readers.iter().copied());
            }
        }
        for query in &dirty {
            self.raise(&graph, *query, DIRTY);
        }
        let mut maybe = BTreeSet::new();
        let mut queue = dirty.iter().copied().collect::<Vec<_>>();
        while let Some(query) = queue.pop() {
            let Some(node) = graph.node(&query) else {
                continue;
            };
            for reader in &node.readers {
                if !dirty.contains(reader) && maybe.insert(*reader) {
                    self.raise(&graph, *reader, MAYBE_DIRTY);
                    queue.push(*reader);
                }
            }
        }
        Invalidation {
            changed_inputs: found.len(),
            dirty: dirty.len(),
            maybe_dirty: maybe.len(),
        }
    }

    fn raise(&self, graph: &Graph<S::Input>, query: QueryId, state: u8) {
        if let Some(Node {
            kind: NodeKind::Memo(table),
            ..
        }) = graph.node(&query)
        {
            self.tables.table(*table).raise(query, state);
        }
    }

    /// Ends a build: keeps the entries this build asked for and the entries
    /// they read, transitively, and drops the rest with their reverse edges.
    /// Returns each table's stats and the inputs no entry reads any more.
    pub(crate) fn end(&self) -> (Vec<(&'static str, TableStats)>, Vec<S::Input>) {
        let revision = self.revision();
        let tables = self.tables.tables();
        let mut graph = self.synced_graph();
        graph.flush();
        let used = tables
            .iter()
            .flat_map(|table| table.used(revision))
            .collect::<Vec<_>>();
        // Every entry asked for in this build (a first build's are): all are
        // live, and nothing is swept.
        let entries = tables.iter().map(|table| table.len()).sum::<usize>();
        if used.len() < entries {
            let mut live = HashSet::default();
            live.reserve(entries);
            live.extend(used.iter().copied());
            let mut queue = used;
            while let Some(query) = queue.pop() {
                if let Some(node) = graph.node(&query) {
                    // Memo reads only (an input read's fingerprint says so),
                    // each entered once.
                    queue.extend(node.reads.iter().filter_map(|(read, fingerprint)| {
                        (*fingerprint != INPUT_READ && live.insert(*read)).then_some(*read)
                    }));
                }
            }
            let mut lost: [(Vec<u64>, Vec<u64>); TABLE_SHARDS] =
                std::array::from_fn(|_| (Vec::new(), Vec::new()));
            let mut stale = Vec::new();
            for table in &tables {
                for query in table.sweep(&live) {
                    let Some(node) = graph.remove(&query) else {
                        continue;
                    };
                    for (read, _) in &node.reads {
                        lost[graph_shard(*read)].0.push(edge(*read, query));
                    }
                    stale.extend(node.readers);
                }
            }
            graph.apply_edges(lost);
            // Live entries never read a dropped one (live is closed under
            // reads); any other reader must not trust its record.
            for reader in stale {
                self.raise(&graph, reader, DIRTY);
            }
        }
        let mut released = Vec::new();
        // An input is unread only if it lost a reader or was allocated
        // since the last end: every other one kept the reader it had.
        for shard in graph.shards.iter_mut() {
            let candidates = std::mem::take(&mut shard.unread);
            let nodes = &mut shard.nodes;
            for query in candidates {
                let Some(Node {
                    kind: NodeKind::Input(input),
                    readers,
                    ..
                }) = nodes.get(&query)
                else {
                    continue;
                };
                if !readers.is_empty() {
                    continue;
                }
                let input = *input;
                nodes.remove(&query);
                self.inputs[input_shard(&input)]
                    .lock()
                    .expect("memo input lock")
                    .ids
                    .remove(&input);
                released.push(input);
            }
        }
        released.sort_unstable();
        // Recorded reads by table, in one pass over the graph.
        let mut reads = BTreeMap::<TableId, u64>::new();
        for node in graph.nodes() {
            if let NodeKind::Memo(id) = node.kind {
                *reads.entry(id).or_default() += node.reads.len() as u64;
            }
        }
        let stats = tables
            .iter()
            .map(|table| {
                let mut table_stats = table.take_stats();
                table_stats.reads = reads.get(&table.id()).copied().unwrap_or_default();
                (table.name(), table_stats)
            })
            .collect::<Vec<_>>();
        (stats, released)
    }

    /// Empties every table and the read graph. The revision keeps counting.
    pub(crate) fn clear(&self) {
        for table in self.tables.tables() {
            table.clear();
        }
        let mut graph = self.graph.write().expect("memo graph lock");
        *graph = Graph::default();
        for shard in self.inputs.iter() {
            *shard.lock().expect("memo input lock") = Interned::default();
        }
        for shard in self.records.iter() {
            shard.lock().expect("memo record lock").clear();
        }
        self.next.store(0, Ordering::Release);
    }

    /// A new query id in `shard` (modulo [`TABLE_SHARDS`]): a memo key's id
    /// names the table shard that holds it.
    fn allocate(&self, shard: usize) -> QueryId {
        let next = self.next.fetch_add(1, Ordering::Relaxed);
        QueryId(
            next.checked_mul(TABLE_SHARDS as u32)
                .and_then(|id| id.checked_add(shard as u32))
                .expect("fewer than 2^28 queries"),
        )
    }

    /// The interned id of `input`, if a recorded read named it.
    fn interned(&self, input: &S::Input) -> Option<QueryId> {
        self.inputs[input_shard(input)]
            .lock()
            .expect("memo input lock")
            .ids
            .get(input)
            .copied()
    }

    /// The id of `input`, interned if no read named it yet.
    fn intern(&self, input: S::Input) -> QueryId {
        let shard = input_shard(&input);
        let mut interned = self.inputs[shard].lock().expect("memo input lock");
        if let Some(id) = interned.ids.get(&input) {
            return *id;
        }
        let id = self.allocate(shard);
        interned.ids.insert(input, id);
        interned.fresh.push((id, input));
        id
    }

    /// The graph lock, with every input interned and every install
    /// recorded since the last call moved into the graph (each record in
    /// its shard's order: a query's records are all in its own shard).
    /// Locks are taken graph, then input or record shard.
    fn synced_graph(&self) -> parking_lot::RwLockWriteGuard<'_, Graph<S::Input>> {
        let mut graph = self.graph.write().expect("memo graph lock");
        // Shard by shard: an input's id and a record's query are in the
        // graph shard of their own shard.
        each_shard(
            graph
                .shards
                .iter_mut()
                .zip(self.inputs.iter().zip(self.records.iter()))
                .collect(),
            |(shard, (inputs, records))| {
                let fresh = std::mem::take(&mut inputs.lock().expect("memo input lock").fresh);
                let records = std::mem::take(&mut *records.lock().expect("memo record lock"));
                shard.nodes.reserve(fresh.len() + records.len());
                for (id, input) in fresh {
                    shard.nodes.insert(id, Node::new(NodeKind::Input(input)));
                    shard.unread.push(id);
                }
                for (query, table, reads) in records {
                    // Swept between the key's interning and now, or never in
                    // the graph: (re-)register.
                    shard
                        .nodes
                        .entry(query)
                        .or_insert_with(|| Node::new(NodeKind::Memo(table)));
                    shard.record(query, reads);
                }
            },
        );
        graph
    }

    /// The answer for `key` to a top-level reader (outside every memo). A
    /// cycle answers [`Memo::on_cycle`], is reported, and poisons the store.
    #[cfg(test)]
    pub(crate) fn answer<M: Memo<S>>(&self, world: &S::World<'_>, key: &M::Key) -> M::Value {
        self.answer_read::<M>(world, key).0
    }

    /// [`Self::answer`], with the query and the revision its value last
    /// changed in (`None` for a cycle's fail-closed answer): what a reader
    /// outside every memo records ([`crate::base::reads::MemoRead`]).
    pub(crate) fn answer_read<M: Memo<S>>(
        &self,
        world: &S::World<'_>,
        key: &M::Key,
    ) -> (M::Value, Option<(QueryId, Revision)>) {
        let root = Ctx::root(world, self);
        match self.get::<M>(&root, key) {
            Ok(answer) => (answer.value, Some((answer.query, answer.changed_at))),
            Err(cycle) => {
                self.poisoned.store(true, Ordering::Release);
                (self.report)(&format!(
                    "IR memo cycle: {} (failed closed)",
                    cycle.path.join(" -> ")
                ));
                (M::on_cycle(key), None)
            }
        }
    }

    /// The revision the entry `query` of `table` last changed in, brought up
    /// to date for a reader outside every memo; `None` when the entry is
    /// gone or its computation met a cycle.
    pub(crate) fn changed_at(
        &self,
        world: &S::World<'_>,
        table: TableId,
        query: QueryId,
    ) -> Option<Revision> {
        let table = self.tables.table(table);
        // A clean entry answers as it is (verify-reused mode recomputes it).
        if !self.verify_reused
            && let Some(changed_at) = table.clean_changed_at(query, self.revision())
        {
            return Some(changed_at);
        }
        let root = Ctx::root(world, self);
        table.ensure(self, &root, query).ok().flatten()
    }

    fn query_of<M: Memo<S>>(&self, key: &M::Key) -> QueryId {
        let table = M::table(&self.tables);
        let shard = Table::<S, M>::key_shard(key);
        if let Some(query) = table.data[shard]
            .read()
            .expect("memo table lock")
            .ids
            .get(key)
        {
            return *query;
        }
        let mut data = table.data[shard].write().expect("memo table lock");
        if let Some(query) = data.ids.get(key) {
            return *query;
        }
        // Its graph node is created when an install records it.
        let query = self.allocate(shard);
        data.ids.insert(key.clone(), query);
        data.keys.insert(query, key.clone());
        query
    }

    fn get<M: Memo<S>>(
        &self,
        parent: &Ctx<'_, '_, S>,
        key: &M::Key,
    ) -> Result<Answer<M::Value>, Cycle> {
        let table = M::table(&self.tables);
        let revision = self.revision();
        let look = |entry: &Entry<M::Value>| {
            let state = entry.state.load(Ordering::Acquire);
            let first_use = entry.used_at.swap(revision, Ordering::AcqRel) != revision;
            (
                state,
                first_use,
                entry.value.clone(),
                entry.changed_at,
                entry.computed_at,
            )
        };
        // An interned key's id and its entry under one read lock: the path
        // of every answer asked again.
        let (interned, mut first) = {
            let data = table.data[Table::<S, M>::key_shard(key)]
                .read()
                .expect("memo table lock");
            match data.ids.get(key) {
                Some(query) => (Some(*query), data.entries.get(query).map(look)),
                None => (None, None),
            }
        };
        let query = interned.unwrap_or_else(|| self.query_of::<M>(key));
        loop {
            let found = match first.take() {
                Some(found) => Some(found),
                None => {
                    let data = table.shard(query).read().expect("memo table lock");
                    data.entries.get(&query).map(look)
                }
            };
            match found {
                Some((CLEAN, first_use, value, changed_at, computed_at)) => {
                    if computed_at != revision && first_use {
                        AtomicStats::bump(&table.stats.reused);
                    }
                    if self.verify_reused && computed_at != revision {
                        return self.verify_reuse::<M>(parent, query, key, value, changed_at);
                    }
                    return Ok(Answer {
                        value,
                        query,
                        changed_at,
                    });
                }
                Some((MAYBE_DIRTY, ..)) => {
                    if parent.frame.contains(query) {
                        return Err(self.cycle::<M>(parent, key));
                    }
                    if self.reads_unchanged(parent, query, M::ID)? {
                        let data = table.shard(query).read().expect("memo table lock");
                        // Only this verification lowers maybe-dirty; the
                        // next turn answers the clean entry (its use is
                        // already counted, so it is not counted as reused).
                        if let Some(entry) = data.entries.get(&query)
                            && entry
                                .state
                                .compare_exchange(
                                    MAYBE_DIRTY,
                                    CLEAN,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                        {
                            AtomicStats::bump(&table.stats.verified);
                        }
                        continue;
                    }
                    return self.recompute::<M>(parent, query, key);
                }
                _ => return self.recompute::<M>(parent, query, key),
            }
        }
    }

    fn cycle<M: Memo<S>>(&self, parent: &Ctx<'_, '_, S>, key: &M::Key) -> Cycle {
        AtomicStats::bump(&M::table(&self.tables).stats.cycles);
        let mut path = vec![format!("{} {key:?}", M::NAME)];
        let mut frame = Some(&parent.frame);
        while let Some(current) = frame {
            if let Some((query, table)) = current.current {
                path.push(self.tables.table(table).describe(query));
            }
            frame = current.parent;
        }
        path.reverse();
        Cycle { path }
    }

    /// Whether every memo `query` read answers with the revision recorded,
    /// each brought up to date in recorded order (stopping at the first that
    /// changed: a recomputation may no longer read the rest). Input reads
    /// need no check: a changed input made the entry dirty. A read that is
    /// clean answers as it is, without its key or its value cloned (verify-
    /// reused mode recomputes it, as in [`Self::changed_at`]).
    fn reads_unchanged(
        &self,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
        table: TableId,
    ) -> Result<bool, Cycle> {
        let revision = self.revision();
        let reads = {
            let graph = self.graph.read().expect("memo graph lock");
            let Some(node) = graph.node(&query) else {
                return Ok(false);
            };
            node.reads
                .iter()
                .map(|(read, fingerprint)| {
                    (*read, *fingerprint, graph.node(read).map(|node| node.kind))
                })
                .collect::<Vec<_>>()
        };
        let verifying = parent.child((query, table), Vec::new());
        for (read, fingerprint, kind) in reads {
            match kind {
                None => return Ok(false),
                Some(NodeKind::Input(_)) => {}
                Some(NodeKind::Memo(dependency)) => {
                    let dependency = self.tables.table(dependency);
                    let clean = if self.verify_reused {
                        None
                    } else {
                        dependency.clean_changed_at(read, revision)
                    };
                    let current = match clean {
                        Some(changed_at) => Some(changed_at),
                        None => dependency.ensure(self, &verifying, read)?,
                    };
                    if current.map(u64::from) != Some(fingerprint) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    fn compute<M: Memo<S>>(
        &self,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
        key: &M::Key,
    ) -> Result<Computed<M::Value, S::Input>, Cycle> {
        if parent.frame.contains(query) {
            return Err(self.cycle::<M>(parent, key));
        }
        let implied = M::implied(key);
        let ctx = parent.child((query, M::ID), implied.clone());
        // The memo records its own reads; none reaches a stage chunk the
        // computation happens to run inside ([`crate::base::reads`]).
        let (value, unpublished) =
            derived::computing(|| crate::base::reads::suppress(|| M::compute(&ctx, key)));
        if unpublished {
            // Undecided before its pass publishes ([`derived::deferring`]):
            // nothing is installed, and the proof asking counts it as moved.
            return Err(Cycle {
                path: vec![format!(
                    "{} {key:?}: reads a derived table not published in this build",
                    M::NAME
                )],
            });
        }
        Ok((value?, ctx.take_reads(), implied))
    }

    fn recompute<M: Memo<S>>(
        &self,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
        key: &M::Key,
    ) -> Result<Answer<M::Value>, Cycle> {
        let (value, reads, implied) = self.compute::<M>(parent, query, key)?;
        Ok(self.install::<M>(query, key, value, reads, &implied))
    }

    /// Stores a computed value with its reads. A value equal to the old one
    /// keeps the old revision (early cutoff).
    fn install<M: Memo<S>>(
        &self,
        query: QueryId,
        key: &M::Key,
        value: M::Value,
        reads: Vec<Read<S::Input>>,
        implied: &[S::Input],
    ) -> Answer<M::Value> {
        let revision = self.revision();
        let table = M::table(&self.tables);
        // Each input is recorded once ([`Ctx::record_input`]) and an implied
        // one never, so only a memo read can repeat: the first is kept.
        let memo_reads = reads
            .iter()
            .filter(|read| matches!(read, Read::Memo(..)))
            .count();
        let reads = if memo_reads > 1 {
            let mut seen = HashSet::default();
            implied
                .iter()
                .map(|input| Read::Input(*input))
                .chain(reads)
                .filter(|read| match read {
                    Read::Input(_) => true,
                    Read::Memo(id, _) => seen.insert(*id),
                })
                .collect::<Vec<_>>()
        } else {
            implied
                .iter()
                .map(|input| Read::Input(*input))
                .chain(reads)
                .collect::<Vec<_>>()
        };
        // Interned before the table's write lock is taken, so a shard's
        // writer does not wait on input shards (an input interned for an
        // install that then finds the entry computed is released unread at
        // the end). Queued for the graph ([`Self::synced_graph`]): an install
        // takes an input shard's lock per read and its record shard's, never
        // the graph's.
        let recorded = reads
            .into_iter()
            .map(|read| match read {
                Read::Input(input) => (self.intern(input), INPUT_READ),
                Read::Memo(id, changed_at) => (id, u64::from(changed_at)),
            })
            .collect::<Vec<_>>();
        // The record and the value change together, under the table's write
        // lock.
        let mut data = table.shard(query).write().expect("memo table lock");
        if data.entries.get(&query).is_some_and(|entry| {
            entry.computed_at == revision && entry.state.load(Ordering::Acquire) == CLEAN
        }) {
            // Another thread computed it in this build: its value is the
            // same, by determinism, and so are its reads.
            let entry = &data.entries[&query];
            debug_assert_eq!(entry.value, value, "{} {key:?}: nondeterministic", M::NAME);
            return Answer {
                value: entry.value.clone(),
                query,
                changed_at: entry.changed_at,
            };
        }
        self.records[query.0 as usize % TABLE_SHARDS]
            .lock()
            .expect("memo record lock")
            .push((query, M::ID, recorded));
        if !data.ids.contains_key(key) {
            data.ids.insert(key.clone(), query);
            data.keys.insert(query, key.clone());
        }
        match data.entries.get_mut(&query) {
            Some(entry) => {
                if entry.value == value {
                    AtomicStats::bump(&table.stats.unchanged);
                } else {
                    AtomicStats::bump(&table.stats.changed);
                    entry.value = value;
                    entry.changed_at = revision;
                }
                entry.computed_at = revision;
                entry.used_at.store(revision, Ordering::Release);
                entry.state.store(CLEAN, Ordering::Release);
                Answer {
                    value: entry.value.clone(),
                    query,
                    changed_at: entry.changed_at,
                }
            }
            None => {
                AtomicStats::bump(&table.stats.fresh);
                data.entries.insert(
                    query,
                    Entry {
                        value: value.clone(),
                        state: AtomicU8::new(CLEAN),
                        changed_at: revision,
                        computed_at: revision,
                        used_at: AtomicU32::new(revision),
                        reuse_checked_at: AtomicU32::new(0),
                    },
                );
                Answer {
                    value,
                    query,
                    changed_at: revision,
                }
            }
        }
    }

    /// Verify-reused mode: recomputes a reused answer and compares. A
    /// difference means the entry's record missed a read: it is reported
    /// naming the table and key, the store is poisoned, and the fresh value
    /// is installed and served.
    fn verify_reuse<M: Memo<S>>(
        &self,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
        key: &M::Key,
        value: M::Value,
        changed_at: Revision,
    ) -> Result<Answer<M::Value>, Cycle> {
        let revision = self.revision();
        let table = M::table(&self.tables);
        let first = {
            let data = table.shard(query).read().expect("memo table lock");
            data.entries.get(&query).is_some_and(|entry| {
                entry.reuse_checked_at.swap(revision, Ordering::AcqRel) != revision
            })
        };
        if !first {
            return Ok(Answer {
                value,
                query,
                changed_at,
            });
        }
        let (fresh, reads, implied) = self.compute::<M>(parent, query, key)?;
        if fresh == value {
            return Ok(Answer {
                value,
                query,
                changed_at,
            });
        }
        AtomicStats::bump(&table.stats.mismatches);
        self.poisoned.store(true, Ordering::Release);
        let recorded = {
            let graph = self.graph.read().expect("memo graph lock");
            graph
                .node(&query)
                .map(|node| {
                    node.reads
                        .iter()
                        .map(|(read, _)| match graph.node(read).map(|node| node.kind) {
                            Some(NodeKind::Input(input)) => format!("{input:?}"),
                            Some(NodeKind::Memo(table)) => self.tables.table(table).describe(*read),
                            None => format!("{read:?} (dropped)"),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        (self.report)(&format!(
            "IR memo verify-reused mismatch: table `{}` key {key:?}: the reused answer {value:?} differs from a fresh {fresh:?}; a read is missing from its record ({} recorded: {})",
            M::NAME,
            recorded.len(),
            recorded.join(", ")
        ));
        // Fail closed: serve and keep the fresh value.
        {
            let mut data = table.shard(query).write().expect("memo table lock");
            if let Some(entry) = data.entries.get_mut(&query) {
                entry.state.store(DIRTY, Ordering::Release);
            }
        }
        Ok(self.install::<M>(query, key, fresh, reads, &implied))
    }
}
