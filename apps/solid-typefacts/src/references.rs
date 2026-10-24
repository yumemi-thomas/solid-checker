//! The reference index (Go: `tsgo/references.go`).
//!
//! It owns the generation-scoped merged index and its per-file contributions.
//! The scan is the producer's own walk over syntax, not a language-service
//! call: every non-declaration identifier is asked for its symbol, and a
//! shorthand property's name is also a read of its value (ADR 0336,
//! `GetShorthandAssignmentValueSymbol`).
//!
//! Each file's contribution is scanned under that file's own checker, in its
//! pass (`passes`, ADR 0398), and the contributions are merged once every
//! file was ([`ReferenceIndex::finish_per_file`], [`PassIndex`]); a
//! cross-file answer asked inside a pass is deferred by its caller until
//! then (`semantic_runs::Deferrals`). A pass's contribution is reused with
//! the pass while everything it read is unchanged.

use crate::gocompat::clean;
use crate::identity::Env;
use crate::model::SymbolId;
use std::collections::HashMap;
use tsrs_ast::{
    Node, SourceFile, is_declaration_name_or_import_property_name, is_identifier,
    is_part_of_type_node, is_qualified_name, is_shorthand_property_assignment,
};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{Location, ReferenceSpace};

/// Per-file spans omit the file path (`ReferenceIndex::files` owns it once).
/// Compiler positions fit in 32 bits, so a reference is one compact
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

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: SymbolId,
    /// Emptied once merged into the index, so the index does not duplicate
    /// what a contribution holds.
    pub spans: Vec<Span>,
}

/// One file's contribution to the reference index: every resolvable
/// non-declaration identifier in the file, grouped by the id of its
/// alias-canonicalized symbol, each group in ascending byte order.
#[derive(Clone, Debug, PartialEq)]
pub struct FileReferences {
    pub refs: Vec<Group>,
    spaces: Vec<(SymbolId, u8)>,
}

/// How much of a table's work was reused: pass outputs kept across tables
/// (`passes`), and `symbols` rows answered from a pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BinderRetention {
    /// Files whose reference contribution came from a kept pass.
    pub reused_reference_files: usize,
    /// Files scanned in a pass that ran.
    pub scanned_reference_files: usize,
    /// Entity symbols of kept passes.
    pub reused_symbols: usize,
    /// Entity symbols resolved in a pass that ran.
    pub resolved_symbols: usize,
    /// `symbols` rows answered from a pass's rows.
    pub reused_symbol_facts: usize,
    /// `symbols` rows resolved in a scratch pass.
    pub resolved_symbol_facts: usize,
    /// Time spent scanning and merging the reference index.
    pub build: std::time::Duration,
}

#[derive(Default)]
pub struct ReferenceIndex {
    /// `None` until every file's contribution was merged.
    merged: Option<HashMap<SymbolId, Vec<Indexed>>>,
    spaces: Option<HashMap<SymbolId, u8>>,
    paths: Vec<String>,
    /// Contributions scanned and not merged yet.
    files: HashMap<String, FileReferences>,
    retention: BinderRetention,
    /// Per-file passes (instant-edits P3): per id, how many files' spaces
    /// carry each bit, so a file's contribution can leave the merged spaces
    /// ([`PassIndex`]). `Some` exactly while a pass index is installed.
    pass_counts: Option<HashMap<SymbolId, [u32; 2]>>,
}

/// Per-file passes (instant-edits P3): the merged reference index kept
/// across tables, patched per file whose contribution changed. Its contents
/// equal [`ReferenceIndex::finish_per_file`]'s over the same contributions:
/// buckets ordered by (path, start, end), spaces the union of every file's
/// bits.
pub(crate) struct PassIndex {
    merged: HashMap<SymbolId, Vec<Indexed>>,
    spaces: HashMap<SymbolId, u8>,
    counts: HashMap<SymbolId, [u32; 2]>,
    paths: Vec<String>,
}

impl PassIndex {
    /// The index of `contributions` over `paths` (every non-declaration
    /// file; sorted here, as `merge_files` sorts them).
    pub(crate) fn build<'a>(
        mut paths: Vec<String>,
        contributions: impl Iterator<Item = (&'a str, &'a FileReferences)>,
    ) -> Self {
        paths.sort();
        let mut index = Self {
            merged: HashMap::new(),
            spaces: HashMap::new(),
            counts: HashMap::new(),
            paths,
        };
        let mut by_path: Vec<Option<&FileReferences>> = vec![None; index.paths.len()];
        for (path, references) in contributions {
            if let Some(at) = index.position(path) {
                by_path[at] = Some(references);
            }
        }
        for (at, references) in by_path.into_iter().enumerate() {
            if let Some(references) = references {
                index.add(at, references, false);
            }
        }
        index
    }

    /// Whether `other` answers every question this index answers the same
    /// way (buckets, non-empty spaces, paths): a patched index against a
    /// rebuilt one (`passes::ENV_VERIFY_REUSE`).
    pub(crate) fn same_answers(&self, other: &PassIndex) -> bool {
        let spaces = |index: &PassIndex| -> HashMap<SymbolId, u8> {
            index
                .spaces
                .iter()
                .filter(|(_, bits)| **bits != 0)
                .map(|(id, bits)| (id.clone(), *bits))
                .collect()
        };
        let buckets = |index: &PassIndex| -> usize {
            index
                .merged
                .values()
                .filter(|bucket| !bucket.is_empty())
                .count()
        };
        self.paths == other.paths
            && buckets(self) == buckets(other)
            && self
                .merged
                .iter()
                .filter(|(_, bucket)| !bucket.is_empty())
                .all(|(id, bucket)| other.merged.get(id) == Some(bucket))
            && spaces(self) == spaces(other)
    }

    /// The program's non-declaration paths the index covers.
    pub(crate) fn paths(&self) -> &[String] {
        &self.paths
    }

    fn position(&self, path: &str) -> Option<usize> {
        self.paths
            .binary_search_by(|candidate| candidate.as_str().cmp(path))
            .ok()
    }

    /// Replaces `path`'s contribution `old` by `new`. `false` when `path` is
    /// not one of the index's paths (the caller rebuilds).
    pub(crate) fn patch(
        &mut self,
        path: &str,
        old: Option<&FileReferences>,
        new: Option<&FileReferences>,
    ) -> bool {
        let Some(at) = self.position(path) else {
            return false;
        };
        if let Some(old) = old {
            self.remove(at, old);
        }
        if let Some(new) = new {
            self.add(at, new, true);
        }
        true
    }

    fn add(&mut self, at: usize, references: &FileReferences, ordered: bool) {
        let path = u32::try_from(at).unwrap_or(u32::MAX);
        for group in &references.refs {
            let bucket = self.merged.entry(group.id.clone()).or_default();
            let spans = group.spans.iter().map(|span| Indexed {
                path,
                start: span.start,
                end: span.end,
            });
            if ordered {
                // Every other file's entries are already in (path, start)
                // order and this file's spans are in byte order.
                let from = bucket.partition_point(|entry| entry.path < path);
                bucket.splice(from..from, spans);
            } else {
                bucket.extend(spans);
            }
        }
        for (id, bits) in &references.spaces {
            let counts = self.counts.entry(id.clone()).or_default();
            if bits & 1 != 0 {
                counts[0] += 1;
            }
            if bits & 2 != 0 {
                counts[1] += 1;
            }
            *self.spaces.entry(id.clone()).or_default() |= *bits;
        }
    }

    fn remove(&mut self, at: usize, references: &FileReferences) {
        let path = u32::try_from(at).unwrap_or(u32::MAX);
        for group in &references.refs {
            if let Some(bucket) = self.merged.get_mut(&group.id) {
                bucket.retain(|entry| entry.path != path);
                if bucket.is_empty() {
                    self.merged.remove(&group.id);
                }
            }
        }
        for (id, bits) in &references.spaces {
            let Some(counts) = self.counts.get_mut(id) else {
                continue;
            };
            if bits & 1 != 0 {
                counts[0] = counts[0].saturating_sub(1);
            }
            if bits & 2 != 0 {
                counts[1] = counts[1].saturating_sub(1);
            }
            let union = u8::from(counts[0] > 0) | (u8::from(counts[1] > 0) << 1);
            if union == 0 && counts == &[0, 0] {
                self.counts.remove(id);
                // A file whose spaces held the id with no bit still put it
                // in the union map (`finish_per_file` keeps such an entry);
                // whether another file does is not counted, so the entry
                // stays: `merged_space` reads a zero entry as absent.
                if let Some(bits) = self.spaces.get_mut(id) {
                    *bits = 0;
                }
            } else if let Some(bits) = self.spaces.get_mut(id) {
                *bits = union;
            }
        }
    }
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

fn resolve(env: &mut Env<'_>, node: P<Node>) -> Option<P<tsrs_ast::Symbol>> {
    if is_declaration_name_or_import_property_name(node) {
        env.checker
            .get_shorthand_assignment_value_symbol(node.parent())
    } else {
        env.checker.get_symbol_at_location_exported(node)
    }
}

/// Whether `node` is an identifier the scan asks the checker about: every
/// non-declaration identifier, and a shorthand property's name (which reads
/// the value binding by scope lookup with the alias kept).
fn scanned_identifier(node: P<Node>) -> bool {
    if !is_declaration_name_or_import_property_name(node) {
        return true;
    }
    node.parent().is_some_and(|parent| {
        is_shorthand_property_assignment(parent) && parent.name() == Some(node)
    })
}

/// A symbol's two identities in one file, and its accumulator slots.
struct SymbolSlot {
    slots: (usize, usize),
}

struct GroupAcc {
    id: SymbolId,
    spans: Vec<Span>,
    /// A span arrived before an earlier-starting one.
    unordered: bool,
}

/// One file's contribution as it is scanned.
#[derive(Default)]
struct Acc {
    groups: Vec<GroupAcc>,
    group_slots: HashMap<SymbolId, usize>,
    spaces: Vec<(SymbolId, u8)>,
    space_slots: HashMap<SymbolId, usize>,
    /// A symbol's slots, resolved the first time the file names it: later
    /// identifiers of the same symbol add their span through them, without
    /// hashing identity strings again.
    symbols: HashMap<usize, SymbolSlot>,
}

impl Acc {
    fn slots(&mut self, reference: &SymbolId, canonical: &SymbolId) -> (usize, usize) {
        let space = *self
            .space_slots
            .entry(reference.clone())
            .or_insert_with(|| {
                self.spaces.push((reference.clone(), 0));
                self.spaces.len() - 1
            });
        let group = *self
            .group_slots
            .entry(canonical.clone())
            .or_insert_with(|| {
                self.groups.push(GroupAcc {
                    id: canonical.clone(),
                    spans: Vec::new(),
                    unordered: false,
                });
                self.groups.len() - 1
            });
        (space, group)
    }

    fn push(&mut self, (space, group): (usize, usize), span: Span, bits: u8) {
        let group = &mut self.groups[group];
        // Parsed children normally arrive in source order. Preserve the
        // ordering contract without sorting every bucket, but detect and
        // repair any non-lexical compiler traversal defensively.
        if group
            .spans
            .last()
            .is_some_and(|last| last.start > span.start)
        {
            group.unordered = true;
        }
        group.spans.push(span);
        self.spaces[space].1 |= bits;
    }

    fn visit(&mut self, env: &mut Env<'_>, text: &str, node: P<Node>) {
        if is_identifier(node)
            && scanned_identifier(node)
            && let Some(symbol) = resolve(env, node)
        {
            let start = skip_trivia(text, node.pos().source_offset().unwrap_or(0));
            let end = node.end().source_offset().unwrap_or(start);
            let bits = if is_type_space_reference(node) { 2 } else { 1 };
            let slots = match self.symbols.get(&symbol.addr()) {
                Some(slot) => slot.slots,
                None => {
                    let reference = env.id_for(symbol);
                    let canonical = env.canonical_symbol(symbol);
                    let canonical = env.id_for(canonical);
                    let slots = self.slots(&reference, &canonical);
                    self.symbols.insert(symbol.addr(), SymbolSlot { slots });
                    slots
                }
            };
            self.push(slots, Span { start, end }, bits);
        }
        node.for_each_child(&mut |child| {
            self.visit(env, text, child);
            false
        });
    }

    fn finish(self) -> FileReferences {
        // A symbol with no id (no exact key) has no reference list: its
        // identifiers join no bucket and no space.
        let mut refs: Vec<Group> = self
            .groups
            .into_iter()
            .filter(|group| !group.id.is_empty())
            .map(|mut group| {
                if group.unordered {
                    group.spans.sort_by_key(|span| span.start);
                }
                Group {
                    id: group.id,
                    spans: group.spans,
                }
            })
            .collect();
        refs.sort_by(|left, right| left.id.cmp(&right.id));
        let mut spaces = self.spaces;
        spaces.retain(|(id, _)| !id.is_empty());
        spaces.sort_by(|left, right| left.0.cmp(&right.0));
        FileReferences { refs, spaces }
    }
}

/// One file's contribution, scanned under the current checker.
fn contribution(env: &mut Env<'_>, file: P<SourceFile>) -> FileReferences {
    let mut acc = Acc::default();
    let text = file.text();
    file.as_node().for_each_child(&mut |child| {
        acc.visit(env, text, child);
        false
    });
    acc.finish()
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

    /// Reuse counts since the last [`ReferenceIndex::take_retention`].
    pub fn take_retention(&mut self) -> BinderRetention {
        std::mem::take(&mut self.retention)
    }

    /// Go: `reset`. Drops this generation's index.
    pub fn reset(&mut self) {
        self.merged = None;
        self.pass_counts = None;
        self.spaces = None;
        self.files.clear();
        self.paths.clear();
    }

    /// Drops everything (close).
    pub fn forget(&mut self) {
        self.reset();
    }

    /// Go: `releaseAnalysisState`: drops the whole index.
    pub fn release_analysis_state(&mut self) {
        self.reset();
    }

    /// Scans `file` under the current (its own) checker and holds its
    /// contribution until [`ReferenceIndex::finish_per_file`].
    pub fn scan_file(&mut self, env: &mut Env<'_>, file: P<SourceFile>) {
        let started = std::time::Instant::now();
        let entry = contribution(env, file);
        self.files.insert(clean(file.file_name()), entry);
        self.retention.build += started.elapsed();
    }

    /// Per-file passes on their own index (instant-edits P3): the
    /// contribution [`ReferenceIndex::scan_file`] just held for `path`.
    pub(crate) fn take_file(&mut self, path: &str) -> Option<FileReferences> {
        self.files.remove(path)
    }

    /// Per-file passes with reuse (instant-edits P3): reference
    /// contributions and entity symbols served from a kept pass output, and
    /// computed anew.
    pub(crate) fn note_pass_reuse(&mut self, reuse: BinderRetention) {
        self.retention.reused_reference_files += reuse.reused_reference_files;
        self.retention.scanned_reference_files += reuse.scanned_reference_files;
        self.retention.reused_symbols += reuse.reused_symbols;
        self.retention.resolved_symbols += reuse.resolved_symbols;
    }

    /// `symbols` rows answered from a pass's rows (`reused`) and resolved in
    /// a scratch pass (`resolved`).
    pub(crate) fn note_symbol_rows(&mut self, reused: usize, resolved: usize) {
        self.retention.reused_symbol_facts += reused;
        self.retention.resolved_symbol_facts += resolved;
    }

    /// Per-file passes: takes the installed pass index (merged buckets,
    /// spaces with their counts, paths) out, leaving no merged index.
    pub(crate) fn take_pass_index(&mut self) -> Option<PassIndex> {
        let counts = self.pass_counts.take()?;
        let merged = self.merged.take()?;
        let spaces = self.spaces.take().unwrap_or_default();
        Some(PassIndex {
            merged,
            spaces,
            counts,
            paths: std::mem::take(&mut self.paths),
        })
    }

    /// Per-file passes: installs `index` as this generation's merged index.
    pub(crate) fn install_pass_index(&mut self, index: PassIndex) {
        self.merged = Some(index.merged);
        self.spaces = Some(index.spaces);
        self.pass_counts = Some(index.counts);
        self.paths = index.paths;
        self.files.clear();
    }

    /// An index of a pass's own (instant-edits P3): filled per file and
    /// never merged.
    #[must_use]
    pub(crate) fn for_pass() -> Self {
        Self::default()
    }

    /// Merges the scanned contributions of `paths` (every non-declaration
    /// file of the program) into the index. Pointer-free.
    pub fn finish_per_file(&mut self, mut paths: Vec<String>) {
        let started = std::time::Instant::now();
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
            for (id, bits) in &entry.spaces {
                *spaces.entry(id.clone()).or_default() |= *bits;
            }
        }
        self.paths = paths;
        self.spaces = Some(spaces);
        self.merged = Some(references);
        self.pass_counts = None;
        self.retention.build += started.elapsed();
    }

    /// Whether the merged index is built.
    #[must_use]
    pub fn has_merged(&self) -> bool {
        self.merged.is_some()
    }

    /// The reference space of `id` from the merged index.
    #[must_use]
    pub fn merged_space(&self, id: &SymbolId) -> ReferenceSpace {
        match self.spaces.as_ref().and_then(|spaces| spaces.get(id)) {
            Some(bits) if *bits != 0 => space_from_bits(*bits),
            _ => ReferenceSpace::Neither,
        }
    }

    /// The locations of `id` from the merged index, in (path, start byte)
    /// order.
    pub fn query_merged(&mut self, id: &SymbolId) -> Vec<Location> {
        assert!(
            self.merged.is_some(),
            "the reference index is merged before it is queried"
        );
        self.locations(id)
    }

    /// Go: `locations`.
    #[must_use]
    pub fn locations(&self, id: &SymbolId) -> Vec<Location> {
        if id.is_empty() {
            return Vec::new();
        }
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
}

#[cfg(test)]
impl ReferenceIndex {
    /// The scan as ported, one identity lookup per identifier; the equivalence
    /// test compares [`contribution`] against it.
    pub(crate) fn scan_reference(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
    ) -> FileReferences {
        struct Scan {
            references: HashMap<SymbolId, Vec<Span>>,
            spaces: HashMap<SymbolId, u8>,
            unordered: std::collections::HashSet<SymbolId>,
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
                    let start = skip_trivia(text, node.pos().source_offset().unwrap_or(0));
                    let end = node.end().source_offset().unwrap_or(start);
                    // A symbol with no id has no bucket and no space.
                    if !id.is_empty() {
                        let spans = scan.references.entry(id.clone()).or_default();
                        // Parsed children normally arrive in source order.
                        // Preserve the ordering contract without sorting every
                        // bucket, but detect and repair any non-lexical compiler
                        // traversal defensively.
                        if spans.last().is_some_and(|last| last.start > start) {
                            scan.unordered.insert(id);
                        }
                        spans.push(Span { start, end });
                    }
                    if !reference_id.is_empty() {
                        *scan.spaces.entry(reference_id).or_default() |=
                            if is_type_space_reference(node) { 2 } else { 1 };
                    }
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
            unordered: std::collections::HashSet::new(),
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
        FileReferences { refs, spaces }
    }

    /// The same groups, spans and spaces.
    pub(crate) fn same_content(left: &FileReferences, right: &FileReferences) -> bool {
        left.spaces == right.spaces
            && left.refs.len() == right.refs.len()
            && left
                .refs
                .iter()
                .zip(&right.refs)
                .all(|(a, b)| a.id == b.id && a.spans == b.spans)
    }

    pub(crate) fn scan_pair_for_test(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
    ) -> (FileReferences, FileReferences) {
        let fast = contribution(env, file);
        let reference = self.scan_reference(env, file);
        (fast, reference)
    }
}
