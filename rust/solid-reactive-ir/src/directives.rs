//! Interprocedural discovery of reactive primitives created during directive
//! application.

use super::*;

pub(super) struct DirectiveCreationCollector<'a, 'c> {
    lookup: &'c SemanticLookup<'a>,
    symbol_names: &'c HashMap<String, String>,
    visiting: HashSet<(String, Span)>,
    creations: &'c mut Vec<PrimitiveCreation>,
    seen: &'c mut HashSet<(String, u64, u64)>,
}

impl<'a, 'c> DirectiveCreationCollector<'a, 'c> {
    pub(super) fn new(
        lookup: &'c SemanticLookup<'a>,
        symbol_names: &'c HashMap<String, String>,
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
        function: &solid_ast_facts::FunctionFact,
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
                solid_ast_facts::ReturnValueKind::Function => {
                    if let Some(returned_function) = file
                        .ast
                        .functions
                        .iter()
                        .find(|candidate| candidate.span == returned.span)
                    {
                        self.collect_function(file, returned_function);
                    }
                }
                solid_ast_facts::ReturnValueKind::Call => {
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
        function: &solid_ast_facts::FunctionFact,
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
            )
            .filter(|primitive| is_created_primitive(primitive))
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

pub(super) fn is_created_primitive(primitive: &str) -> bool {
    matches!(
        primitive,
        "createSignal"
            | "createMemo"
            | "createStore"
            | "createProjection"
            | "createOptimistic"
            | "createOptimisticStore"
            | "createEffect"
            | "createRenderEffect"
            | "createTrackedEffect"
            | "createReaction"
            | "createRoot"
            | "createOwner"
    )
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
