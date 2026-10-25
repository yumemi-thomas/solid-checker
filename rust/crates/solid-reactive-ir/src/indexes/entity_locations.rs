//! The TypeScript table's entities by where they are, kept per path.
//!
//! Two questions ask the entity rows by position: the entity at an exact
//! `(path, start, end)` (the first row there, in the table's order), and the
//! smallest entity inside a span. Both used to be answered from maps built over
//! every entity of the project on each build (414,000 rows on app-game, two
//! passes and their drop: about 40 ms). A path's rows change only where a table
//! delta names the path (`TypeScriptChanges::entity_paths`, the premise
//! [`super::EntitySymbols`] is patched under), so each path keeps its own pair
//! of orders over its own rows, in the build caches, and a build replaces the
//! paths its delta names. A rebuilt table, an inexact delta or a first build
//! starts over from every path ([`EntityLocations::of`]).

use std::sync::Arc;

use solid_facts::TypeScriptTable;
use solid_facts::core::Span;
use typefacts::EntityFact;

use crate::HashMap;

/// One path's entity rows and the two orders its lookups ask them in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathLocations {
    /// The path's rows in the table's order: the slice the table holds, shared.
    rows: Arc<[EntityFact]>,
    /// The ordinal of the first row at each exact `(start_byte, end_byte)`.
    first_at: HashMap<(u64, u64), u32>,
    /// Every row's ordinal by `start_byte`, equal starts in row order.
    by_start: Vec<u32>,
}

impl PathLocations {
    fn new(path: &str, rows: Arc<[EntityFact]>) -> Self {
        debug_assert!(
            rows.iter().all(|row| row.location.path.as_ref() == path),
            "a path's entity rows all lie in that path"
        );
        let mut first_at = HashMap::default();
        first_at.reserve(rows.len());
        for (ordinal, row) in rows.iter().enumerate() {
            first_at
                .entry((row.location.start_byte, row.location.end_byte))
                .or_insert(ordinal_of(ordinal));
        }
        let mut by_start = (0..rows.len()).map(ordinal_of).collect::<Vec<_>>();
        by_start.sort_by_key(|&ordinal| rows[ordinal as usize].location.start_byte);
        Self {
            rows,
            first_at,
            by_start,
        }
    }

    fn row(&self, ordinal: u32) -> &EntityFact {
        &self.rows[ordinal as usize]
    }
}

fn ordinal_of(index: usize) -> u32 {
    u32::try_from(index).expect("a path holds fewer than 2^32 entity rows")
}

/// Every path's [`PathLocations`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EntityLocations {
    by_path: HashMap<String, Arc<PathLocations>>,
}

impl EntityLocations {
    /// Every path of `table`: one pass over its rows in the table's order finds
    /// the paths, and each path's own rows are indexed once.
    pub(crate) fn of(table: &TypeScriptTable) -> Self {
        let mut by_path = HashMap::default();
        let mut previous: Option<&Arc<str>> = None;
        for row in table.entities() {
            let path = &row.location.path;
            if previous.is_some_and(|previous| Arc::ptr_eq(previous, path) || previous == path) {
                continue;
            }
            previous = Some(path);
            if !by_path.contains_key(path.as_ref()) {
                by_path.insert(
                    path.to_string(),
                    Arc::new(PathLocations::new(
                        path,
                        table.entities_shared_for_path(path),
                    )),
                );
            }
        }
        Self { by_path }
    }

    /// Replaces what is held for `path` with `table`'s rows for it, or drops it
    /// when the table holds none: the update for a path a delta names.
    pub(crate) fn replace_path(&mut self, table: &TypeScriptTable, path: &str) {
        let rows = table.entities_shared_for_path(path);
        if rows.is_empty() {
            self.by_path.remove(path);
        } else {
            self.by_path
                .insert(path.to_owned(), Arc::new(PathLocations::new(path, rows)));
        }
    }

    /// The first entity at exactly `span` of `path`, in the table's order.
    pub(crate) fn at(&self, path: &str, span: Span) -> Option<&EntityFact> {
        let locations = self.by_path.get(path)?;
        let ordinal = *locations
            .first_at
            .get(&(u64::from(span.start), u64::from(span.end)))?;
        Some(locations.row(ordinal))
    }

    /// The smallest entity of `path` that lies inside `span` and satisfies
    /// `predicate`; of equal sizes, the first by start byte, then by row.
    pub(crate) fn smallest_contained(
        &self,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<&EntityFact> {
        let start = u64::from(span.start);
        let end = u64::from(span.end);
        let locations = self.by_path.get(path)?;
        let by_start = &locations.by_start;
        let first =
            by_start.partition_point(|&ordinal| locations.row(ordinal).location.start_byte < start);
        let last =
            by_start.partition_point(|&ordinal| locations.row(ordinal).location.start_byte <= end);
        by_start[first..last]
            .iter()
            .enumerate()
            .filter_map(|(index, &ordinal)| {
                let entity = locations.row(ordinal);
                (entity.location.end_byte <= end && predicate(entity)).then_some((index, entity))
            })
            .min_by_key(|(index, entity)| {
                (
                    entity.location.end_byte - entity.location.start_byte,
                    *index,
                )
            })
            .map(|(_, entity)| entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded generator (the repository's fuzz harnesses use the same one).
    fn random(seed: u64) -> impl FnMut(u64) -> u64 {
        let mut state = seed as u32;
        move |bound| {
            state = state.wrapping_add(0x6d2b_79f5);
            let mut t = state;
            t = (t ^ (t >> 15)).wrapping_mul(t | 1);
            t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
            u64::from(t ^ (t >> 14)) % bound
        }
    }

    fn entity(path: &str, start: u64, end: u64, symbol: &str) -> EntityFact {
        serde_json::from_value(serde_json::json!({
            "location": { "path": path, "startByte": start, "endByte": end },
            "symbol": symbol,
        }))
        .expect("an entity row")
    }

    fn table(rows: Vec<EntityFact>) -> TypeScriptTable {
        TypeScriptTable::from_parts(3, 1, "fixture", Vec::new(), rows, Vec::new(), Vec::new())
    }

    /// Rows of `paths`, sorted by path as a table holds them, with spans
    /// drawn from a small range so exact duplicates and nesting are common.
    fn rows_of(paths: &[&str], draw: &mut impl FnMut(u64) -> u64) -> Vec<EntityFact> {
        let mut rows = Vec::new();
        for path in paths {
            for index in 0..draw(40) {
                let start = draw(30);
                let end = start + draw(6);
                rows.push(entity(path, start, end, &format!("{path}#{index}")));
            }
        }
        rows.sort_by(|left, right| left.location.path.cmp(&right.location.path));
        rows
    }

    /// The whole-project map this index replaced: the first row in the table's
    /// order at each exact location.
    fn first_entity_by_location(
        table: &TypeScriptTable,
    ) -> HashMap<(String, u64, u64), EntityFact> {
        let mut map = HashMap::default();
        for entity in table.entities() {
            map.entry((
                entity.location.path.to_string(),
                entity.location.start_byte,
                entity.location.end_byte,
            ))
            .or_insert_with(|| entity.clone());
        }
        map
    }

    /// The smallest-contained scan this index replaced.
    fn smallest_contained_scan(
        table: &TypeScriptTable,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<EntityFact> {
        let mut entities = table
            .entities()
            .filter(|entity| entity.location.path.as_ref() == path)
            .collect::<Vec<_>>();
        entities.sort_by_key(|entity| entity.location.start_byte);
        let (start, end) = (u64::from(span.start), u64::from(span.end));
        let first = entities.partition_point(|entity| entity.location.start_byte < start);
        let last = entities.partition_point(|entity| entity.location.start_byte <= end);
        entities[first..last]
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                (entity.location.end_byte <= end && predicate(entity)).then_some((index, *entity))
            })
            .min_by_key(|(index, entity)| {
                (
                    entity.location.end_byte - entity.location.start_byte,
                    *index,
                )
            })
            .map(|(_, entity)| entity.clone())
    }

    fn assert_answers_like_the_scans(table: &TypeScriptTable, locations: &EntityLocations) {
        let reference = first_entity_by_location(table);
        for entity in table.entities() {
            let span = Span::new(
                u32::try_from(entity.location.start_byte).unwrap(),
                u32::try_from(entity.location.end_byte).unwrap(),
            );
            assert_eq!(
                locations.at(&entity.location.path, span),
                reference
                    .get(&(
                        entity.location.path.to_string(),
                        entity.location.start_byte,
                        entity.location.end_byte
                    ))
                    .as_ref()
                    .map(|row| *row),
                "{:?}",
                entity.location
            );
        }
        let mut draw = random(7);
        for path in ["/p/a.ts", "/p/b.ts", "/p/c.ts", "/p/absent.ts"] {
            for _ in 0..200 {
                let start = draw(40);
                let span = Span::new(start as u32, (start + draw(12)) as u32);
                let key = (path.to_owned(), u64::from(span.start), u64::from(span.end));
                assert_eq!(
                    locations.at(path, span),
                    reference.get(&key),
                    "{path} {span:?}"
                );
                for parity in [0, 1] {
                    let wanted = |row: &EntityFact| row.location.end_byte % 2 == parity;
                    assert_eq!(
                        locations.smallest_contained(path, span, wanted),
                        smallest_contained_scan(table, path, span, wanted).as_ref(),
                        "{path} {span:?} {parity}"
                    );
                }
            }
        }
    }

    /// An exact location answers the table's first row there, and the smallest
    /// contained entity is the scan's, whatever the rows' order: duplicates,
    /// nesting, equal starts, absent paths.
    #[test]
    fn the_index_answers_what_the_whole_project_scans_answered() {
        for seed in 0..40 {
            let mut draw = random(seed);
            let table = table(rows_of(&["/p/a.ts", "/p/b.ts", "/p/c.ts"], &mut draw));
            assert_answers_like_the_scans(&table, &EntityLocations::of(&table));
        }
    }

    /// Replacing the paths a delta names equals building the index again:
    /// rows changed, rows removed (a path that held some holds none) and a
    /// path the table gained.
    #[test]
    fn replacing_the_named_paths_equals_a_rebuild() {
        for seed in 0..40 {
            let mut draw = random(seed + 100);
            let before = table(rows_of(&["/p/a.ts", "/p/b.ts"], &mut draw));
            let mut locations = EntityLocations::of(&before);
            // `a` changes, `b` is removed, `c` is new.
            let after = table(rows_of(&["/p/a.ts", "/p/c.ts"], &mut draw));
            for path in ["/p/a.ts", "/p/b.ts", "/p/c.ts"] {
                locations.replace_path(&after, path);
            }
            assert_eq!(locations, EntityLocations::of(&after), "seed {seed}");
            assert_answers_like_the_scans(&after, &locations);
        }
    }

    /// A path no delta names keeps the rows it held: the same shared slice, so
    /// a build that patches one path rebuilds nothing of the others.
    #[test]
    fn an_unnamed_path_is_not_rebuilt() {
        let mut draw = random(3);
        let before = table(rows_of(&["/p/a.ts", "/p/b.ts"], &mut draw));
        let mut locations = EntityLocations::of(&before);
        let kept = Arc::clone(&locations.by_path["/p/b.ts"]);
        locations.replace_path(&before, "/p/a.ts");
        assert!(Arc::ptr_eq(&kept, &locations.by_path["/p/b.ts"]));
    }
}
