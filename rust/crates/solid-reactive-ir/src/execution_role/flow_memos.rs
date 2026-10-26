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
//! - a project callee at a parameter and a member tag's declared method
//!   resolve over [`ExactCallees`] ([`crate::indexes::CalleeWorld`]): the
//!   call's file's member targets, literal receivers and entity rows, the
//!   `FunctionForSymbol`, `BindingAt`, `ClassFor` and `PathFile` probes, the
//!   files those name, and the assigned-member census memo. Under runtime
//!   redirects (ADR 0322) the lookup answers ([`Ctx::project`]);
//! - an absent member's vetoes are the assigned-member census and the
//!   `DefinedReflectively` probe; a callback that runs in its call's window
//!   the dialect and the effect-apply memo (a returned adapter's invocation
//!   census is the lookup's);
//! - a call site's role is [`FlowSiteRole`], keyed by the walk's context. Its
//!   role machine reads the lookup ([`Ctx::project`]): it runs again in every
//!   build in which the program moved, and a walk is reused while every site
//!   it asked answers the same.
//!
//! The budget ([`FLOW_BUDGET`]) bounds what one walk visits; a walk that
//! exhausts it has recorded exactly the reads it made, so its answer is as
//! exact as any other. A role query nested in a walk (the site role's
//! classification asks other literals' flow) stays inside it: it runs in the
//! walk's `classifying` context, inside the site role's project read.

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
pub(crate) const FLOW_TABLES_FROM: u16 = 96;
pub(crate) const FLOW_TABLES_END: u16 = 128;
/// The flow narrowing's ids (256 to 319), also in [`FlowTables`].
pub(crate) const FLOW_NARROW_TABLES_FROM: u16 = 256;
pub(crate) const FLOW_NARROW_TABLES_END: u16 = 320;

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
    site_roles: FlowSiteRole = 256;
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

/// A project function: its file and ordinal there.
type ProjectFunction = Option<(FileId, u32)>;

/// A file's object-member targets ([`crate::object_members::ObjectMemberTargets`]):
/// `(read, path, function)`.
type MemberTargets = Option<Arc<Vec<(Span, Arc<str>, Span)>>>;

/// A file's literal-receiver members ([`crate::object_members::LiteralReceivers`]).
type ReceiverMembers = Option<Arc<Vec<(Span, bool)>>>;

/// The binding a reference resolves to: its file and ordinal there.
type ResolvedBinding = Option<(FileId, u32)>;

/// What [`FlowWorld::project_callee`] answered for `(file, call, argument)`.
type ProjectCallee<'w> = Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact, bool)>;

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
    /// Callee resolution's reads (ADR 0278, 0279, 0299, 0314): each file's
    /// object-member targets and literal receivers, the functions and
    /// classes symbols name, the bindings references resolve to, the
    /// analyzed files paths name and the assigned-member census.
    targets: RefCell<BTreeMap<FileId, MemberTargets>>,
    receivers: RefCell<BTreeMap<FileId, ReceiverMembers>>,
    functions: RefCell<BTreeMap<String, ProjectFunction>>,
    classes: RefCell<BTreeMap<String, Option<(FileId, Span)>>>,
    bindings: RefCell<BTreeMap<(FileId, Span), ResolvedBinding>>,
    paths: RefCell<BTreeMap<String, Option<FileId>>>,
    assigned: RefCell<Option<Option<Arc<crate::object_members::AssignedAnswer>>>>,
    callees: RefCell<BTreeMap<(FileId, u32, usize), ProjectCallee<'w>>>,
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
            targets: RefCell::new(BTreeMap::new()),
            receivers: RefCell::new(BTreeMap::new()),
            functions: RefCell::new(BTreeMap::new()),
            classes: RefCell::new(BTreeMap::new()),
            bindings: RefCell::new(BTreeMap::new()),
            paths: RefCell::new(BTreeMap::new()),
            assigned: RefCell::new(None),
            callees: RefCell::new(BTreeMap::new()),
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

    /// The facts of slot `rank` of `id`, read like [`Self::read`]'s first.
    fn read_slot(&self, id: FileId, rank: u32) -> Option<&'w FileFacts> {
        if rank == 0 {
            return self.read(id);
        }
        let read = self.ctx.file(id, rank as usize)?;
        read.compiler();
        let facts = read.facts();
        self.ids.borrow_mut().insert(facts.path.as_str(), id);
        Some(facts)
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

    /// The object-member targets of `id`'s first slot
    /// ([`crate::object_members::ObjectMemberTargets`]).
    fn targets(&self, id: FileId) -> MemberTargets {
        if let Some(targets) = self.targets.borrow().get(&id) {
            return targets.clone();
        }
        let targets = self
            .memo::<crate::object_members::ObjectMemberTargets>(&(id, 0))
            .map(Arc::new);
        self.targets.borrow_mut().insert(id, targets.clone());
        targets
    }

    /// The literal-receiver members of `id`'s first slot
    /// ([`crate::object_members::LiteralReceivers`]).
    fn receivers(&self, id: FileId) -> ReceiverMembers {
        if let Some(receivers) = self.receivers.borrow().get(&id) {
            return receivers.clone();
        }
        let receivers = self
            .memo::<crate::object_members::LiteralReceivers>(&(id, 0))
            .map(Arc::new);
        self.receivers.borrow_mut().insert(id, receivers.clone());
        receivers
    }

    /// The function `symbol` names (the `FunctionForSymbol` probe).
    fn function_named(&self, symbol: &str) -> ProjectFunction {
        if let Some(found) = self.functions.borrow().get(symbol) {
            return *found;
        }
        let found = self
            .ctx
            .function_for_symbol(symbol)
            .map(|(id, _, ordinal)| (id, ordinal));
        self.functions.borrow_mut().insert(symbol.to_owned(), found);
        found
    }

    /// The function `(file, ordinal)` names, read.
    fn function(
        &self,
        found: ProjectFunction,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        let (id, ordinal) = found?;
        let file = self.read(id)?;
        Some((file, file.ast.functions.get(ordinal as usize)?))
    }

    /// The function the value at `callee` of `id` names, as the lookup
    /// resolves it: the value symbol (the file's entity rows) and the
    /// function that symbol names.
    fn called_function(
        &self,
        id: FileId,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        let cached = self.called.borrow().get(&(id, callee)).copied();
        let called = cached.unwrap_or_else(|| {
            let called = self
                .ctx
                .file(id, 0)
                .and_then(|read| read.value_symbol(callee))
                .and_then(|symbol| self.function_named(symbol.as_str()));
            self.called.borrow_mut().insert((id, callee), called);
            called
        });
        self.function(called)
    }

    /// The analyzed file at `path` (the `PathFile` probe).
    fn analyzed(&self, path: &str) -> Option<FileId> {
        if let Some(id) = self.paths.borrow().get(path) {
            return *id;
        }
        let id = self.ctx.path_file(path).map(|(id, _)| id);
        self.paths.borrow_mut().insert(path.to_owned(), id);
        id
    }

    /// The assigned-member census ([`crate::object_members::AssignedMembersAnswer`]).
    fn assigned(&self) -> Option<Arc<crate::object_members::AssignedAnswer>> {
        if let Some(assigned) = self.assigned.borrow().as_ref() {
            return assigned.clone();
        }
        let assigned = self
            .memo::<crate::object_members::AssignedMembersAnswer>(&())
            .map(Arc::new);
        *self.assigned.borrow_mut() = Some(assigned.clone());
        assigned
    }

    /// `value`, unless a read met a cycle.
    fn finish<T>(self, value: T) -> Result<T, Cycle> {
        match self.cycle.into_inner() {
            Some(cycle) => Err(cycle),
            None => Ok(value),
        }
    }
}

/// Callee resolution in a flow memo ([`crate::indexes::CalleeWorld`]): every
/// answer a recorded read. Built only for a build without runtime redirects,
/// under which a file's own rows name a callee's symbol
/// ([`crate::base::memo::build::FileRead::callee_symbol`]); with them the
/// question reads the lookup.
struct ExactCallees<'f, 'r, 'c, 'w: 'c>(&'f CtxFlow<'r, 'c, 'w>);

impl<'w> crate::indexes::CalleeWorld<'w> for ExactCallees<'_, '_, '_, 'w> {
    fn call_withheld(&self, file: &FileFacts, callee: Span) -> bool {
        let Some(id) = self.0.id(file) else {
            return true;
        };
        // As `object_members::call_withheld`: a literal receiver's member
        // call (at the peeled callee) that no member target resolves (at the
        // callee itself). A cycle answers withheld.
        let peeled = file.ast.peel_ts_sugar_span(callee);
        let Some(receivers) = self.0.receivers(id) else {
            return true;
        };
        if !receivers.iter().any(|(span, _)| *span == peeled) {
            return false;
        }
        self.0
            .targets(id)
            .is_none_or(|targets| !targets.iter().any(|(read, _, _)| *read == callee))
    }

    fn object_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        let id = self.0.id(file)?;
        let targets = self.0.targets(id)?;
        // The lookup's map keeps the last row of a callee.
        let (_, path, function) = targets.iter().rev().find(|(read, _, _)| *read == callee)?;
        let target = self.0.read(self.0.analyzed(path)?)?;
        Some((target, target.ast.function_at(*function)?))
    }

    fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'w str> {
        let id = self.0.id(file)?;
        self.0.ctx.file(id, 0)?.callee_symbol(callee)
    }

    fn function_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        self.0.function(self.0.function_named(symbol))
    }

    fn function_called_at(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        self.0.called_function(self.0.id(file)?, callee)
    }

    fn entity_symbol_at(&self, file: &FileFacts, span: Span) -> Option<&'w crate::SymbolId> {
        let id = self.0.id(file)?;
        self.0.ctx.file(id, 0)?.symbol_at(span)
    }

    fn binding_at_reference(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::BindingFact)> {
        let id = self.0.id(file)?;
        let cached = self.0.bindings.borrow().get(&(id, span)).copied();
        let found = cached.unwrap_or_else(|| {
            let found = self
                .0
                .ctx
                .binding_at(id, span)
                .map(|(bound, _, ordinal)| (bound, ordinal));
            self.0.bindings.borrow_mut().insert((id, span), found);
            found
        });
        let (bound, ordinal) = found?;
        let bound = self.0.read(bound)?;
        Some((bound, bound.ast.bindings.get(ordinal as usize)?))
    }

    fn class_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::ClassFact)> {
        let cached = self.0.classes.borrow().get(symbol).copied();
        let found = cached.unwrap_or_else(|| {
            let found = self
                .0
                .ctx
                .class_for_symbol(symbol)
                .map(|(id, _, span)| (id, span));
            self.0.classes.borrow_mut().insert(symbol.to_owned(), found);
            found
        });
        let (id, span) = found?;
        let file = self.0.read(id)?;
        Some((
            file,
            file.ast.classes.iter().find(|class| class.span == span)?,
        ))
    }

    // A cycle in the census fails closed: every member may be written.
    fn member_name_assigned(&self, name: &str) -> bool {
        self.0.assigned().is_none_or(|assigned| {
            assigned
                .names
                .binary_search_by(|candidate| candidate.as_ref().cmp(name))
                .is_ok()
        })
    }

    fn builtin_prototype_written(&self) -> bool {
        self.0
            .assigned()
            .is_none_or(|assigned| assigned.global || assigned.hosts.iter().any(|(_, dom)| !dom))
    }

    fn constructor_prototypes_written(&self) -> bool {
        self.0
            .assigned()
            .is_none_or(|assigned| !assigned.constructors.is_empty())
    }

    fn constructor_prototype_written(&self, path: &str, declaration: Span) -> bool {
        self.0.assigned().is_none_or(|assigned| {
            assigned
                .constructors
                .iter()
                .any(|(written, at)| written.as_ref() == path && *at == declaration)
        })
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
        // Whether a callback runs in its call's own window
        // ([`callback_execution_at_call`]): the dialect's answer, once the
        // effect apply census proves the apply runs, and once the
        // returned-invocation census proves a returned adapter is invoked.
        if self
            .dialect()
            .callback_inherits_calling_window(primitive, index)
        {
            let semantics =
                self.dialect()
                    .callback_semantics_at(primitive, index, call.arguments.len());
            let inherits = if semantics.requires_return_invocation {
                // The returned-invocation census is assembled per build, not
                // a memo: the lookup answers.
                let ordinal = ordinal_of(&file.ast.calls, call);
                self.project(|lookup, table| {
                    path_file(lookup, table, id)
                        .zip(ordinal)
                        .and_then(|(file, ordinal)| {
                            Some((file, file.ast.calls.get(ordinal as usize)?))
                        })
                        .is_some_and(|(file, call)| {
                            callback_execution_at_call(file, call, primitive, index, lookup)
                                .is_some()
                        })
                })
            } else {
                let applied = self
                    .dialect()
                    .effect_apply_defer_options(primitive, index)
                    .is_none()
                    || self
                        .memo::<crate::indexes::EffectApplyCalls>(&())
                        .is_some_and(|rows| {
                            rows.iter().any(|(path, span, argument, execution)| {
                                path.as_str() == file.path.as_str()
                                    && *span == call.span
                                    && *argument == index
                                    && *execution == crate::effect_execution::ApplyExecution::Proven
                            })
                        });
                applied && semantics.execution.is_some()
            };
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
        if let Some(found) = self.callees.borrow().get(&(id, ordinal, index)) {
            return *found;
        }
        let found = if self.ctx.constant(Constant::RuntimeRedirects) {
            // A redirected callee symbol (ADR 0322) is the lookup's.
            self.project(|lookup, table| {
                let file = path_file(lookup, table, id)?;
                let call = file.ast.calls.get(ordinal as usize)?;
                let (callee_file, callee, exact) =
                    project_callee_at_parameter(file, call, index, lookup)?;
                Some((
                    table.id_of(callee_file.path.as_str())?,
                    ordinal_of(&callee_file.ast.functions, callee)?,
                    exact,
                ))
            })
            .and_then(|(callee, function, exact)| {
                let callee_file = self.read(callee)?;
                let function = callee_file.ast.functions.get(function as usize)?;
                Some((callee_file, function, exact))
            })
        } else {
            project_callee_in(&ExactCallees(self), file, call, index)
        };
        self.callees
            .borrow_mut()
            .insert((id, ordinal, index), found);
        found
    }

    fn function_called_at(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w solid_facts::ast::FunctionFact)> {
        self.called_function(self.id(file)?, callee)
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
        if !self.ctx.constant(Constant::RuntimeRedirects) {
            return member_runs_declared_method_in(
                &ExactCallees(self),
                file,
                tag,
                component_file,
                component,
            );
        }
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
        // As [`absent_member_inert_in`]: the assigned-member census and the
        // reflective definers.
        let callees = ExactCallees(self);
        let builtin = |name: &str| {
            crate::indexes::callee_world::member_may_be_reassigned_on_builtin_in(&callees, name)
        };
        !builtin(key) && !builtin("__proto__") && !self.ctx.defined_reflectively(key)
    }

    fn site_role(
        &self,
        file: &'w FileFacts,
        call: Span,
        classifying: &mut HashSet<(String, Span)>,
    ) -> Option<ExecutionRole> {
        let id = self.id(file)?;
        // Asked in the walk's own context ([`FlowSiteRole`]), which the
        // query leaves as it found it.
        let mut context = classifying.iter().cloned().collect::<Vec<_>>();
        context.sort_unstable();
        self.memo::<FlowSiteRole>(&(id, call, context)).flatten()
    }

    fn exported_call_sites(
        &self,
        file: &'w FileFacts,
        function: &'w solid_facts::ast::FunctionFact,
    ) -> Option<Vec<(&'w FileFacts, Span)>> {
        let id = self.id(file)?;
        let name = function.name.as_ref()?;
        // The census's own answers (the call graph, the hand-outs, the
        // symbol's references), each a recorded read.
        let sites =
            match crate::indexes::exported_call_sites_of(self.ctx, id, function.span, name.span) {
                Ok(sites) => sites?,
                Err(cycle) => {
                    self.cycle.borrow_mut().get_or_insert(cycle);
                    return None;
                }
            };
        sites
            .into_iter()
            .map(|site| {
                let caller = self.read_slot(site.file, site.rank)?;
                let call = caller.ast.calls.iter().find(|call| {
                    !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == site.callee
                })?;
                Some((caller, call.span))
            })
            .collect()
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

/// A call site's role as a flow walk asks it ([`FlowWorld::site_role`]): the
/// file, the call, and the walk's context (the literals it is classifying,
/// sorted), which the role query consults to cut a cycle and leaves as it
/// found it. A function of its key, so a memo: a walk that asks reads this
/// answer, not the whole program, and is reused while every site it asked
/// answers the same. The role machine reads the lookup's tables
/// (`semantic_execution_role_within` and the named-callback census), so the
/// answer itself reads the whole program ([`Ctx::project`]) and runs in
/// every build in which the program moved.
pub(crate) struct FlowSiteRole;

impl Memo<BuildSchema> for FlowSiteRole {
    type Key = (FileId, Span, Vec<(String, Span)>);
    type Value = Option<ExecutionRole>;
    const NAME: &'static str = "flow_site_role";
    const ID: TableId = Self::TABLE;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        Self::of(tables)
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (id, call, context): &(FileId, Span, Vec<(String, Span)>),
    ) -> Result<Option<ExecutionRole>, Cycle> {
        // As every flow memo, a nested role query computes in this context
        // whoever asks ([`computing_flow`]).
        Ok(computing_flow(|| {
            ctx.project_file_recorded(*id, |lookup, table| {
                let file = path_file(lookup, table, *id)?;
                let mut classifying = context.iter().cloned().collect::<HashSet<_>>();
                site_role_in(lookup, file, *call, &mut classifying)
            })
        }))
    }

    fn on_cycle(_: &(FileId, Span, Vec<(String, Span)>)) -> Option<ExecutionRole> {
        None
    }
}
