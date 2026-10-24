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
    /// in; an input read records 0, since its changes are pushed by
    /// [`Store::begin`] and never compared.
    reads: Vec<(QueryId, u64)>,
    /// Exact once [`Graph::flush`] has applied the pending edges.
    readers: BTreeSet<QueryId>,
}

struct Graph<I> {
    next: u32,
    inputs: HashMap<I, QueryId>,
    nodes: HashMap<QueryId, Node<I>>,
    /// Nodes that lost a reader or were allocated since the last
    /// [`Store::end`]: the only inputs it can find unread.
    unread: Vec<QueryId>,
    /// Reverse-edge changes [`Self::record`] has not applied yet, in record
    /// order: each reader with the reads it dropped. Only [`Store::mark`] and
    /// [`Store::end`] read `readers`, and both flush first, so the reverse
    /// map they see is the one recording it eagerly would have built, while
    /// an install holds the graph lock only to intern and push.
    pending: Vec<(QueryId, Vec<(QueryId, u64)>)>,
}

impl<I> Default for Graph<I> {
    fn default() -> Self {
        Self {
            next: 0,
            inputs: HashMap::default(),
            nodes: HashMap::default(),
            unread: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl<I: Copy + Eq + Hash> Graph<I> {
    fn allocate(&mut self, kind: NodeKind<I>) -> QueryId {
        self.allocate_in(kind, 0)
    }

    /// A new query whose id is `shard` modulo [`TABLE_SHARDS`]: a memo key's
    /// id names the table shard that holds it.
    fn allocate_in(&mut self, kind: NodeKind<I>, shard: usize) -> QueryId {
        let id = QueryId(
            self.next
                .checked_mul(TABLE_SHARDS as u32)
                .and_then(|id| id.checked_add(shard as u32))
                .expect("fewer than 2^28 queries"),
        );
        self.next = self.next.checked_add(1).expect("fewer than 2^28 queries");
        self.nodes.insert(
            id,
            Node {
                kind,
                reads: Vec::new(),
                readers: BTreeSet::new(),
            },
        );
        id
    }

    fn input(&mut self, input: I) -> QueryId {
        if let Some(id) = self.inputs.get(&input) {
            return *id;
        }
        let id = self.allocate(NodeKind::Input(input));
        self.inputs.insert(input, id);
        self.unread.push(id);
        id
    }

    /// Replaces `query`'s recorded reads; the reverse map follows at the
    /// next [`Self::flush`].
    fn record(&mut self, query: QueryId, reads: Vec<(QueryId, u64)>) {
        let previous = self
            .nodes
            .get_mut(&query)
            .map(|node| std::mem::take(&mut node.reads))
            .unwrap_or_default();
        if let Some(node) = self.nodes.get_mut(&query) {
            node.reads = reads;
        }
        self.pending.push((query, previous));
    }

    /// Applies the pending reverse-edge changes, in record order. Each
    /// reader gains its reads as they are now: a reader recorded twice since
    /// the last flush drops each earlier set and ends with the last one, the
    /// map recording eagerly builds. Nodes are removed only by `Store::end`
    /// and `Store::clear`, which flush or drop the pending changes first, so
    /// a read present at its record is present here.
    fn flush(&mut self) {
        for (query, previous) in std::mem::take(&mut self.pending) {
            for (read, _) in previous {
                if let Some(node) = self.nodes.get_mut(&read) {
                    node.readers.remove(&query);
                    self.unread.push(read);
                }
            }
            let gained = self
                .nodes
                .get(&query)
                .map(|node| node.reads.iter().map(|(read, _)| *read).collect::<Vec<_>>())
                .unwrap_or_default();
            for read in gained {
                if let Some(node) = self.nodes.get_mut(&read) {
                    node.readers.insert(query);
                }
            }
        }
    }
}

/// One recorded read, before its input is interned.
enum Read<I> {
    Input(I),
    Memo(QueryId, Revision),
}

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

    /// Records a read of `input`.
    fn record_input(&self, input: S::Input) {
        if self.frame.current.is_none() || self.implied.contains(&input) {
            return;
        }
        if self.recorded.borrow_mut().insert(input) {
            self.reads.borrow_mut().push(Read::Input(input));
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
    graph: Mutex<Graph<S::Input>>,
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
            graph: Mutex::default(),
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
        let mut graph = self.graph.lock().expect("memo graph lock");
        // An input is interned when an install records it, so one no
        // reader records, flushed or pending, marks nothing.
        if !changed.iter().any(|input| graph.inputs.contains_key(input)) {
            return Invalidation::default();
        }
        graph.flush();
        let mut dirty = BTreeSet::new();
        let mut found = 0;
        for input in changed {
            let Some(query) = graph.inputs.get(input) else {
                continue;
            };
            found += 1;
            if let Some(node) = graph.nodes.get(query) {
                dirty.extend(node.readers.iter().copied());
            }
        }
        for query in &dirty {
            self.raise(&graph, *query, DIRTY);
        }
        let mut maybe = BTreeSet::new();
        let mut queue = dirty.iter().copied().collect::<Vec<_>>();
        while let Some(query) = queue.pop() {
            let Some(node) = graph.nodes.get(&query) else {
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
            changed_inputs: found,
            dirty: dirty.len(),
            maybe_dirty: maybe.len(),
        }
    }

    fn raise(&self, graph: &Graph<S::Input>, query: QueryId, state: u8) {
        if let Some(Node {
            kind: NodeKind::Memo(table),
            ..
        }) = graph.nodes.get(&query)
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
        let mut graph = self.graph.lock().expect("memo graph lock");
        graph.flush();
        let mut live = HashSet::default();
        let mut queue = tables
            .iter()
            .flat_map(|table| table.used(revision))
            .collect::<Vec<_>>();
        while let Some(query) = queue.pop() {
            if !live.insert(query) {
                continue;
            }
            if let Some(node) = graph.nodes.get(&query) {
                queue.extend(node.reads.iter().filter_map(|(read, _)| {
                    matches!(
                        graph.nodes.get(read).map(|node| node.kind),
                        Some(NodeKind::Memo(_))
                    )
                    .then_some(*read)
                }));
            }
        }
        for table in &tables {
            for query in table.sweep(&live) {
                let Some(node) = graph.nodes.remove(&query) else {
                    continue;
                };
                for (read_id, _) in &node.reads {
                    if let Some(read) = graph.nodes.get_mut(read_id) {
                        read.readers.remove(&query);
                        graph.unread.push(*read_id);
                    }
                }
                // Live entries never read a dropped one (live is closed
                // under reads); any other reader must not trust its record.
                for reader in &node.readers {
                    self.raise(&graph, *reader, DIRTY);
                }
            }
        }
        let mut released = Vec::new();
        // An input is unread only if it lost a reader or was allocated
        // since the last end: every other one kept the reader it had.
        let candidates = std::mem::take(&mut graph.unread);
        let Graph { inputs, nodes, .. } = &mut *graph;
        for query in candidates {
            let Some(Node {
                kind: NodeKind::Input(input),
                readers,
                ..
            }) = nodes.get(&query)
            else {
                continue;
            };
            if readers.is_empty() {
                let input = *input;
                nodes.remove(&query);
                inputs.remove(&input);
                released.push(input);
            }
        }
        released.sort_unstable();
        // Recorded reads by table, in one pass over the graph.
        let mut reads = BTreeMap::<TableId, u64>::new();
        for node in nodes.values() {
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
        *self.graph.lock().expect("memo graph lock") = Graph::default();
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
        // Locks are taken table first, then graph, everywhere a build reads.
        let mut data = table.data[shard].write().expect("memo table lock");
        if let Some(query) = data.ids.get(key) {
            return *query;
        }
        let query = self
            .graph
            .lock()
            .expect("memo graph lock")
            .allocate_in(NodeKind::Memo(M::ID), shard);
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
    /// need no check: a changed input made the entry dirty.
    fn reads_unchanged(
        &self,
        parent: &Ctx<'_, '_, S>,
        query: QueryId,
        table: TableId,
    ) -> Result<bool, Cycle> {
        let reads = {
            let graph = self.graph.lock().expect("memo graph lock");
            let Some(node) = graph.nodes.get(&query) else {
                return Ok(false);
            };
            node.reads
                .iter()
                .map(|(read, fingerprint)| {
                    (
                        *read,
                        *fingerprint,
                        graph.nodes.get(read).map(|node| node.kind),
                    )
                })
                .collect::<Vec<_>>()
        };
        let verifying = parent.child((query, table), Vec::new());
        for (read, fingerprint, kind) in reads {
            match kind {
                None => return Ok(false),
                Some(NodeKind::Input(_)) => {}
                Some(NodeKind::Memo(dependency)) => {
                    let current = self
                        .tables
                        .table(dependency)
                        .ensure(self, &verifying, read)?;
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
        let mut seen = HashSet::default();
        let reads = implied
            .iter()
            .map(|input| Read::Input(*input))
            .chain(reads)
            .filter(|read| match read {
                Read::Input(_) => true,
                Read::Memo(id, _) => seen.insert(*id),
            })
            .collect::<Vec<_>>();
        // The reads and the value change together, under the table's write
        // lock: a verifier sees old reads with the old value or new reads
        // with the new one, never new reads with the old value.
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
        {
            let mut graph = self.graph.lock().expect("memo graph lock");
            let recorded = reads
                .into_iter()
                .map(|read| match read {
                    Read::Input(input) => (graph.input(input), 0),
                    Read::Memo(id, changed_at) => (id, u64::from(changed_at)),
                })
                .collect::<Vec<_>>();
            // Swept between the key's interning and now: re-register.
            graph.nodes.entry(query).or_insert_with(|| Node {
                kind: NodeKind::Memo(M::ID),
                reads: Vec::new(),
                readers: BTreeSet::new(),
            });
            graph.record(query, recorded);
        }
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
            let graph = self.graph.lock().expect("memo graph lock");
            graph
                .nodes
                .get(&query)
                .map(|node| {
                    node.reads
                        .iter()
                        .map(
                            |(read, _)| match graph.nodes.get(read).map(|node| node.kind) {
                                Some(NodeKind::Input(input)) => format!("{input:?}"),
                                Some(NodeKind::Memo(table)) => {
                                    self.tables.table(table).describe(*read)
                                }
                                None => format!("{read:?} (dropped)"),
                            },
                        )
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
