//! L0: interned file ids, the analyzed file table, per-file item tables, and
//! the per-file view a contribution function receives.

use std::sync::{Arc, OnceLock};

use solid_facts::core::Span;
use solid_facts::{FileFacts, TypeScriptTable};
use typefacts::{EntityFact, ResolvedCall};

use crate::indexes::{CachedAstFileIndex, EntitySymbols};
use crate::{HashMap, SymbolId};

/// An analyzed file, by path. Ids are handed out in first-seen order by a
/// [`FileInterner`] and never reused, so an id names the same path for the
/// interner's whole life (a retained interner keeps them stable across
/// builds).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct FileId(u32);

/// Path interning behind [`FileId`].
#[derive(Clone, Debug, Default)]
pub(crate) struct FileInterner {
    by_path: HashMap<Arc<str>, FileId>,
}

impl FileInterner {
    pub(crate) fn intern(&mut self, path: &str) -> FileId {
        if let Some(id) = self.by_path.get(path) {
            return *id;
        }
        let id = FileId(u32::try_from(self.by_path.len()).expect("fewer than 2^32 analyzed paths"));
        self.by_path.insert(Arc::from(path), id);
        id
    }

    pub(crate) fn get(&self, path: &str) -> Option<FileId> {
        self.by_path.get(path).copied()
    }
}

/// The analyzed files of one build: each slot of `ProjectFacts::files` with
/// its id, and each id's slots in file order. A path listed twice is one id
/// with two slots; every answer below says which of them it reads.
#[derive(Debug)]
pub(crate) struct FileTable {
    interner: FileInterner,
    slot_ids: Vec<FileId>,
    slots: HashMap<FileId, Vec<usize>>,
    /// Distinct ids in the order of their first slot.
    ids: Vec<FileId>,
}

impl FileTable {
    pub(crate) fn new(files: &[FileFacts]) -> Self {
        Self::with_interner(files, FileInterner::default())
    }

    /// The table of `files`, interning new paths into `interner` (a retained
    /// interner keeps every id it already gave).
    pub(crate) fn with_interner(files: &[FileFacts], mut interner: FileInterner) -> Self {
        let mut slot_ids = Vec::with_capacity(files.len());
        let mut slots = HashMap::<FileId, Vec<usize>>::default();
        let mut ids = Vec::with_capacity(files.len());
        for (slot, file) in files.iter().enumerate() {
            let id = interner.intern(file.path.as_str());
            slot_ids.push(id);
            let entry = slots.entry(id).or_default();
            if entry.is_empty() {
                ids.push(id);
            }
            entry.push(slot);
        }
        Self {
            interner,
            slot_ids,
            slots,
            ids,
        }
    }

    pub(crate) fn interner(&self) -> &FileInterner {
        &self.interner
    }

    /// The id of the file in slot `slot`.
    pub(crate) fn slot_id(&self, slot: usize) -> FileId {
        self.slot_ids[slot]
    }

    /// The slots carrying `id`, in file order.
    pub(crate) fn slots_of(&self, id: FileId) -> &[usize] {
        self.slots.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Which of its id's slots `slot` is (0 for the first).
    pub(crate) fn slot_rank(&self, slot: usize) -> usize {
        self.slots_of(self.slot_id(slot))
            .iter()
            .position(|candidate| *candidate == slot)
            .expect("a slot is one of its id's slots")
    }

    /// Every analyzed id, in the order of its first slot.
    pub(crate) fn ids(&self) -> &[FileId] {
        &self.ids
    }

    pub(crate) fn id_of(&self, path: &str) -> Option<FileId> {
        self.interner
            .get(path)
            .filter(|id| self.slots.contains_key(id))
    }

    /// The first slot carrying `path`, as a scan in file order finds it.
    pub(crate) fn first_slot(&self, path: &str) -> Option<usize> {
        self.id_of(path)
            .and_then(|id| self.slots_of(id).first().copied())
    }

    /// The last slot carrying `path`, as a map collected in file order keeps it.
    pub(crate) fn last_slot(&self, path: &str) -> Option<usize> {
        self.id_of(path)
            .and_then(|id| self.slots_of(id).last().copied())
    }
}

/// The kinds of item a file's item table numbers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ItemKind {
    Function,
    Call,
    Jsx,
    Binding,
}

/// A file's item table: the span of each function, call, JSX element and
/// binding, by ordinal (the item's index in its `AstFacts` list). An item is
/// `(file, kind, ordinal)`: an identity that, unlike a span, does not move
/// when text before the item is edited.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FileItems {
    functions: Vec<Span>,
    calls: Vec<Span>,
    jsx: Vec<Span>,
    bindings: Vec<Span>,
}

impl FileItems {
    pub(crate) fn of(file: &FileFacts) -> Self {
        Self {
            functions: file.ast.functions.iter().map(|item| item.span).collect(),
            calls: file.ast.calls.iter().map(|item| item.span).collect(),
            jsx: file.ast.jsx_elements.iter().map(|item| item.span).collect(),
            bindings: file
                .ast
                .bindings
                .iter()
                .map(|item| item.declaration)
                .collect(),
        }
    }

    pub(crate) fn spans(&self, kind: ItemKind) -> &[Span] {
        match kind {
            ItemKind::Function => &self.functions,
            ItemKind::Call => &self.calls,
            ItemKind::Jsx => &self.jsx,
            ItemKind::Binding => &self.bindings,
        }
    }
}

/// Each analyzed path's TypeFacts references, as `(symbol, span)` in the
/// table's symbol order: the per-file slice of the reference census.
pub(crate) type ReferenceRows = HashMap<Arc<str>, Vec<(Arc<str>, Span)>>;

pub(crate) fn reference_rows(typescript: &TypeScriptTable) -> ReferenceRows {
    let mut rows = ReferenceRows::default();
    for symbol in typescript.symbols() {
        let id: Arc<str> = Arc::from(symbol.id());
        for reference in symbol.references() {
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                continue;
            };
            rows.entry(Arc::clone(&reference.path))
                .or_default()
                .push((Arc::clone(&id), Span::new(start, end)));
        }
    }
    rows
}

/// Where the views of one build read the per-file slices of the shared
/// TypeFacts tables from. Each view answers only for its own path.
#[derive(Clone, Copy)]
pub(crate) struct ViewSources<'a> {
    pub(crate) ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
    pub(crate) symbols: &'a EntitySymbols,
    pub(crate) entities: &'a HashMap<(&'a str, u64, u64), &'a EntityFact>,
    pub(crate) typescript: &'a TypeScriptTable,
    pub(crate) references: &'a OnceLock<ReferenceRows>,
}

/// What a contribution function may read: one file's own facts and its own
/// slices of the TypeFacts tables, nothing of any other file. It has no
/// project lookup, so by type a contribution cannot read another file.
#[derive(Clone, Copy)]
pub(crate) struct FileView<'a> {
    pub(crate) file: &'a FileFacts,
    sources: ViewSources<'a>,
}

impl<'a> FileView<'a> {
    pub(crate) fn new(file: &'a FileFacts, sources: ViewSources<'a>) -> Self {
        Self { file, sources }
    }

    fn path(&self) -> &'a str {
        self.file.path.as_str()
    }

    pub(crate) fn ast_index(&self) -> Option<&'a CachedAstFileIndex> {
        self.sources.ast_indexes.get(self.path())
    }

    /// The symbol that is the value at `span` of this file
    /// ([`EntitySymbols::value_symbol`]).
    pub(crate) fn value_symbol(&self, span: Span) -> Option<&'a SymbolId> {
        self.sources.symbols.value_symbol(self.file, span)
    }

    pub(crate) fn entity_at(&self, span: Span) -> Option<&'a EntityFact> {
        self.sources
            .entities
            .get(&(self.path(), u64::from(span.start), u64::from(span.end)))
            .copied()
    }

    /// [`crate::indexes::SemanticLookup::resolved_callee_call`] for a callee
    /// of this file.
    pub(crate) fn resolved_callee_call(&self, callee: Span) -> Option<&'a ResolvedCall> {
        crate::indexes::resolved_callee_call_in(self.file, self.ast_index(), callee, |span| {
            self.entity_at(span)
        })
    }

    /// This file's TypeFacts references, `(symbol, span)`.
    pub(crate) fn references(&self) -> &'a [(Arc<str>, Span)] {
        self.sources
            .references
            .get_or_init(|| reference_rows(self.sources.typescript))
            .get(self.path())
            .map_or(&[], Vec::as_slice)
    }

    /// The entity-symbol rows of this file, sorted: `(span, symbol, exact)`.
    /// What the fingerprint of the view covers beside the source.
    pub(crate) fn symbol_rows(&self) -> Vec<((u64, u64), &'a SymbolId, bool)> {
        let exact = self.sources.symbols.exact_by_path.get(self.path());
        let mut rows = self
            .sources
            .symbols
            .by_path
            .get(self.path())
            .into_iter()
            .flatten()
            .map(|(key, symbol)| {
                (
                    *key,
                    symbol,
                    exact.is_some_and(|exact| exact.get(key) == Some(symbol)),
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    }

    /// A digest of everything this view exposes: the path, the source (the
    /// AST and its index are functions of it), the entity-symbol rows, the
    /// resolved-call facts of the file's entities, and its references. Two
    /// views with the same fingerprint give every contribution function the
    /// same answers, so a file whose fingerprint is unchanged keeps its
    /// contributions.
    pub(crate) fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        self.path().hash(&mut hasher);
        self.file.source_hash.hash(&mut hasher);
        for (key, symbol, exact) in self.symbol_rows() {
            (key, symbol.as_str(), exact).hash(&mut hasher);
        }
        for entity in self.sources.typescript.entities_for_path(self.path()) {
            (entity.location.start_byte, entity.location.end_byte).hash(&mut hasher);
            if let Some(call) = entity.resolved_call.as_deref() {
                (
                    call.validity as u8,
                    call.kind as u8,
                    call.targets.is_some(),
                    call.declaration.as_ref().map(|declaration| {
                        (
                            declaration.standard_library,
                            declaration.qualified_name.as_ref(),
                        )
                    }),
                )
                    .hash(&mut hasher);
            } else {
                0xffu8.hash(&mut hasher);
            }
        }
        for (symbol, span) in self.references() {
            (symbol.as_ref(), span.start, span.end).hash(&mut hasher);
        }
        hasher.finish()
    }
}
