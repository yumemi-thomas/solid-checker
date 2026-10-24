//! The two cross-file proofs the per-file caches bake in, as memos (study
//! S6a, ADR 03XX-s6a).
//!
//! **Returned-function invocations.** Is a call an invocation of a function a
//! primitive returned, and of which primitive and result slot? The answer
//! walks the callee's binding chain through TypeScript's reference tables:
//! the binding a reference resolves to (`binding_at_reference`, read through
//! the [`Probe::BindingNameAt`](crate::base::memo::probes::Probe) probe), and
//! from a binding initialized by an identifier, that identifier's binding, in
//! any file, until a binding initialized by a call. That call's primitive
//! ([`CallPrimitives`](super::census)) and the destructured slot holding the
//! symbol answer. [`ReturnedPrimitiveCalls`] answers it for every call of a
//! file at once; a wrapped callee (`(track as T)()`, `track!()`) resolves as
//! the identifier it peels to, as the lookup's resolution does.
//!
//! **Returned sources** (ADR 0222, 0287). A call of an exact project function
//! whose every completion returns one fresh source.
//! [`ReturnedSourceCalls`] answers it per file for the binding initializers a
//! source-discovery fragment asks about. It reads the lookup through
//! [`Ctx::project`] (the function-by-symbol table, primitives, the tuple veto
//! are not memos yet): exact, recomputed in a build in which anything moved,
//! and its value stops propagation when it did not.
//!
//! The cross-file digest binds these answers instead of the bytes of every
//! file that could hold one ([`SemanticLookup::returned_callback_proof_digest`],
//! [`SemanticLookup::returned_source_digest`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::core::Span;

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, FileRead, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};

/// The first table id of these tables (ids 160 to 191).
pub(crate) const INVOCATION_TABLES_FROM: u8 = 160;

macro_rules! invocation_tables {
    ($($field:ident: $memo:ty = $id:literal;)*) => {
        /// The returned-invocation and returned-source tables, inside the
        /// build's ([`crate::base::memo::build::BuildTables`]).
        #[derive(Default)]
        pub(crate) struct InvocationTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl InvocationTables {
            pub(crate) fn table(&self, id: TableId) -> Option<&dyn ErasedTable<BuildSchema>> {
                match id.0 {
                    $($id => Some(&self.$field),)*
                    _ => None,
                }
            }

            pub(crate) fn tables(&self) -> Vec<&dyn ErasedTable<BuildSchema>> {
                vec![$(&self.$field),*]
            }
        }

        $(impl $memo {
            const TABLE: TableId = TableId($id);

            fn of(tables: &BuildTables) -> &Table<BuildSchema, Self> {
                &tables.invocations().$field
            }
        })*
    };
}

invocation_tables! {
    calls: ReturnedPrimitiveCalls = 160;
    sources: ReturnedSourceCalls = 161;
}

/// A call's answer: the primitive whose returned function it invokes, the
/// destructured slot holding that function, and the call's argument count.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ReturnedInvocation {
    pub(crate) primitive: Primitive,
    pub(crate) slot: Option<u32>,
    pub(crate) arguments: u32,
}

impl Canonical for ReturnedInvocation {}

/// The positive answers of one file, by `(call span, callee span)`.
pub(crate) type FileInvocations = Arc<Vec<((Span, Span), ReturnedInvocation)>>;

/// A returned-source answer: the caller's call, the file and call creating
/// the source, and its primitive.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ReturnedSource {
    pub(crate) call: Span,
    pub(crate) origin: FileId,
    pub(crate) created: Span,
    pub(crate) primitive: Primitive,
}

impl Canonical for ReturnedSource {}

/// The returned-source answers of one file, by call span.
pub(crate) type FileSources = Arc<Vec<ReturnedSource>>;

/// The per-build views of both tables, one slot per file of the lookup.
#[derive(Default)]
pub(super) struct State {
    calls: OnceLock<Vec<OnceLock<FileInvocations>>>,
    sources: OnceLock<Vec<OnceLock<FileSources>>>,
}

fn u32_of(value: usize) -> u32 {
    u32::try_from(value).expect("fewer than 2^32 items")
}

/// The invocations of returned functions among the calls of a file
/// ([`SemanticLookup::returned_primitive_at`]).
pub(crate) struct ReturnedPrimitiveCalls;

impl Memo<BuildSchema> for ReturnedPrimitiveCalls {
    type Key = FileId;
    type Value = FileInvocations;
    const NAME: &'static str = "returned_primitive_invocations";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<FileInvocations, Cycle> {
        let Some(read) = ctx.file(*id, 0) else {
            return Ok(Arc::default());
        };
        let mut resolvable = Resolvable::default();
        let mut answers = BTreeMap::new();
        for call in &read.facts().ast.calls {
            let key = (call.span, call.callee);
            if answers.contains_key(&key) {
                continue;
            }
            if let Some((primitive, slot)) =
                invocation_chain(ctx, *id, call.callee, &mut resolvable)?
            {
                answers.insert(
                    key,
                    ReturnedInvocation {
                        primitive,
                        slot,
                        arguments: u32_of(call.arguments.len()),
                    },
                );
            }
        }
        Ok(Arc::new(answers.into_iter().collect()))
    }

    fn on_cycle(_: &FileId) -> FileInvocations {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Entities),
        ]
    }
}

/// Per file, the spans at which the lookup's reference resolution has an
/// entry: an entity row or a TypeFacts reference located there (a binding
/// name the resolution keys by its own span carries an entity row).
#[derive(Default)]
struct Resolvable {
    references: BTreeMap<FileId, BTreeSet<Span>>,
}

impl Resolvable {
    fn has_entry(&mut self, read: &FileRead<'_, '_, '_>, id: FileId, span: Span) -> bool {
        read.symbol_at(span).is_some()
            || self
                .references
                .entry(id)
                .or_insert_with(|| read.references().iter().map(|(_, span)| *span).collect())
                .contains(&span)
    }
}

/// The binding `span` of file `id` resolves to, with the symbol of the name
/// that resolved it: `binding_resolution_at`'s answer, asked only where its
/// table can have one (its key, or the identifier its wrappers peel to).
fn resolve_binding(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    span: Span,
    resolvable: &mut Resolvable,
) -> Option<(FileId, u32, u32, Arc<str>)> {
    let read = ctx.file(id, 0)?;
    let facts = read.facts();
    let keyed = resolvable.has_entry(&read, id, span) || {
        let peeled = facts.ast.peel_ts_sugar_span(span);
        peeled != span
            && (facts
                .ast
                .identifiers
                .binary_search_by_key(&peeled, |identifier| identifier.span)
                .is_ok()
                || facts.ast.reference_declaration(peeled).is_some())
            && resolvable.has_entry(&read, id, peeled)
    };
    if !keyed {
        return None;
    }
    ctx.binding_name_at(id, span)
}

/// `returned_primitive_invocation`'s binding-chain walk from the callee at
/// `callee` of file `id`.
fn invocation_chain(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    callee: Span,
    resolvable: &mut Resolvable,
) -> Result<Option<(Primitive, Option<u32>)>, Cycle> {
    let mut reference = (id, callee);
    let mut seen = BTreeSet::<Arc<str>>::new();
    loop {
        // Callee result values do not always receive an entity fact of their
        // own (tuple slots are the motivating case). Resolve them from the
        // canonical reference set of each demanded binding declaration, then
        // follow local or cross-file aliases one binding at a time.
        let Some((file, rank, ordinal, symbol)) =
            resolve_binding(ctx, reference.0, reference.1, resolvable)
        else {
            return Ok(None);
        };
        if !seen.insert(Arc::clone(&symbol)) {
            return Ok(None);
        }
        let Some(read) = ctx.file(file, rank as usize) else {
            return Ok(None);
        };
        let Some(binding) = read.facts().ast.bindings.get(ordinal as usize) else {
            return Ok(None);
        };
        if let Some(initializer) = binding.call_initializer {
            let slot = match binding.shape {
                solid_facts::ast::BindingShape::Array => {
                    let Some(position) = binding.array_slots.iter().position(|slot| {
                        slot.as_ref()
                            .and_then(|name| read.symbol_at(name.span))
                            .is_some_and(|named| named.as_str() == &*symbol)
                    }) else {
                        return Ok(None);
                    };
                    Some(u32_of(position))
                }
                _ => None,
            };
            // A call with no primitive and no call at the span both prove
            // nothing.
            let primitives = ctx.memo::<super::census::CallPrimitives>(&file)?;
            return Ok(primitives
                .get(&initializer)
                .map(|primitive| (*primitive, slot)));
        }
        let Some(initializer) = binding.initializer_identifier.as_ref() else {
            return Ok(None);
        };
        reference = (file, initializer.span);
    }
}

/// The returned-source answers of a file's binding initializers
/// ([`SemanticLookup::returned_source_at`]).
pub(crate) struct ReturnedSourceCalls;

impl Memo<BuildSchema> for ReturnedSourceCalls {
    type Key = FileId;
    type Value = FileSources;
    const NAME: &'static str = "returned_source_calls";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<FileSources, Cycle> {
        Ok(ctx.project_files(|lookup, table| {
            let Some(file) = table
                .slots_of(*id)
                .first()
                .and_then(|slot| lookup.files().get(*slot))
            else {
                return Arc::default();
            };
            let Some(index) = lookup.ast_file_index(file.path.as_str()) else {
                return Arc::default();
            };
            let mut answers = BTreeMap::new();
            for binding in &file.ast.bindings {
                // The shape a source-discovery fragment asks about
                // (`discover_file_sources`), before its TypeFacts gates.
                let Some(initializer) = binding.call_initializer else {
                    continue;
                };
                let Some(call) = index.call_by_span(initializer) else {
                    continue;
                };
                if !binding.immutable
                    || binding.shape != solid_facts::ast::BindingShape::Identifier
                    || binding.names.len() != 1
                    || binding
                        .initializer
                        .is_none_or(|value| file.ast.peel_ts_sugar_span(value) != call.span)
                    || answers.contains_key(&call.span)
                {
                    continue;
                }
                let Some((origin, created, primitive)) =
                    crate::source_discovery::project_returned_source(
                        lookup,
                        file,
                        call,
                        lookup.entities(),
                        lookup.symbol_names(),
                    )
                else {
                    continue;
                };
                let Some(origin) = table.id_of(origin.path.as_str()) else {
                    continue;
                };
                answers.insert(
                    call.span,
                    ReturnedSource {
                        call: call.span,
                        origin,
                        created: created.span,
                        primitive,
                    },
                );
            }
            Arc::new(answers.into_values().collect())
        }))
    }

    fn on_cycle(_: &FileId) -> FileSources {
        Arc::default()
    }
}

impl<'a> SemanticLookup<'a> {
    fn file_answers<V: Clone>(
        &self,
        slots: &OnceLock<Vec<OnceLock<V>>>,
        file: &FileFacts,
        answer: impl FnOnce(FileId) -> V,
        empty: V,
    ) -> V {
        let index = self.file_index(file);
        let slots =
            slots.get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
        slots[index]
            .get_or_init(|| match self.memos.table().id_of(file.path.as_str()) {
                Some(id) => answer(id),
                None => empty,
            })
            .clone()
    }

    /// The positive returned-invocation answers of `file`, sorted.
    pub(crate) fn returned_invocations(&self, file: &FileFacts) -> FileInvocations {
        self.file_answers(
            &self.invocation_answers.calls,
            file,
            |id| self.memo_world().answer::<ReturnedPrimitiveCalls>(&id),
            Arc::default(),
        )
    }

    /// The primitive whose returned function `call` invokes, and the result
    /// slot holding it.
    pub(crate) fn returned_primitive_at(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<(Primitive, Option<usize>)> {
        let answers = self.returned_invocations(file);
        let found = answers
            .binary_search_by_key(&(call.span, call.callee), |(key, _)| *key)
            .ok()?;
        let answer = answers[found].1;
        Some((answer.primitive, answer.slot.map(|slot| slot as usize)))
    }

    /// The returned-source answers of `file`, by call span.
    pub(crate) fn returned_sources(&self, file: &FileFacts) -> FileSources {
        self.file_answers(
            &self.invocation_answers.sources,
            file,
            |id| self.memo_world().answer::<ReturnedSourceCalls>(&id),
            Arc::default(),
        )
    }

    /// The fresh source the binding-initializer `call` of `file` returns
    /// (ADR 0222, 0287): the file and call creating it, and its primitive.
    pub(crate) fn returned_source_at(
        &self,
        file: &FileFacts,
        call: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::CallFact, Primitive)> {
        let answers = self.returned_sources(file);
        let found = answers
            .binary_search_by_key(&call, |answer| answer.call)
            .ok()?;
        let answer = answers[found];
        let slot = *self.memos.table().slots_of(answer.origin).first()?;
        let facts: &'a ProjectFacts = self.facts;
        let origin = facts.files.get(slot)?;
        let created = origin.ast.call_at(answer.created)?;
        Some((origin, created, answer.primitive))
    }
}
