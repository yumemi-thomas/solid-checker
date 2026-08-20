//! Interprocedural discovery of reactive primitives created during directive
//! application.

use std::collections::{HashMap, HashSet};

use solid_dialect::Dialect;
use solid_facts::core::Span;

use super::{
    ExecutionRole, PrimitiveCreation, PrimitiveName, SemanticLookup, SymbolId, execution_role,
    location, primitive_name,
};
use crate::owners::containing_ast_function;
use crate::pipeline::{AnalysisContext, ProgramDraft};

pub(super) struct DirectiveCreationCollector<'a, 'c> {
    lookup: &'c SemanticLookup<'a>,
    symbol_names: &'c HashMap<SymbolId, SymbolId>,
    visiting: HashSet<(String, Span)>,
    creations: &'c mut Vec<PrimitiveCreation>,
    seen: &'c mut HashSet<(String, u64, u64)>,
}

impl<'a, 'c> DirectiveCreationCollector<'a, 'c> {
    pub(super) fn new(
        lookup: &'c SemanticLookup<'a>,
        symbol_names: &'c HashMap<SymbolId, SymbolId>,
        creations: &'c mut Vec<PrimitiveCreation>,
        seen: &'c mut HashSet<(String, u64, u64)>,
    ) -> Self {
        Self {
            lookup,
            symbol_names,
            visiting: HashSet::new(),
            creations,
            seen,
        }
    }

    pub(super) fn collect_returned(
        &mut self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) {
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
                        self.collect_function(file, returned_function);
                    }
                }
                solid_facts::ast::ReturnValueKind::Call => {
                    if let Some(callee) = returned.callee
                        && let Some((target_file, target)) =
                            self.lookup.function_called_at(file.path.as_str(), callee)
                    {
                        self.collect_returned(target_file, target);
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
    ) {
        let key = (file.path.to_string(), function.span);
        if !self.visiting.insert(key.clone()) {
            return;
        }
        for call in file.ast.calls.iter().filter(|call| {
            containing_ast_function(&file.ast, call.span)
                .is_some_and(|owner| owner.span == function.span)
        }) {
            if let Some(primitive) = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
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
                );
            } else if let Some((target_file, target)) = self
                .lookup
                .function_called_at(file.path.as_str(), call.callee)
            {
                self.collect_function(target_file, target);
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
        });
    }
}

/// The directive-creation stage: primitives created in directive-apply
/// positions, from direct calls and from functions returned into them.
pub(crate) fn discover_directive_creations(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let mut seen_directive_creations = HashSet::new();
    for file in &ctx.facts.files {
        for call in &file.ast.calls {
            let role = execution_role(&file.compiler, call.callee, &[]);
            if role == ExecutionRole::DirectiveApply
                && let Some(primitive) = primitive_name(
                    file.path.as_str(),
                    call.callee,
                    call.static_callee(&file.source),
                    ctx.entities,
                    ctx.symbol_names,
                    ctx.dialect,
                )
                .filter(|primitive| creation_registers_work(ctx.dialect, file, call, primitive))
            {
                push_directive_creation(
                    &mut draft.directive_creations,
                    &mut seen_directive_creations,
                    primitive.to_string(),
                    file.path.as_str(),
                    call.callee,
                    false,
                );
            }
        }
        for callback in &file.compiler.callback_roles {
            if callback.role != solid_facts::compiler::CallbackRoleKind::DirectiveApply {
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
                    DirectiveCreationCollector::new(
                        ctx.semantic_lookup,
                        ctx.symbol_names,
                        &mut draft.directive_creations,
                        &mut seen_directive_creations,
                    )
                    .collect_returned(target_file, target);
                }
            }
        }
    }
}
