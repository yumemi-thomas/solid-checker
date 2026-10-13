//! Instantiate value Gets at a package call, never on a getter declaration.
//! The first slice admits fresh closed literals and one non-escaped const
//! binding. Spreads and aliases remain obligations. ADR 0274 adds a local
//! helper that forwards its exact parameter to the package call, and every
//! getter a hazard-free prefix reaches.

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
    let mut preceding_mutation_hazards = 0;
    let mut unresolved = false;
    for property in &shape.properties {
        let Some(getter_span) = property.getter else {
            // Obtaining a function/method value never executes that body.
            continue;
        };
        let occurrence_guaranteed = crate::contract_semantics::enumeration_get_is_guaranteed(
            context.callback.protocol,
            true,
            preceding_mutation_hazards,
        );
        let Some(getter) = context
            .file
            .ast
            .functions
            .iter()
            .find(|function| function.span == getter_span)
        else {
            unresolved = true;
            preceding_mutation_hazards += 1;
            continue;
        };
        // The receiver is a fresh unaliased literal, so a getter can reach it
        // only as its receiver: `this`, `super` (whose member calls receive
        // the literal), or code `eval` builds. Any such route refuses the later
        // getters; a false match only withholds.
        if context
            .file
            .source_text(getter.body)
            .is_none_or(mentions_receiver_route)
            || direct_eval_in(context.file, getter.span)
        {
            preceding_mutation_hazards += 1;
        }
        for call in context.file.ast.calls.iter().filter(|call| {
            crate::owners::written_directly_in(&context.file.ast, getter, call.span)
                && getter.body.contains(call.span)
        }) {
            // A class written in the getter defers its field initializers to
            // instantiation; nothing proves when, or that, they run.
            if inside_class_in(&context.file.ast, getter.body, call.span) {
                unresolved = true;
                continue;
            }
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
            // A read written directly in a reached getter body executes under
            // the statically-present-code premise; `entry_call` is no longer
            // the only guaranteed read.
            let guaranteed = occurrence_guaranteed;
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
                transported: false,
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

/// ADR 0274: the value-enumeration rows a local helper forwards, keyed by the
/// helper's own parameter.
///
/// The helper's body must call the package unconditionally and directly (no
/// nested function, no optional chain), passing that parameter itself as the
/// enumerated argument. Nothing may reference the parameter before that call:
/// no earlier Get, descriptor change, alias or escape can run first. The
/// parameter is a plain, unwritten, default-less identifier. A synchronous
/// helper that is not a generator then performs the package copy on its
/// caller's stack, before it returns.
fn helper_forwards(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ContractCallback> {
    if function.r#async || function.generator || !lookup.function_value_is_current(file, function) {
        return Vec::new();
    }
    let mut forwarded = Vec::new();
    for inner in file
        .ast
        .calls
        .iter()
        .filter(|inner| unconditional_in(&file.ast, function, inner))
    {
        let Some(symbol) = lookup.callee_symbol(file, inner.callee) else {
            continue;
        };
        for callback in lookup.value_enumeration_callbacks(symbol) {
            if let Some(index) = forwarded_parameter(file, function, inner, callback.parameter) {
                let mut callback = callback.clone();
                callback.parameter = index;
                forwarded.push(callback);
            }
        }
    }
    forwarded
}

/// Whether `inner` is written directly in `function`'s straight-line flow.
fn unconditional_in(
    ast: &solid_facts::ast::AstFacts,
    function: &solid_facts::ast::FunctionFact,
    inner: &solid_facts::ast::CallFact,
) -> bool {
    function.body.contains(inner.span)
        && crate::owners::written_directly_in(ast, function, inner.span)
        && !inside_class_in(ast, function.body, inner.span)
        && if inner.construct {
            ast.straight_line_constructs.contains(&inner.span)
        } else {
            ast.straight_line_calls.contains(&inner.span)
        }
}

/// The index of `function`'s parameter that `inner` receives, untouched, at
/// `argument`: a plain, unwritten, default-less identifier parameter that
/// nothing references before that call.
fn forwarded_parameter(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    inner: &solid_facts::ast::CallFact,
    argument: usize,
) -> Option<usize> {
    let ast = &file.ast;
    if inner.arguments.iter().take(argument + 1).any(|a| a.spread) {
        return None;
    }
    let passed = ast.peel_ts_sugar_span(inner.arguments.get(argument)?.span);
    let declaration = ast.reference_declaration(passed)?;
    let index = function.parameters.iter().position(|parameter| {
        parameter.shape == BindingShape::Identifier
            && parameter.names.len() == 1
            && parameter.names[0].span == declaration
            && parameter.initializer.is_none()
    })?;
    // A reference written after the call runs after it, unless it sits in a
    // hoisted function declaration, which may be called before the call
    // however late it is written. A function expression or class written
    // later does not exist yet.
    let touched_before = ast.reference_declarations.iter().any(|(reference, bound)| {
        *bound == declaration
            && *reference != passed
            && (reference.start < inner.span.end
                || ast.functions.iter().any(|nested| {
                    nested.kind == solid_facts::ast::FunctionKind::Declaration
                        && function.body.contains(nested.span)
                        && nested.span.contains(*reference)
                }))
    });
    // `arguments` aliases every parameter without a reference bound to it. An
    // identifier that decodes to it is spelled so or carries an escape, and
    // direct `eval` reaches the parameter with no static reference at all.
    let aliased = ast.identifiers.iter().any(|identifier| {
        function.span.contains(identifier.span)
            && file
                .source_text(identifier.span)
                .is_none_or(|text| text == "arguments" || text.contains('\\'))
    }) || direct_eval_in(file, Span::new(0, u32::MAX));
    (!touched_before && !aliased && !crate::indexes::binding_written(file, declaration))
        .then_some(index)
}

/// Whether `within` holds a direct `eval`, which reaches local bindings and
/// `this` with no static reference. Transparent TypeScript wrappers erase at
/// run time, so `(eval as typeof eval)(...)` and `eval!(...)` are direct too;
/// an escaped spelling is refused by the callers' backslash checks.
fn direct_eval_in(file: &solid_facts::FileFacts, within: Span) -> bool {
    file.ast.module_hazards.iter().any(|hazard| {
        hazard.kind == solid_facts::ast::ModuleHazardKind::Eval && within.contains(hazard.span)
    }) || file.ast.calls.iter().any(|call| {
        within.contains(call.span)
            && file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("eval")
    })
}

/// Whether `span` lies in a class written inside `within`: a field
/// initializer runs at instantiation, not where the class is defined.
fn inside_class_in(ast: &solid_facts::ast::AstFacts, within: Span, span: Span) -> bool {
    ast.classes
        .iter()
        .any(|class| within.contains(class.span) && class.span.contains(span))
}

/// The value-enumeration rows a call of a local helper performs (ADR 0274).
/// Empty unless the callee resolves to exactly one current project function.
pub(super) fn forwarded_enumerations(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ContractCallback> {
    if call.construct {
        return Vec::new();
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let Some((helper_file, helper)) = lookup.function_called_at(file.path.as_str(), callee) else {
        return Vec::new();
    };
    helper_forwards(helper_file, helper, lookup)
        .into_iter()
        .filter(|callback| {
            !call
                .arguments
                .iter()
                .take(callback.parameter + 1)
                .any(|argument| argument.spread)
        })
        .collect()
}

/// Whether `file` holds a helper that forwards an enumeration. A caller's
/// result depends on that helper's syntax beyond its read summary.
pub(super) fn forwards_enumeration(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .functions
        .iter()
        .any(|function| !helper_forwards(file, function, lookup).is_empty())
}

/// Whether `text` contains a `this` or `super` keyword as a token, or an
/// escaped identifier that may spell one.
fn mentions_receiver_route(text: &str) -> bool {
    let bytes = text.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    text.contains('\\')
        || ["this", "super"].iter().any(|word| {
            text.match_indices(word).any(|(at, _)| {
                (at == 0 || !ident(bytes[at - 1]))
                    && bytes.get(at + word.len()).is_none_or(|b| !ident(*b))
            })
        })
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

    fn file(source: &str) -> solid_facts::FileFacts {
        use solid_facts::core::{Generation, SourceHash, SourcePath};
        let ast = solid_facts::ast::extract("helper.ts", source).unwrap();
        solid_facts::FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("helper.ts").unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(ast),
            compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    #[test]
    fn a_helper_forwards_only_an_untouched_parameter_on_its_straight_line() {
        for (source, expected) in [
            ("function h(o) { const v = new V(o); return v; }", Some(0)),
            ("function h(a, o) { return new V(o); }", Some(1)),
            (
                "function h(o) { setup(); const v = new V(o as T); return v; }",
                Some(0),
            ),
            (
                "function h(o) { const n = o.count; return new V(o); }",
                None,
            ),
            ("function h(o) { keep(o); return new V(o); }", None),
            ("function h(o) { o = other; return new V(o); }", None),
            ("function h(o = {}) { return new V(o); }", None),
            ("function h({ o }) { return new V(o); }", None),
            ("function h(o) { if (ready) { return new V(o); } }", None),
            ("function h(o) { return ready ? new V(o) : null; }", None),
            ("function h(o) { return untrack(() => new V(o)); }", None),
            ("function h(o) { return new V(...rest, o); }", None),
            (
                "function h(o) { const copy = { ...o }; return new V(copy); }",
                None,
            ),
            (
                "function h(o) { touch(arguments[0]); return new V(o); }",
                None,
            ),
            (
                "function h(o) { touch(argum\\u0065nts[0]); return new V(o); }",
                None,
            ),
            ("function h(o) { eval('o.x = 1'); return new V(o); }", None),
            (
                "function h(o) { (eval as typeof eval)('o.x = 1'); return new V(o); }",
                None,
            ),
            ("function h(o) { eval!('o.x = 1'); return new V(o); }", None),
            ("function h(o, p = arguments[0]) { return new V(o); }", None),
            (
                "function h(o) { mutate(); return new V(o); function mutate() { o.x = 1; } }",
                None,
            ),
            (
                "function h(o) { const v = new V(o); later(() => o); return v; }",
                Some(0),
            ),
            (
                "function h(o) { const v = new V(o); keep(o); return v; }",
                Some(0),
            ),
            (
                "function h(o) { class Later { value = new V(o); } return Later; }",
                None,
            ),
        ] {
            let facts = file(source);
            let function = facts
                .ast
                .functions
                .iter()
                .find(|f| f.name.is_some())
                .unwrap();
            let found = facts
                .ast
                .calls
                .iter()
                .filter(|call| call.construct)
                .find(|call| unconditional_in(&facts.ast, function, call))
                .and_then(|call| {
                    let last = call.arguments.len().saturating_sub(1);
                    forwarded_parameter(&facts, function, call, last)
                });
            assert_eq!(found, expected, "{source}");
        }
    }

    #[test]
    fn receiver_routes_are_matched_as_tokens() {
        assert!(mentions_receiver_route("{ delete this.count; }"));
        assert!(mentions_receiver_route("this"));
        assert!(mentions_receiver_route("{ return super.valueOf(); }"));
        assert!(mentions_receiver_route("{ return th\\u0069s.x; }"));
        assert!(!mentions_receiver_route("{ return thisValue(); }"));
        assert!(!mentions_receiver_route("{ return history() + superb(); }"));
        assert!(!mentions_receiver_route("{ return $this; }"));
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
