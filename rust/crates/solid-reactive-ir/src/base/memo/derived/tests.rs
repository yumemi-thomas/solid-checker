//! Publication end to end over a toy schema: a kept monotone solver is
//! updated through random edits, publishes what moved, and the memos that
//! read its output answer what a fresh store over a fresh solve answers,
//! with only the readers of moved keys recomputed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use super::super::{Cycle, ErasedTable, Memo, Schema, Store, Table, TableId, TableSet, World};
use super::{Derived, DerivedId, DerivedSchema, DerivedTables};
use crate::base::fixpoint::{Change, Definition, Lattice, Monotone};

struct Toy;

struct ToyWorld<'w> {
    derived: &'w DerivedTables,
    /// The readers computed in this build.
    computed: Mutex<Vec<u32>>,
}

impl World<Toy> for ToyWorld<'_> {
    fn fingerprint(&self, input: u32) -> u64 {
        self.derived.fingerprint(DERIVED, input)
    }
}

impl Schema for Toy {
    type Input = u32;
    type Tables = ToyTables;
    type World<'w> = ToyWorld<'w>;
}

impl DerivedSchema for Toy {
    fn derived<'a>(world: &'a ToyWorld<'_>) -> &'a DerivedTables {
        world.derived
    }

    fn input(_: DerivedId, key: u32) -> u32 {
        key
    }
}

const DERIVED: DerivedId = DerivedId(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Bits(u8);

impl Lattice for Bits {
    fn bottom() -> Self {
        Self(0)
    }
    fn join(&mut self, other: &Self) {
        self.0 |= other.0;
    }
}

impl super::super::Canonical for Bits {}

fn transfer(edge: &u8, value: &Bits) -> Bits {
    Bits((value.0 & edge).rotate_left(1))
}

/// The pass's output.
struct Solution;

impl Derived for Solution {
    type Key = u32;
    type Value = Bits;
    const NAME: &'static str = "solution";
    const ID: DerivedId = DERIVED;
}

#[derive(Default)]
struct ToyTables {
    reader: Table<Toy, Reader>,
}

impl TableSet<Toy> for ToyTables {
    fn table(&self, _: TableId) -> &dyn ErasedTable<Toy> {
        &self.reader
    }

    fn tables(&self) -> Vec<&dyn ErasedTable<Toy>> {
        vec![&self.reader]
    }
}

/// The keys reader `key` reads.
fn read_keys(key: u32) -> [u32; 3] {
    [key, key + 1, (key * 7) % 40]
}

/// A memo over three published keys.
struct Reader;

impl Memo<Toy> for Reader {
    type Key = u32;
    type Value = Vec<Option<Bits>>;
    const NAME: &'static str = "reader";
    const ID: TableId = TableId(0);

    fn table(tables: &ToyTables) -> &Table<Toy, Self> {
        &tables.reader
    }

    fn compute(ctx: &super::super::Ctx<'_, '_, Toy>, key: &u32) -> Result<Self::Value, Cycle> {
        ctx.world().computed.lock().unwrap().push(*key);
        Ok(read_keys(*key)
            .iter()
            .map(|key| ctx.derived::<Solution>(key))
            .collect())
    }

    fn on_cycle(_: &u32) -> Self::Value {
        Vec::new()
    }
}

fn ignore(_: &str) {}

/// Mulberry32.
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

fn random_definition(random: &mut Random) -> Definition<u32, Bits, u8> {
    Definition {
        input: Bits(u8::try_from(random.below(256)).unwrap() & 0b0100_1001),
        preds: (0..random.below(3))
            .map(|_| (random.below(44), u8::try_from(random.below(256)).unwrap()))
            .collect(),
    }
}

/// A build's reader answers, the readers it computed, and the moved keys.
type Built = (BTreeMap<u32, Vec<Option<Bits>>>, Vec<u32>, BTreeSet<u32>);

/// One build: the pass solves and publishes, then readers are asked.
fn build(
    store: &Store<Toy>,
    derived: &DerivedTables,
    solver: &mut Monotone<u32, Bits, u8>,
    changes: Vec<Change<u32, Bits, u8>>,
    asked: &[u32],
) -> Built {
    store.begin(&[]);
    let solved = solver.update(changes, &transfer);
    store.publish::<Solution>(
        derived,
        solved
            .changed
            .iter()
            .map(|key| (*key, solver.value(key).copied())),
    );
    let world = ToyWorld {
        derived,
        computed: Mutex::default(),
    };
    let answers = asked
        .iter()
        .map(|key| (*key, store.answer::<Reader>(&world, key)))
        .collect();
    store.end();
    let mut computed = world.computed.into_inner().unwrap();
    computed.sort_unstable();
    (answers, computed, solved.changed.into_iter().collect())
}

#[test]
fn published_outputs_invalidate_exactly_their_readers() {
    let mut recomputed = 0;
    let mut reused = 0;
    for seed in 1..=150 {
        let mut random = Random(seed);
        let mut definitions = BTreeMap::new();
        for key in 0..40 {
            if random.below(4) > 0 {
                definitions.insert(key, random_definition(&mut random));
            }
        }
        let store = Store::<Toy>::new(false, ignore);
        let derived = DerivedTables::default();
        let mut solver = Monotone::default();
        let asked = (0..40).collect::<Vec<_>>();
        build(
            &store,
            &derived,
            &mut solver,
            definitions
                .iter()
                .map(|(key, definition)| (*key, Some(definition.clone())))
                .collect(),
            &asked,
        );
        for step in 0..20 {
            let mut changes = Vec::new();
            for _ in 0..=random.below(2) {
                let key = random.below(44);
                let next = (random.below(4) > 0).then(|| random_definition(&mut random));
                match &next {
                    Some(definition) => definitions.insert(key, definition.clone()),
                    None => definitions.remove(&key),
                };
                changes.push((key, next));
            }
            let (answers, computed, moved) = build(&store, &derived, &mut solver, changes, &asked);
            assert!(!store.poisoned(), "seed {seed} step {step}: poisoned");
            // A fresh store over a fresh solve.
            let fresh_store = Store::<Toy>::new(false, ignore);
            let fresh_derived = DerivedTables::default();
            let (fresh, ..) = build(
                &fresh_store,
                &fresh_derived,
                &mut Monotone::default(),
                definitions
                    .iter()
                    .map(|(key, definition)| (*key, Some(definition.clone())))
                    .collect(),
                &asked,
            );
            assert_eq!(answers, fresh, "seed {seed} step {step}: a stale reader");
            // Exactly the readers of a moved key ran again.
            let expected = asked
                .iter()
                .copied()
                .filter(|key| read_keys(*key).iter().any(|read| moved.contains(read)))
                .collect::<Vec<_>>();
            assert_eq!(
                computed, expected,
                "seed {seed} step {step}: inexact invalidation"
            );
            recomputed += computed.len();
            reused += asked.len() - computed.len();
        }
    }
    assert!(
        recomputed > 1_000 && reused > 10_000,
        "{recomputed} {reused}"
    );
}

#[test]
#[should_panic(expected = "read before it was published")]
fn a_read_before_the_pass_publishes_fails_closed() {
    let store = Store::<Toy>::new(false, ignore);
    let derived = DerivedTables::default();
    store.begin(&[]);
    let world = ToyWorld {
        derived: &derived,
        computed: Mutex::default(),
    };
    store.answer::<Reader>(&world, &0);
}

/// A stage chunk's proof checks its recorded memo reads before the passes
/// run. A reader made dirty since must not be brought up to date over a
/// table its pass has not published yet: under `deferring` the check
/// answers undecided (the proof counts the read as moved), installs nothing
/// and leaves the store unpoisoned, and once the pass publishes the reader
/// computes over this build's values.
#[test]
fn a_proof_check_before_the_pass_publishes_is_undecided_not_a_failure() {
    let store = Store::<Toy>::new(false, ignore);
    let derived = DerivedTables::default();
    let world = ToyWorld {
        derived: &derived,
        computed: Mutex::default(),
    };
    store.begin(&[]);
    store.publish::<Solution>(&derived, [(0, Some(Bits(1))), (1, Some(Bits(2)))]);
    let (first, read) = store.answer_read::<Reader>(&world, &0);
    let (query, changed_at) = read.expect("no cycle");
    store.end();
    assert_eq!(first[..2], [Some(Bits(1)), Some(Bits(2))]);

    // The next build: key 0's input moved, so the reader is dirty, and the
    // pass has not published yet.
    store.begin(&[0]);
    world.computed.lock().unwrap().clear();
    let checked = super::deferring(|| store.changed_at(&world, TableId(0), query));
    assert_eq!(checked, None, "undecided before the pass publishes");
    assert!(!store.poisoned(), "a proof's check is not a pass-order bug");
    assert_eq!(
        *world.computed.lock().unwrap(),
        [0],
        "the reader ran once and installed nothing"
    );

    // The pass publishes a moved key 0; the reader recomputes over it.
    store.publish::<Solution>(&derived, [(0, Some(Bits(4)))]);
    let (next, read) = store.answer_read::<Reader>(&world, &0);
    assert!(!store.poisoned());
    assert_eq!(next[..2], [Some(Bits(4)), Some(Bits(2))]);
    assert_ne!(read.expect("no cycle").1, changed_at);
    store.end();

    // A clean reader checks without running: the pass has not published.
    store.begin(&[]);
    world.computed.lock().unwrap().clear();
    let read = store.changed_at(&world, TableId(0), query);
    assert!(read.is_some());
    assert!(world.computed.lock().unwrap().is_empty());
    assert!(!store.poisoned());
}
