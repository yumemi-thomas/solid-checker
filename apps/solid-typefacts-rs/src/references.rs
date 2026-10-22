//! The reference index (Go: `tsgo/references.go`).
//!
//! It owns the generation-scoped merged index, its reusable per-file
//! contributions, and the exact invalidation delta exposed to retained
//! closure consumers. The scan is the producer's own walk over syntax, not a
//! language-service call: every non-declaration identifier is asked for its
//! symbol, and a shorthand property's name is also a read of its value
//! (ADR 0336, `GetShorthandAssignmentValueSymbol`).

use crate::identity::Env;
use crate::model::{SymbolId, durable_symbol_id};
use std::collections::{HashMap, HashSet};
use tsrs_ast::{
    Node, SourceFile, is_declaration_name_or_import_property_name, is_identifier,
    is_part_of_type_node, is_qualified_name, is_shorthand_property_assignment,
};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{Location, ReferenceSpace};

/// Per-file spans omit the file path (`ReferenceIndex::files` owns it once).
/// Compiler positions fit in 32 bits, so a retained reference is one compact
/// eight-byte span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Indexed {
    path: u32,
    start: u32,
    end: u32,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: SymbolId,
    /// Emptied once merged into the index, so the index does not duplicate
    /// what a retained contribution holds.
    pub spans: Vec<Span>,
}

/// One file's contribution to the reference index: every resolvable
/// non-declaration identifier in the file, grouped by the durable id of its
/// alias-canonicalized symbol, each group in ascending byte order. `durable`
/// reports whether every grouped id is durable; an entry with
/// generation-scoped counter ids cannot outlive its generation.
#[derive(Clone, Debug)]
pub struct FileReferences {
    pub refs: Vec<Group>,
    spaces: Vec<(SymbolId, u8)>,
    pub durable: bool,
    /// The spans were moved into a merged index and cleared. Such an entry
    /// still carries exact symbol evidence but cannot rebuild a dropped
    /// index; `build` rescans it.
    merged: bool,
    /// Which scan produced this entry; tests use it to tell a retained
    /// contribution from a replaced one (Go compares the pointers).
    pub serial: u64,
}

#[derive(Default)]
pub struct ReferenceIndex {
    /// `None` until the first reference query of a generation.
    merged: Option<HashMap<SymbolId, Vec<Indexed>>>,
    spaces: Option<HashMap<SymbolId, u8>>,
    paths: Vec<String>,
    /// The canonical symbols whose merged rows escaped into a retained fact.
    /// `ReleaseAnalysisState` prunes every other merged bucket; per-file
    /// symbol evidence remains sufficient for exact invalidation.
    used: HashSet<SymbolId>,
    /// `Some` once `release_analysis_state` pruned merged buckets: the only
    /// buckets still known whole. A bucket absent from it may be missing or
    /// hold just the rows an incremental refresh added, so a query for it
    /// rebuilds the merged index first.
    complete: Option<HashSet<SymbolId>>,
    /// Affected files removed from an already-materialized merged index. They
    /// are rescanned lazily at the established reference closure point so
    /// symbol counter minting retains its order.
    refresh_paths: HashSet<String>,
    /// The exact union removed from and added to refreshed contributions.
    /// `delta_exact` distinguishes known-empty from unavailable.
    changed_symbols: HashSet<SymbolId>,
    delta_exact: bool,
    /// Durable per-file contributions carried across generations.
    files: HashMap<String, FileReferences>,
    next_serial: u64,
}

fn space_from_bits(bits: u8) -> ReferenceSpace {
    match bits {
        1 => ReferenceSpace::Value,
        2 => ReferenceSpace::Type,
        3 => ReferenceSpace::Both,
        _ => ReferenceSpace::Neither,
    }
}

/// Go: `isTypeSpaceReference`. `IsPartOfTypeNode` handles the right side of a
/// `QualifiedName`, but an imported namespace is commonly its leftmost
/// identifier. Classify the complete qualified name so its enclosing
/// `TypeReference` (or `TypeQuery`) determines the reference space for every
/// identifier in the chain.
fn is_type_space_reference(mut node: P<Node>) -> bool {
    while let Some(parent) = node.parent()
        && is_qualified_name(parent)
    {
        node = parent;
    }
    is_part_of_type_node(node)
}

impl ReferenceIndex {
    /// Contribution files held (tests).
    #[must_use]
    pub fn files(&self) -> &HashMap<String, FileReferences> {
        &self.files
    }

    /// Whether the merged index is built (tests).
    #[must_use]
    pub fn is_merged(&self) -> bool {
        self.merged.is_some()
    }

    /// Whether the changed-symbol set is an exact delta (tests).
    #[must_use]
    pub fn delta_exact(&self) -> bool {
        self.delta_exact
    }

    #[must_use]
    pub fn changed_symbol_set(&self) -> &HashSet<SymbolId> {
        &self.changed_symbols
    }

    /// Go: `invalidate`. Retains safe contributions and records exact symbol
    /// evidence for every discarded file. Small updates patch the merged index
    /// in place; broad updates rebuild it, but no longer discard the exact
    /// changed-symbol set.
    ///
    /// `is_scannable` answers whether the program holds a non-declaration
    /// file at a path; `retained` whether a file's contribution may survive.
    pub fn invalidate(
        &mut self,
        affected: &[String],
        is_scannable: &dyn Fn(&str) -> bool,
        retained: &dyn Fn(&str) -> bool,
    ) {
        self.spaces = None;
        let composing =
            self.delta_exact && (self.merged.is_none() || !self.refresh_paths.is_empty());
        let had_exact_base = self.merged.is_some() || composing;
        // Large affected sets still favor a full merged-index rebuild.
        // Exactness and the physical update strategy are independent
        // decisions.
        let incremental = self.merged.is_some() && affected.len() <= 64 && !composing;
        let mut refresh_paths: HashSet<String> = HashSet::with_capacity(affected.len());
        let mut changed_symbols = if composing {
            std::mem::take(&mut self.changed_symbols)
        } else {
            HashSet::new()
        };
        let mut doomed: Vec<String> = Vec::new();
        for (path, entry) in &self.files {
            // A non-durable entry holds generation-scoped counter ids no later
            // generation can resolve; fail closed and re-scan.
            if retained(path) && entry.durable {
                continue;
            }
            doomed.push(path.clone());
        }
        // Go ranges the map in unspecified order; the effects below are
        // order-insensitive (set unions and per-path removals).
        for path in doomed {
            let Some(entry) = self.files.remove(&path) else {
                continue;
            };
            if had_exact_base {
                for group in &entry.refs {
                    changed_symbols.insert(group.id.clone());
                }
            }
            if incremental {
                let path_index = self.paths.partition_point(|candidate| *candidate < path);
                if let Some(merged) = &mut self.merged {
                    for group in &entry.refs {
                        let Some(references) = merged.get_mut(&group.id) else {
                            continue;
                        };
                        references.retain(|reference| reference.path as usize != path_index);
                        if references.is_empty() {
                            merged.remove(&group.id);
                        }
                    }
                }
                if is_scannable(&path) {
                    refresh_paths.insert(path);
                }
            }
        }
        if incremental {
            self.refresh_paths = refresh_paths;
            self.changed_symbols = changed_symbols;
            self.delta_exact = true;
            return;
        }
        self.merged = None;
        self.spaces = None;
        self.paths.clear();
        self.refresh_paths.clear();
        self.changed_symbols = changed_symbols;
        self.delta_exact = had_exact_base;
    }

    /// Go: `reset`.
    pub fn reset(&mut self) {
        self.merged = None;
        self.spaces = None;
        self.refresh_paths.clear();
        self.changed_symbols.clear();
        self.delta_exact = false;
        self.files.clear();
        self.paths.clear();
        self.used.clear();
        self.complete = None;
    }

    /// The reference half of the rebase policy: the program is a different
    /// arena with equal content, so the generation-scoped ids of non-durable
    /// contributions no longer name anything. They are dropped as `invalidate`
    /// drops them (their symbols join the exact delta) and the merged index
    /// is rebuilt from the durable contributions that remain.
    pub fn rebase(&mut self) {
        let had_exact_base = self.delta_exact || self.merged.is_some();
        let doomed: Vec<String> = self
            .files
            .iter()
            .filter(|(_, entry)| !entry.durable)
            .map(|(path, _)| path.clone())
            .collect();
        for path in doomed {
            if let Some(entry) = self.files.remove(&path)
                && had_exact_base
            {
                {
                    for group in &entry.refs {
                        self.changed_symbols.insert(group.id.clone());
                    }
                }
            }
        }
        self.delta_exact = had_exact_base;
        self.merged = None;
        self.spaces = None;
        self.paths.clear();
        self.refresh_paths.clear();
        self.used.clear();
        self.complete = None;
    }

    /// Go: `releaseAnalysisState`.
    pub fn release_analysis_state(&mut self) {
        let mut pruned = false;
        if let Some(merged) = &mut self.merged {
            let before = merged.len();
            merged.retain(|id, _| self.used.contains(id));
            pruned = merged.len() != before;
        }
        if pruned || self.complete.is_some() {
            // Buckets kept before this release but unused since are pruned
            // now, so only the ids used since the last release remain whole.
            self.complete = Some(self.used.clone());
        }
        self.used.clear();
        // Reference-space rows are cheap to rebuild from the compact per-file
        // entries and otherwise duplicate that complete union between
        // analyses.
        self.spaces = None;
    }

    /// Go: `ensure`.
    pub fn ensure(&mut self, env: &mut Env<'_>) {
        if self.merged.is_none() {
            self.merged = Some(self.build(env));
            self.refresh_paths.clear();
            self.complete = None;
            return;
        }
        if self.refresh_paths.is_empty() {
            return;
        }
        let mut paths: Vec<String> = self.refresh_paths.iter().cloned().collect();
        paths.sort();
        let mut touched: HashSet<SymbolId> = HashSet::new();
        for path in paths {
            let Some(file) = env.program.get_source_file(&path) else {
                continue;
            };
            if file.is_declaration_file() {
                continue;
            }
            let entry = self.scan(env, file);
            self.files.insert(path.clone(), entry);
            let path_index = self.paths.partition_point(|candidate| *candidate < path);
            if self.paths.get(path_index) != Some(&path) {
                // Incremental refresh is only selected when program
                // membership is unchanged, so every refreshed path belongs to
                // the established compact path table.
                continue;
            }
            let Some(merged) = &mut self.merged else {
                continue;
            };
            let entry = self.files.get_mut(&path).expect("just inserted");
            for group in &mut entry.refs {
                touched.insert(group.id.clone());
                if self.delta_exact {
                    self.changed_symbols.insert(group.id.clone());
                }
                let references = merged.entry(group.id.clone()).or_default();
                for span in std::mem::take(&mut group.spans) {
                    references.push(Indexed {
                        path: u32::try_from(path_index).unwrap_or(u32::MAX),
                        start: span.start,
                        end: span.end,
                    });
                }
            }
            entry.merged = true;
        }
        if let Some(merged) = &mut self.merged {
            for id in touched {
                if let Some(references) = merged.get_mut(&id) {
                    references
                        .sort_by_key(|reference| (reference.path, reference.start, reference.end));
                }
            }
        }
        self.refresh_paths.clear();
    }

    /// Go: `build`: merges the per-file contributions into the current
    /// generation's reference index, scanning only files without a retained
    /// entry (`Update` already evicted the affected set and departed files).
    /// Merging in path order, with each per-file group already in byte order,
    /// preserves the References ordering contract (path, then start byte)
    /// without a global sort.
    fn build(&mut self, env: &mut Env<'_>) -> HashMap<SymbolId, Vec<Indexed>> {
        let mut paths: Vec<String> = Vec::with_capacity(env.program.source_files().len());
        for file in env.program.source_files() {
            if file.is_declaration_file() {
                continue;
            }
            let path = crate::gocompat::clean(file.file_name());
            paths.push(path.clone());
            match self.files.get(&path) {
                None => {
                    let entry = self.scan(env, *file);
                    if self.delta_exact {
                        for group in &entry.refs {
                            self.changed_symbols.insert(group.id.clone());
                        }
                    }
                    self.files.insert(path, entry);
                }
                Some(prior) if prior.merged => {
                    // Retained across a broad invalidation: its text and
                    // durable identities are unchanged, so this is not a
                    // changed symbol.
                    let entry = self.scan(env, *file);
                    self.files.insert(path, entry);
                }
                Some(_) => {}
            }
        }
        paths.sort();
        let mut references: HashMap<SymbolId, Vec<Indexed>> = HashMap::new();
        let mut spaces: HashMap<SymbolId, u8> = HashMap::new();
        for (path_index, path) in paths.iter().enumerate() {
            let Some(entry) = self.files.get_mut(path) else {
                continue;
            };
            for group in &mut entry.refs {
                let indexed = references.entry(group.id.clone()).or_default();
                for span in std::mem::take(&mut group.spans) {
                    indexed.push(Indexed {
                        path: u32::try_from(path_index).unwrap_or(u32::MAX),
                        start: span.start,
                        end: span.end,
                    });
                }
            }
            entry.merged = true;
            for (id, bits) in &entry.spaces {
                *spaces.entry(id.clone()).or_default() |= *bits;
            }
        }
        self.paths = paths;
        self.spaces = Some(spaces);
        references
    }

    /// Go: `locations`.
    #[must_use]
    pub fn locations(&self, id: &SymbolId) -> Vec<Location> {
        let Some(references) = self.merged.as_ref().and_then(|merged| merged.get(id)) else {
            return Vec::new();
        };
        references
            .iter()
            .map(|reference| Location {
                path: self.paths[reference.path as usize].as_str().into(),
                start_byte: u64::from(reference.start),
                end_byte: u64::from(reference.end),
            })
            .collect()
    }

    /// Go: `query`: completes the bucket for `id`, marks it used, and returns
    /// its locations. The merged index must already be ensured.
    pub fn query(&mut self, env: &mut Env<'_>, id: &SymbolId) -> Vec<Location> {
        if self
            .complete
            .as_ref()
            .is_some_and(|complete| !complete.contains(id))
        {
            // Pruned by `release_analysis_state`; retained contributions
            // rescan into a complete index.
            self.merged = Some(self.build(env));
            self.refresh_paths.clear();
            self.complete = None;
        }
        self.mark_used(id);
        self.locations(id)
    }

    /// Go: `markUsed`.
    fn mark_used(&mut self, id: &SymbolId) {
        self.used.insert(id.clone());
    }

    /// Go: `ChangedReferences` after `ensure`: the sorted changed symbols, or
    /// `None` when the delta is not exact.
    #[must_use]
    pub fn changed(&self) -> Option<Vec<SymbolId>> {
        if !self.delta_exact {
            return None;
        }
        let mut ids: Vec<SymbolId> = self.changed_symbols.iter().cloned().collect();
        ids.sort();
        Some(ids)
    }

    /// Go: `spaceFor`.
    pub fn space_for(&mut self, env: &mut Env<'_>, id: &SymbolId) -> ReferenceSpace {
        self.ensure(env);
        let spaces = self.spaces.get_or_insert_with(|| {
            let mut spaces: HashMap<SymbolId, u8> = HashMap::new();
            for entry in self.files.values() {
                for (id, bits) in &entry.spaces {
                    *spaces.entry(id.clone()).or_default() |= *bits;
                }
            }
            spaces
        });
        match spaces.get(id) {
            Some(bits) if *bits != 0 => space_from_bits(*bits),
            _ => ReferenceSpace::Neither,
        }
    }

    /// Go: `scan`: resolves every non-declaration identifier in one file.
    fn scan(&mut self, env: &mut Env<'_>, file: P<SourceFile>) -> FileReferences {
        struct Scan {
            references: HashMap<SymbolId, Vec<Span>>,
            spaces: HashMap<SymbolId, u8>,
            unordered: HashSet<SymbolId>,
            durable: bool,
        }

        fn visit(env: &mut Env<'_>, text: &str, node: P<Node>, scan: &mut Scan) {
            if is_identifier(node) {
                let symbol = if !is_declaration_name_or_import_property_name(node) {
                    env.checker.get_symbol_at_location_exported(node)
                } else if let Some(parent) = node.parent()
                    && is_shorthand_property_assignment(parent)
                    && parent.name() == Some(node)
                {
                    // `{ value }` names the property it declares and reads the
                    // value binding at once. The name is a declaration name,
                    // so without this the read of `value` is in no reference
                    // list, and a census of its references misses the object
                    // holding it.
                    env.checker
                        .get_shorthand_assignment_value_symbol(Some(parent))
                } else {
                    None
                };
                if let Some(symbol) = symbol {
                    let reference_id = env.id_for(symbol);
                    let canonical = env.canonical_symbol(symbol);
                    let id = env.id_for(canonical);
                    if !durable_symbol_id(&id) || !durable_symbol_id(&reference_id) {
                        scan.durable = false;
                    }
                    let start = skip_trivia(text, node.pos().source_offset().unwrap_or(0));
                    let end = node.end().source_offset().unwrap_or(start);
                    let spans = scan.references.entry(id.clone()).or_default();
                    // Parsed children normally arrive in source order.
                    // Preserve the ordering contract without sorting every
                    // bucket, but detect and repair any non-lexical compiler
                    // traversal defensively.
                    if spans.last().is_some_and(|last| last.start > start) {
                        scan.unordered.insert(id);
                    }
                    spans.push(Span { start, end });
                    *scan.spaces.entry(reference_id).or_default() |=
                        if is_type_space_reference(node) { 2 } else { 1 };
                }
            }
            node.for_each_child(&mut |child| {
                visit(env, text, child, scan);
                false
            });
        }

        let mut scan = Scan {
            references: HashMap::new(),
            spaces: HashMap::new(),
            unordered: HashSet::new(),
            durable: true,
        };
        let text = file.text();
        file.as_node().for_each_child(&mut |child| {
            visit(env, text, child, &mut scan);
            false
        });
        let mut refs: Vec<Group> = scan
            .references
            .into_iter()
            .map(|(id, mut spans)| {
                if scan.unordered.contains(&id) {
                    spans.sort_by_key(|span| span.start);
                }
                Group { id, spans }
            })
            .collect();
        refs.sort_by(|left, right| left.id.cmp(&right.id));
        let mut spaces: Vec<(SymbolId, u8)> = scan.spaces.into_iter().collect();
        spaces.sort_by(|left, right| left.0.cmp(&right.0));
        self.next_serial += 1;
        FileReferences {
            refs,
            spaces,
            durable: scan.durable,
            merged: false,
            serial: self.next_serial,
        }
    }
}

#[cfg(test)]
impl ReferenceIndex {
    /// Test seam for the broad-invalidation case, which Go builds by filling
    /// the struct directly.
    /// Go sets `referenceIndex.used` directly.
    pub(crate) fn set_used_for_test(&mut self, ids: &[SymbolId]) {
        self.used = ids.iter().cloned().collect();
    }

    pub(crate) fn insert_for_test(&mut self, path: &str, id: &str, durable: bool) {
        let id: SymbolId = id.into();
        let path_index = u32::try_from(self.paths.len()).unwrap();
        self.paths.push(path.to_owned());
        self.next_serial += 1;
        self.files.insert(
            path.to_owned(),
            FileReferences {
                refs: vec![Group {
                    id: id.clone(),
                    spans: vec![Span { start: 1, end: 2 }],
                }],
                spaces: Vec::new(),
                durable,
                merged: false,
                serial: self.next_serial,
            },
        );
        self.merged.get_or_insert_with(HashMap::new).insert(
            id,
            vec![Indexed {
                path: path_index,
                start: 1,
                end: 2,
            }],
        );
    }
}
