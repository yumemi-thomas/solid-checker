//! The memo engine over a toy schema: cells are inputs, tables read cells
//! and each other. Checks invalidation completeness against fresh stores,
//! early cutoff, cycle detection, verify-reused mode, sweeping, implied
//! inputs, concurrent readers, and that only canonical answers compile.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use super::*;

struct Toy;

impl Schema for Toy {
    type Input = u32;
    type Tables = ToyTables;
    type World<'w> = ToyWorld;
}

/// A random DAG node: the cells it reads and the nodes (lower ids) it reads.
#[derive(Clone, Default)]
struct DagNode {
    cells: Vec<u32>,
    deps: Vec<u32>,
}

#[derive(Default)]
struct ToyWorld {
    cells: BTreeMap<u32, i64>,
    dag: Vec<DagNode>,
    /// Every computation run, as `table key`.
    computed: Mutex<Vec<String>>,
}

impl ToyWorld {
    fn with_cells(cells: &[(u32, i64)]) -> Self {
        Self {
            cells: cells.iter().copied().collect(),
            ..Self::default()
        }
    }

    fn take_computed(&self) -> Vec<String> {
        let mut computed = std::mem::take(&mut *self.computed.lock().unwrap());
        computed.sort();
        computed
    }
}

impl World<Toy> for ToyWorld {
    fn fingerprint(&self, input: u32) -> u64 {
        self.cells.get(&input).copied().unwrap_or(0).cast_unsigned()
    }
}

impl Ctx<'_, '_, Toy> {
    fn cell(&self, cell: u32) -> i64 {
        self.record_input(cell);
        self.world().cells.get(&cell).copied().unwrap_or(0)
    }

    /// A read that bypasses recording: the bug verify-reused mode catches.
    fn unrecorded_cell(&self, cell: u32) -> i64 {
        self.world().cells.get(&cell).copied().unwrap_or(0)
    }

    fn note(&self, entry: String) {
        self.world().computed.lock().unwrap().push(entry);
    }
}

macro_rules! toy_tables {
    ($($id:literal $field:ident: $memo:ident),* $(,)?) => {
        #[derive(Default)]
        struct ToyTables {
            $($field: Table<Toy, $memo>,)*
        }

        impl TableSet<Toy> for ToyTables {
            fn table(&self, id: TableId) -> &dyn ErasedTable<Toy> {
                match id.0 {
                    $($id => &self.$field,)*
                    _ => unreachable!(),
                }
            }

            fn tables(&self) -> Vec<&dyn ErasedTable<Toy>> {
                vec![$(&self.$field),*]
            }
        }
    };
}

toy_tables! {
    0 sum: Sum,
    1 parity: Parity,
    2 top: Top,
    3 node: Node,
    4 cyclic: Cyclic,
    5 leaky: Leaky,
    6 own: Own,
}

macro_rules! memo {
    ($memo:ident, $id:literal, $field:ident, $key:ty, $value:ty, |$ctx:ident, $k:ident| $body:expr) => {
        struct $memo;
        impl Memo<Toy> for $memo {
            type Key = $key;
            type Value = $value;
            const NAME: &'static str = stringify!($field);
            const ID: TableId = TableId($id);
            fn table(tables: &ToyTables) -> &Table<Toy, Self> {
                &tables.$field
            }
            fn compute($ctx: &Ctx<'_, '_, Toy>, $k: &$key) -> Result<$value, Cycle> {
                $ctx.note(format!("{} {:?}", stringify!($field), $k));
                $body
            }
            fn on_cycle(_: &$key) -> $value {
                <$value>::default()
            }
        }
    };
}

memo!(Sum, 0, sum, (u32, u32), i64, |ctx, key| Ok(
    ctx.cell(key.0) + ctx.cell(key.1)
));
memo!(Parity, 1, parity, u32, i64, |ctx, key| Ok(ctx
    .memo::<Sum>(&(*key, key + 1))?
    .rem_euclid(2)));
memo!(Top, 2, top, u32, i64, |ctx, key| Ok(ctx
    .memo::<Parity>(key)?
    + 10 * ctx.memo::<Parity>(&(key + 1))?));
memo!(Node, 3, node, u32, i64, |ctx, key| {
    let node = ctx.world().dag[*key as usize].clone();
    let mut total = 0;
    for cell in node.cells {
        total += ctx.cell(cell);
    }
    for dep in node.deps {
        total += ctx.memo::<Node>(&dep)?;
    }
    // Coarse on purpose: many changes below cut off here.
    Ok(total.rem_euclid(5))
});
memo!(Cyclic, 4, cyclic, u32, i64, |ctx, key| Ok(
    1 + ctx.memo::<Cyclic>(&((key + 1) % 3))?
));
memo!(Leaky, 5, leaky, u32, i64, |ctx, key| Ok(
    ctx.unrecorded_cell(*key)
));

/// A table whose key names its own cell: the read of it is implied.
struct Own;
impl Memo<Toy> for Own {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "own";
    const ID: TableId = TableId(6);
    fn table(tables: &ToyTables) -> &Table<Toy, Self> {
        &tables.own
    }
    fn compute(ctx: &Ctx<'_, '_, Toy>, key: &u32) -> Result<i64, Cycle> {
        ctx.note(format!("own {key}"));
        let own = ctx.cell(*key);
        assert!(ctx.reads.borrow().is_empty(), "an implied read is free");
        Ok(own * 2)
    }
    fn on_cycle(_: &u32) -> i64 {
        0
    }
    fn implied(key: &u32) -> Vec<u32> {
        vec![*key]
    }
}

thread_local! {
    static REPORTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn capture(message: &str) {
    REPORTS.with(|reports| reports.borrow_mut().push(message.to_owned()));
}

fn take_reports() -> Vec<String> {
    REPORTS.with(|reports| std::mem::take(&mut *reports.borrow_mut()))
}

fn store() -> Store<Toy> {
    Store::new(false, capture)
}

/// The cells whose values differ between two worlds.
fn changed(before: &ToyWorld, after: &ToyWorld) -> Vec<u32> {
    before
        .cells
        .keys()
        .chain(after.cells.keys())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|cell| before.cells.get(cell) != after.cells.get(cell))
        .collect()
}

fn fresh<M: Memo<Toy>>(world: &ToyWorld, key: &M::Key) -> M::Value {
    let store = store();
    store.begin(&[]);
    store.answer::<M>(world, key)
}

#[test]
fn early_cutoff_stops_propagation() {
    let store = store();
    let world = ToyWorld::with_cells(&[(0, 1), (1, 2), (2, 3)]);
    store.begin(&[]);
    assert_eq!(store.answer::<Top>(&world, &0), 1 + 10);
    assert_eq!(
        world.take_computed().len(),
        5,
        "top, two parities, two sums"
    );
    store.end();

    // Same parity of 0 + 1: the sum reruns, the parity reruns to the same
    // value, and the top answer is verified without running.
    let next = ToyWorld::with_cells(&[(0, 3), (1, 2), (2, 3)]);
    let marked = store.begin(&changed(&world, &next));
    assert_eq!((marked.dirty, marked.maybe_dirty), (1, 2));
    assert_eq!(store.answer::<Top>(&next, &0), 11);
    assert_eq!(next.take_computed(), ["parity 0", "sum (0, 1)"]);
    let (stats, _) = store.end();
    let stats = stats.into_iter().collect::<BTreeMap<_, _>>();
    assert_eq!(stats["sum"].changed, 1);
    assert_eq!(stats["parity"].unchanged, 1);
    assert_eq!(stats["top"].verified, 1);
    assert_eq!(stats["top"].changed + stats["top"].unchanged, 0);

    // A parity flip reaches the top.
    let last = ToyWorld::with_cells(&[(0, 4), (1, 2), (2, 3)]);
    store.begin(&changed(&next, &last));
    assert_eq!(store.answer::<Top>(&last, &0), 10);
    assert_eq!(last.take_computed(), ["parity 0", "sum (0, 1)", "top 0"]);
}

#[test]
fn an_unchanged_build_reuses_everything() {
    let store = store();
    let world = ToyWorld::with_cells(&[(0, 1), (1, 2), (2, 3)]);
    store.begin(&[]);
    store.answer::<Top>(&world, &0);
    store.end();
    world.take_computed();
    let marked = store.begin(&[]);
    assert_eq!(marked, Invalidation::default());
    assert_eq!(store.answer::<Top>(&world, &0), 11);
    assert!(world.take_computed().is_empty());
    let (stats, _) = store.end();
    assert_eq!(stats.iter().map(|(_, stats)| stats.reused).sum::<u64>(), 1);
    // The sweep keeps what the reused answer read: a later change still
    // reaches it through its reads.
    let next = ToyWorld::with_cells(&[(0, 1), (1, 3), (2, 3)]);
    store.begin(&changed(&world, &next));
    assert_eq!(store.answer::<Top>(&next, &0), 0, "both parities flipped");
    assert_eq!(fresh::<Top>(&next, &0), 0);
}

/// Mulberry32, as the edit harness uses.
struct Random(u32);

impl Random {
    fn below(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) % n.max(1)
    }
}

fn random_world(random: &mut Random, nodes: u32, cells: u32) -> ToyWorld {
    let dag = (0..nodes)
        .map(|node| DagNode {
            cells: (0..random.below(3)).map(|_| random.below(cells)).collect(),
            deps: (0..random.below(3))
                .filter(|_| node > 0)
                .map(|_| random.below(node))
                .collect(),
        })
        .collect();
    ToyWorld {
        cells: (0..cells)
            .map(|cell| (cell, i64::from(random.below(4))))
            .collect(),
        dag,
        computed: Mutex::default(),
    }
}

#[test]
fn invalidation_is_complete_on_random_graphs() {
    let mut reused = 0;
    let mut computed = 0;
    for seed in 1..=300 {
        let mut random = Random(seed);
        let nodes = 4 + random.below(12);
        let cells = 2 + random.below(8);
        let mut world = random_world(&mut random, nodes, cells);
        let store = store();
        store.begin(&[]);
        for step in 0..12 {
            for node in 0..nodes {
                if random.below(3) > 0 {
                    assert_eq!(
                        store.answer::<Node>(&world, &node),
                        fresh::<Node>(&world, &node),
                        "seed {seed} step {step} node {node}"
                    );
                }
            }
            let (stats, _) = store.end();
            for (_, stats) in stats {
                reused += stats.reused + stats.verified;
                computed += stats.fresh + stats.unchanged + stats.changed;
            }
            let mut next = ToyWorld {
                cells: world.cells.clone(),
                dag: world.dag.clone(),
                computed: Mutex::default(),
            };
            for _ in 0..=random.below(2) {
                let cell = random.below(cells);
                next.cells.insert(cell, i64::from(random.below(4)));
            }
            // An unchanged input reported changed only costs work.
            let mut reported = changed(&world, &next);
            if random.below(4) == 0 {
                reported.push(random.below(cells));
            }
            store.begin(&reported);
            world = next;
        }
    }
    eprintln!("random graphs: {reused} reused, {computed} computed");
    assert!(reused > computed / 4, "{reused} reused of {computed}");
}

// A native-only test of concurrent readers; it never runs on wasm32-wasip1.
#[test]
#[allow(clippy::disallowed_methods)]
fn concurrent_readers_agree_with_a_fresh_store() {
    for seed in 1..=40 {
        let mut random = Random(seed);
        let mut world = random_world(&mut random, 24, 6);
        let store = store();
        store.begin(&[]);
        for _ in 0..6 {
            std::thread::scope(|scope| {
                for thread in 0..4 {
                    let (store, world) = (&store, &world);
                    scope.spawn(move || {
                        for node in (0..24).rev().skip(thread) {
                            assert_eq!(
                                store.answer::<Node>(world, &node),
                                fresh::<Node>(world, &node),
                                "seed {seed} node {node}"
                            );
                        }
                    });
                }
            });
            store.end();
            let mut next = ToyWorld {
                cells: world.cells.clone(),
                dag: world.dag.clone(),
                computed: Mutex::default(),
            };
            next.cells
                .insert(random.below(6), i64::from(random.below(4)));
            store.begin(&changed(&world, &next));
            world = next;
        }
    }
}

#[test]
fn a_cycle_fails_closed_and_poisons_the_store() {
    let store = store();
    let world = ToyWorld::default();
    store.begin(&[]);
    take_reports();
    assert_eq!(store.answer::<Cyclic>(&world, &0), 0, "the on-cycle answer");
    let reports = take_reports();
    assert_eq!(reports.len(), 1);
    assert!(
        reports[0].contains("cyclic 0 -> cyclic 1 -> cyclic 2 -> cyclic 0"),
        "{reports:?}"
    );
    assert!(store.poisoned());
    // The next build starts from an empty store.
    store.answer::<Sum>(&world, &(0, 1));
    store.end();
    store.begin(&[]);
    assert!(!store.poisoned());
    assert_eq!(store.tables().sum.len(), 0);
}

#[test]
fn verify_reused_names_the_table_and_key_that_missed_a_read() {
    let check = |verify| {
        let store = Store::<Toy>::new(verify, capture);
        let world = ToyWorld::with_cells(&[(5, 1)]);
        store.begin(&[]);
        assert_eq!(store.answer::<Leaky>(&world, &5), 1);
        store.end();
        let next = ToyWorld::with_cells(&[(5, 2)]);
        // The leaky read was never recorded, so nothing is marked.
        assert_eq!(store.begin(&changed(&world, &next)).dirty, 0);
        take_reports();
        let answer = store.answer::<Leaky>(&next, &5);
        (answer, take_reports(), store.poisoned())
    };
    // Without the mode the stale answer is served: what the mode exists for.
    assert_eq!(check(false), (1, Vec::new(), false));
    let (answer, reports, poisoned) = check(true);
    assert_eq!(answer, 2, "the fresh answer is served");
    assert!(poisoned);
    assert_eq!(reports.len(), 1);
    assert!(
        reports[0].contains("verify-reused mismatch: table `leaky` key 5"),
        "{reports:?}"
    );
}

#[test]
fn verify_reused_is_silent_when_every_read_is_recorded() {
    let store = Store::<Toy>::new(true, capture);
    let world = ToyWorld::with_cells(&[(0, 1), (1, 2), (2, 3)]);
    store.begin(&[]);
    store.answer::<Top>(&world, &0);
    store.end();
    take_reports();
    let next = ToyWorld::with_cells(&[(0, 1), (1, 2), (2, 4)]);
    store.begin(&changed(&world, &next));
    assert_eq!(store.answer::<Top>(&next, &0), fresh::<Top>(&next, &0));
    assert!(take_reports().is_empty());
    assert!(!store.poisoned());
}

#[test]
fn an_implied_input_is_free_and_still_invalidates() {
    let store = store();
    let world = ToyWorld::with_cells(&[(3, 4)]);
    store.begin(&[]);
    assert_eq!(store.answer::<Own>(&world, &3), 8);
    store.end();
    let next = ToyWorld::with_cells(&[(3, 5)]);
    assert_eq!(store.begin(&changed(&world, &next)).dirty, 1);
    assert_eq!(store.answer::<Own>(&next, &3), 10);
}

#[test]
fn the_sweep_drops_what_no_build_reached() {
    let store = store();
    let world = ToyWorld::with_cells(&[(0, 1), (1, 2), (2, 3)]);
    store.begin(&[]);
    store.answer::<Top>(&world, &0);
    store.answer::<Sum>(&world, &(7, 8));
    store.end();
    store.begin(&[]);
    store.answer::<Parity>(&world, &1);
    let (stats, released) = store.end();
    let entries = stats
        .iter()
        .map(|(name, stats)| (*name, stats.entries))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        (entries["top"], entries["parity"], entries["sum"]),
        (0, 1, 1)
    );
    // Cells 0, 7 and 8 are read by nothing left.
    assert_eq!(released, [0, 7, 8]);
}

/// `Canonical` holds for ordered data and not for hash containers.
#[test]
fn only_canonical_answers_cross_a_context() {
    struct Probe<T>(std::marker::PhantomData<T>);
    trait NotCanonical {
        const CANONICAL: bool = false;
    }
    impl<T> NotCanonical for Probe<T> {}
    impl<T: Canonical> Probe<T> {
        const CANONICAL: bool = true;
    }
    const { assert!(Probe::<Vec<(u32, String)>>::CANONICAL) };
    const { assert!(Probe::<BTreeMap<u32, Option<bool>>>::CANONICAL) };
    const { assert!(Probe::<BTreeSet<std::sync::Arc<str>>>::CANONICAL) };
    const { assert!(!Probe::<std::collections::HashMap<u32, u32>>::CANONICAL) };
    const { assert!(!Probe::<std::collections::HashSet<u32>>::CANONICAL) };
    const { assert!(!Probe::<crate::HashMap<u32, u32>>::CANONICAL) };
    const { assert!(!Probe::<Vec<crate::HashSet<u32>>>::CANONICAL) };
}
