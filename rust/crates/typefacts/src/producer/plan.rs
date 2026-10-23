//! Transition planning (Go: `wire_transition_diff.go`): which path and symbol
//! operations take a client from its retained table to the target.

use super::table::ProducerTable;
use super::{EncodeError, TransportChanges};
use crate::{EntityFact, FileFact, Location, SourceDigest, SourceHash, SymbolFact};

#[derive(Clone, Copy, Debug)]
pub(super) enum Slot<T> {
    Unchanged,
    Replace(T),
    Remove,
}

impl<T> Slot<T> {
    pub(super) fn tag(&self) -> u64 {
        match self {
            Self::Unchanged => crate::v3::rows::SLOT_UNCHANGED,
            Self::Replace(_) => crate::v3::rows::SLOT_REPLACE,
            Self::Remove => crate::v3::rows::SLOT_REMOVE,
        }
    }

    fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

/// One path's operations, borrowing every replacement row.
#[derive(Clone, Copy, Debug)]
pub(super) struct PathOpRef<'a> {
    pub path: &'a str,
    pub source: Slot<&'a SourceHash>,
    pub entities: Slot<&'a [EntityFact]>,
    pub file: Slot<&'a FileFact>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SymbolBody<'a> {
    Replace(&'a SymbolFact),
    Remove,
    ReplaceReferencePath {
        path: &'a str,
        references: &'a [Location],
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SymbolOpRef<'a> {
    pub id: &'a str,
    pub body: SymbolBody<'a>,
}

impl SymbolBody<'_> {
    /// The two bits a delta symbol header carries under the id.
    pub(super) fn tag(&self) -> u64 {
        match self {
            Self::Replace(_) => 0,
            Self::Remove => 1,
            Self::ReplaceReferencePath { .. } => 2,
        }
    }
}

/// One path's rows in one table.
#[derive(Clone, Copy)]
struct PathRows<'a> {
    path: &'a str,
    source: Option<&'a SourceDigest>,
    entities: &'a [EntityFact],
    file: Option<&'a FileFact>,
}

impl<'a> PathRows<'a> {
    fn empty(path: &'a str) -> Self {
        Self {
            path,
            source: None,
            entities: &[],
            file: None,
        }
    }

    fn at(table: &'a ProducerTable, path: &'a str) -> Self {
        Self {
            path,
            source: table.source(path),
            entities: table.entities.path(path),
            file: table.file(path),
        }
    }
}

/// Which collections of a path may differ.
#[derive(Clone, Copy)]
struct Candidates {
    source: bool,
    entity: bool,
    file: bool,
}

const ALL: Candidates = Candidates {
    source: true,
    entity: true,
    file: true,
};

fn rows_difference<'a>(
    previous: PathRows<'a>,
    next: PathRows<'a>,
    candidates: Candidates,
) -> Option<PathOpRef<'a>> {
    let mut operation = PathOpRef {
        path: next.path,
        source: Slot::Unchanged,
        entities: Slot::Unchanged,
        file: Slot::Unchanged,
    };
    if candidates.source {
        match (previous.source, next.source) {
            (Some(_), None) => operation.source = Slot::Remove,
            (previous, Some(next)) if previous.is_none_or(|p| p.sha256 != next.sha256) => {
                operation.source = Slot::Replace(&next.sha256);
            }
            _ => {}
        }
    }
    if candidates.entity {
        if !previous.entities.is_empty() && next.entities.is_empty() {
            operation.entities = Slot::Remove;
        } else if !next.entities.is_empty() && previous.entities != next.entities {
            operation.entities = Slot::Replace(next.entities);
        }
    }
    if candidates.file {
        match (previous.file, next.file) {
            (Some(_), None) => operation.file = Slot::Remove,
            (previous, Some(next)) if previous.is_none_or(|p| p != next) => {
                operation.file = Slot::Replace(next);
            }
            _ => {}
        }
    }
    let changed = !operation.source.is_unchanged()
        || !operation.entities.is_unchanged()
        || !operation.file.is_unchanged();
    changed.then_some(operation)
}

fn path_difference<'a>(
    base: Option<&'a ProducerTable>,
    target: &'a ProducerTable,
    path: &'a str,
    candidates: Candidates,
) -> Option<PathOpRef<'a>> {
    let previous = base.map_or_else(|| PathRows::empty(path), |base| PathRows::at(base, path));
    rows_difference(previous, PathRows::at(target, path), candidates)
}

/// Merge-walks one table's three independently canonical path streams without
/// flattening or sorting them.
struct PathCursor<'a> {
    table: &'a ProducerTable,
    source: usize,
    entity: usize,
    file: usize,
}

impl<'a> PathCursor<'a> {
    fn new(table: &'a ProducerTable) -> Self {
        Self {
            table,
            source: 0,
            entity: 0,
            file: 0,
        }
    }

    fn valid(&self) -> bool {
        self.source < self.table.sources.len()
            || self.entity < self.table.entities.cursor_len()
            || self.file < self.table.files.len()
    }

    fn path(&self) -> &'a str {
        let table = self.table;
        let sources = table.sources.get(self.source).map(|row| row.path.as_ref());
        let entities = (self.entity < table.entities.cursor_len())
            .then(|| table.entities.cursor_path(self.entity));
        let files = table.files.get(self.file).map(|row| row.path.as_ref());
        [sources, entities, files]
            .into_iter()
            .flatten()
            .min()
            .expect("a valid cursor has a head")
    }

    fn rows(&self) -> PathRows<'a> {
        let table = self.table;
        let path = self.path();
        let mut rows = PathRows::empty(path);
        if let Some(source) = table.sources.get(self.source)
            && source.path.as_ref() == path
        {
            rows.source = Some(source);
        }
        if self.entity < table.entities.cursor_len()
            && table.entities.cursor_path(self.entity) == path
        {
            rows.entities = table.entities.cursor_run(self.entity);
        }
        if let Some(file) = table.files.get(self.file)
            && file.path.as_ref() == path
        {
            rows.file = Some(file);
        }
        rows
    }

    fn next(&mut self) {
        let table = self.table;
        let path = self.path();
        if table
            .sources
            .get(self.source)
            .is_some_and(|row| row.path.as_ref() == path)
        {
            self.source += 1;
        }
        if self.entity < table.entities.cursor_len()
            && table.entities.cursor_path(self.entity) == path
        {
            self.entity = table.entities.cursor_next(self.entity);
        }
        if table
            .files
            .get(self.file)
            .is_some_and(|row| row.path.as_ref() == path)
        {
            self.file += 1;
        }
    }
}

/// Every path of a full transition, each carrying all of its rows.
pub(super) fn plan_full(target: &ProducerTable) -> Result<Vec<PathOpRef<'_>>, EncodeError> {
    let mut operations = Vec::new();
    let mut cursor = PathCursor::new(target);
    while cursor.valid() {
        let path = cursor.path();
        let Some(operation) = path_difference(None, target, path, ALL) else {
            return Err(EncodeError::invalid(format!(
                "encode path operation {}: full path {path:?} has no rows",
                operations.len()
            )));
        };
        operations.push(operation);
        cursor.next();
    }
    Ok(operations)
}

fn sorted_manifest_paths(manifest: &TransportChanges) -> Vec<&str> {
    let mut paths: Vec<&str> = manifest
        .source_paths
        .iter()
        .chain(&manifest.entity_paths)
        .chain(&manifest.file_paths)
        .map(String::as_str)
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

/// A delta whose manifest names the exact candidate paths and symbols.
pub(super) fn plan_exact_path_delta<'a>(
    base: &'a ProducerTable,
    target: &'a ProducerTable,
    manifest: &'a TransportChanges,
) -> Vec<PathOpRef<'a>> {
    sorted_manifest_paths(manifest)
        .into_iter()
        .filter_map(|path| {
            let candidates = Candidates {
                source: manifest.source_paths.contains(path),
                entity: manifest.entity_paths.contains(path),
                file: manifest.file_paths.contains(path),
            };
            path_difference(Some(base), target, path, candidates)
        })
        .collect()
}

/// A delta found by walking both tables when no manifest vouches for one.
pub(super) fn plan_fallback_path_delta<'a>(
    base: &'a ProducerTable,
    target: &'a ProducerTable,
) -> Vec<PathOpRef<'a>> {
    let mut operations = Vec::new();
    let mut previous = PathCursor::new(base);
    let mut next = PathCursor::new(target);
    let mut push = |previous: PathRows<'a>, next: PathRows<'a>| {
        operations.extend(rows_difference(previous, next, ALL));
    };
    while previous.valid() && next.valid() {
        let (previous_rows, next_rows) = (previous.rows(), next.rows());
        match previous_rows.path.cmp(next_rows.path) {
            std::cmp::Ordering::Less => {
                push(previous_rows, PathRows::empty(previous_rows.path));
                previous.next();
            }
            std::cmp::Ordering::Greater => {
                push(PathRows::empty(next_rows.path), next_rows);
                next.next();
            }
            std::cmp::Ordering::Equal => {
                push(previous_rows, next_rows);
                previous.next();
                next.next();
            }
        }
    }
    while previous.valid() {
        let rows = previous.rows();
        push(rows, PathRows::empty(rows.path));
        previous.next();
    }
    while next.valid() {
        let rows = next.rows();
        push(PathRows::empty(rows.path), rows);
        next.next();
    }
    operations
}

/// Unconditional upserts and removals for exactly the manifest's paths. The
/// client owns the base table, so no expanded base rows are needed.
pub(super) fn plan_sparse_paths<'a>(
    target: &'a ProducerTable,
    manifest: &'a TransportChanges,
) -> Vec<PathOpRef<'a>> {
    sorted_manifest_paths(manifest)
        .into_iter()
        .map(|path| {
            let rows = PathRows::at(target, path);
            let mut operation = PathOpRef {
                path,
                source: Slot::Unchanged,
                entities: Slot::Unchanged,
                file: Slot::Unchanged,
            };
            if manifest.source_paths.contains(path) {
                operation.source = rows
                    .source
                    .map_or(Slot::Remove, |source| Slot::Replace(&source.sha256));
            }
            if manifest.entity_paths.contains(path) {
                operation.entities = if rows.entities.is_empty() {
                    Slot::Remove
                } else {
                    Slot::Replace(rows.entities)
                };
            }
            if manifest.file_paths.contains(path) {
                operation.file = rows.file.map_or(Slot::Remove, Slot::Replace);
            }
            operation
        })
        .collect()
}

/// Symbol operations for exactly the manifest's candidate symbols.
pub(super) fn plan_exact_symbol_delta<'a>(
    base: &'a ProducerTable,
    target: &'a ProducerTable,
    manifest: &'a TransportChanges,
) -> Vec<SymbolOpRef<'a>> {
    let mut ids: Vec<&str> = manifest.symbol_ids.iter().map(AsRef::as_ref).collect();
    ids.sort_unstable();
    let mut operations = Vec::new();
    for id in ids {
        match (base.symbol(id), target.symbol(id)) {
            (Some(_), None) => operations.push(SymbolOpRef {
                id,
                body: SymbolBody::Remove,
            }),
            (None, Some(next)) => operations.push(SymbolOpRef {
                id,
                body: SymbolBody::Replace(next),
            }),
            (Some(previous), Some(next)) => {
                if previous.alias_target != next.alias_target
                    || previous.declarations != next.declarations
                {
                    operations.push(SymbolOpRef {
                        id,
                        body: SymbolBody::Replace(next),
                    });
                } else {
                    plan_exact_reference_delta(
                        id,
                        &previous.references,
                        &next.references,
                        &mut operations,
                    );
                }
            }
            (None, None) => {}
        }
    }
    operations
}

/// Merge-walks the two canonical path runs of one candidate symbol. This
/// derives the exact operations directly instead of multiplying every
/// candidate symbol by every changed source path.
pub(super) fn plan_exact_reference_delta<'a>(
    id: &'a str,
    mut previous: &'a [Location],
    mut next: &'a [Location],
    operations: &mut Vec<SymbolOpRef<'a>>,
) {
    while !previous.is_empty() || !next.is_empty() {
        let path: &'a str = match (previous.first(), next.first()) {
            (None, Some(next)) => &next.path,
            (Some(previous), None) => &previous.path,
            (Some(previous_first), Some(next_first)) => {
                if previous_first.path < next_first.path {
                    &previous_first.path
                } else {
                    &next_first.path
                }
            }
            (None, None) => unreachable!("the loop guard saw a row"),
        };
        let previous_end = reference_path_end(previous, path);
        let next_end = reference_path_end(next, path);
        let (previous_run, next_run) = (&previous[..previous_end], &next[..next_end]);
        if previous_run != next_run {
            operations.push(SymbolOpRef {
                id,
                body: SymbolBody::ReplaceReferencePath {
                    path,
                    references: next_run,
                },
            });
        }
        previous = &previous[previous_end..];
        next = &next[next_end..];
    }
}

fn reference_path_end(references: &[Location], path: &str) -> usize {
    references
        .iter()
        .take_while(|reference| reference.path.as_ref() == path)
        .count()
}

/// A symbol delta found by merging both tables' symbol rows.
pub(super) fn plan_fallback_symbol_delta<'a>(
    base: &'a ProducerTable,
    target: &'a ProducerTable,
) -> Vec<SymbolOpRef<'a>> {
    let mut operations = Vec::new();
    let (mut left, mut right) = (
        base.symbols.iter().peekable(),
        target.symbols.iter().peekable(),
    );
    while let (Some(previous), Some(next)) = (left.peek(), right.peek()) {
        match previous.id.cmp(&next.id) {
            std::cmp::Ordering::Less => {
                operations.push(SymbolOpRef {
                    id: &previous.id,
                    body: SymbolBody::Remove,
                });
                left.next();
            }
            std::cmp::Ordering::Greater => {
                operations.push(SymbolOpRef {
                    id: &next.id,
                    body: SymbolBody::Replace(next),
                });
                right.next();
            }
            std::cmp::Ordering::Equal => {
                if previous != next {
                    operations.push(SymbolOpRef {
                        id: &next.id,
                        body: SymbolBody::Replace(next),
                    });
                }
                left.next();
                right.next();
            }
        }
    }
    operations.extend(left.map(|fact| SymbolOpRef {
        id: &fact.id,
        body: SymbolBody::Remove,
    }));
    operations.extend(right.map(|fact| SymbolOpRef {
        id: &fact.id,
        body: SymbolBody::Replace(fact),
    }));
    operations
}
