use std::{fmt, sync::Arc};

use crate::{
    Declaration, EntityFact, FileFact, Location, SourceDigest, SymbolFact,
    v3::{PathOp, SlotOp, SymbolOp, TransitionMode, WireTableTransition},
};

const LEAF_TARGET: usize = 64;
const LEAF_MAX: usize = LEAF_TARGET * 2;
const LINEAR_REBUILD_MIN_OPERATIONS: usize = 32;

trait Keyed {
    fn key(&self) -> &str;
}

/// A packed, immutable, ordered index.
///
/// Clones share both the root directory and every leaf. A sparse edit copies
/// one small leaf plus the root's `Arc` pointers; a broad edit is rebuilt with
/// one canonical linear merge. This is deliberately a private storage detail:
/// callers query [`FactTable`] instead of depending on row contiguity.
#[derive(Clone)]
struct PackedIndex<T> {
    leaves: Arc<[Arc<[T]>]>,
    len: usize,
}

impl<T> Default for PackedIndex<T> {
    fn default() -> Self {
        Self {
            leaves: Arc::from([]),
            len: 0,
        }
    }
}

impl<T> PackedIndex<T> {
    fn from_sorted(rows: Vec<T>) -> Self {
        let len = rows.len();
        if rows.is_empty() {
            return Self::default();
        }
        let mut leaves = Vec::with_capacity(len.div_ceil(LEAF_TARGET));
        let mut rows = rows.into_iter();
        loop {
            let leaf = rows.by_ref().take(LEAF_TARGET).collect::<Vec<_>>();
            if leaf.is_empty() {
                break;
            }
            leaves.push(Arc::from(leaf));
        }
        Self {
            leaves: leaves.into(),
            len,
        }
    }

    fn iter(&self) -> PackedIter<'_, T> {
        PackedIter {
            leaves: self.leaves.iter(),
            current: None,
            remaining: self.len,
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<T: Keyed> PackedIndex<T> {
    fn get(&self, key: &str) -> Option<&T> {
        let leaf = self.leaf_for(key)?;
        leaf.binary_search_by(|row| row.key().cmp(key))
            .ok()
            .map(|index| &leaf[index])
    }

    fn leaf_for(&self, key: &str) -> Option<&[T]> {
        if self.leaves.is_empty() {
            return None;
        }
        let index = self
            .leaves
            .partition_point(|leaf| leaf.last().expect("non-empty leaf").key() < key)
            .min(self.leaves.len() - 1);
        Some(&self.leaves[index])
    }
}

impl<T: Clone + Keyed> PackedIndex<T> {
    fn updated(&self, key: &str, replacement: Option<T>) -> Self {
        if self.leaves.is_empty() {
            return replacement.map_or_else(Self::default, |row| Self::from_sorted(vec![row]));
        }

        let mut leaves = self.leaves.to_vec();
        let leaf_index = leaves
            .partition_point(|leaf| leaf.last().expect("non-empty leaf").key() < key)
            .min(leaves.len() - 1);
        let mut leaf = leaves[leaf_index].to_vec();
        match leaf.binary_search_by(|row| row.key().cmp(key)) {
            Ok(index) => match replacement {
                Some(row) => leaf[index] = row,
                None => {
                    leaf.remove(index);
                }
            },
            Err(index) => {
                if let Some(row) = replacement {
                    leaf.insert(index, row);
                }
            }
        }

        if leaf.is_empty() {
            leaves.remove(leaf_index);
        } else if leaf.len() > LEAF_MAX {
            let right = leaf.split_off(LEAF_TARGET);
            leaves[leaf_index] = Arc::from(leaf);
            leaves.insert(leaf_index + 1, Arc::from(right));
        } else {
            leaves[leaf_index] = Arc::from(leaf);
        }

        let len = leaves.iter().map(|leaf| leaf.len()).sum();
        Self {
            leaves: leaves.into(),
            len,
        }
    }
}

struct PackedIter<'a, T> {
    leaves: std::slice::Iter<'a, Arc<[T]>>,
    current: Option<std::slice::Iter<'a, T>>,
    remaining: usize,
}

impl<'a, T> Iterator for PackedIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(row) = self.current.as_mut().and_then(Iterator::next) {
                self.remaining -= 1;
                return Some(row);
            }
            self.current = Some(self.leaves.next()?.iter());
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for PackedIter<'_, T> {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathEntry {
    path: Arc<str>,
    source: Option<SourceDigest>,
    entities: Arc<[EntityFact]>,
    file: Option<FileFact>,
}

impl Keyed for PathEntry {
    fn key(&self) -> &str {
        &self.path
    }
}

impl PathEntry {
    fn is_empty(&self) -> bool {
        self.source.is_none() && self.entities.is_empty() && self.file.is_none()
    }

    fn apply(mut self, operation: PathOp) -> Option<Self> {
        match operation.source {
            SlotOp::Unchanged => {}
            SlotOp::Replace(sha256) => {
                self.source = Some(SourceDigest {
                    path: operation.path.clone(),
                    sha256,
                });
            }
            SlotOp::Remove => self.source = None,
        }
        match operation.entities {
            SlotOp::Unchanged => {}
            SlotOp::Replace(entities) => self.entities = entities.into(),
            SlotOp::Remove => self.entities = Arc::from([]),
        }
        match operation.file {
            SlotOp::Unchanged => {}
            SlotOp::Replace(file) => self.file = Some(file),
            SlotOp::Remove => self.file = None,
        }
        (!self.is_empty()).then_some(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReferenceRun {
    path: Arc<str>,
    locations: Arc<[Location]>,
}

impl Keyed for ReferenceRun {
    fn key(&self) -> &str {
        &self.path
    }
}

#[derive(Clone)]
struct StoredSymbol {
    id: Arc<str>,
    alias_target: Arc<str>,
    declarations: Arc<[Declaration]>,
    references: PackedIndex<ReferenceRun>,
    reference_count: usize,
}

impl Keyed for StoredSymbol {
    fn key(&self) -> &str {
        &self.id
    }
}

impl StoredSymbol {
    fn from_fact(symbol: SymbolFact) -> Self {
        let mut runs = Vec::new();
        let mut references = symbol.references.iter().cloned().peekable();
        while let Some(first) = references.next() {
            let path = first.path.clone();
            let mut locations = vec![first];
            while references
                .peek()
                .is_some_and(|location| location.path == path)
            {
                locations.push(references.next().expect("peeked reference"));
            }
            runs.push(ReferenceRun {
                path,
                locations: locations.into(),
            });
        }
        let reference_count = symbol.references.len();
        Self {
            id: symbol.id,
            alias_target: symbol.alias_target,
            declarations: symbol.declarations,
            references: PackedIndex::from_sorted(runs),
            reference_count,
        }
    }

    fn replace_reference_path(mut self, path: Arc<str>, references: Vec<Location>) -> Self {
        let old_len = self
            .references
            .get(&path)
            .map_or(0, |run| run.locations.len());
        let new_len = references.len();
        let replacement = (!references.is_empty()).then(|| ReferenceRun {
            path: path.clone(),
            locations: references.into(),
        });
        self.references = self.references.updated(&path, replacement);
        self.reference_count = self.reference_count - old_len + new_len;
        self
    }

    fn to_fact(&self) -> SymbolFact {
        SymbolFact {
            id: Arc::clone(&self.id),
            alias_target: Arc::clone(&self.alias_target),
            declarations: Arc::clone(&self.declarations),
            references: self
                .references
                .iter()
                .flat_map(|run| run.locations.iter().cloned())
                .collect::<Vec<_>>()
                .into(),
        }
    }
}

impl fmt::Debug for StoredSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredSymbol")
            .field("id", &self.id)
            .field("alias_target", &self.alias_target)
            .field("declarations", &self.declarations)
            .field("references", &self.references.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl PartialEq for StoredSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.alias_target == other.alias_target
            && self.declarations == other.declarations
            && self.reference_count == other.reference_count
            && self.references.iter().eq(other.references.iter())
    }
}

impl Eq for StoredSymbol {}

/// A read-only view of one symbol in a retained table.
#[derive(Clone, Copy, Debug)]
pub struct Symbol<'a> {
    inner: &'a StoredSymbol,
}

impl<'a> Symbol<'a> {
    #[must_use]
    pub fn id(self) -> &'a str {
        &self.inner.id
    }

    #[must_use]
    pub fn alias_target(self) -> &'a str {
        &self.inner.alias_target
    }

    #[must_use]
    pub fn declarations(self) -> &'a [Declaration] {
        &self.inner.declarations
    }

    pub fn references(self) -> impl Iterator<Item = &'a Location> {
        self.inner
            .references
            .iter()
            .flat_map(|run| run.locations.iter())
    }
}

/// An immutable, path-queryable table of semantic facts.
///
/// Its storage is intentionally opaque. Keeping an earlier table alive is
/// cheap, and applying a sparse successor shares every untouched leaf.
#[derive(Clone)]
pub struct FactTable {
    schema: u64,
    generation: u64,
    project_id: Arc<str>,
    paths: PackedIndex<PathEntry>,
    symbols: PackedIndex<StoredSymbol>,
    source_count: usize,
    entity_count: usize,
    file_count: usize,
}

impl FactTable {
    pub(crate) fn symbol_fact(&self, id: &str) -> Option<SymbolFact> {
        self.symbols.get(id).map(StoredSymbol::to_fact)
    }

    pub(crate) fn replace_symbols(&mut self, mut symbols: Vec<SymbolFact>) -> Vec<String> {
        symbols.sort_by(|left, right| left.id.cmp(&right.id));
        let next = symbols
            .into_iter()
            .map(StoredSymbol::from_fact)
            .collect::<Vec<_>>();
        let mut previous = self.symbols.iter().peekable();
        let mut successor = next.iter().peekable();
        let mut changed = Vec::new();
        while previous.peek().is_some() || successor.peek().is_some() {
            match (previous.peek(), successor.peek()) {
                (Some(left), Some(right)) if left.id == right.id => {
                    if left != right {
                        changed.push(left.id.to_string());
                    }
                    previous.next();
                    successor.next();
                }
                (Some(left), Some(right)) if left.id < right.id => {
                    changed.push(left.id.to_string());
                    previous.next();
                }
                (Some(_), Some(right)) => {
                    changed.push(right.id.to_string());
                    successor.next();
                }
                (Some(left), None) => {
                    changed.push(left.id.to_string());
                    previous.next();
                }
                (None, Some(right)) => {
                    changed.push(right.id.to_string());
                    successor.next();
                }
                (None, None) => break,
            }
        }
        self.symbols = PackedIndex::from_sorted(next);
        changed
    }

    /// The id and alias target of `id`'s row, shared.
    pub(crate) fn symbol_alias_target(&self, id: &str) -> Option<&Arc<str>> {
        self.symbols.get(id).map(|symbol| &symbol.alias_target)
    }

    /// Every symbol id, shared, in id order.
    pub(crate) fn symbol_ids(&self) -> impl Iterator<Item = &Arc<str>> {
        self.symbols.iter().map(|symbol| &symbol.id)
    }

    /// Replaces the rows of `upserts`, adds the new ones and drops every id
    /// in `removed`, in one ordered pass; returns the ids whose row was
    /// added, removed or changed, in id order (what [`Self::replace_symbols`]
    /// returns for the same successor set).
    pub(crate) fn merge_symbols(
        &mut self,
        upserts: Vec<SymbolFact>,
        removed: &std::collections::HashSet<Arc<str>>,
    ) -> Vec<String> {
        let mut upserts = upserts
            .into_iter()
            .map(StoredSymbol::from_fact)
            .collect::<Vec<_>>();
        upserts.sort_by(|left, right| left.id.cmp(&right.id));
        upserts.dedup_by(|later, earlier| later.id == earlier.id);
        let mut next = Vec::with_capacity(self.symbols.len() + upserts.len());
        let mut changed = Vec::new();
        let mut previous = self.symbols.iter().peekable();
        let mut successor = upserts.into_iter().peekable();
        loop {
            let order = match (previous.peek(), successor.peek()) {
                (None, None) => break,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(left), Some(right)) => left.id.cmp(&right.id),
            };
            match order {
                std::cmp::Ordering::Equal => {
                    let replacement = successor.next().expect("peeked upsert");
                    let left = previous.next().expect("peeked row");
                    if *left == replacement {
                        next.push(left.clone());
                    } else {
                        changed.push(left.id.to_string());
                        next.push(replacement);
                    }
                }
                std::cmp::Ordering::Less => {
                    let left = previous.next().expect("peeked row");
                    if removed.contains(&left.id) {
                        changed.push(left.id.to_string());
                    } else {
                        next.push(left.clone());
                    }
                }
                std::cmp::Ordering::Greater => {
                    let right = successor.next().expect("peeked upsert");
                    changed.push(right.id.to_string());
                    next.push(right);
                }
            }
        }
        if !changed.is_empty() {
            self.symbols = PackedIndex::from_sorted(next);
        }
        changed
    }

    #[must_use]
    pub const fn schema(&self) -> u64 {
        self.schema
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    #[must_use]
    pub fn source(&self, path: &str) -> Option<&SourceDigest> {
        self.paths.get(path).and_then(|entry| entry.source.as_ref())
    }

    #[must_use]
    pub fn entities_for_path(&self, path: &str) -> &[EntityFact] {
        self.paths
            .get(path)
            .map_or(&[], |entry| entry.entities.as_ref())
    }

    #[must_use]
    pub fn file(&self, path: &str) -> Option<&FileFact> {
        self.paths.get(path).and_then(|entry| entry.file.as_ref())
    }

    #[must_use]
    pub fn symbol(&self, id: &str) -> Option<Symbol<'_>> {
        self.symbols.get(id).map(|inner| Symbol { inner })
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceDigest> {
        self.paths.iter().filter_map(|entry| entry.source.as_ref())
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntityFact> {
        self.paths.iter().flat_map(|entry| entry.entities.iter())
    }

    pub fn files(&self) -> impl Iterator<Item = &FileFact> {
        self.paths.iter().filter_map(|entry| entry.file.as_ref())
    }

    pub fn symbols(&self) -> impl ExactSizeIterator<Item = Symbol<'_>> {
        self.symbols.iter().map(|inner| Symbol { inner })
    }

    /// The ids of the symbols whose alias target or declarations differ
    /// between `previous` and this table, added and removed ones included,
    /// in id order. Only the index leaves the two tables do not share are
    /// compared (a leaf both hold is the same rows), so the cost follows
    /// what changed between them, whatever sequence of updates led from one
    /// to the other.
    #[must_use]
    pub fn symbols_with_moved_declarations(&self, previous: &FactTable) -> Vec<Arc<str>> {
        let held = previous
            .symbols
            .leaves
            .iter()
            .map(|leaf| leaf.as_ptr())
            .collect::<std::collections::HashSet<_>>();
        let shared = self
            .symbols
            .leaves
            .iter()
            .map(|leaf| leaf.as_ptr())
            .filter(|leaf| held.contains(leaf))
            .collect::<std::collections::HashSet<_>>();
        fn unshared<'t>(
            index: &'t PackedIndex<StoredSymbol>,
            shared: &std::collections::HashSet<*const StoredSymbol>,
        ) -> Vec<&'t StoredSymbol> {
            index
                .leaves
                .iter()
                .filter(|leaf| !shared.contains(&leaf.as_ptr()))
                .flat_map(|leaf| leaf.iter())
                .collect()
        }
        let (before, after) = (
            unshared(&previous.symbols, &shared),
            unshared(&self.symbols, &shared),
        );
        let same = |left: &StoredSymbol, right: &StoredSymbol| {
            (Arc::ptr_eq(&left.alias_target, &right.alias_target)
                || left.alias_target == right.alias_target)
                && (Arc::ptr_eq(&left.declarations, &right.declarations)
                    || left.declarations == right.declarations)
        };
        let mut moved = Vec::new();
        let (mut before, mut after) = (before.into_iter().peekable(), after.into_iter().peekable());
        loop {
            match (before.peek(), after.peek()) {
                (None, None) => break,
                (Some(left), Some(right)) if left.id == right.id => {
                    if !same(left, right) {
                        moved.push(Arc::clone(&right.id));
                    }
                    before.next();
                    after.next();
                }
                (Some(left), right) if right.is_none_or(|right| left.id < right.id) => {
                    moved.push(Arc::clone(&left.id));
                    before.next();
                }
                (_, right) => {
                    let right = right.expect("a row remains on the right");
                    moved.push(Arc::clone(&right.id));
                    after.next();
                }
            }
        }
        moved
    }

    #[must_use]
    pub const fn source_count(&self) -> usize {
        self.source_count
    }

    #[must_use]
    pub const fn entity_count(&self) -> usize {
        self.entity_count
    }

    #[must_use]
    pub const fn file_count(&self) -> usize {
        self.file_count
    }

    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    #[cfg(test)]
    pub(crate) fn path_root_identity(&self) -> usize {
        Arc::as_ptr(&self.paths.leaves) as *const () as usize
    }

    #[cfg(test)]
    pub(crate) fn path_leaf_identity(&self, path: &str) -> Option<usize> {
        self.paths
            .leaf_for(path)
            .map(|leaf| leaf.as_ptr() as *const () as usize)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        schema: u64,
        generation: u64,
        project_id: impl Into<Arc<str>>,
        sources: Vec<SourceDigest>,
        entities: Vec<EntityFact>,
        symbols: Vec<SymbolFact>,
        files: Vec<FileFact>,
    ) -> Self {
        let mut paths = Vec::<PathEntry>::new();
        for source in sources {
            upsert_path_part(&mut paths, source.path.clone(), |entry| {
                entry.source = Some(source);
            });
        }
        for entity in entities {
            let path = entity.location.path.clone();
            upsert_path_part(&mut paths, path, |entry| {
                let mut entities = entry.entities.to_vec();
                entities.push(entity);
                entry.entities = entities.into();
            });
        }
        for file in files {
            upsert_path_part(&mut paths, file.path.clone(), |entry| {
                entry.file = Some(file);
            });
        }
        let source_count = paths.iter().filter(|entry| entry.source.is_some()).count();
        let entity_count = paths.iter().map(|entry| entry.entities.len()).sum();
        let file_count = paths.iter().filter(|entry| entry.file.is_some()).count();
        Self {
            schema,
            generation,
            project_id: project_id.into(),
            paths: PackedIndex::from_sorted(paths),
            symbols: PackedIndex::from_sorted(
                symbols.into_iter().map(StoredSymbol::from_fact).collect(),
            ),
            source_count,
            entity_count,
            file_count,
        }
    }

    pub(crate) fn materialize_full(transition: WireTableTransition) -> Self {
        debug_assert_eq!(transition.mode, TransitionMode::Full);
        let WireTableTransition {
            table_schema,
            target_generation,
            project_id,
            paths,
            symbols,
            ..
        } = transition;
        let mut entries = Vec::with_capacity(paths.len());
        for operation in paths {
            let entry = PathEntry {
                path: operation.path.clone(),
                source: match operation.source {
                    SlotOp::Replace(sha256) => Some(SourceDigest {
                        path: operation.path.clone(),
                        sha256,
                    }),
                    SlotOp::Unchanged | SlotOp::Remove => None,
                },
                entities: match operation.entities {
                    SlotOp::Replace(entities) => entities.into(),
                    SlotOp::Unchanged | SlotOp::Remove => Arc::from([]),
                },
                file: match operation.file {
                    SlotOp::Replace(file) => Some(file),
                    SlotOp::Unchanged | SlotOp::Remove => None,
                },
            };
            if !entry.is_empty() {
                entries.push(entry);
            }
        }
        let symbols = symbols
            .into_iter()
            .map(|operation| match operation {
                SymbolOp::Replace(symbol) => StoredSymbol::from_fact(symbol),
                SymbolOp::Remove { .. } | SymbolOp::ReplaceReferencePath { .. } => {
                    unreachable!("decoder rejects non-replace full symbol operations")
                }
            })
            .collect::<Vec<_>>();
        let source_count = entries
            .iter()
            .filter(|entry| entry.source.is_some())
            .count();
        let entity_count = entries.iter().map(|entry| entry.entities.len()).sum();
        let file_count = entries.iter().filter(|entry| entry.file.is_some()).count();
        Self {
            schema: table_schema,
            generation: target_generation,
            project_id,
            paths: PackedIndex::from_sorted(entries),
            symbols: PackedIndex::from_sorted(symbols),
            source_count,
            entity_count,
            file_count,
        }
    }

    pub(crate) fn validate_delta(&self, transition: &WireTableTransition) -> Result<(), String> {
        let mut current_id = "";
        let mut exists = false;
        let mut alias = false;
        for operation in &transition.symbols {
            let id = operation.id().as_ref();
            if id != current_id {
                let retained = self.symbols.get(id);
                exists = retained.is_some();
                alias = retained.is_some_and(|symbol| !symbol.alias_target.is_empty());
                current_id = id;
            }
            match operation {
                SymbolOp::Replace(symbol) => {
                    exists = true;
                    alias = !symbol.alias_target.is_empty();
                }
                SymbolOp::Remove { .. } => {
                    exists = false;
                    alias = false;
                }
                SymbolOp::ReplaceReferencePath { .. } if !exists => {
                    return Err(format!("reference operation names missing symbol {id:?}"));
                }
                SymbolOp::ReplaceReferencePath { .. } if alias => {
                    return Err(format!("reference operation names alias symbol {id:?}"));
                }
                SymbolOp::ReplaceReferencePath { .. } => {}
            }
        }
        Ok(())
    }

    pub(crate) fn apply_delta(&self, transition: WireTableTransition) -> Self {
        debug_assert_eq!(transition.mode, TransitionMode::Delta);
        let WireTableTransition {
            table_schema,
            target_generation,
            paths,
            symbols,
            ..
        } = transition;
        let mut next = self.clone();
        next.schema = table_schema;
        next.generation = target_generation;
        if paths.len() >= LINEAR_REBUILD_MIN_OPERATIONS {
            next.paths = apply_path_operations_linear(&self.paths, paths);
            next.source_count = next
                .paths
                .iter()
                .filter(|entry| entry.source.is_some())
                .count();
            next.entity_count = next.paths.iter().map(|entry| entry.entities.len()).sum();
            next.file_count = next
                .paths
                .iter()
                .filter(|entry| entry.file.is_some())
                .count();
        } else {
            let (paths, source_count, entity_count, file_count) =
                apply_path_operations_sparse(self, paths);
            next.paths = paths;
            next.source_count = source_count;
            next.entity_count = entity_count;
            next.file_count = file_count;
        }
        next.symbols = if symbols.len() >= LINEAR_REBUILD_MIN_OPERATIONS {
            apply_symbol_operations_linear(&self.symbols, symbols)
        } else {
            apply_symbol_operations_sparse(&self.symbols, symbols)
        };
        next
    }
}

#[cfg(test)]
fn upsert_path_part(
    paths: &mut Vec<PathEntry>,
    path: Arc<str>,
    update: impl FnOnce(&mut PathEntry),
) {
    let index = paths.partition_point(|entry| entry.path < path);
    if paths.get(index).is_none_or(|entry| entry.path != path) {
        paths.insert(
            index,
            PathEntry {
                path: path.clone(),
                source: None,
                entities: Arc::from([]),
                file: None,
            },
        );
    }
    update(&mut paths[index]);
}

fn apply_path_operations_sparse(
    retained: &FactTable,
    operations: Vec<PathOp>,
) -> (PackedIndex<PathEntry>, usize, usize, usize) {
    let mut result = retained.paths.clone();
    let mut source_count = retained.source_count;
    let mut entity_count = retained.entity_count;
    let mut file_count = retained.file_count;
    for operation in operations {
        let path = operation.path.clone();
        let existing = result.get(&path).cloned();
        let entry = existing.clone().unwrap_or(PathEntry {
            path: path.clone(),
            source: None,
            entities: Arc::from([]),
            file: None,
        });
        let replacement = entry.apply(operation);
        source_count = source_count
            - usize::from(existing.as_ref().is_some_and(|e| e.source.is_some()))
            + usize::from(replacement.as_ref().is_some_and(|e| e.source.is_some()));
        entity_count = entity_count - existing.as_ref().map_or(0, |entry| entry.entities.len())
            + replacement.as_ref().map_or(0, |entry| entry.entities.len());
        file_count = file_count - usize::from(existing.as_ref().is_some_and(|e| e.file.is_some()))
            + usize::from(replacement.as_ref().is_some_and(|e| e.file.is_some()));
        result = result.updated(&path, replacement);
    }
    (result, source_count, entity_count, file_count)
}

fn apply_path_operations_linear(
    retained: &PackedIndex<PathEntry>,
    operations: Vec<PathOp>,
) -> PackedIndex<PathEntry> {
    let mut retained = retained.iter().peekable();
    let mut merged = Vec::with_capacity(retained.len() + operations.len());
    for operation in operations {
        let path = operation.path.clone();
        while retained.peek().is_some_and(|entry| entry.path < path) {
            merged.push((*retained.next().expect("peeked path")).clone());
        }
        let entry = if retained.peek().is_some_and(|entry| entry.path == path) {
            retained.next().expect("peeked path").clone()
        } else {
            PathEntry {
                path: path.clone(),
                source: None,
                entities: Arc::from([]),
                file: None,
            }
        };
        if let Some(replacement) = entry.apply(operation) {
            merged.push(replacement);
        }
    }
    merged.extend(retained.cloned());
    PackedIndex::from_sorted(merged)
}

fn apply_one_symbol(symbol: Option<StoredSymbol>, operation: SymbolOp) -> Option<StoredSymbol> {
    match operation {
        SymbolOp::Replace(symbol) => Some(StoredSymbol::from_fact(symbol)),
        SymbolOp::Remove { .. } => None,
        SymbolOp::ReplaceReferencePath {
            path, references, ..
        } => Some(
            symbol
                .expect("reference operation validated before application")
                .replace_reference_path(path, references),
        ),
    }
}

fn apply_symbol_operations_sparse(
    retained: &PackedIndex<StoredSymbol>,
    operations: Vec<SymbolOp>,
) -> PackedIndex<StoredSymbol> {
    let mut result = retained.clone();
    for operation in operations {
        let id = operation.id().clone();
        let replacement = apply_one_symbol(result.get(&id).cloned(), operation);
        result = result.updated(&id, replacement);
    }
    result
}

fn apply_symbol_operations_linear(
    retained: &PackedIndex<StoredSymbol>,
    operations: Vec<SymbolOp>,
) -> PackedIndex<StoredSymbol> {
    let mut retained = retained.iter().peekable();
    let mut operations = operations.into_iter().peekable();
    let mut merged = Vec::with_capacity(retained.len() + operations.len());
    while let Some(operation) = operations.peek() {
        let id = operation.id().clone();
        while retained.peek().is_some_and(|symbol| symbol.id < id) {
            merged.push((*retained.next().expect("peeked symbol")).clone());
        }
        let mut symbol = if retained.peek().is_some_and(|symbol| symbol.id == id) {
            Some(retained.next().expect("peeked symbol").clone())
        } else {
            None
        };
        while operations
            .peek()
            .is_some_and(|operation| operation.id().as_ref() == id.as_ref())
        {
            symbol = apply_one_symbol(symbol, operations.next().expect("peeked symbol operation"));
        }
        if let Some(symbol) = symbol {
            merged.push(symbol);
        }
    }
    merged.extend(retained.cloned());
    PackedIndex::from_sorted(merged)
}

impl fmt::Debug for FactTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FactTable")
            .field("schema", &self.schema)
            .field("generation", &self.generation)
            .field("project_id", &self.project_id)
            .field("paths", &self.paths.iter().collect::<Vec<_>>())
            .field("symbols", &self.symbols.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl PartialEq for FactTable {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.generation == other.generation
            && self.project_id == other.project_id
            && self.paths.len() == other.paths.len()
            && self.paths.iter().eq(other.paths.iter())
            && self.symbols.len() == other.symbols.len()
            && self.symbols.iter().eq(other.symbols.iter())
    }
}

impl Eq for FactTable {}

#[cfg(test)]
mod moved_symbol_tests {
    use super::*;

    fn symbol(id: &str, target: &str, start: u64) -> SymbolFact {
        SymbolFact {
            id: id.into(),
            references: Arc::from([]),
            alias_target: target.into(),
            declarations: vec![Declaration {
                kind: "variable".into(),
                name: id.into(),
                location: Location {
                    path: "/p/a.ts".into(),
                    start_byte: start,
                    end_byte: start + 1,
                },
            }]
            .into(),
        }
    }

    fn table(symbols: Vec<SymbolFact>) -> FactTable {
        FactTable::from_parts(2, 1, "p", vec![], vec![], symbols, vec![])
    }

    /// The moved symbols are found from the leaves two tables do not share,
    /// and are the ones a full comparison by id finds: a moved declaration,
    /// a new alias target, an added and a removed symbol. A row whose
    /// references alone changed did not move.
    #[test]
    fn moved_declarations_come_from_unshared_leaves_and_match_a_full_comparison() {
        let before = table(
            (0..500)
                .map(|index| symbol(&format!("s{index:04}"), "", index))
                .collect(),
        );
        let mut after = before.clone();
        let edits = [
            ("s0100", Some(symbol("s0100", "", 9_999))),
            ("s0150", Some(symbol("s0150", "s0001", 150))),
            ("s0200", None),
            ("s0300x", Some(symbol("s0300x", "", 1))),
            ("s0400", {
                let mut referenced = symbol("s0400", "", 400);
                referenced.references = vec![Location {
                    path: "/p/b.ts".into(),
                    start_byte: 3,
                    end_byte: 4,
                }]
                .into();
                Some(referenced)
            }),
        ];
        for (id, replacement) in edits {
            after.symbols = after
                .symbols
                .updated(id, replacement.map(StoredSymbol::from_fact));
        }
        let moved = ["s0100", "s0150", "s0200", "s0300x"].map(Arc::<str>::from);
        assert_eq!(after.symbols_with_moved_declarations(&before), moved);
        assert!(
            after.symbols.leaves.iter().any(|leaf| before
                .symbols
                .leaves
                .iter()
                .any(|kept| Arc::ptr_eq(kept, leaf))),
            "a sparse edit shares the leaves it did not touch"
        );
        // A table rebuilt whole shares no leaf, and answers the same.
        let rebuilt = table(after.symbols.iter().map(StoredSymbol::to_fact).collect());
        assert_eq!(rebuilt.symbols_with_moved_declarations(&before), moved);
        assert!(before.symbols_with_moved_declarations(&before).is_empty());
        assert!(rebuilt.symbols_with_moved_declarations(&after).is_empty());
    }
}
