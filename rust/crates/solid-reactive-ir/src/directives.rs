//! Interprocedural discovery of reactive primitives created during directive
//! application.

use std::collections::{HashMap, HashSet};

use solid_dialect::Dialect;
use solid_facts::core::Span;

use super::{
    PrimitiveCreation, PrimitiveName, SemanticLookup, SymbolId, call_primitive_name, location,
};
use crate::owners::containing_ast_function;
use crate::pipeline::{AnalysisContext, ProgramDraft};

pub(super) struct DirectiveCreationCollector<'a, 'c> {
    lookup: &'c SemanticLookup<'a>,
    symbol_names: &'c HashMap<SymbolId, SymbolId>,
    visiting: HashSet<(String, Span)>,
    creations: &'c mut Vec<PrimitiveCreation>,
    seen: &'c mut HashSet<(String, u64, u64)>,
    /// Whether the returned callbacks are not proven to be the applied ones.
    uncertain: bool,
}

impl<'a, 'c> DirectiveCreationCollector<'a, 'c> {
    pub(super) fn new(
        lookup: &'c SemanticLookup<'a>,
        symbol_names: &'c HashMap<SymbolId, SymbolId>,
        creations: &'c mut Vec<PrimitiveCreation>,
        seen: &'c mut HashSet<(String, u64, u64)>,
        uncertain: bool,
    ) -> Self {
        Self {
            lookup,
            symbol_names,
            visiting: HashSet::new(),
            creations,
            seen,
            uncertain,
        }
    }

    pub(super) fn collect_returned(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) {
        self.collect_returned_from(file, function, self.uncertain);
    }

    /// `uncertain` once anything on the way is not proven to run: calling a
    /// generator runs none of its body, it only returns an iterator.
    fn collect_returned_from(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        uncertain: bool,
    ) {
        let uncertain = uncertain || function.generator;
        let key = (file.path.to_string(), function.span);
        if !self.visiting.insert(key.clone()) {
            return;
        }
        for returned in function
            .expression_return
            .iter()
            .chain(file.ast.returns.iter().filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
        {
            match returned.value {
                solid_facts::ast::ReturnValueKind::Function => {
                    if let Some(returned_function) = file
                        .ast
                        .functions
                        .iter()
                        .find(|candidate| candidate.span == returned.span)
                    {
                        self.collect_function(file, returned_function, uncertain);
                    }
                }
                solid_facts::ast::ReturnValueKind::Call => {
                    if let Some(callee) = returned.callee
                        && let Some((target_file, target)) =
                            self.lookup.function_called_at(file.path.as_str(), callee)
                    {
                        self.collect_returned_from(target_file, target, uncertain);
                    }
                }
                _ => {}
            }
        }
        self.visiting.remove(&key);
    }

    fn collect_function(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        uncertain: bool,
    ) {
        let uncertain = uncertain || function.generator;
        let key = (file.path.to_string(), function.span);
        if !self.visiting.insert(key.clone()) {
            return;
        }
        for call in file.ast.calls.iter().filter(|call| {
            containing_ast_function(&file.ast, call.span)
                .is_some_and(|owner| owner.span == function.span)
        }) {
            if let Some(primitive) = call_primitive_name(
                file,
                call,
                self.lookup.entities(),
                self.symbol_names,
                self.lookup.dialect,
            )
            .filter(|primitive| creation_registers_work(self.lookup.dialect, file, call, primitive))
            {
                push_directive_creation(
                    self.creations,
                    self.seen,
                    primitive.to_string(),
                    file.path.as_str(),
                    call.callee,
                    true,
                    uncertain,
                );
            } else if let Some((target_file, target)) = self
                .lookup
                .function_called_at(file.path.as_str(), call.callee)
            {
                self.collect_function(target_file, target, uncertain);
            }
        }
        self.visiting.remove(&key);
    }
}

/// A name this dialect does not export is not one of its primitives, so it
/// cannot create anything a directive application would leak.
pub(super) fn is_created_primitive(dialect: &dyn Dialect, primitive: &PrimitiveName) -> bool {
    primitive
        .primitive()
        .is_some_and(|primitive| dialect.creates_directive_owner(primitive))
}

/// Whether this concrete call registers work an owner would have to dispose.
///
/// Solid 2.0's directive apply callback runs with *no owner* — `@solidjs/web`
/// rc.0's `ref()` is literally `runWithOwner(null, () => applyRef(...))` —
/// so the defect this stage feeds there (SC6001) is the unowned-leak class: a
/// computation created there warns `NO_OWNER_EFFECT` in dev and is never
/// disposed (probed on the rc.0 dev bundle). That leak needs a computation.
/// The same [`CleanupRule::WhenFirstArgumentIsFunction`] distinction the
/// leaf-owner rule (SC3002) applies holds here: `createSignal(0)` or a
/// value-form `createStore` allocates plain state that needs no owner and
/// misbehaves in no way, while `createSignal(fn)` registers a derived
/// computation that does. Dialects whose state constructors never register
/// work from a function argument keep their unconditional answer. A dialect
/// whose directive application preserves Owner (Solid 1.x) answers false from
/// `creates_directive_owner` before this function is reached.
pub(super) fn creation_registers_work(
    dialect: &dyn Dialect,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &PrimitiveName,
) -> bool {
    if !is_created_primitive(dialect, primitive) {
        return false;
    }
    let Some(primitive) = primitive.primitive() else {
        return false;
    };
    match dialect.cleanup_rule(primitive) {
        solid_dialect::CleanupRule::WhenFirstArgumentIsFunction => {
            call.arguments.first().is_some_and(|argument| {
                file.ast
                    .functions
                    .iter()
                    .any(|function| argument.span.contains(function.span))
            })
        }
        _ => true,
    }
}

pub(super) fn push_directive_creation(
    creations: &mut Vec<PrimitiveCreation>,
    seen: &mut HashSet<(String, u64, u64)>,
    primitive: String,
    path: &str,
    span: Span,
    returned_closure: bool,
    uncertain: bool,
) {
    let location = location(path, span);
    if seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
    )) {
        creations.push(PrimitiveCreation {
            primitive,
            location,
            returned_closure,
            uncertain,
        });
    }
}

/// Where code at `span` inside an intrinsic `ref` expression runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefPhase {
    /// Not in an intrinsic ref expression. A component's `ref` is a prop the
    /// component invokes itself, not an ownerless ref application.
    Outside,
    /// In a function that is itself the ref value, written directly in the
    /// expression (alone or as an array element) and inside no call there:
    /// the apply phase, with no owner.
    Apply,
    /// Eagerly, while the component renders under its owner: in no function
    /// (`ref={tip(createMemo(...))}`) or only in IIFE bodies.
    Render,
    /// In a function a call in the expression receives or returns
    /// (`ref={(() => () => createEffect(...))()}`): it may be the applied
    /// callback, or never run.
    Undecided,
}

fn ref_phase(file: &solid_facts::FileFacts, span: Span) -> RefPhase {
    let Some(callback) = file
        .compiler
        .callback_roles
        .iter()
        .filter(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && callback.span.contains(span)
                && crate::owners::ref_application_on_intrinsic(file, callback.span)
        })
        .min_by_key(|callback| callback.span.end - callback.span.start)
    else {
        return RefPhase::Outside;
    };
    // A default parameter initializer runs when its function is called, so
    // the function's whole span, not only its body, places `span`.
    let enclosing = file
        .ast
        .functions_within(callback.span)
        .filter(|function| function.span.contains(span) && function.span != span)
        .collect::<Vec<_>>();
    let Some(outermost) = enclosing
        .iter()
        .max_by_key(|function| function.span.end - function.span.start)
    else {
        return RefPhase::Render;
    };
    // Calling a generator runs none of its body: it may never run.
    if enclosing.iter().any(|function| function.generator) {
        return RefPhase::Undecided;
    }
    let in_expression = |call: &solid_facts::ast::CallFact| callback.span.contains(call.span);
    if !file
        .ast
        .calls
        .iter()
        .any(|call| in_expression(call) && call.span.contains(outermost.span))
    {
        return RefPhase::Apply;
    }
    let iife = |function: Span| {
        file.ast
            .calls
            .iter()
            .any(|call| in_expression(call) && file.ast.peel_ts_sugar_span(call.callee) == function)
    };
    if enclosing.iter().all(|function| iife(function.span)) {
        RefPhase::Render
    } else {
        RefPhase::Undecided
    }
}

/// What owns a creation at `span` written directly in the body of a callback
/// that a call inside the same ref application runs under an owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NestedOwner {
    /// No such callback: the apply phase's own (absent) owner.
    None,
    /// A fresh root the call creates (`createRoot`): owned, nothing leaks.
    CreatedRoot,
    /// An owner the call supplies (`runWithOwner(owner, …)`): it may be
    /// `null`, so the creation's ownership is undecided.
    Supplied,
}

/// The owner a creation at `span` runs under when it is written directly in
/// an owner callback inside the ref application:
/// `ref={el => createRoot(() => createMemo(...))}` creates the memo under the
/// root. Only the callback's own body counts; a function nested in it (a
/// timer) runs when something else calls it, with no owner of the call's.
fn nested_owner(
    ctx: &AnalysisContext<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
) -> NestedOwner {
    // By span, not body: a default parameter initializer runs in its
    // function's call, under that call's owner.
    let Some(mut innermost) = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span.contains(span) && function.span != span)
        .min_by_key(|function| function.span.end - function.span.start)
    else {
        return NestedOwner::None;
    };
    let Some(application) = file
        .compiler
        .callback_roles
        .iter()
        .filter(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
                && callback.span.contains(span)
        })
        .min_by_key(|callback| callback.span.end - callback.span.start)
    else {
        return NestedOwner::None;
    };
    // Synchronous nesting keeps the owner: an IIFE or an inline primitive
    // callback (`untrack`) runs at once in the function around it.
    loop {
        let function = innermost.span;
        let slot = file
            .ast
            .arguments_containing(function)
            .filter(|(call, index)| {
                application.span.contains(call.span)
                    && file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function
            })
            .find_map(|(call, index)| {
                let primitive =
                    call_primitive_name(file, call, ctx.entities, ctx.symbol_names, ctx.dialect)
                        .and_then(|primitive| primitive.primitive())?;
                Some((call, index, primitive))
            });
        if let Some((call, index, primitive)) = slot {
            match crate::owners::callback_owner_at_call(
                file,
                call,
                primitive,
                index,
                ctx.semantic_lookup,
            ) {
                Some(solid_dialect::CallbackOwner::Creates)
                    if ctx.dialect.callback_runs_in_created_root(primitive, index) =>
                {
                    return NestedOwner::CreatedRoot;
                }
                // `runWithOwner(owner, …)`: the owner may be `null`.
                Some(
                    solid_dialect::CallbackOwner::Creates
                    | solid_dialect::CallbackOwner::Conditional,
                ) => return NestedOwner::Supplied,
                // An inline callback (`untrack`) keeps the owner around it.
                None | Some(solid_dialect::CallbackOwner::Inherits)
                    if ctx.dialect.synchronous_callback_slot(
                        primitive,
                        index,
                        call.arguments.len(),
                    ) => {}
                _ => return NestedOwner::None,
            }
        } else if let Some((call, index)) =
            file.ast
                .arguments_containing(function)
                .find(|(call, index)| {
                    application.span.contains(call.span)
                        && file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function
                })
        {
            // A host scheduler runs it later on a fresh stack, with no owner
            // (`setTimeout`). Any other callee no dialect question describes
            // (`own(() => createEffect(...))`) may run it under an owner of
            // its own, or never.
            let fresh_stack =
                crate::owners::fresh_stack_scheduler_edges(file, call, ctx.semantic_lookup)
                    .iter()
                    .any(|edge| edge.argument == index);
            return if fresh_stack {
                NestedOwner::None
            } else {
                NestedOwner::Supplied
            };
        } else if !file.ast.calls.iter().any(|call| {
            application.span.contains(call.span)
                && file.ast.peel_ts_sugar_span(call.callee) == function
        }) {
            // The applied callback itself runs with no owner. A function
            // nested in it that nothing visibly invokes (an object property
            // `own({ run: () => … })`, a local helper, an array element) runs
            // whenever and under whatever owner its receiver chooses, or never.
            let applied = !file
                .ast
                .functions_within(application.span)
                .any(|outer| outer.span != function && outer.span.contains(function));
            return if applied {
                NestedOwner::None
            } else {
                NestedOwner::Supplied
            };
        }
        let Some(outer) = file
            .ast
            .functions
            .iter()
            .filter(|outer| outer.span != function && outer.span.contains(function))
            .min_by_key(|outer| outer.span.end - outer.span.start)
        else {
            return NestedOwner::None;
        };
        innermost = outer;
    }
}

/// The directive-creation stage: primitives created in directive-apply
/// positions, from direct calls and from functions returned into them.
pub(crate) fn discover_directive_creations(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let mut seen_directive_creations = HashSet::new();
    for file in &ctx.facts.files {
        for call in &file.ast.calls {
            let phase = ref_phase(file, call.callee);
            let owner = nested_owner(ctx, file, call.callee);
            if matches!(phase, RefPhase::Apply | RefPhase::Undecided)
                && owner != NestedOwner::CreatedRoot
                && let Some(primitive) =
                    call_primitive_name(file, call, ctx.entities, ctx.symbol_names, ctx.dialect)
                        .filter(|primitive| {
                            creation_registers_work(ctx.dialect, file, call, primitive)
                        })
            {
                push_directive_creation(
                    &mut draft.directive_creations,
                    &mut seen_directive_creations,
                    primitive.to_string(),
                    file.path.as_str(),
                    call.callee,
                    false,
                    owner == NestedOwner::Supplied || phase == RefPhase::Undecided,
                );
            }
        }
        for callback in &file.compiler.callback_roles {
            if callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply
                || !crate::owners::ref_application_on_intrinsic(file, callback.span)
            {
                continue;
            }
            for call in file
                .ast
                .calls
                .iter()
                .filter(|call| callback.span.contains(call.span))
            {
                if let Some((target_file, target)) = ctx
                    .semantic_lookup
                    .function_called_at(file.path.as_str(), call.callee)
                {
                    // Only a factory call that is the ref value itself
                    // (`ref={tooltip(opts)}`) or an element of a ref array
                    // (`ref={[tooltip(opts), other]}`) hands its result to
                    // the ref. Its
                    // result may be discarded or rewrapped anywhere else in the
                    // expression (`ref={ignore(make())}`, a comma, a
                    // conditional), and a call inside a function there
                    // (`ref={() => { make(); }}`) may drop it.
                    let value = file.ast.peel_ts_sugar_span(callback.span);
                    let applied = value == call.span
                        || file.ast.array_literal_elements.iter().any(|array| {
                            array.span == value
                                && array.elements.iter().any(|element| {
                                    element.kind == solid_facts::ast::ArrayElementKind::Value
                                        && file.ast.peel_ts_sugar_span(element.span) == call.span
                                })
                        });
                    DirectiveCreationCollector::new(
                        ctx.semantic_lookup,
                        ctx.symbol_names,
                        &mut draft.directive_creations,
                        &mut seen_directive_creations,
                        !applied,
                    )
                    .collect_returned(target_file, target);
                }
            }
        }
    }
}
