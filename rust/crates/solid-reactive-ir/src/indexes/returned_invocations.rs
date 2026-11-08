//! The two cross-file proofs the per-file caches bake in, as memos (study
//! S6a, ADR 0392).
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
//! A stage chunk that reads an answer records the memo read; the returned
//! sources and callback sites are sections ([`crate::base::reads::Section`])
//! fingerprinted from these answers ([`SemanticLookup::returned_callback_proof_digest`],
//! [`SemanticLookup::returned_source_digest`]), not from the bytes of every
//! file that could hold one.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::core::Span;

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, FileRead, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};
use crate::base::reads::Recorded;

/// The first table id of these tables (ids 160 to 191).
pub(crate) const INVOCATION_TABLES_FROM: u16 = 160;

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
    /// ADR 0549: the file and call that created the function, whether every
    /// binding the walk followed is a `const` (`exact`), and whether that call
    /// is written at its module's top level.
    pub(crate) factory: (FileId, Span),
    pub(crate) exact: bool,
    pub(crate) factory_at_module_scope: bool,
    /// The call whose owner the creating call ran under: the creating call
    /// itself, or, when the walk crossed a hook whose own body makes it, that
    /// hook's call. `None` when the walk proves neither.
    pub(crate) placement: Option<(FileId, Span)>,
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
    calls: OnceLock<Vec<Recorded<FileInvocations>>>,
    sources: OnceLock<Vec<Recorded<FileSources>>>,
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
            if let Some(chain) = invocation_chain(ctx, *id, call.callee, &mut resolvable)? {
                answers.insert(
                    key,
                    ReturnedInvocation {
                        primitive: chain.primitive,
                        slot: chain.slot,
                        arguments: u32_of(call.arguments.len()),
                        factory: chain.factory,
                        exact: chain.exact,
                        factory_at_module_scope: chain.factory_at_module_scope,
                        placement: chain.placement,
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

/// What [`invocation_chain`] found: the primitive, the slot, the creating
/// call, and the facts ADR 0549's owner placement needs of the walk.
struct InvocationChain {
    primitive: Primitive,
    slot: Option<u32>,
    factory: (FileId, Span),
    exact: bool,
    factory_at_module_scope: bool,
    placement: Option<(FileId, Span)>,
}

/// The slot a walk carries to the binding whose initializer holds it
/// (ADR 0549's folds): an object pattern's or a member callee's property, or
/// a tuple pattern's position.
#[derive(Clone)]
enum Slot {
    Key(String),
    Index(usize),
}

/// `returned_primitive_invocation`'s binding-chain walk from the callee at
/// `callee` of file `id`. ADR 0549 folds exact local value flows into it: a
/// member callee on an identifier (`thing.useIt()`), and a slot of a pattern
/// or member whose binding's initializer is a closed object literal, a tuple
/// literal, or the call of a `const` function whose only return is a complete
/// object or tuple literal (`const { useIt } = useThing()`), followed through
/// the slot's value when it is an identifier.
fn invocation_chain(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    callee: Span,
    resolvable: &mut Resolvable,
) -> Result<Option<InvocationChain>, Cycle> {
    let mut reference = (id, callee);
    let mut pending: Option<Slot> = None;
    if let Some(read) = ctx.file(id, 0) {
        let ast = &read.facts().ast;
        let peeled = ast.peel_ts_sugar_span(callee);
        if let Some(member) = ast.member_at(peeled)
            && ast.computed_members.binary_search(&peeled).is_err()
            && ast.optional_members.binary_search(&peeled).is_err()
            && let Some(property) = read.facts().source_text(member.property)
        {
            reference = (id, ast.peel_ts_sugar_span(member.object));
            pending = Some(Slot::Key(property.to_owned()));
        }
    }
    let mut seen = BTreeSet::<Arc<str>>::new();
    let mut exact = true;
    // The last hook the walk crossed: its call, and the function it runs.
    // The hooks the walk crossed: each one's call and the function it runs.
    // A creating call inside one of them ran under that hook's call; through
    // two, the inner hook's call runs under the outer's every caller, which
    // places no one creation.
    let mut hooks: Vec<((FileId, Span), (FileId, Span))> = Vec::new();
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
        let facts = read.facts();
        let Some(binding) = facts.ast.bindings.get(ordinal as usize) else {
            return Ok(None);
        };
        exact &= binding.immutable;
        let named = |name: &solid_facts::ast::NamedSpan| {
            read.symbol_at(name.span)
                .is_some_and(|named| named.as_str() == &*symbol)
        };
        // The slot this binding's pattern selects, or the one a member callee
        // carried to an identifier binding.
        let slot = match binding.shape {
            solid_facts::ast::BindingShape::Array => {
                let Some(position) = binding
                    .array_slots
                    .iter()
                    .position(|slot| slot.as_ref().is_some_and(named))
                else {
                    return Ok(None);
                };
                if pending.is_some() {
                    return Ok(None);
                }
                Some(Slot::Index(position))
            }
            solid_facts::ast::BindingShape::Object => {
                let pattern = facts.source_text(binding.pattern).unwrap_or("=");
                if pending.is_some() || pattern.contains('=') || pattern.contains("...") {
                    return Ok(None);
                }
                let Some(slot) = binding
                    .object_slots
                    .iter()
                    .find(|slot| slot.plain_identifier_key && named(&slot.local))
                else {
                    return Ok(None);
                };
                Some(Slot::Key(slot.property.to_string()))
            }
            solid_facts::ast::BindingShape::Identifier => pending.take(),
        };
        if let Some(initializer) = binding.call_initializer {
            // A call with no primitive and no call at the span both prove
            // nothing.
            let primitives = ctx.memo::<super::census::CallPrimitives>(&file)?;
            if let Some(primitive) = primitives.get(&initializer) {
                let slot = match slot {
                    None => None,
                    Some(Slot::Index(position)) => Some(u32_of(position)),
                    Some(Slot::Key(_)) => return Ok(None),
                };
                let enclosing = crate::owners::containing_ast_function(&facts.ast, initializer);
                let factory_at_module_scope = enclosing.is_none();
                let placement = match enclosing {
                    Some(function)
                        if hooks.iter().any(|(_, hook)| *hook == (file, function.span)) =>
                    {
                        match hooks.as_slice() {
                            [(call, _)] => Some(*call),
                            _ => None,
                        }
                    }
                    _ => Some((file, initializer)),
                };
                return Ok(Some(InvocationChain {
                    primitive: *primitive,
                    slot,
                    factory: (file, initializer),
                    exact,
                    factory_at_module_scope,
                    placement,
                }));
            }
            // ADR 0549: a hook's slot, by its only, complete-literal return,
            // or (no slot) the identifier it returns.
            let Some(call) = facts.ast.call_at(initializer) else {
                return Ok(None);
            };
            let Some((value, hook)) = hook_slot_value(
                ctx,
                file,
                call.callee,
                slot.as_ref(),
                resolvable,
                &mut exact,
            )?
            else {
                return Ok(None);
            };
            hooks.push(((file, call.span), hook));
            reference = value;
            continue;
        }
        if let Some(slot) = slot {
            // ADR 0549: a slot of a binding that names the literal
            // (`const [, c] = pair`) is that slot of the named binding's.
            if let Some(named) = binding.initializer_identifier.as_ref() {
                reference = (file, named.span);
                pending = Some(slot);
                continue;
            }
            // ADR 0549: a slot of a closed local literal.
            let Some(initializer) = binding
                .initializer
                .map(|span| facts.ast.peel_ts_sugar_span(span))
            else {
                return Ok(None);
            };
            let Some(value) = literal_slot_value(facts, initializer, &slot) else {
                return Ok(None);
            };
            reference = (file, value);
            continue;
        }
        let Some(initializer) = binding.initializer_identifier.as_ref() else {
            return Ok(None);
        };
        reference = (file, initializer.span);
    }
}

/// ADR 0549: the identifier a closed object literal's property or a tuple
/// literal's element holds, written at `literal` (behind TypeScript sugar).
fn literal_slot_value(facts: &FileFacts, literal: Span, slot: &Slot) -> Option<Span> {
    let ast = &facts.ast;
    let literal = ast.peel_ts_sugar_span(literal);
    let value = match slot {
        Slot::Key(key) => crate::execution_role::closed_literal_slot_value(facts, literal, key)?,
        Slot::Index(position) => {
            let array = ast.array_literal_at(literal)?;
            if array
                .elements
                .iter()
                .take(position + 1)
                .any(|element| element.kind == solid_facts::ast::ArrayElementKind::Spread)
            {
                return None;
            }
            let element = array.elements.get(*position)?;
            (element.kind == solid_facts::ast::ArrayElementKind::Value).then_some(element.span)?
        }
    };
    let value = ast.peel_ts_sugar_span(value);
    ast.identifiers
        .iter()
        .any(|identifier| {
            identifier.span == value
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        })
        .then_some(value)
}

/// What [`hook_slot_value`] found: the identifier the slot holds, and the
/// hook function, each with its file.
type HookSlot = ((FileId, Span), (FileId, Span));

/// ADR 0549: the identifier the slot holds in what the function `callee` names
/// returns: a `const` bound to a function literal whose only return (an
/// expression body, or one straight-line `return`) is a complete object or
/// tuple literal. The identifier is resolved in the function's file; a name
/// the function declares itself (a parameter) resolves to a binding with no
/// initializer, which proves nothing.
fn hook_slot_value(
    ctx: &Ctx<'_, '_, BuildSchema>,
    id: FileId,
    callee: Span,
    slot: Option<&Slot>,
    resolvable: &mut Resolvable,
    exact: &mut bool,
) -> Result<Option<HookSlot>, Cycle> {
    let Some((file, rank, ordinal, _)) = resolve_binding(ctx, id, callee, resolvable) else {
        return Ok(None);
    };
    let Some(read) = ctx.file(file, rank as usize) else {
        return Ok(None);
    };
    let facts = read.facts();
    let Some(binding) = facts.ast.bindings.get(ordinal as usize) else {
        return Ok(None);
    };
    *exact &= binding.immutable;
    let Some(initializer) = binding
        .initializer
        .filter(|_| binding.initializer_function)
        .map(|span| facts.ast.peel_ts_sugar_span(span))
    else {
        return Ok(None);
    };
    let Some(function) = facts.ast.function_at(initializer) else {
        return Ok(None);
    };
    if function.r#async || function.generator {
        return Ok(None);
    }
    let returned = match &function.expression_return {
        Some(returned) => returned,
        None => {
            let mut returns = facts.ast.returns.iter().filter(|returned| {
                crate::owners::containing_ast_function(&facts.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            });
            let Some(returned) = returns.next() else {
                return Ok(None);
            };
            if returns.next().is_some() || !returned.straight_line {
                return Ok(None);
            }
            returned
        }
    };
    let Some(argument) = returned.argument else {
        return Ok(None);
    };
    let value = match slot {
        Some(slot) => literal_slot_value(facts, argument, slot),
        None => {
            let argument = facts.ast.peel_ts_sugar_span(argument);
            facts
                .ast
                .identifiers
                .iter()
                .any(|identifier| {
                    identifier.span == argument
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                })
                .then_some(argument)
        }
    };
    Ok(value.map(|value| ((file, value), (file, function.span))))
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
                .and_then(|slot| lookup.file_slots().get(*slot))
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
        slots: &OnceLock<Vec<Recorded<V>>>,
        file: &FileFacts,
        answer: impl FnOnce(FileId) -> V,
        empty: V,
    ) -> V {
        let index = self.file_index(file);
        let slots = slots.get_or_init(|| {
            crate::base::reads::file_slots(self.facts)
                .iter()
                .map(|_| Recorded::new())
                .collect()
        });
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

    /// ADR 0549: the answer for `call` with its placement call when that is
    /// in `file` itself. Nothing of the other file is read here: the answer
    /// carries what the owner pass needs, and the cross-file proof digest
    /// hashes it.
    pub(crate) fn returned_factory_at(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<(Option<Span>, ReturnedInvocation)> {
        let answers = self.returned_invocations(file);
        let found = answers
            .binary_search_by_key(&(call.span, call.callee), |(key, _)| *key)
            .ok()?;
        let answer = answers[found].1;
        let placed_here = answer
            .placement
            .filter(|(placed, _)| self.memos.table().path_of(*placed) == file.path.as_str())
            .map(|(_, at)| at);
        Some((placed_here, answer))
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
        // The caller reads the creating call in another file: the section
        // binds what it reads of it (`returned_source_facts`).
        crate::base::reads::note(crate::base::reads::Section::ReturnedSources);
        let answers = self.returned_sources(file);
        let found = answers
            .binary_search_by_key(&call, |answer| answer.call)
            .ok()?;
        let answer = answers[found];
        let slot = *self.memos.table().slots_of(answer.origin).first()?;
        let facts: &'a ProjectFacts = self.facts;
        let origin = crate::base::reads::file_slots(facts).get(slot)?;
        let created = origin.ast.call_at(answer.created)?;
        Some((origin, created, answer.primitive))
    }
}
