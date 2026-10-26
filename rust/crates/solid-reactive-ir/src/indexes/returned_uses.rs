//! Returned-callback use sites on memos (study S5e, ADR 0390).
//!
//! `createReaction(onInvalidate)`, `lazy(loader)`, `lazyModule(loader)`,
//! `mapArray(..)` and `repeat(..)` return a function, and the callbacks they
//! take run only when that function is used. Whether a returned function is
//! used is a project-wide fact: the factory call's result is bound, aliased
//! through other bindings and files, then called, rendered as a tag, passed to
//! a callback-taking primitive, or `.preload()`ed.
//!
//! [`ReturnedUses`] answers it per factory call (file, span) with its reads
//! recorded: the factory's own file, the closure of exact symbols that hold
//! its result, and per symbol two L1 index keys, TypeFacts' references of the
//! symbol and the spans whose entity is the symbol
//! ([`crate::base::contributions::use_spans`]). Only the files that hold such a
//! span are read. An edit of a file that neither holds the factory nor uses
//! its result leaves the answer unrun.
//!
//! The answer is structural: where the result is used and how (`Call`,
//! `Argument(i)`, `Preload`, `Jsx`). What a use inherits (the primitive it is
//! handed to, the owner of that callback) depends on tables that are not
//! memos yet and is assembled by the consumer
//! ([`crate::owners::returned_callback_invocation_sites`]).
//!
//! **Premise.** As for the census: the slots of one path carry one source, so
//! every memo here reads a path's first slot.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::core::Span;

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, FileRead, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};

/// The first table id of the returned-callback tables (ids 128 to 159).
pub(crate) const RETURNED_TABLES_FROM: u16 = 128;

/// How a use consumes the result of a factory call.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum UseKind {
    /// The result is the callee of the call at the use's span.
    Call,
    /// The result is the argument at this index of the call at the use's span.
    Argument(u32),
    /// `result.preload()`: the call at the use's span invokes the loader.
    Preload,
    /// The result is the tag of the JSX element at the use's span.
    Jsx,
}

/// One proven use of a factory call's result: the call or element at `span`
/// of `file`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ReturnedUse {
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) kind: UseKind,
}

impl Canonical for UseKind {}
impl Canonical for ReturnedUse {}

macro_rules! returned_tables {
    ($($field:ident: $memo:ty = $id:literal;)*) => {
        /// The returned-callback tables, inside the build's
        /// ([`crate::base::memo::build::BuildTables`]).
        #[derive(Default)]
        pub(crate) struct ReturnedTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl ReturnedTables {
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
                &tables.returned().$field
            }
        })*
    };
}

returned_tables! {
    uses: ReturnedUses = 128;
}

impl SemanticLookup<'_> {
    /// The proven uses of the function the call at `span` of `path` returns,
    /// sorted. Empty for a call the lookup's table does not name.
    pub(crate) fn returned_uses(&self, path: &str, span: Span) -> Arc<Vec<ReturnedUse>> {
        let Some(id) = self.memos.table().id_of(path) else {
            return Arc::default();
        };
        self.memo_world().answer::<ReturnedUses>(&(id, span))
    }

    /// The file a returned use names, in this lookup (its last slot, as
    /// `file_by_path` reads it).
    pub(crate) fn returned_use_file(&self, used: &ReturnedUse) -> Option<&FileFacts> {
        let slot = *self.memos.table().slots_of(used.file).last()?;
        // A hand-out (`hand_outs`): the use's file.
        self.note_slot_read(slot);
        self.facts.files.get(slot)
    }
}

/// The uses of the result of the factory call at `span` of `file`
/// ([`SemanticLookup::returned_uses`]).
pub(crate) struct ReturnedUses;

impl Memo<BuildSchema> for ReturnedUses {
    type Key = (FileId, Span);
    type Value = Arc<Vec<ReturnedUse>>;
    const NAME: &'static str = "returned_callback_uses";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (file, factory): &(FileId, Span),
    ) -> Result<Arc<Vec<ReturnedUse>>, Cycle> {
        let Some(read) = ctx.file(*file, 0) else {
            return Ok(Arc::default());
        };
        let facts = read.facts();
        let mut uses = BTreeSet::new();
        direct_uses(*file, facts, *factory, &mut uses);

        // The exact symbols the result is bound to, then every symbol bound
        // from one of them (an alias, in any file).
        let mut symbols = BTreeSet::<Arc<str>>::new();
        let mut queue = Vec::new();
        for binding in facts
            .ast
            .bindings
            .iter()
            .filter(|binding| binding.call_initializer == Some(*factory))
        {
            for name in &binding.names {
                if let Some(symbol) = read.symbol_at(name.span)
                    && symbols.insert(symbol.shared())
                {
                    queue.push(symbol.shared());
                }
            }
        }
        while let Some(symbol) = queue.pop() {
            // The spans that name the symbol: its TypeFacts references, and
            // the spans whose entity it is, by file.
            let mut spans = BTreeMap::<FileId, BTreeSet<Span>>::new();
            for (id, _, span) in ctx.references(&symbol) {
                spans.entry(id).or_default().insert(span);
            }
            for (id, _, span) in ctx.use_spans(&symbol) {
                spans.entry(id).or_default().insert(span);
            }
            for (id, named) in spans {
                let Some(user) = ctx.file(id, 0) else {
                    continue;
                };
                let mut aliases = Vec::new();
                classify(&user, id, &named, &mut uses, &mut aliases);
                for alias in aliases {
                    if symbols.insert(Arc::clone(&alias)) {
                        queue.push(alias);
                    }
                }
            }
        }
        Ok(Arc::new(uses.into_iter().collect()))
    }

    fn on_cycle(_: &(FileId, Span)) -> Arc<Vec<ReturnedUse>> {
        Arc::default()
    }

    fn implied((file, _): &(FileId, Span)) -> Vec<Input> {
        vec![
            Input::File(*file, Section::Source),
            Input::File(*file, Section::Entities),
        ]
    }
}

/// The property span of the member expression at exactly `span`, the last
/// when several share it.
fn member_property(facts: &FileFacts, span: Span) -> Option<Span> {
    facts
        .ast
        .members_within(span)
        .filter(|member| member.span == span)
        .last()
        .map(|member| member.property)
}

/// The uses written around the factory call itself: `factory(...)()`,
/// `factory(...).preload()` and `primitive(factory(...))`.
fn direct_uses(file: FileId, facts: &FileFacts, factory: Span, uses: &mut BTreeSet<ReturnedUse>) {
    let ast = &facts.ast;
    let direct_factory_value = |argument: Span| {
        ast.calls_within(argument)
            .max_by_key(|nested| nested.span.end - nested.span.start)
            .is_some_and(|nested| nested.span == factory)
    };
    for outer in ast
        .calls_with_callee_containing(factory)
        .filter(|outer| outer.span != factory)
        .filter(|outer| {
            ast.calls_within(outer.callee)
                .filter(|nested| nested.span != outer.span)
                .max_by_key(|nested| nested.span.end - nested.span.start)
                .is_some_and(|nested| nested.span == factory)
                // `factory(...).member()` invokes the member, not the
                // returned function; `preload` is the one member the
                // runtime routes to the loader.
                && member_property(facts, outer.callee)
                    .is_none_or(|property| facts.source_text(property) == Some("preload"))
        })
    {
        uses.insert(ReturnedUse {
            file,
            span: outer.span,
            kind: UseKind::Call,
        });
    }
    for (outer, index) in ast.arguments_containing(factory) {
        if direct_factory_value(outer.arguments[index].span) {
            uses.insert(ReturnedUse {
                file,
                span: outer.span,
                kind: UseKind::Argument(argument_index(index)),
            });
        }
    }
}

fn argument_index(index: usize) -> u32 {
    u32::try_from(index).expect("fewer than 2^32 arguments")
}

/// The uses among the spans `named` that name a symbol holding the result,
/// and the symbols a binding initialized from one of them declares.
fn classify(
    read: &FileRead<'_, '_, '_>,
    file: FileId,
    named: &BTreeSet<Span>,
    uses: &mut BTreeSet<ReturnedUse>,
    aliases: &mut Vec<Arc<str>>,
) {
    let facts = read.facts();
    for call in &facts.ast.calls {
        if named.contains(&call.callee) {
            uses.insert(ReturnedUse {
                file,
                span: call.span,
                kind: UseKind::Call,
            });
        }
        for (index, argument) in call.arguments.iter().enumerate() {
            if named.contains(&argument.span) {
                uses.insert(ReturnedUse {
                    file,
                    span: call.span,
                    kind: UseKind::Argument(argument_index(index)),
                });
            }
        }
    }
    // `result.preload()`: the call whose callee holds a `preload` member of
    // the result.
    for member in &facts.ast.members {
        if named.contains(&member.object) && facts.source_text(member.property) == Some("preload") {
            for outer in facts.ast.calls_with_callee_containing(member.span) {
                uses.insert(ReturnedUse {
                    file,
                    span: outer.span,
                    kind: UseKind::Preload,
                });
            }
        }
    }
    for element in &facts.ast.jsx_elements {
        if named.contains(&element.name.span) {
            uses.insert(ReturnedUse {
                file,
                span: element.span,
                kind: UseKind::Jsx,
            });
        }
    }
    for binding in &facts.ast.bindings {
        if binding
            .initializer_identifier
            .as_ref()
            .is_some_and(|initializer| named.contains(&initializer.span))
        {
            aliases.extend(
                binding
                    .names
                    .iter()
                    .filter_map(|name| read.symbol_at(name.span))
                    .map(|symbol| symbol.shared()),
            );
        }
    }
}
