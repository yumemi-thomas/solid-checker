//! The invocation-flow walk's answers on memos (study S5a): each file's
//! ADR 0319 answers and its event-only census (ADR 0327) are L2 answers with
//! their reads recorded ([`crate::base::memo`]).
//!
//! A top-level answer is computed from an empty context; the answers of one
//! file's literals are one memo, each still computed from an empty context.
//! A walk costs microseconds, so a memo per literal spent more on its own
//! bookkeeping (key, entry, read graph, the table's lock under parallel
//! installs) than on the walk: per file, one record covers its literals.
//!
//! The walk ([`InvocationFlow`]) runs over [`CtxFlow`], whose every question
//! is a recorded read, each made once per memo:
//! - a file the walk holds is read through the context (its source and
//!   compiler sections): the key's own file, and each file an answer below
//!   hands it;
//! - component identity is the census's per-file memo
//!   ([`crate::indexes::FileComponentStatuses`]), the primitive at a call the
//!   census's per-file adapter ([`crate::indexes::CallPrimitives`]), `eval`
//!   the hazard probe, the component a tag renders the tag's entity row and
//!   the `FunctionForSymbol` probe, a primitive slot the call's signature
//!   section and the dialect;
//! - the questions whose tables are not ported (a call site's role, a project
//!   callee at a parameter, a callback that runs in its call's window, a
//!   member tag's declared method, an absent member's vetoes) are
//!   [`Ctx::project`] reads: a file whose walks ask one is recomputed in every
//!   build in which the program moved, and narrows as those tables port.
//!
//! The budget ([`FLOW_BUDGET`]) bounds what one walk visits; a walk that
//! exhausts it has recorded exactly the reads it made, so its answer is as
//! exact as any other. A role query nested in a walk (the site role's
//! classification asks other literals' flow) stays inside it: it runs in the
//! walk's `classifying` context, inside the project read.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use solid_facts::FileFacts;

use super::*;
use crate::base::memo::build::{BuildSchema, BuildTables, Constant, Input, Section};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};
use crate::base::{FileId, FileTable, Hazard};
use crate::indexes::ComponentIdentity;

impl Canonical for ExecutionRole {}

/// The first table id of the flow memos, and the first past them (S5a holds
/// ids 96 to 127).
pub(crate) const FLOW_TABLES_FROM: u8 = 96;
pub(crate) const FLOW_TABLES_END: u8 = 128;

macro_rules! flow_tables {
    ($($field:ident: $memo:ty = $id:literal;)*) => {
        /// The flow memos' tables, inside the build's
        /// ([`crate::base::memo::build::BuildTables`]).
        #[derive(Default)]
        pub(crate) struct FlowTables {
            $($field: Table<BuildSchema, $memo>,)*
        }

        impl FlowTables {
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
                &tables.flow().$field
            }
        })*
    };
}

flow_tables! {
    answers: InvocationFlowAnswers = 96;
    event_invoked: EventInvokedLiterals = 97;
}

/// Every analyzed file as the flow memos key it, with its path. A path
/// listed twice is one source: its first slot stands for it.
pub(super) fn file_keys(lookup: &SemanticLookup<'_>) -> Vec<(FileId, String)> {
    lookup
        .files()
        .iter()
        .enumerate()
        .filter_map(|(slot, file)| {
            let (id, rank) = lookup.memo_slot_of(slot);
            (rank == 0).then(|| (id, file.path.to_string()))
        })
        .collect()
}

/// The position of `item` in `items`, by identity.
fn ordinal_of<T>(items: &[T], item: &T) -> Option<u32> {
    items
        .iter()
        .position(|candidate| std::ptr::eq(candidate, item))
        .and_then(|index| u32::try_from(index).ok())
}

/// A file's primitives by call span ([`crate::indexes::CallPrimitives`]).
type CallPrimitives = Arc<BTreeMap<Span, Primitive>>;

/// The function a value names: its file and ordinal there.
type CalledFunction = Option<(FileId, u32)>;

/// The walk's world in a memo computation: every answer a recorded read.
/// Each input is read once per walk: a repeated question is answered from
/// the walk's own copy, so the record stays short.
struct CtxFlow<'r, 'c, 'w: 'c> {
    ctx: &'r Ctx<'c, 'w, BuildSchema>,
    /// The files the walk holds, by path, each read through the context.
    ids: RefCell<BTreeMap<&'w str, FileId>>,
    files: RefCell<BTreeMap<FileId, Option<&'w FileFacts>>>,
    statuses: RefCell<BTreeMap<FileId, Option<Arc<Vec<ComponentIdentity>>>>>,
    primitives: RefCell<BTreeMap<FileId, Option<CallPrimitives>>>,
    /// A tag's or callee's component: `(file, ordinal)`.
    called: RefCell<BTreeMap<(FileId, Span), CalledFunction>>,
    /// The first cycle a memo read met: the computation fails closed.
    cycle: RefCell<Option<Cycle>>,
}

impl<'r, 'c, 'w: 'c> CtxFlow<'r, 'c, 'w> {
    fn new(ctx: &'r Ctx<'c, 'w, BuildSchema>) -> Self {
        Self {
            ctx,
            ids: RefCell::new(BTreeMap::new()),
            files: RefCell::new(BTreeMap::new()),
            statuses: RefCell::new(BTreeMap::new()),
            primitives: RefCell::new(BTreeMap::new()),
            called: RefCell::new(BTreeMap::new()),
            cycle: RefCell::new(None),
        }
    }

    /// The facts of `id`'s first slot, read: its source and its compiler map
    /// (the owner question reads the compiler's event-handler facts).
    fn read(&self, id: FileId) -> Option<&'w FileFacts> {
        if let Some(facts) = self.files.borrow().get(&id) {
            return *facts;
        }
        let facts = self.ctx.file(id, 0).map(|read| {
            read.compiler();
            read.facts()
        });
        if let Some(facts) = facts {
            self.ids.borrow_mut().insert(facts.path.as_str(), id);
        }
        self.files.borrow_mut().insert(id, facts);
        facts
    }

    /// The id of a file the walk holds; `None` for any other.
    fn id(&self, file: &FileFacts) -> Option<FileId> {
        self.ids.borrow().get(file.path.as_str()).copied()
    }

    fn memo<M: Memo<BuildSchema>>(&self, key: &M::Key) -> Option<M::Value> {
        match self.ctx.memo::<M>(key) {
            Ok(value) => Some(value),
            Err(cycle) => {
                self.cycle.borrow_mut().get_or_insert(cycle);
                None
            }
        }
    }

    /// A question of a table not ported yet, asked of the build's lookup:
    /// recorded as a read of the whole program ([`Ctx::project`]).
    fn project<R>(&self, f: impl for<'x> FnOnce(&SemanticLookup<'x>, &FileTable) -> R) -> R {
        self.ctx.project_files(f)
    }

    /// `value`, unless a read met a cycle.
    fn finish<T>(self, value: T) -> Result<T, Cycle> {
        match self.cycle.into_inner() {
            Some(cycle) => Err(cycle),
            None => Ok(value),
        }
    }
}

impl<'w> FlowWorld<'w> for CtxFlow<'_, '_, 'w> {
    fn dialect(&self) -> &'w dyn solid_dialect::Dialect {
        self.ctx.dialect()
    }

    fn canonical(&self, file: &'w FileFacts) -> Option<&'w FileFacts> {
        self.id(file).map(|_| file)
    }

    fn program_has_eval(&self) -> bool {
        self.ctx.hazard(Hazard::Eval)
    }

    fn component_status(
        &self,
        file: &'w FileFacts,
        function: &'w solid_facts::ast::FunctionFact,
    ) -> ComponentStatus {
        let identity = self.id(file).and_then(|id| {
            let ordinal = ordinal_of(&file.ast.functions, function)?;
            let cached = self.statuses.borrow().get(&id).cloned();
            let statuses = cached.unwrap_or_else(|| {
                let statuses = self.memo::<crate::indexes::FileComponentStatuses>(&(id, 0));
                self.statuses.borrow_mut().insert(id, statuses.clone());
                statuses
            })?;
            statuses.get(ordinal as usize).copied()
        });
        match identity {
            Some(ComponentIdentity::Proven | ComponentIdentity::Escaped) => ComponentStatus::Proven,
            Some(ComponentIdentity::No) => {
                crate::indexes::unproven_component_status(self.ctx.dialect(), file, function)
            }
            Some(ComponentIdentity::Possible) | None => ComponentStatus::Uncertain,
        }
    }

    fn primitive_at_call(&self, file: &'w FileFacts, call: Span) -> Option<Primitive> {
        let id = self.id(file)?;
        let cached = self.primitives.borrow().get(&id).cloned();
        let primitives = cached.unwrap_or_else(|| {
            let primitives = self.memo::<crate::indexes::CallPrimitives>(&id);
            self.primitives.borrow_mut().insert(id, primitives.clone());
            primitives
        })?;
        primitives.get(&call).copied()
    }

    fn primitive_slot(
        &self,
        file: &'w FileFacts,
        call: &'w solid_facts::ast::CallFact,
        index: usize,
    ) -> (bool, Option<ExecutionRole>) {
        let Some(primitive) = self.primitive_at_call(file, call.span) else {
            return (false, None);
        };
        let Some(id) = self.id(file) else {
            return (false, None);
        };
        // Whether a callback runs in its call's own window is the effect
        // apply census's and the returned-invocation census's question.
        if self
            .dialect()
            .callback_inherits_calling_window(primitive, index)
        {
            let ordinal = ordinal_of(&file.ast.calls, call);
            let inherits = self.project(|lookup, table| {
                path_file(lookup, table, id)
                    .zip(ordinal)
                    .and_then(|(file, ordinal)| Some((file, file.ast.calls.get(ordinal as usize)?)))
                    .is_some_and(|(file, call)| {
                        callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    })
            });
            if inherits {
                return (true, None);
            }
        }
        let valid = self
            .ctx
            .file(id, 0)
            .and_then(|read| read.resolved_callee_call(call.callee))
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid);
        (
            false,
            primitive_slot_role(self.dialect(), valid, Some(primitive), call, index),
        )
    }

    fn project_callee(
        &self,
        file: &'w FileFacts,
        call: &'w solid_facts::ast::CallFact,
        index: usize,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact, bool)> {
        let id = self.id(file)?;
        let ordinal = ordinal_of(&file.ast.calls, call)?;
        let (callee, function, exact) = self.project(|lookup, table| {
            let file = path_file(lookup, table, id)?;
            let call = file.ast.calls.get(ordinal as usize)?;
            let (callee_file, callee, exact) =
                project_callee_at_parameter(file, call, index, lookup)?;
            Some((
                table.id_of(callee_file.path.as_str())?,
                ordinal_of(&callee_file.ast.functions, callee)?,
                exact,
            ))
        })?;
        let callee_file = self.read(callee)?;
        let function = callee_file.ast.functions.get(function as usize)?;
        Some((callee_file, function, exact))
    }

    fn function_called_at(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        // As the lookup resolves it: the tag's value symbol (the file's
        // entity rows) and the function that symbol names.
        let id = self.id(file)?;
        let cached = self.called.borrow().get(&(id, callee)).copied();
        let called = cached.unwrap_or_else(|| {
            let called = self
                .ctx
                .file(id, 0)
                .and_then(|read| read.value_symbol(callee))
                .and_then(|symbol| self.ctx.function_for_symbol(symbol.as_str()))
                .map(|(target, _, ordinal)| (target, ordinal));
            self.called.borrow_mut().insert((id, callee), called);
            called
        });
        let (target, ordinal) = called?;
        let target_file = self.read(target)?;
        let function = target_file.ast.functions.get(ordinal as usize)?;
        Some((target_file, function))
    }

    fn member_runs_declared_method(
        &self,
        file: &'w FileFacts,
        tag: Span,
        component_file: &'w FileFacts,
        component: &'w solid_facts::ast::FunctionFact,
    ) -> bool {
        let (Some(id), Some(component_id), Some(ordinal)) = (
            self.id(file),
            self.id(component_file),
            ordinal_of(&component_file.ast.functions, component),
        ) else {
            return false;
        };
        self.project(|lookup, table| {
            let (Some(file), Some(component_file)) = (
                path_file(lookup, table, id),
                path_file(lookup, table, component_id),
            ) else {
                return false;
            };
            component_file
                .ast
                .functions
                .get(ordinal as usize)
                .is_some_and(|component| {
                    member_runs_declared_method(file, tag, component_file, component, lookup)
                })
        })
    }

    fn absent_member_inert(&self, key: &str) -> bool {
        self.project(|lookup, _| absent_member_inert_in(lookup, key))
    }

    fn site_role(
        &self,
        file: &'w FileFacts,
        call: Span,
        classifying: &mut HashSet<(String, Span)>,
    ) -> Option<ExecutionRole> {
        let id = self.id(file)?;
        // Asked in the walk's own context, which the query leaves as it
        // leaves it on the lookup.
        self.project(|lookup, table| {
            let file = path_file(lookup, table, id)?;
            site_role_in(lookup, file, call, classifying)
        })
    }
}

/// The first slot of `id` as the lookup holds it, for a project read.
fn path_file<'l>(
    lookup: &SemanticLookup<'l>,
    table: &FileTable,
    id: FileId,
) -> Option<&'l FileFacts> {
    let slot = *table.slots_of(id).first()?;
    lookup.files().get(slot)
}

/// ADR 0319: the one role every invocation of each function literal of a
/// file runs in, each from an empty context: `(literal span, role)` for the
/// literals the walk proves a role for, in literal order. Empty in an open
/// program; nothing is proven under an `eval`, for a component, for an
/// invocation the walk does not understand, or past the budget.
pub(crate) struct InvocationFlowAnswers;

impl Memo<BuildSchema> for InvocationFlowAnswers {
    type Key = FileId;
    type Value = Arc<Vec<(Span, ExecutionRole)>>;
    const NAME: &'static str = "invocation_flow_answers";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<Vec<(Span, ExecutionRole)>>, Cycle> {
        if !ctx.constant(Constant::ProgramClosed) {
            return Ok(Arc::default());
        }
        let world = CtxFlow::new(ctx);
        let mut roles = Vec::new();
        if let Some(file) = world.read(*id) {
            computing_flow(|| {
                for literal in &file.ast.functions {
                    let mut classifying = HashSet::default();
                    if let Some(role) = literal_flow_role(&world, file, literal, &mut classifying) {
                        roles.push((literal.span, role));
                    }
                }
            });
        }
        world.finish(Arc::new(roles))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<(Span, ExecutionRole)>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Compiler),
        ]
    }
}

/// ADR 0327: the function literals of a file every invocation of which,
/// followed forward as ADR 0319 follows it, runs through event delivery
/// ([`event_invoked_literals`]), each from an empty context, in literal
/// order.
pub(crate) struct EventInvokedLiterals;

impl Memo<BuildSchema> for EventInvokedLiterals {
    type Key = FileId;
    type Value = Arc<Vec<Span>>;
    const NAME: &'static str = "event_invoked_literals";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<Arc<Vec<Span>>, Cycle> {
        if !ctx.constant(Constant::ProgramClosed) || ctx.hazard(Hazard::Eval) {
            return Ok(Arc::default());
        }
        let world = CtxFlow::new(ctx);
        let mut invoked = Vec::new();
        if let Some(file) = world.read(*id) {
            computing_flow(|| {
                for literal in &file.ast.functions {
                    if literal_event_invoked(&world, file, literal) {
                        invoked.push(literal.span);
                    }
                }
            });
        }
        world.finish(Arc::new(invoked))
    }

    fn on_cycle(_: &FileId) -> Arc<Vec<Span>> {
        Arc::default()
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![
            Input::File(*id, Section::Source),
            Input::File(*id, Section::Compiler),
        ]
    }
}
