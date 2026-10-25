//! Property tests of the kept solvers: through random graphs and random
//! edits, each incremental update answers what a fresh solve of the same
//! definitions answers, reports exactly the keys whose output moved, and
//! the lemma the module docs state holds as stated.

use std::collections::{BTreeMap, BTreeSet};

use super::super::file_table::{FileId, FileInterner};
use super::*;

/// Mulberry32, as the other base tests use: a failing run replays from its
/// seed.
struct Random(u32);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        let unit = f64::from(t ^ (t >> 14)) / 4_294_967_296.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = (unit * n as f64) as usize;
        index.min(n.saturating_sub(1))
    }

    fn small(&mut self, n: usize) -> u32 {
        u32::try_from(self.below(n)).unwrap()
    }
}

/// The keys whose value differs between two maps.
fn moved<K: Ord + Clone, V: Eq>(before: &BTreeMap<K, V>, after: &BTreeMap<K, V>) -> BTreeSet<K> {
    before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}

fn sorted<K: Ord + Clone>(keys: &[K]) -> BTreeSet<K> {
    let set = keys.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(set.len(), keys.len(), "a changed key is reported once");
    set
}

// ---------------------------------------------------------------------------
// Node keys and the dense index.

#[test]
fn a_dense_index_reuses_freed_slots_and_answers_by_key() {
    let mut random = Random(7);
    let mut index = DenseIndex::<u32>::default();
    let mut model = BTreeMap::<u32, Slot>::new();
    for _ in 0..2_000 {
        let key = random.small(64);
        if random.below(3) == 0 {
            if let Some(slot) = model.remove(&key) {
                index.remove(slot);
            }
        } else {
            let (slot, new) = index.insert(&key);
            assert_eq!(new, !model.contains_key(&key));
            assert_eq!(*model.entry(key).or_insert(slot), slot);
        }
        assert_eq!(index.len(), model.len());
        for (key, slot) in &model {
            assert_eq!(index.get(key), Some(*slot));
            assert_eq!(index.key(*slot), Some(key));
        }
        assert!(index.capacity() <= 64, "freed slots are reused");
    }
}

#[test]
fn node_keys_index_densely_across_files() {
    let mut interner = FileInterner::default();
    let [a, b] = ["a", "b"].map(|path| interner.intern(path));
    let mut index = DenseIndex::default();
    let keys = [
        NodeKey {
            file: a,
            item: ItemKey::Item(super::super::ItemKind::Function, 0),
        },
        NodeKey {
            file: b,
            item: ItemKey::Span(Span { start: 1, end: 4 }),
        },
    ];
    let slots = keys.map(|key| index.insert(&key).0);
    assert_eq!(slots, [0, 1]);
    index.remove(slots[0]);
    assert_eq!(index.get(&keys[1]), Some(1));
    assert_eq!(index.insert(&keys[0]), (0, true), "the freed slot");
}

#[test]
fn a_file_order_keeps_relative_order_through_inserts_and_removals() {
    let mut interner = FileInterner::default();
    let [a, b, c, d] = ["a", "b", "c", "d"].map(|path| interner.intern(path));
    let before = FileOrder::new(&[a, b, d]);
    assert!(FileOrder::new(&[a, b, c, d]).keeps_relative_order(&before));
    assert!(FileOrder::new(&[a, d]).keeps_relative_order(&before));
    assert!(!FileOrder::new(&[b, a, d]).keeps_relative_order(&before));
    assert_eq!(FileOrder::new(&[a, c, d]).rank(d), Some(2));
}

// ---------------------------------------------------------------------------
// K1: keyed fold.

/// Files by path: the order is the paths' sorted order, whatever their ids.
struct Files {
    interner: FileInterner,
    rows: BTreeMap<String, (FileId, Vec<(u8, u8)>)>,
}

impl Files {
    fn order(&self) -> FileOrder {
        FileOrder::new(&self.rows.values().map(|(id, _)| *id).collect::<Vec<_>>())
    }
}

fn random_rows(random: &mut Random) -> Vec<(u8, u8)> {
    (0..random.below(6))
        .map(|_| {
            (
                u8::try_from(random.below(10)).unwrap(),
                u8::try_from(random.below(4)).unwrap(),
            )
        })
        .collect()
}

type FoldRule = fn(&u8, &[&u8]) -> Option<Vec<u8>>;

const FOLD_RULES: [(&str, FoldRule); 4] = [
    ("first wins", |_, values| values.first().map(|v| vec![**v])),
    ("last wins", |_, values| values.last().map(|v| vec![**v])),
    ("in order", |_, values| {
        Some(values.iter().map(|v| **v).collect())
    }),
    // A rule that answers nothing for some keys.
    ("odd sum", |_, values| {
        let sum = values.iter().map(|v| u32::from(**v)).sum::<u32>();
        (sum % 2 == 1).then(|| vec![u8::try_from(sum % 256).unwrap()])
    }),
];

#[test]
fn a_keyed_fold_updated_through_random_edits_equals_a_fresh_fold() {
    let mut partial = 0;
    for (name, rule) in FOLD_RULES {
        for seed in 1..=60 {
            let mut random = Random(seed);
            let mut files = Files {
                interner: FileInterner::default(),
                rows: BTreeMap::new(),
            };
            for _ in 0..random.below(8) {
                let path = format!("p{:03}", random.below(1000));
                let id = files.interner.intern(&path);
                files.rows.insert(path, (id, random_rows(&mut random)));
            }
            let mut kept = KeyedFold::default();
            kept.update(
                files.order(),
                files
                    .rows
                    .values()
                    .map(|(id, rows)| (*id, Some(rows.clone()))),
                &rule,
            );
            for step in 0..25 {
                let before = kept.values().clone();
                let mut changes = Vec::new();
                for _ in 0..=random.below(2) {
                    match random.below(4) {
                        // A file added at a random path: often mid-order.
                        0 => {
                            let path = format!("p{:03}", random.below(1000));
                            let id = files.interner.intern(&path);
                            let rows = random_rows(&mut random);
                            files.rows.insert(path, (id, rows.clone()));
                            changes.push((id, Some(rows)));
                        }
                        1 if !files.rows.is_empty() => {
                            let path = files
                                .rows
                                .keys()
                                .nth(random.below(files.rows.len()))
                                .unwrap()
                                .clone();
                            let (id, _) = files.rows.remove(&path).unwrap();
                            changes.push((id, None));
                        }
                        _ if !files.rows.is_empty() => {
                            let at = random.below(files.rows.len());
                            let (id, rows) = files.rows.values_mut().nth(at).unwrap();
                            *rows = random_rows(&mut random);
                            changes.push((*id, Some(rows.clone())));
                        }
                        _ => {}
                    }
                }
                let solved = kept.update(files.order(), changes, &rule);
                let mut fresh = KeyedFold::default();
                fresh.update(
                    files.order(),
                    files
                        .rows
                        .values()
                        .map(|(id, rows)| (*id, Some(rows.clone()))),
                    &rule,
                );
                assert_eq!(
                    kept.values(),
                    fresh.values(),
                    "{name} seed {seed} step {step}: update differs from a fresh fold"
                );
                // The reference: every row in path order, then the rule.
                let mut by_key = BTreeMap::<u8, Vec<&u8>>::new();
                for (_, rows) in files.rows.values() {
                    for (key, value) in rows {
                        by_key.entry(*key).or_default().push(value);
                    }
                }
                let reference = by_key
                    .iter()
                    .filter_map(|(key, values)| rule(key, values).map(|value| (*key, value)))
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(
                    kept.values(),
                    &reference,
                    "{name} seed {seed} step {step}: not the path-order fold"
                );
                assert_eq!(
                    sorted(&solved.changed),
                    moved(&before, kept.values()),
                    "{name} seed {seed} step {step}: inexact changed keys"
                );
                partial += usize::from(!solved.fresh);
            }
        }
    }
    assert!(partial > 1_000, "the incremental path ran ({partial})");
}

// ---------------------------------------------------------------------------
// K2: monotone cone.

/// A bit set joined by union; an edge masks and rotates it (monotone).
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

fn bits_transfer(edge: &u8, value: &Bits) -> Bits {
    Bits((value.0 & edge).rotate_left(u32::from(edge % 3)))
}

/// A shortest distance with a payload tie-break, as passes 5 and 6 of
/// source discovery want: the least `(distance, payload)`; bottom is
/// unreached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Nearest(Option<(u32, u8)>);

impl Lattice for Nearest {
    fn bottom() -> Self {
        Self(None)
    }
    fn join(&mut self, other: &Self) {
        if let Some(other) = other.0
            && self.0.is_none_or(|current| other < current)
        {
            self.0 = Some(other);
        }
    }
}

fn nearest_transfer(edge: &u8, value: &Nearest) -> Nearest {
    Nearest(
        value
            .0
            .map(|(distance, payload)| (distance + 1 + u32::from(*edge % 2), payload)),
    )
}

type Definitions<V, E> = BTreeMap<u32, Definition<u32, V, E>>;

fn random_definition<V>(random: &mut Random, nodes: usize, direct: V) -> Definition<u32, V, u8> {
    // Sparse, and mostly backwards: long chains, a few cycles.
    let preds = (0..random.below(3))
        .map(|_| {
            (
                random.small(nodes + 4),
                u8::try_from(random.below(256)).unwrap(),
            )
        })
        .collect();
    Definition {
        input: direct,
        preds,
    }
}

/// The least solution by global rounds over every defined node: the
/// reference both solvers are checked against.
fn least_solution<V: Lattice, E>(
    definitions: &Definitions<V, E>,
    transfer: impl Fn(&E, &V) -> V,
) -> BTreeMap<u32, V> {
    let mut values = definitions
        .iter()
        .map(|(key, definition)| (*key, definition.input.clone()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut next = values.clone();
        for (key, definition) in definitions {
            let value = next.get_mut(key).unwrap();
            for (pred, edge) in &definition.preds {
                if let Some(pred) = values.get(pred) {
                    value.join(&transfer(edge, pred));
                }
            }
        }
        if next == values {
            return values;
        }
        values = next;
    }
}

fn monotone_values<V: Lattice>(solver: &Monotone<u32, V, u8>) -> BTreeMap<u32, V> {
    solver
        .values()
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}

fn check_monotone<V: Lattice>(
    name: &str,
    direct: impl Fn(&mut Random) -> V,
    transfer: fn(&u8, &V) -> V,
) -> usize {
    let mut partial = 0;
    for seed in 1..=120 {
        let mut random = Random(seed);
        let nodes = 8 + random.below(60);
        let mut definitions = Definitions::new();
        for key in 0..nodes {
            if random.below(5) > 0 {
                let value = direct(&mut random);
                definitions.insert(
                    u32::try_from(key).unwrap(),
                    random_definition(&mut random, nodes, value),
                );
            }
        }
        let mut kept = Monotone::default();
        kept.update(
            definitions
                .iter()
                .map(|(key, definition)| (*key, Some(definition.clone()))),
            &transfer,
        );
        for step in 0..30 {
            let before = monotone_values(&kept);
            let mut changes = Vec::new();
            for _ in 0..=random.below(2) {
                let key = random.small(nodes + 4);
                let next = match random.below(5) {
                    // Undefine: a removed node or a vetoed source.
                    0 => None,
                    // Remove one mid-chain edge.
                    1 => definitions.get(&key).cloned().map(|mut definition| {
                        if !definition.preds.is_empty() {
                            let at = random.below(definition.preds.len());
                            definition.preds.remove(at);
                        }
                        definition
                    }),
                    // Flip the direct value only.
                    2 => definitions.get(&key).cloned().map(|mut definition| {
                        definition.input = direct(&mut random);
                        definition
                    }),
                    _ => {
                        let value = direct(&mut random);
                        Some(random_definition(&mut random, nodes, value))
                    }
                };
                match &next {
                    Some(definition) => definitions.insert(key, definition.clone()),
                    None => definitions.remove(&key),
                };
                changes.push((key, next));
            }
            let solved = kept.update(changes, &transfer);
            let after = monotone_values(&kept);
            let mut fresh = Monotone::default();
            fresh.update(
                definitions
                    .iter()
                    .map(|(key, definition)| (*key, Some(definition.clone()))),
                &transfer,
            );
            assert_eq!(
                after,
                monotone_values(&fresh),
                "{name} seed {seed} step {step}: cone solve differs from a fresh solve"
            );
            assert_eq!(
                after,
                least_solution(&definitions, transfer),
                "{name} seed {seed} step {step}: not the least solution"
            );
            assert_eq!(
                sorted(&solved.changed),
                moved(&before, &after),
                "{name} seed {seed} step {step}: inexact changed keys"
            );
            assert_eq!(kept.len(), definitions.len());
            partial += usize::from(!solved.fresh);
            // `replace` with the same definitions changes nothing.
            if step % 7 == 0 {
                let solved = kept.replace(
                    definitions
                        .iter()
                        .map(|(key, definition)| (*key, definition.clone()))
                        .collect(),
                    &transfer,
                );
                assert!(solved.changed.is_empty() && solved.recomputed == 0);
            }
        }
    }
    partial
}

#[test]
fn a_monotone_cone_through_random_edits_equals_a_fresh_solve() {
    let partial = check_monotone(
        "bits",
        |random| Bits(u8::try_from(random.below(256)).unwrap() & 0b1001_0110),
        bits_transfer,
    );
    assert!(partial > 1_000, "the cone path ran ({partial})");
    let partial = check_monotone(
        "nearest",
        |random| {
            Nearest(
                (random.below(3) == 0)
                    .then(|| (random.small(4), u8::try_from(random.below(5)).unwrap())),
            )
        },
        nearest_transfer,
    );
    assert!(partial > 1_000, "the cone path ran ({partial})");
}

#[test]
fn a_monotone_replace_finds_the_changed_definitions() {
    let mut random = Random(3);
    for _ in 0..50 {
        let nodes = 40;
        let definitions = (0..nodes)
            .map(|key| {
                let value = Bits(u8::try_from(random.below(16)).unwrap());
                (key, random_definition(&mut random, 40, value))
            })
            .collect::<Vec<_>>();
        let mut kept = Monotone::default();
        kept.replace(definitions.clone(), &bits_transfer);
        let mut next = definitions.clone();
        next.truncate(35);
        next[3].1.input = Bits(0xff);
        let solved = kept.replace(next.clone(), &bits_transfer);
        let mut fresh = Monotone::default();
        fresh.replace(next, &bits_transfer);
        assert_eq!(monotone_values(&kept), monotone_values(&fresh));
        assert!(solved.changed.len() >= 5, "the five dropped keys moved");
    }
}

// ---------------------------------------------------------------------------
// K3: round replay.

/// Three loops: an ordered list grown by its predecessors' items (the
/// refresh pass's shape: arrival order kept, truncated at the cap); a
/// saturating count; and one that may oscillate (non-monotone).
#[derive(Clone, Copy)]
enum Loop {
    Arrivals,
    Saturating,
    Oscillating,
}

impl Step<u32, u8, Vec<u8>, u8> for Loop {
    fn init(&self, _: &u32, input: &u8) -> Vec<u8> {
        match self {
            Self::Arrivals => vec![*input],
            Self::Saturating | Self::Oscillating => vec![*input % 4],
        }
    }

    fn step(
        &self,
        _: &u32,
        input: &u8,
        own: &Vec<u8>,
        preds: &[(&u8, Option<&Vec<u8>>)],
    ) -> Vec<u8> {
        match self {
            Self::Arrivals => {
                let mut next = own.clone();
                for (edge, value) in preds {
                    for item in value.iter().copied().flatten() {
                        let item = item.wrapping_add(**edge % 2);
                        if !next.contains(&item) && next.len() < 12 {
                            next.push(item);
                        }
                    }
                }
                next
            }
            Self::Saturating => {
                let most = preds
                    .iter()
                    .filter_map(|(edge, value)| value.map(|value| value[0] + **edge % 2))
                    .max()
                    .unwrap_or(0);
                vec![own[0].max(most).max(*input % 4).min(9)]
            }
            Self::Oscillating => {
                let sum = preds
                    .iter()
                    .filter_map(|(_, value)| value.map(|value| u32::from(value[0])))
                    .sum::<u32>();
                vec![u8::try_from((sum + u32::from(*input)) % 3).unwrap()]
            }
        }
    }
}

fn round_values(
    solver: &RoundReplay<u32, u8, Vec<u8>, u8>,
    keys: impl Iterator<Item = u32>,
) -> BTreeMap<u32, Vec<u8>> {
    keys.filter_map(|key| solver.value(&key).map(|value| (key, value.clone())))
        .collect()
}

fn random_input_definition(random: &mut Random, nodes: usize) -> Definition<u32, u8, u8> {
    let input = u8::try_from(random.below(16)).unwrap();
    random_definition(random, nodes, input)
}

#[test]
fn a_round_replay_through_random_edits_equals_a_fresh_solve() {
    for (name, rules, cap) in [
        ("arrivals", Loop::Arrivals, 8),
        ("saturating", Loop::Saturating, 6),
        ("oscillating", Loop::Oscillating, 5),
    ] {
        let mut partial = 0;
        let mut capped = 0;
        for seed in 1..=100 {
            let mut random = Random(seed);
            let nodes = 8 + random.below(50);
            let mut definitions = BTreeMap::new();
            for key in 0..nodes {
                if random.below(5) > 0 {
                    definitions.insert(
                        u32::try_from(key).unwrap(),
                        random_input_definition(&mut random, nodes),
                    );
                }
            }
            let universe = 0..u32::try_from(nodes + 4).unwrap();
            let mut kept = RoundReplay::new(cap);
            kept.update(
                definitions
                    .iter()
                    .map(|(key, definition)| (*key, Some(definition.clone()))),
                &rules,
            );
            for step in 0..25 {
                let before = round_values(&kept, universe.clone());
                let mut changes = Vec::new();
                for _ in 0..=random.below(2) {
                    let key = random.small(nodes + 4);
                    let next = match random.below(4) {
                        0 => None,
                        1 => definitions.get(&key).cloned().map(|mut definition| {
                            if !definition.preds.is_empty() {
                                definition
                                    .preds
                                    .remove(random.below(definition.preds.len()));
                            }
                            definition
                        }),
                        _ => Some(random_input_definition(&mut random, nodes)),
                    };
                    match &next {
                        Some(definition) => definitions.insert(key, definition.clone()),
                        None => definitions.remove(&key),
                    };
                    changes.push((key, next));
                }
                let solved = kept.update(changes, &rules);
                let mut fresh = RoundReplay::new(cap);
                fresh.update(
                    definitions
                        .iter()
                        .map(|(key, definition)| (*key, Some(definition.clone()))),
                    &rules,
                );
                let after = round_values(&kept, universe.clone());
                assert_eq!(
                    after,
                    round_values(&fresh, universe.clone()),
                    "{name} seed {seed} step {step}: replay differs from a fresh solve"
                );
                assert_eq!(
                    (kept.converged(), kept.rounds()),
                    (fresh.converged(), fresh.rounds()),
                    "{name} seed {seed} step {step}: a different stop"
                );
                // Every stored round, not only the last.
                for key in universe.clone() {
                    for round in 0..=cap {
                        assert_eq!(
                            kept.value_at(&key, round),
                            fresh.value_at(&key, round),
                            "{name} seed {seed} step {step}: node {key} round {round}"
                        );
                    }
                }
                assert_eq!(
                    sorted(&solved.changed),
                    moved(&before, &after),
                    "{name} seed {seed} step {step}: inexact changed keys"
                );
                partial += usize::from(!solved.fresh);
                capped += usize::from(!fresh.converged());
            }
        }
        assert!(partial > 300, "{name}: the replay path ran ({partial})");
        if matches!(rules, Loop::Oscillating) {
            assert!(capped > 100, "{name}: the cap fallback ran ({capped})");
        }
    }
}

/// Lemma 4′: a node none of whose ancestors (in the new graph) had its
/// definition changed keeps its whole trajectory, extended past its last
/// stored round by its final value, when the old loop converged.
#[test]
fn lemma_4_prime_an_unseeded_node_keeps_its_trajectory() {
    let mut checked = 0;
    for seed in 1..=300 {
        let mut random = Random(seed);
        let nodes = 6 + random.below(30);
        let mut old = BTreeMap::new();
        for key in 0..nodes {
            old.insert(
                u32::try_from(key).unwrap(),
                random_input_definition(&mut random, nodes),
            );
        }
        let mut new = old.clone();
        let mut seeds = BTreeSet::new();
        for _ in 0..=random.below(2) {
            let key = random.small(nodes);
            new.insert(key, random_input_definition(&mut random, nodes));
            seeds.insert(key);
        }
        let rules = Loop::Arrivals;
        let cap = 40;
        let solve = |definitions: &BTreeMap<u32, Definition<u32, u8, u8>>| {
            let mut solver = RoundReplay::new(cap);
            solver.update(
                definitions
                    .iter()
                    .map(|(key, definition)| (*key, Some(definition.clone()))),
                &rules,
            );
            solver
        };
        let (before, after) = (solve(&old), solve(&new));
        if !before.converged() {
            continue;
        }
        // The forward cone of the seeds in the new graph.
        let mut cone = seeds.clone();
        loop {
            let grown = new
                .iter()
                .filter(|(_, definition)| {
                    definition.preds.iter().any(|(pred, _)| cone.contains(pred))
                })
                .map(|(key, _)| *key)
                .collect::<BTreeSet<_>>();
            if grown.is_subset(&cone) {
                break;
            }
            cone.extend(grown);
        }
        for key in new.keys().filter(|key| !cone.contains(key)) {
            for round in 0..=cap {
                assert_eq!(
                    before.value_at(key, round),
                    after.value_at(key, round),
                    "seed {seed}: node {key} outside the cone moved at round {round}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 1_000, "the lemma was exercised ({checked})");
}

// ---------------------------------------------------------------------------
// Publication.

#[test]
fn a_published_value_keeps_its_revision_when_recomputed_equal() {
    let mut published = Published::<u32, u8>::default();
    let read = published.intern(&1);
    assert_eq!(
        published.publish(1, [(1, Some(10)), (2, Some(20))]),
        vec![read]
    );
    assert_eq!(published.revision(&1), 1);
    // Equal: no reader moves, the revision stays.
    assert!(published.publish(2, [(1, Some(10))]).is_empty());
    assert_eq!(published.revision(&1), 1);
    assert_eq!(published.published_at(), Some(2));
    // An unread key moves no reader.
    assert!(published.publish(3, [(2, Some(21))]).is_empty());
    assert_eq!(published.publish(4, [(1, None)]), vec![read]);
    assert_eq!(published.get(&1), None);
    assert_eq!(published.revision(&1), 0);
    published.release(read);
    assert_eq!(published.id(&1), None);
}

// ---------------------------------------------------------------------------
// Costs.

/// A timing report, not a check: each solver's fresh solve and its mean
/// one-definition update at roughly the sizes the S7 design estimates
/// (`cargo test --release -p solid-reactive-ir --lib solver_costs --
/// --ignored --nocapture`).
#[test]
#[ignore = "a timing report"]
fn solver_costs() {
    use std::time::Instant;
    const NODES: usize = 20_000;
    const EDITS: u32 = 50;
    let mut random = Random(11);
    let report =
        |name: &str, fresh: std::time::Duration, total: std::time::Duration, work: usize| {
            println!(
                "{name}: fresh {:.2} ms, update {:.3} ms mean, {:.1} nodes recomputed per update",
                fresh.as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 / f64::from(EDITS),
                work as f64 / f64::from(EDITS),
            );
        };
    // K2.
    let definitions = (0..NODES)
        .map(|key| {
            let key = u32::try_from(key).unwrap();
            let value = Bits(u8::try_from(random.below(256)).unwrap() & 0b1001_0110);
            let mut definition = random_definition(&mut random, NODES, value);
            // Mostly local chains, as call graphs are.
            for (pred, _) in &mut definition.preds {
                *pred = key.saturating_sub(1 + *pred % 8);
            }
            (key, definition)
        })
        .collect::<BTreeMap<_, _>>();
    let started = Instant::now();
    let mut kept = Monotone::default();
    kept.update(
        definitions
            .iter()
            .map(|(key, definition)| (*key, Some(definition.clone()))),
        &bits_transfer,
    );
    let fresh = started.elapsed();
    let (mut total, mut work) = (std::time::Duration::ZERO, 0);
    for _ in 0..EDITS {
        let key = random.small(NODES);
        let mut definition = definitions[&key].clone();
        definition.input = Bits(definition.input.0 ^ 0b0001_0000);
        let started = Instant::now();
        work += kept
            .update([(key, Some(definition))], &bits_transfer)
            .recomputed;
        total += started.elapsed();
    }
    report("K2 monotone", fresh, total, work);
    // K3, over the same graph.
    let inputs = definitions
        .iter()
        .map(|(key, definition)| {
            let input = u8::try_from(key % 16).unwrap();
            (
                *key,
                Definition {
                    input,
                    preds: definition.preds.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let started = Instant::now();
    let mut kept = RoundReplay::new(64);
    kept.update(
        inputs
            .iter()
            .map(|(key, definition)| (*key, Some(definition.clone()))),
        &Loop::Arrivals,
    );
    let fresh = started.elapsed();
    let (mut total, mut work) = (std::time::Duration::ZERO, 0);
    for _ in 0..EDITS {
        let key = random.small(NODES);
        let mut definition = inputs[&key].clone();
        definition.input = definition.input.wrapping_add(1);
        let started = Instant::now();
        work += kept
            .update([(key, Some(definition))], &Loop::Arrivals)
            .recomputed;
        total += started.elapsed();
    }
    report("K3 round replay", fresh, total, work);
    // K1: 1,600 files of 20 rows.
    let mut interner = FileInterner::default();
    let files = (0..1_600)
        .map(|file| {
            let id = interner.intern(&format!("f{file:04}"));
            let rows = (0..20)
                .map(|_| (random.small(20_000), random.small(4)))
                .collect::<Vec<_>>();
            (id, rows)
        })
        .collect::<Vec<_>>();
    let order = FileOrder::new(&files.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    let last_wins = |_: &u32, values: &[&u32]| values.last().map(|value| **value);
    let started = Instant::now();
    let mut kept = KeyedFold::default();
    kept.update(
        order.clone(),
        files.iter().map(|(id, rows)| (*id, Some(rows.clone()))),
        &last_wins,
    );
    let fresh = started.elapsed();
    let (mut total, mut work) = (std::time::Duration::ZERO, 0);
    for _ in 0..EDITS {
        let (id, mut rows) = files[random.below(files.len())].clone();
        rows[0].1 ^= 1;
        let started = Instant::now();
        work += kept
            .update(order.clone(), [(id, Some(rows))], &last_wins)
            .recomputed;
        total += started.elapsed();
    }
    report("K1 keyed fold", fresh, total, work);
}
