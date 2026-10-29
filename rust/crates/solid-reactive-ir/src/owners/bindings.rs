//! Binding and read-escape helpers: lazy default renders, binding names, returned arrows, and read escapes.

use super::*;

/// Every condition of a lazy default render witness
/// ([`crate::indexes::LazyWitnessesIn`]) but the component proof and the
/// reflective-write veto: `(component, default property, its value)`.
/// Component identity itself asks this, so the `default` value is a reference
/// the loader's render accounts for rather than an escape. It reads the call's
/// own file, and `primitive_at` (the primitive resolved at a call span of it).
pub(crate) fn lazy_default_render_candidate<'f>(
    file: &'f solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive_at: &dyn Fn(Span) -> Option<solid_dialect::Primitive>,
    dialect: &dyn solid_dialect::Dialect,
) -> Option<(&'f solid_facts::ast::FunctionFact, Span, Span)> {
    use crate::interproc::accessor_provenance::{closed_properties, single_return};
    let primitive = primitive_at(call.span)?;
    let slot = dialect.renders_loaded_default_argument(primitive, call.arguments.len())?;
    if call.arguments.iter().any(|argument| argument.spread) {
        return None;
    }
    let loader_span = file.ast.peel_ts_sugar_span(call.arguments.get(slot)?.span);
    let loader = file.ast.function_at(loader_span)?;
    if !loader.r#async || loader.generator || !lazy_wrapper_only_rendered(file, call) {
        return None;
    }
    let module = file.ast.peel_ts_sugar_span(single_return(file, loader)?);
    let properties = closed_properties(file, module)?;
    // Review round 1: an own `then` makes the module object a thenable. The
    // async loader's return resolves through it, so the promise `lazy` reads
    // `default` from fulfills with whatever `then` delivers, or never.
    // `closed_properties` already refuses getters and computed keys; this
    // refuses a stored function too, including receiver-inert methods.
    if properties.iter().any(|property| {
        file.source_text(property.key)
            .is_some_and(|key| key.trim_matches(['\'', '"']) == "then")
    }) {
        return None;
    }
    let property = properties
        .into_iter()
        .find(|property| file.source_text(property.key) == Some("default"))?;
    if property.shorthand_binding.is_some() {
        return None;
    }
    let value = file.ast.peel_ts_sugar_span(property.value);
    let declaration = file.ast.reference_declaration(value)?;
    // The value's one reference: no registry, call, tag or alias elsewhere.
    if file
        .ast
        .reference_declarations
        .iter()
        .any(|(reference, target)| *target == declaration && *reference != value)
        || crate::indexes::binding_written(file, declaration)
    {
        return None;
    }
    let function = file.ast.functions.iter().find(|function| {
        let named = match function.kind {
            solid_facts::ast::FunctionKind::Declaration => function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration),
            _ => file.ast.bindings.iter().any(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
                    && binding.names[0].span == declaration
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast.peel_ts_sugar_span(initializer) == function.span
                    })
            }),
        };
        // Declared in the loader's own body, inside no other function there.
        named
            && loader.body.contains(function.span)
            && !file.ast.functions_within(loader.body).any(|outer| {
                outer.span != function.span
                    && outer.span != loader.span
                    && outer.span.contains(function.span)
            })
    })?;
    (!function.r#async
        && !function.generator
        && crate::indexes::function_value_is_current_in(file, function))
    .then_some((function, property.span, value))
}

/// Whether the loader call initializes an unexported, unwritten `const` whose
/// every reference is a JSX tag name, with at least one opening tag.
pub(super) fn lazy_wrapper_only_rendered(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == call.span)
    }) else {
        return false;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || binding.names.len() != 1
        || file
            .ast
            .exports
            .iter()
            .any(|export| export.span.contains(binding.declaration))
    {
        return false;
    }
    let name = binding.names[0].span;
    if crate::indexes::binding_written(file, name) {
        return false;
    }
    let mut opened = false;
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, target)| *target == name)
    {
        let mut tag = false;
        for element in &file.ast.jsx_elements {
            if element.member_object.is_some() {
                continue;
            }
            if element.name.span == *reference {
                opened = true;
                tag = true;
            } else if element.closing_name == Some(*reference) {
                tag = true;
            }
        }
        if !tag {
            return false;
        }
    }
    opened
}

/// The name that binds a function declaration, arrow bindings included.
///
/// `function.name` covers `function X() {}` and `method_name` covers a class
/// or object method; neither exists for `const X = () => {}`, whose only name
/// is the binding that initializes it. Exported to the backend because
/// an export-attribution consumer must answer "which export is this
/// function" for exactly the same set of declarations the IR does — reading
/// only `name`/`method_name` there made every arrow-bound export unnameable,
/// and an unnameable export silently became "not an export".
pub fn function_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    function
        .name
        .as_ref()
        .or(own_method_name(file, function))
        .or_else(|| {
            file.ast
                .bindings_initializer_containing(function.span)
                .find(|binding| {
                    binding.initializer_function
                        && binding.initializer.is_some_and(|initializer| {
                            file.ast
                                .functions_within(initializer)
                                .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                                .is_some_and(|candidate| candidate.span == function.span)
                        })
                })
                .and_then(|binding| binding.names.first())
        })
}

/// The method name of a function that *is* an object or class method. The
/// facts walker gives every non-arrow function nested anywhere in a method's
/// body that method's name too (`{ m() { register(function () {}) } }`); the
/// method itself is the outermost function carrying that name.
pub(crate) fn own_method_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    let name = function.method_name.as_ref()?;
    (!file.ast.functions.iter().any(|outer| {
        outer.span != function.span
            && outer.span.contains(function.span)
            && outer
                .method_name
                .as_ref()
                .is_some_and(|outer_name| outer_name.span == name.span)
    }))
    .then_some(name)
}

/// Like [`function_binding_name`], but also sees through a call initializer:
/// `const Wrapped = withTheme(({ name }) => ...)` names the arrow `Wrapped`,
/// so a wrapped component is classified like an unwrapped one. Object and
/// array initializers stay excluded — they merely *contain* functions, and
/// naming those would mint components out of data.
pub(crate) fn component_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
    function_binding_name(file, function).or_else(|| {
        file.ast
            .bindings_initializer_containing(function.span)
            .find(|binding| {
                binding.call_initializer.is_some()
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast
                            .functions_within(initializer)
                            .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                            .is_some_and(|candidate| candidate.span == function.span)
                    })
            })
            .and_then(|binding| binding.names.first())
    })
}

pub(crate) fn binding_returns_reactive_source(
    binding: &solid_facts::ast::BindingFact,
    call: &solid_facts::ast::CallFact,
) -> bool {
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Array
        && binding.array_slots.first().is_some_and(Option::is_some)
        && binding.call_initializer == Some(call.span)
}

/// Whether a reactive read at `span` happens outside the synchronous extent
/// of the function that lexically contains it — so calling that function does
/// not perform the read, and the read must not enter its caller-visible
/// summary.
///
/// The dialect's callback vocabulary answers this exactly, and the three
/// executions divide cleanly. [`solid_dialect::Execution::Inline`] reads
/// "subscribe whatever was tracking at the call site", so they *are* the
/// caller's read and stay. [`solid_dialect::Execution::Tracked`] reads
/// subscribe the callback's own observer, and
/// [`solid_dialect::Execution::Deferred`] reads subscribe nothing the caller
/// owns; either way the caller performs no read, and attributing one to it
/// invents an untracked-read violation in a function whose only read is
/// inside a tracked or deferred callback.
///
/// One `Deferred` slot is the exception, and the owner word says which:
/// a [`solid_dialect::CallbackOwner::InheritsFirstRun`] callback (2.0
/// `createRenderEffect`'s apply) runs its first time during the call with the
/// caller's listener still current, so that run's reads are the caller's.
///
/// The read must sit inside a function *literal* in that argument. An
/// eagerly evaluated argument — `createEffect(count())` — is read while the
/// argument list is built, which is the caller's read after all.
pub(crate) fn read_escapes_synchronous_extent(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        if !file
            .ast
            .functions_within(argument.span)
            .any(|function| function.span.contains(span))
        {
            return false;
        }
        crate::call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                let semantics =
                    dialect.callback_semantics_at(primitive, index, call.arguments.len());
                // A callback the primitive first runs during the call, under the
                // caller's owner and listener, performs its first run's reads
                // inside the caller's synchronous extent -- 2.0
                // `createRenderEffect`'s apply, whose attribution word is
                // `Deferred` for its later runs only. Probed: a memo whose compute
                // creates the render effect re-runs when a signal read only in
                // that apply changes. Dropping the read from the caller's summary
                // would close a `reads` domain over a read the call performs.
                if semantics.owner == Some(solid_dialect::CallbackOwner::InheritsFirstRun) {
                    // The first apply is absent, or unproven, if options can
                    // defer it. Later runs do not read for the creating caller.
                    // ADR 0502: so does `schedule`, which queues the first run.
                    let withheld = |options: Option<usize>, key: &str| {
                        options.is_some_and(|options| {
                            matches!(
                                crate::call_option_literal(file, call, options, key),
                                solid_dialect::OptionLiteral::True
                                    | solid_dialect::OptionLiteral::Unknown
                            )
                        })
                    };
                    return withheld(
                        dialect.effect_apply_defer_options(primitive, index),
                        "defer",
                    ) || withheld(
                        dialect.effect_apply_schedule_options(primitive, index),
                        "schedule",
                    );
                }
                matches!(
                    semantics.execution,
                    Some(solid_dialect::Execution::Tracked | solid_dialect::Execution::Deferred)
                )
            })
    })
}

/// An exact global scheduler's queued callback cannot read during the
/// enclosing function's invocation. Keep the callback implementation's own
/// summary: the scheduler must be inside `owner_body`, rather than outside
/// the function being read.
pub(crate) fn read_escapes_runtime_stack(
    file: &solid_facts::FileFacts,
    span: Span,
    owner_body: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    crate::execution_role::fresh_stack_callback_call(file, span, lookup).is_some_and(
        |(call, resolved)| {
            owner_body.contains(call.span)
                && crate::runtime_semantics::augmented_global_runtime_call(resolved)
                && !crate::runtime_semantics::dispatches_through_receiver_then(resolved)
        },
    )
}

/// The typed descriptor at a callee, kept only when it names a callable Solid
/// accessor. Tuple signals and object stores are source values but not direct
/// accessor call targets.
pub(crate) fn typed_accessor_descriptor_at<'a>(
    lookup: &SemanticLookup<'a>,
    path: &str,
    callee: Span,
) -> Option<&'a typefacts::TypeDescriptor> {
    lookup
        .smallest_contained_descriptor(path, callee)
        .filter(|descriptor| solid_accessor_declaration(descriptor, lookup.dialect).is_some())
}

/// The exact Solid accessor alias proven by Type Facts. Module plus exported
/// alias identity is required; type text and user-local names are ignored.
pub(crate) fn solid_accessor_declaration<'a>(
    descriptor: &'a typefacts::TypeDescriptor,
    dialect: &dyn Dialect,
) -> Option<&'a typefacts::Declaration> {
    descriptor.alias_declarations.iter().find(|declaration| {
        dialect.type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
            == Some(solid_dialect::TypeRole::Accessor)
    })
}

pub(crate) fn source_function_exported(
    indexes: &ProjectIndexes<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    indexes
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.functions.iter().any(|candidate| {
                candidate.exported
                    && candidate.body.start_byte == u64::from(function.body.start)
                    && candidate.body.end_byte == u64::from(function.body.end)
            })
        })
        || file.ast.exports_containing(function.span).any(|export| {
            !file.ast.functions_within(export.span).any(|candidate| {
                candidate.span != function.span && candidate.span.contains(function.span)
            })
        })
}

pub(crate) fn enclosing_render_function(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup.inside_component(file, span) || lookup.inside_possible_component(file, span)
}

pub(crate) fn function_is_solid_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let primitives = lookup.primitives(file);
    if file.ast.jsx_containing(function.span).any(|element| {
        !file
            .ast
            .functions_within(element.span)
            .any(|outer| outer.span != function.span && outer.span.contains(function.span))
            && crate::jsx_primitive_name_cached(
                lookup,
                file,
                element,
                entities,
                symbol_names,
                lookup.dialect,
            )
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| lookup.dialect.renders_children_through_callback(primitive))
    }) {
        return true;
    }
    let Some(symbol) = function_symbol(file, function, entities) else {
        return false;
    };
    let binding_name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))
        .map(|name| file.source_text(name.span).unwrap_or_default());
    file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            call.arguments.iter().enumerate().any(|(index, argument)| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .tracks_reads
                    && argument_references_callback_symbol(
                        file,
                        argument,
                        symbol,
                        entities,
                        symbol_names,
                    )
            })
        })
    }) || file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .any(|(element_index, element)| {
            known_primitive(&primitives.jsx[element_index]).is_some_and(|primitive| {
                lookup.dialect.renders_children_through_callback(primitive)
            }) && file.ast.identifiers_within(element.span).any(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && !file.ast.jsx_containing(identifier.span).any(|nested| {
                        nested.span != element.span && element.span.contains(nested.span)
                    })
                    && (entities.get(&location(file.path.shared(), identifier.span))
                        == Some(symbol)
                        || binding_name == file.source_text(identifier.span))
            })
        })
}

pub(crate) fn counts_as_strict_read_root(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> bool {
    matches!(
        execution,
        ExecutionRole::EffectApply
            | ExecutionRole::UntrackedCallback
            | ExecutionRole::UntrackedUnwindowed
    ) || enclosing_render_function(file, span, lookup)
}

pub(crate) fn enclosing_function_label(file: &solid_facts::FileFacts, span: Span) -> String {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return String::new();
    };
    function_binding_name(file, function).map_or_else(String::new, |name| {
        file.source_text(name.span).unwrap_or_default().to_owned()
    })
}

pub(crate) fn analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
    lookup: &SemanticLookup<'_>,
) -> String {
    let enclosing = enclosing_function_label(file, span);
    if let Some(rendering) = file
        .ast
        .functions_body_containing(span)
        .filter(|function| lookup.function_is_component(file, function))
        .filter_map(|function| function_binding_name(file, function).or(function.name.as_ref()))
        .min_by_key(|name| name.span.end - name.span.start)
    {
        return file
            .source_text(rendering.span)
            .unwrap_or_default()
            .to_owned();
    }
    let callback = file
        .ast
        .arguments_containing(span)
        .map(|(call, index)| (call, index, call.arguments[index].span))
        .min_by_key(|(_, _, argument)| argument.end - argument.start);
    if let Some((call, argument, _)) = callback
        && let Some(primitive) =
            crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
    {
        // An owner-transparent inline callback (`untrack`, `flush(fn)`) runs
        // under its call site's owner, so it is described as that call site.
        // It has no label of its own: an anonymous one used to leave the
        // sentence with an empty scope name.
        if let Some(resolved) = primitive.primitive()
            && dialect.callback_preserves_owner_write_context(resolved)
            && dialect
                .callback_semantics_at(resolved, argument, call.arguments.len())
                .execution
                == Some(solid_dialect::Execution::Inline)
        {
            return analysis_context(file, call.span, entities, symbol_names, dialect, lookup);
        }
        // Which phase of a primitive an argument is, asked of the dialect
        // rather than matched here. The pair this had hardcoded is 2.0's:
        // `createEffect(compute, apply)`. 1.x's second argument is a seed
        // value threaded to the next run as `prev`, so a read in it would be
        // described as living in an "apply callback" that 1.x does not have.
        let phase = primitive.primitive().and_then(|resolved| {
            // A root body names its root: an anonymous one has no enclosing
            // label to fall back to. It runs under the root whenever it runs,
            // so its timing is not asked (rc.17 `hydrate` states none).
            if dialect.callback_runs_in_created_root(resolved, argument) {
                return Some("callback");
            }
            let semantics = dialect.callback_semantics_at(resolved, argument, call.arguments.len());
            semantics.execution.and_then(|execution| match execution {
                // "compute" only where reads actually subscribe: an
                // `onSettled` callback is tracked by the dialect (the graph
                // schedules it) but imperative to its reads, and calling
                // it a compute would describe the wrong phase.
                solid_dialect::Execution::Tracked if semantics.tracks_reads => Some("compute"),
                // Only an effect's apply slot is an apply phase; a deferred
                // executor's callback keeps its enclosing label. Which slot
                // that is -- or whether the primitive has one -- is the
                // dialect's answer, not a pair named here.
                solid_dialect::Execution::Deferred
                    if dialect.apply_callback_argument(resolved) == Some(argument) =>
                {
                    Some("apply callback")
                }
                _ => None,
            })
        });
        if let Some(phase) = phase {
            return format!("{primitive} {phase}");
        }
    }
    enclosing
}
