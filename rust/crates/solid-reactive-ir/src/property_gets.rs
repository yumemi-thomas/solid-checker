//! Instantiate value Gets at a package call, never on a getter declaration.
//! The first slice admits fresh closed literals and one non-escaped const
//! binding. Spreads, aliases and later accessor prefixes remain obligations.

use super::*;
use solid_facts::ast::{BindingShape, ObjectGetShapeFact};

pub(super) struct ReadContext<'a, 'b> {
    pub file: &'a solid_facts::FileFacts,
    pub call: &'a solid_facts::ast::CallFact,
    pub callback: &'a ContractCallback,
    pub valid: bool,
    pub execution: ExecutionRole,
    pub label: &'a str,
    pub source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub entities: &'a EntitySymbols,
    pub symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub lookup: &'a SemanticLookup<'b>,
}

/// The binder names the declaration, not a same-spelled local. The object
/// must never escape to an alias, member dispatch or descriptor mutation.
/// A spread *of* it cannot change its own descriptors and is allowed; a
/// spread *inside* its literal is not resolved in this initial slice.
fn receiver<'a>(
    ast: &'a solid_facts::ast::AstFacts,
    call: &solid_facts::ast::CallFact,
    parameter: usize,
) -> Option<&'a ObjectGetShapeFact> {
    let argument = call.arguments.get(parameter)?;
    if argument.spread {
        return None;
    }
    let mut span = ast.peel_ts_sugar_span(argument.span);
    if let Some(declaration) = ast.reference_declaration(span) {
        let mut candidates = ast.bindings.iter().filter(|binding| {
            binding.shape == BindingShape::Identifier
                && binding.immutable
                && binding.names.len() == 1
                && binding.names[0].span == declaration
        });
        let binding = candidates.next()?;
        if candidates.next().is_some()
            || binding.declaration.end > call.span.start
            || containing_ast_function(ast, binding.declaration).map(|owner| owner.span)
                != containing_ast_function(ast, call.span).map(|owner| owner.span)
            || ast
                .loop_statements
                .iter()
                .any(|looped| looped.contains(call.span))
            // Only a module-level binding can be exported. An exported
            // function's span contains its whole body, so a local `const`
            // inside `export function App() {}` is not itself exported.
            || (containing_ast_function(ast, binding.declaration).is_none()
                && ast
                    .exports
                    .iter()
                    .any(|export| export.span.contains(binding.declaration)))
            || ast
                .exports
                .iter()
                .flat_map(|export| &export.specifiers)
                .any(|specifier| {
                    ast.reference_declaration(specifier.local.span) == Some(declaration)
                })
        {
            return None;
        }
        for (reference, bound) in &ast.reference_declarations {
            if *bound != declaration {
                continue;
            }
            let allowed = ast.peel_ts_sugar_span(argument.span) == *reference
                || ast.spreads.iter().any(|spread| {
                    if ast.peel_ts_sugar_span(spread.argument) != *reference
                        || spread.span.start < call.span.end
                    {
                        return false;
                    }
                    let call_owner =
                        containing_ast_function(ast, call.span).map(|owner| owner.span);
                    let spread_owner =
                        containing_ast_function(ast, spread.span).map(|owner| owner.span);
                    // A same-frame later read cannot mutate this allocation
                    // before its sole construction. A future function literal
                    // handed to a later same-frame call likewise cannot run
                    // yet. A hoisted/named callback is not such a proof.
                    spread_owner == call_owner
                        || ast.arguments_containing(spread.span).any(|(outer, slot)| {
                            let argument = &outer.arguments[slot];
                            outer.span.start > call.span.end
                                && !argument.spread
                                && Some(ast.peel_ts_sugar_span(argument.span)) == spread_owner
                                && containing_ast_function(ast, outer.span).map(|owner| owner.span)
                                    == call_owner
                        })
                });
            if !allowed {
                return None;
            }
        }
        span = ast.peel_ts_sugar_span(binding.initializer?);
    }
    ast.object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)
}

fn obligation(context: &ReadContext<'_, '_>) -> Option<StaticDefect> {
    let symbol = context
        .lookup
        .callee_symbol(context.file, context.call.callee)?;
    let (package, export) = context.lookup.contract_export_identity(symbol)?;
    Some(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: package.to_owned(),
            export: export.to_owned(),
            reexported: false,
            site: crate::ContractDefectSite::Argument,
            admission_refusal: None,
        },
        location: location(context.file.path.shared(), context.call.span),
        analysis_context: "unbound-contract-claims:property Get receiver or getter body".into(),
        fixes: vec![],
        uncertain: true,
    })
}

pub(super) fn reads(
    context: ReadContext<'_, '_>,
) -> (
    Vec<ReactiveRead>,
    Option<StaticDefect>,
    HashSet<InterproceduralResultDependency>,
) {
    let mut reads = Vec::new();
    let mut dependencies = HashSet::new();
    if !context.valid {
        // Invalid calls belong to TypeScript. They cannot supply an execution
        // proof for this checker, nor a duplicate package-call complaint.
        return (reads, None, dependencies);
    }
    if !context.callback.path.is_empty() {
        return (reads, obligation(&context), dependencies);
    }
    let Some(shape) = receiver(&context.file.ast, context.call, context.callback.parameter) else {
        return (reads, obligation(&context), dependencies);
    };
    let mut preceding_getters = 0;
    let mut unresolved = false;
    for property in &shape.properties {
        let Some(getter_span) = property.getter else {
            // Obtaining a function/method value never executes that body.
            continue;
        };
        let occurrence_guaranteed = crate::contract_semantics::enumeration_get_is_guaranteed(
            context.callback.protocol,
            true,
            preceding_getters,
        );
        preceding_getters += 1;
        let Some(getter) = context
            .file
            .ast
            .functions
            .iter()
            .find(|function| function.span == getter_span)
        else {
            unresolved = true;
            continue;
        };
        for call in context.file.ast.calls.iter().filter(|call| {
            crate::owners::written_directly_in(&context.file.ast, getter, call.span)
                && getter.body.contains(call.span)
        }) {
            let Some(symbol) = context
                .entities
                .get(&location(context.file.path.shared(), call.callee))
            else {
                unresolved = true;
                continue;
            };
            if context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Accessor)
                || !source_has_runtime_witness(
                    symbol,
                    context.accessors,
                    context.entities,
                    context.symbol_names,
                    context.lookup,
                )
            {
                // Local helper calls and arbitrary package calls need their
                // own entered-body proof; do not call them inert.
                unresolved = true;
                continue;
            }
            let Some((display, declaration)) = context.accessors.get(symbol) else {
                unresolved = true;
                continue;
            };
            dependencies.insert(InterproceduralResultDependency::Symbol(symbol.clone()));
            let guaranteed = occurrence_guaranteed && property.entry_call == Some(call.span);
            reads.push(ReactiveRead {
                kind: "accessor".into(),
                accessor: display.to_string().into(),
                location: location(context.file.path.shared(), context.call.span),
                declaration: declaration.clone(),
                execution: context.execution,
                context: enclosing_function_label(context.file, context.call.span).into(),
                via: context.label.into(),
                origin: Some(location(context.file.path.shared(), call.span)),
                origin_context: format!("get {}", property.key).into(),
                uncertain: false,
                missing_jsx_census: missing_jsx_census(
                    context.file,
                    context.call.span,
                    context.execution,
                ),
                host_callback_timing: host_callback_timing(
                    context.file,
                    context.call.span,
                    context.execution,
                    context.lookup,
                ),
                callee_callback_timing: callee_callback_timing(
                    context.file,
                    context.call.span,
                    context.execution,
                    context.lookup,
                ),
                project_consumer_non_strict: false,
                callback_invocation_unproven: !guaranteed,
                package_internal: false,
                summary_attributed: !guaranteed,
            });
        }
        // Member reads can themselves enter getters or a reactive proxy.
        // This first slice proves accessor Calls only; a member-only getter
        // never becomes a clean result just because that call loop is empty.
        unresolved |= context.file.ast.members.iter().any(|member| {
            crate::owners::written_directly_in(&context.file.ast, getter, member.span)
                && getter.body.contains(member.span)
        });
    }
    (
        reads,
        unresolved.then(|| obligation(&context)).flatten(),
        dependencies,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_receivers_use_binder_identity_and_refuse_escapes() {
        for (source, expected) in [
            (
                "const opts = { get count() { return n(); } }; new V(opts);",
                true,
            ),
            (
                "const opts = { get count() { return n(); } }; new V((opts as Options)!);",
                true,
            ),
            (
                "const opts = { get count() { return n(); } }; const alias = opts; new V(opts);",
                false,
            ),
            (
                "let opts = { get count() { return n(); } }; new V(opts);",
                false,
            ),
            ("const opts = new Proxy({}, {}); new V(opts);", false),
            (
                "const opts = { ...other, get count() { return n(); } }; new V(opts);",
                false,
            ),
            (
                "const opts = { get count() { return n(); } }; Object.defineProperty(opts, 'count', {}); new V(opts);",
                false,
            ),
            (
                "const opts = { get count() { return n(); } }; const copy = { ...opts }; new V(opts);",
                false,
            ),
            (
                "const opts = { get count() { return n(); } }; new V(opts); effect(() => ({ ...opts }));",
                true,
            ),
            (
                "const opts = { get count() { return n(); } }; before(); new V(opts); function before() { return { ...opts }; }",
                false,
            ),
            (
                "const opts = { get count() { return n(); } }; while (again) { new V(opts); }",
                false,
            ),
            (
                "const opts = { get count() { return n(); } }; { const opts = other; new V(opts); }",
                false,
            ),
            // A local inside an exported function is not exported; the
            // exported module-level binding is.
            (
                "export function App() { const opts = { get count() { return n(); } }; new V(opts); }",
                true,
            ),
            (
                "export const opts = { get count() { return n(); } }; new V(opts);",
                false,
            ),
        ] {
            let ast = solid_facts::ast::extract("/project/get.ts", source).unwrap();
            let call = ast
                .calls
                .iter()
                .find(|call| call.construct && call.arguments.len() == 1)
                .unwrap();
            assert_eq!(receiver(&ast, call, 0).is_some(), expected, "{source}");
        }
    }

    #[test]
    fn copying_data_function_values_does_not_select_their_bodies() {
        let ast = solid_facts::ast::extract(
            "/project/get.ts",
            "new V({ fn: () => n(), get count() { return n(); } });",
        )
        .unwrap();
        let call = ast.calls.iter().find(|call| call.construct).unwrap();
        let shape = receiver(&ast, call, 0).unwrap();
        assert!(shape.properties[0].getter.is_none());
        assert!(shape.properties[1].getter.is_some());
    }
}
