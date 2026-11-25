//! Read analysis context: compute reads, async roles, and the spans allowed to hold callbacks.

use super::*;

pub(crate) fn read_analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
    lookup: &SemanticLookup<'_>,
) -> String {
    if execution == ExecutionRole::EffectApply {
        // The role is shared by every effect primitive's apply position; the
        // name of the primitive whose apply this is is not. It used to be
        // `createEffect` unconditionally, which misnamed every read in a
        // `createRenderEffect` apply. Where the primitive is not one exact
        // answer -- a named function passed as the apply of two different
        // primitives -- or the dialect has no spelling for it, the sentence
        // stays true without naming one: this is read context in a message,
        // not a premise of any proof.
        match effect_apply_primitive(file, span, lookup)
            .and_then(|primitive| lookup.dialect.name_of(primitive))
        {
            Some(name) => format!("{name} apply callback"),
            None => "effect apply callback".into(),
        }
    } else {
        let context = enclosing_function_label(file, span);
        format_read_context(&context, file.ast.any_conditional_test_containing(span))
    }
}

/// Message-only: join an apply read to a read of the exact same reactive
/// declaration in its paired compute. No name or type-alias matching, and no
/// claim that a conditional compute read necessarily executes on every run.
pub(crate) fn apply_compute_reads(
    facts: &solid_facts::ProjectFacts,
    reads: &[crate::ReactiveRead],
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, typefacts::Location)>,
    origins: &HashMap<SymbolId, (SymbolId, SymbolId, typefacts::Location)>,
) -> HashSet<typefacts::Location> {
    let callback = |file: &solid_facts::FileFacts, argument: Span| {
        let argument = file.ast.peel_ts_sugar_span(argument);
        file.ast
            .function_at(argument)
            .map(|function| (file.path.to_string(), function.span))
            .or_else(|| {
                if !file.ast.identifiers.iter().any(|identifier| {
                    identifier.span == argument
                        && identifier.role == solid_facts::ast::IdentifierRole::Reference
                }) {
                    return None;
                }
                lookup
                    .entity_symbol(file, argument)
                    .and_then(|symbol| lookup.function_for_symbol(symbol))
                    .filter(|(file, function)| lookup.function_value_is_current(file, function))
                    .map(|(file, function)| (file.path.to_string(), function.span))
            })
    };
    let mut pairs = HashMap::<(String, Span), Vec<Option<(String, Span)>>>::default();
    for file in crate::base::reads::all_files(facts) {
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            let Some(apply_index) =
                effect_apply_argument(lookup.dialect, primitive, call.arguments.len())
            else {
                continue;
            };
            let Some(compute_index) = (0..call.arguments.len()).find(|index| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, *index, call.arguments.len())
                    .execution
                    == Some(Execution::Tracked)
            }) else {
                continue;
            };
            let Some(apply) = callback(file, call.arguments[apply_index].span) else {
                continue;
            };
            let compute = callback(file, call.arguments[compute_index].span);
            pairs.entry(apply).or_default().push(compute);
        }
    }
    let function_at = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        // ADR 0454: the body that runs the read; a read in a parameter list
        // runs when its function is called without the argument.
        crate::owners::containing_body_function(&file.ast, span)
            .map(|function| (file.path.to_string(), function.span))
    };
    // A declaration site can allocate several sources (two calls of a
    // factory). Correlate immutable getter values, not allocation templates.
    let mut values = HashMap::<SymbolId, Option<(Option<SymbolId>, bool)>>::default();
    for file in crate::base::reads::all_files(facts) {
        for binding in file.ast.bindings.iter().filter(|binding| binding.immutable) {
            for name in &binding.names {
                let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span).cloned()
                else {
                    continue;
                };
                let alias = (binding.shape == solid_facts::ast::BindingShape::Identifier)
                    .then_some(binding.initializer_identifier.as_ref())
                    .flatten()
                    .and_then(|name| lookup.entities().at(file.path.as_str(), name.span))
                    .cloned();
                let global = !file.ast.any_function_body_containing(name.span)
                    && !file
                        .ast
                        .classes
                        .iter()
                        .any(|class| class.span.contains(name.span))
                    && !file.ast.is_inside_module_block(name.span);
                if values
                    .insert(symbol.clone(), Some((alias, global)))
                    .is_some()
                {
                    values.insert(symbol, None);
                }
            }
        }
    }
    let value_at =
        |file: &solid_facts::FileFacts, span: Span| {
            let call = file.ast.calls.iter().find(|call| call.span == span)?;
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if !file.ast.identifiers.iter().any(|id| {
                id.span == callee && id.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                return None;
            }
            let mut symbol = lookup.entities().at(file.path.as_str(), callee)?.clone();
            let mut visiting = HashSet::default();
            loop {
                if !visiting.insert(symbol.clone()) {
                    return None;
                }
                let (alias, global) = values.get(&symbol)?.as_ref()?;
                if let Some(alias) = alias
                    && values.get(alias).is_some_and(Option::is_some)
                {
                    symbol = alias.clone();
                    continue;
                }
                return accessors.contains_key(&symbol).then_some((symbol, *global));
            }
        };
    let read_value = |read: &crate::ReactiveRead| {
        let file = lookup.file_by_path(&read.location.path)?;
        let span = Span::new(
            u32::try_from(read.location.start_byte).ok()?,
            u32::try_from(read.location.end_byte).ok()?,
        );
        if let Some((symbol, _)) = value_at(file, span) {
            return Some(symbol);
        }
        // A helper's template-local capture is not an instance identity.
        // Only a module's immutable singleton may be recovered from origin.
        let origin = read.origin.as_ref()?;
        let file = lookup.file_by_path(&origin.path)?;
        let span = Span::new(
            u32::try_from(origin.start_byte).ok()?,
            u32::try_from(origin.end_byte).ok()?,
        );
        value_at(file, span)
            .filter(|(_, global)| *global)
            .map(|(symbol, _)| symbol)
    };
    let mut tracked_sources = reads
        .iter()
        .filter(|read| {
            !read.is_uncertifiable()
                && !read.summary_attributed
                && matches!(
                    read.execution,
                    ExecutionRole::DeferredCallback | ExecutionRole::TrackedJsx
                )
        })
        .filter_map(|read| {
            function_at(read)
                .zip(read_value(read))
                .map(|(function, value)| {
                    (function, value, read.declaration.clone(), read.kind.clone())
                })
        })
        .collect::<HashSet<_>>();
    // Local read collection can omit a named compute whose invocation has no
    // reachability row. Its exact compute position and a direct call of a proven
    // accessor still establish the wording observation. Do not follow helpers,
    // nested callbacks, asynchronous bodies, or type-only accessor candidates.
    let computes = pairs
        .values()
        .flatten()
        .filter_map(Clone::clone)
        .collect::<HashSet<_>>();
    let mut guaranteed = HashMap::<(String, Span), HashSet<Span>>::default();
    for file in crate::base::reads::all_files(facts) {
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| !call.construct && call.arguments.is_empty())
        {
            let Some(function) = crate::owners::containing_body_function(&file.ast, call.span)
            else {
                continue;
            };
            if !computes.contains(&(file.path.to_string(), function.span))
                || function.r#async
                || function.generator
            {
                continue;
            }
            if file.ast.any_jsx_containing(call.span) {
                continue;
            }
            let Some(symbol) = lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            let Some((_, declaration)) = accessors.get(symbol) else {
                continue;
            };
            let declaration = origins.get(symbol).map_or(declaration, |origin| &origin.2);
            let Some((value, _)) = value_at(file, call.span) else {
                continue;
            };
            let source = (
                (file.path.to_string(), function.span),
                value,
                declaration.clone(),
                "accessor".into(),
            );
            if tracked_sources.contains(&source) {
                continue;
            }
            let calls = guaranteed
                .entry((file.path.to_string(), function.span))
                .or_insert_with(|| {
                    solid_facts::ast::synchronously_unconditional_calls(
                        std::path::Path::new(file.path.as_str()),
                        &file.source,
                        function.span,
                        &[],
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
                });
            if !calls.contains(&call.span) {
                continue;
            }
            tracked_sources.insert(source);
        }
    }
    reads
        .iter()
        .filter(|read| {
            read.execution == ExecutionRole::EffectApply
                && read.kind.as_ref() == "accessor"
                && !read.is_uncertifiable()
                && !read.summary_attributed
        })
        .filter(|read| {
            function_at(read)
                .and_then(|apply| pairs.get(&apply))
                .is_some_and(|computes| {
                    // Several uses of a named apply need the same answer at every pair.
                    computes.iter().all(|compute| {
                        compute.as_ref().is_some_and(|compute| {
                            read_value(read).is_some_and(|value| {
                                tracked_sources.contains(&(
                                    compute.clone(),
                                    value,
                                    read.declaration.clone(),
                                    read.kind.clone(),
                                ))
                            })
                        })
                    })
                })
        })
        .map(|read| read.location.clone())
        .collect()
}

/// The effect primitive whose apply callback `span` runs in, mirroring the two
/// ways [`semantic_execution_role_within`] reaches `EffectApply`: a named
/// function passed at an apply position, then the innermost inline apply
/// argument containing the span (inline wrappers such as `batch` inside it
/// inherit that apply's role).
pub(super) fn effect_apply_primitive(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Primitive> {
    let roles = lookup.named_callback_roles(file);
    if let Some(callback) = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)
        && containing_ast_function(&file.ast, span).is_some_and(|owner| owner.span == callback.span)
        && roles.get(callback.span).effect_apply
    {
        return roles.get(callback.span).effect_apply_primitive.single();
    }
    file.ast
        .arguments_containing(span)
        .filter_map(|(call, index)| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            (effect_apply_argument(lookup.dialect, primitive, call.arguments.len()) == Some(index))
                .then(|| (call.arguments[index].span, primitive))
        })
        .min_by_key(|(argument, _)| argument.end - argument.start)
        .map(|(_, primitive)| primitive)
}

pub(super) fn format_read_context(context: &str, in_conditional_test: bool) -> String {
    if in_conditional_test {
        if context.is_empty() {
            "while evaluating a condition".into()
        } else {
            format!("{context} while evaluating a condition")
        }
    } else {
        context.into()
    }
}

pub(crate) fn async_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    if execution == ExecutionRole::DeferredCallback && file.ast.any_jsx_containing(span) {
        role_arm!(
            "read_context.async_execution_role.deferred-in-jsx",
            ExecutionRole::TrackedJsx
        )
    } else {
        execution
    }
}

/// [`async_execution_role`] for the row an async read is placed by. A
/// deferred site inside JSX is taken as rendered there. ADR 0836: not when
/// the only JSX around it is an element whose child the site's function is
/// (a children callback, a render prop: `<Show when={1}>{(_u) => {
/// hc1(() => { a1(); }); … }}</Show>`): that function runs where the element
/// calls it, and the closure where its carrier runs it, so nothing places the
/// read in a tracked insert. It is unplaced: never a proven missing-boundary
/// violation, and an obligation for every pending rule. Callers that open an
/// obligation only for a tracked answer keep [`async_execution_role`].
pub(crate) fn async_read_placement(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    match async_execution_role(file, span, execution) {
        ExecutionRole::TrackedJsx
            if execution == ExecutionRole::DeferredCallback
                && only_child_function_jsx_around(file, span) =>
        {
            ExecutionRole::Unknown
        }
        role => role,
    }
}

/// Whether the innermost JSX element around `span` holds it only through a
/// function written as one of its children, with no JSX element or fragment
/// between that function and `span`.
fn only_child_function_jsx_around(file: &solid_facts::FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let size = |span: Span| span.end - span.start;
    let Some(element) = ast
        .jsx_containing(span)
        .min_by_key(|element| size(element.span))
    else {
        return false;
    };
    let Some(child) = ast
        .functions_body_containing(span)
        .filter(|function| {
            element.span.contains(function.span)
                && element
                    .children
                    .iter()
                    .any(|child| child.contains(function.span))
        })
        .max_by_key(|function| size(function.body))
    else {
        return false;
    };
    // A fragment inside the child function around the site is JSX the site
    // renders in; an element there would be the innermost one.
    !ast.jsx_fragments
        .iter()
        .any(|fragment| fragment.contains(span) && child.body.contains(*fragment))
}

/// The label of an exact `untrack` call. `strictRead = strictReadLabel || false`
/// (`@solidjs/signals@2.0.0-rc.21` `dist/dev-shared.js:5853`): a non-empty
/// string opens a window, and a call that passes anything else opens none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UntrackLabel {
    /// No label: the call keeps the window around it for one pending check.
    Absent,
    /// A non-empty string literal: a strict-read window of its own.
    Window,
    /// `false`, `undefined` or `""` written there: no window.
    Closed,
    /// Any other expression.
    Unproven,
}

pub(crate) fn untrack_label(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> UntrackLabel {
    use solid_facts::ast::ArgumentValueKind;
    let Some(label) = call.arguments.get(1) else {
        return UntrackLabel::Absent;
    };
    if label.spread {
        return UntrackLabel::Unproven;
    }
    let text = file
        .source_text(file.ast.peel_ts_sugar_span(label.span))
        .unwrap_or_default();
    let quoted = text.len() >= 2
        && (text.starts_with('"') || text.starts_with('\''))
        && text.ends_with(&text[..1]);
    match label.value {
        ArgumentValueKind::Undefined => UntrackLabel::Closed,
        _ if text == "false" || (quoted && text.len() == 2) => UntrackLabel::Closed,
        _ if quoted => UntrackLabel::Window,
        _ => UntrackLabel::Unproven,
    }
}

/// What rc.21's `read()` does to a pending source read in an exact `untrack`
/// (ADRs 0619, 0628). A labelled one opens a window, so it throws on any pending
/// source. An unlabelled one in a strict-read window tests
/// `strictPending && owner._parent === context`, so the answer turns on which
/// owner created the source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UntrackPendingRead {
    /// No such `untrack` around the read: the read keeps its own role.
    NotUnderUntrack,
    /// The read throws `PENDING_ASYNC_UNTRACKED_READ`, as a direct read does.
    Throws,
    /// Another owner created the source: the read suspends.
    OtherOwner,
    /// Nothing decides which owner created it: an obligation.
    Unproven,
}

/// The `untrack` call running `function`: the call `function` is the sole
/// first argument of, or (`const read = () => m(); untrack(read)`) one of the
/// calls a never-written binding of it is handed to.
fn untrack_carrier<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::CallFact> {
    let carries = |call: &&solid_facts::ast::CallFact, argument: Span| {
        file.ast.peel_ts_sugar_span(call.arguments[0].span) == argument
            && lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Untrack)
    };
    if let Some(call) = file
        .ast
        .arguments_containing(function.span)
        .filter(|(call, index)| *index == 0 && carries(call, function.span))
        .min_by_key(|(call, _)| call.span.end - call.span.start)
        .map(|(call, _)| call)
    {
        return Some(call);
    }
    if let Some(call) = crate::owners::function_binding_name(file, function)
        .filter(|name| !crate::indexes::binding_written(file, name.span))
        .and_then(|name| {
            file.ast
                .identifiers
                .iter()
                .filter(|identifier| {
                    identifier.role == solid_facts::ast::IdentifierRole::Reference
                        && file.ast.reference_declaration(identifier.span) == Some(name.span)
                })
                .find_map(|reference| {
                    file.ast
                        .arguments_containing(reference.span)
                        .find(|(call, index)| *index == 0 && carries(call, reference.span))
                        .map(|(call, _)| call)
                })
        })
    {
        return Some(call);
    }
    // ADR 0628: a `const` chain or an object literal's member names it exactly.
    if crate::local_access::assigned_function(file, function) {
        return None;
    }
    file.ast.calls.iter().find(|call| {
        lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Untrack)
            && call
                .arguments
                .first()
                .is_some_and(|first| value_is_function(file, first.span, function, 4))
    })
}

/// `value` holds `function`: it is written there, or through unwritten `const`s
/// and members of object literals they hold.
fn value_is_function(
    file: &solid_facts::FileFacts,
    value: Span,
    function: &solid_facts::ast::FunctionFact,
    depth: usize,
) -> bool {
    let ast = &file.ast;
    let value = ast.peel_ts_sugar_span(value);
    if value == function.span {
        return true;
    }
    let Some(depth) = depth.checked_sub(1) else {
        return false;
    };
    let held = |name: Span| {
        ast.reference_declaration(name)
            .filter(|declaration| !crate::indexes::binding_written(file, *declaration))
            .and_then(|declaration| {
                ast.bindings.iter().find(|binding| {
                    binding.immutable
                        && binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.names.iter().any(|name| name.span == declaration)
                })
            })
            .and_then(|binding| binding.initializer)
            .map(|initializer| ast.peel_ts_sugar_span(initializer))
    };
    if let Some(member) = ast.member_at(value) {
        return !ast.computed_members.contains(&member.span)
            && held(ast.peel_ts_sugar_span(member.object))
                .zip(file.source_text(member.property))
                .and_then(|(holder, key)| {
                    super::pending_sources::closed_literal_slot_value(file, holder, key)
                })
                .is_some_and(|slot| value_is_function(file, slot, function, depth));
    }
    held(value).is_some_and(|initializer| value_is_function(file, initializer, function, depth))
}

/// The layers a read runs in under exact `untrack`s and the outermost window.
pub(crate) struct UntrackLayers<'f> {
    /// The functions from the read up to the one holding the outermost
    /// `untrack`: `untrack` changes no owner, so each runs under one owner.
    layers: Vec<&'f solid_facts::ast::FunctionFact>,
    /// The innermost `untrack` holds a window label itself.
    labelled: bool,
    /// The strict body is proven: the outermost call runs in a component
    /// body's window, or in a label of its own.
    in_window: bool,
    /// No window is proven, and the read may still not return: a body `dynamic`
    /// mounts re-runs onto a fresh pending memo, and never ends.
    window_unproven: bool,
    /// A function between the read and the `untrack` runs by a hand-off nothing
    /// follows: the read is written inside the `untrack`, and may run on its
    /// stack or later.
    lexical: bool,
}

/// [`UntrackLayers`] of a site, or of a value handed to `carrier` by reference
/// (`untrack(m)`): `None` where no `untrack` holds it.
pub(crate) fn untrack_layers<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    carrier: Option<&'f solid_facts::ast::CallFact>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<UntrackLayers<'f>> {
    let mut labelled = None::<bool>;
    let mut window_label = false;
    let mut outermost = None;
    let mut note = |call: &'f solid_facts::ast::CallFact| -> bool {
        match untrack_label(file, call) {
            UntrackLabel::Absent => {
                labelled.get_or_insert(false);
                outermost = Some(call.span);
                true
            }
            UntrackLabel::Window => {
                labelled.get_or_insert(true);
                outermost = Some(call.span);
                window_label = true;
                false
            }
            UntrackLabel::Closed | UntrackLabel::Unproven => false,
        }
    };
    let mut layers = Vec::new();
    // The layers up to the outermost `untrack`, and the function holding it.
    let mut held = (0, None);
    let mut at = span;
    let mut walking = true;
    let mut lexical = false;
    if let Some(call) = carrier {
        walking = note(call);
        at = call.span;
        held = (0, containing_ast_function(&file.ast, at));
    }
    // Bounded as the in-place climb is. A function that runs in place (an
    // IIFE, a closure called once, a helper's callback) runs on the stack of
    // its call, under the same owner.
    for _ in 0..16 {
        if !walking {
            break;
        }
        let Some(function) = containing_ast_function(&file.ast, at) else {
            break;
        };
        if let Some(call) = untrack_carrier(file, function, lookup) {
            layers.push(function);
            walking = note(call);
            at = call.span;
            held = (layers.len(), containing_ast_function(&file.ast, at));
        } else if let Some(call) = in_place_step(file, function, lookup) {
            layers.push(function);
            at = call;
        } else if !file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && lookup.primitive_at_call(file, call.span).is_some()
            })
            && let Some(outer) = enclosing_function(file, function)
        {
            // A callback a primitive owns has its own role. Anything else
            // written inside an `untrack` may run there.
            lexical = true;
            layers.push(function);
            at = outer.body;
        } else {
            break;
        }
    }
    let labelled = labelled?;
    let outermost = outermost?;
    let role = (!window_label).then(|| {
        let allowed = allowed_callback_spans(file, lookup);
        super::read_execution_role(file, outermost, &allowed, entities, symbol_names, lookup)
    });
    let in_window = window_label || role == Some(ExecutionRole::UntrackedRendering);
    let (kept, body) = held;
    layers.truncate(kept);
    layers.extend(body);
    Some(UntrackLayers {
        layers,
        labelled,
        in_window,
        window_unproven: matches!(
            role,
            Some(ExecutionRole::Unknown | ExecutionRole::UntrackedUnwindowed)
        ),
        lexical,
    })
}

/// The smallest function holding `function`.
fn enclosing_function<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    file.ast
        .functions
        .iter()
        .filter(|outer| outer.span != function.span && outer.span.contains(function.span))
        .min_by_key(|outer| outer.span.end - outer.span.start)
}

/// The position that runs `function` on its stack: an in-place layer's call,
/// or a standard-library carrier that provably runs it (`f.call()`,
/// `[1].forEach(f)`, a tagged template, ADR 0566).
fn in_place_step(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    super::in_place_layer_call(file, function, lookup).or_else(|| {
        crate::local_access::inline_carriers(file, function, lookup)
            .into_iter()
            .find(|carrier| crate::carrier_runs::carrier_provably_runs(file, *carrier, lookup))
    })
}

/// ADR 0636: the position in `target` (`None`: at module scope) whose evaluation
/// runs `site` on one stack: `site` itself written there, or the call entering
/// layers that run synchronously (a closure, an IIFE, a helper's callback, an
/// `untrack`, a root's or another primitive's synchronous callback, a carrier).
/// A function that runs later (an effect's apply, a handler, a timer) ends it.
pub(crate) fn entry_on_stack_of(
    file: &solid_facts::FileFacts,
    site: Span,
    target: Option<&solid_facts::ast::FunctionFact>,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let mut at = site;
    for _ in 0..16 {
        let layer = containing_ast_function(&file.ast, at);
        if layer.map(|function| function.span) == target.map(|function| function.span) {
            return Some(at);
        }
        let layer = layer?;
        let synchronous = file
            .ast
            .arguments_containing(layer.span)
            .find(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
                    && lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            lookup.dialect.synchronous_callback_slot(
                                primitive,
                                *index,
                                call.arguments.len(),
                            )
                        })
            })
            .map(|(call, _)| call.span);
        at = synchronous
            .or_else(|| untrack_carrier(file, layer, lookup).map(|call| call.span))
            .or_else(|| in_place_step(file, layer, lookup))?;
    }
    None
}

/// How many layers [`some_entry_on_stack_of`] climbs through, over every route it tries.
const SOME_ENTRY_BUDGET: usize = 64;

/// ADR 1216: a position in `target` (`None`: at module scope) whose evaluation runs `site` on one stack on at least one
/// execution, for a claim that one run suffices (a write that throws on its first run). Beside [`entry_on_stack_of`]'s
/// steps, a layer is climbed through:
/// - the callback of a dialect primitive that runs it during the call (`Execution::Inline`, or a tracked callback whose
///   first run is `TrackedCallbackTiming::DuringCall`, with no `lazy` options argument written): `flush(fn)`, a memo's or
///   an effect's compute;
/// - any direct call of the name an unrebound named function is bound to (`const f = () => …`, `function f() {}`):
///   other uses of the name do not matter here, since one run is the claim.
///
/// Each such call is a straight-line call of its function (`straight_line_calls`: run on every execution that reaches
/// it) written outside JSX, and no layer it crosses is async or a generator or may follow a suspension. Not for a silence: a function also
/// entered elsewhere runs there too.
pub(crate) fn some_entry_on_stack_of(
    file: &solid_facts::FileFacts,
    site: Span,
    target: Option<&solid_facts::ast::FunctionFact>,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, Option<Span>)> {
    let mut budget = SOME_ENTRY_BUDGET;
    let mut seen = Vec::new();
    some_entry_from(file, site, target, lookup, &mut budget, &mut seen)
}

fn some_entry_from(
    file: &solid_facts::FileFacts,
    at: Span,
    target: Option<&solid_facts::ast::FunctionFact>,
    lookup: &SemanticLookup<'_>,
    budget: &mut usize,
    seen: &mut Vec<Span>,
) -> Option<(Span, Option<Span>)> {
    *budget = budget.checked_sub(1)?;
    let layer = containing_ast_function(&file.ast, at);
    if layer.map(|function| function.span) == target.map(|function| function.span) {
        return Some((at, None));
    }
    let layer = layer?;
    if seen.contains(&layer.span)
        || layer.r#async
        || layer.generator
        || super::may_follow_suspension(file, at)
    {
        return None;
    }
    seen.push(layer.span);
    let mut next = Vec::new();
    // The steps of `entry_on_stack_of`.
    if let Some(step) = file
        .ast
        .arguments_containing(layer.span)
        .find(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
                && lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| {
                        lookup.dialect.synchronous_callback_slot(
                            primitive,
                            *index,
                            call.arguments.len(),
                        )
                    })
        })
        .map(|(call, _)| call.span)
        .or_else(|| untrack_carrier(file, layer, lookup).map(|call| call.span))
        .or_else(|| in_place_step(file, layer, lookup))
    {
        next.push(step);
    }
    // A primitive that runs the layer during its own call.
    next.extend(
        file.ast
            .arguments_containing(layer.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == layer.span
                    && runs_during_call(file, call, *index, lookup)
                    && straight_line(file, call.span)
            })
            .map(|(call, _)| call.span),
    );
    // ADR 1216 slice 2: a callback prop's function runs at the child's calls of the prop (ADR 1171, 1190: inline, named,
    // destructured, forwarded and spread forms).
    if let Some(calls) = prop_literal_call_sites(file, layer, lookup) {
        next.extend(calls.into_iter().filter(|call| straight_line(file, *call)));
    }
    // ADR 1216 slice 2: a component runs where the file renders it: an element of its tag written in the children of
    // intrinsic elements only, so it is built when the tree around it is.
    next.extend(eager_render_sites(file, layer, lookup));
    // ADR 1216 slice 3: a function stored in a fresh local collection runs where the collection hands it back and calls it.
    next.extend(stored_run_sites(file, layer, lookup));
    // The direct calls of the name the layer is bound to.
    if let Some(name) = crate::owners::function_binding_name(file, layer)
        && lookup.function_value_is_current(file, layer)
    {
        for identifier in &file.ast.identifiers {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || file.ast.reference_declaration(identifier.span) != Some(name.span)
            {
                continue;
            }
            let called = file.ast.calls.iter().find(|call| {
                !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == identifier.span
                    && straight_line(file, call.span)
            });
            if let Some(call) = called {
                next.push(call.span);
            }
        }
    }
    // ADR 1224: an effect's apply runs on the creating pass (a render effect, at its call) or in the flush that ends it (a
    // user effect); the walk goes on from the effect call and names it, for the mount's premises
    // (`flush_mount_hold_free`). One such step per walk.
    let apply_step = super::first_run_effect_of_apply(file, layer, lookup);
    next.into_iter()
        .find_map(|step| some_entry_from(file, step, target, lookup, budget, seen))
        .or_else(|| {
            let effect = apply_step?;
            let (entry, earlier) = some_entry_from(file, effect, target, lookup, budget, seen)?;
            earlier.is_none().then_some((entry, Some(effect)))
        })
}

/// ADR 1216 slice 3: the positions where `function`, stored once in a fresh local collection, is handed back and called
/// on some execution. `function` is bound to an unrebound name stored by one straight-line call of the standard
/// `Map.set(k, f)`, `WeakMap.set(k, f)` or `Set.add(f)` on a `const` local initialized there by the standard constructor
/// with no argument. Every other use of the collection is the receiver of a standard `forEach` whose callback is a
/// literal of at most one parameter, or of `WeakMap.get`, so the collection holds exactly that entry afterwards. A site
/// is a straight-line `forEach(literal)` whose callback calls its parameter straight-line, or a straight-line call of
/// `get(k)` with the stored key (the same `const` binding). Both the store and the site run in the function that declares
/// the collection, the site after the store, each climbed to it through straight-line `untrack` calls.
fn stored_run_sites(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let ast = &file.ast;
    let Some(name) = crate::owners::function_binding_name(file, function) else {
        return Vec::new();
    };
    if !lookup.function_value_is_current(file, function) {
        return Vec::new();
    }
    let standard = |call: &solid_facts::ast::CallFact| {
        let kind = if call.construct {
            typefacts::CallKind::Construct
        } else {
            typefacts::CallKind::Call
        };
        crate::indexes::standard_library_declaration_of(
            lookup.resolved_callee_call(file, call.callee),
            kind,
        )
        .map(|declaration| declaration.qualified_name.to_string())
    };
    let references = |declaration: Span| {
        ast.identifiers.iter().filter(move |identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && ast.reference_declaration(identifier.span) == Some(declaration)
        })
    };
    let receiver_of = |call: &solid_facts::ast::CallFact| {
        ast.member_at(ast.peel_ts_sugar_span(call.callee))
            .map(|member| ast.peel_ts_sugar_span(member.object))
    };
    let mut sites = Vec::new();
    for reference in references(name.span) {
        let Some((store, index)) =
            ast.arguments_containing(reference.span)
                .find(|(call, index)| {
                    ast.peel_ts_sugar_span(call.arguments[*index].span) == reference.span
                })
        else {
            continue;
        };
        let Some(method) = standard(store) else {
            continue;
        };
        let Some(constructor) = stored_collection_constructor(method.as_str(), index) else {
            continue;
        };
        if store.construct
            || store.arguments.iter().any(|argument| argument.spread)
            || !straight_line(file, store.span)
        {
            continue;
        }
        let Some(collection) =
            receiver_of(store).and_then(|object| ast.reference_declaration(object))
        else {
            continue;
        };
        let Some(home) = containing_ast_function(ast, collection) else {
            continue;
        };
        let fresh = ast.bindings.iter().any(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.iter().any(|name| name.span == collection)
                && binding
                    .initializer
                    .map(|initializer| ast.peel_ts_sugar_span(initializer))
                    .and_then(|initializer| {
                        ast.calls
                            .iter()
                            .find(|call| call.span == initializer && call.construct)
                    })
                    .is_some_and(|created| {
                        created.arguments.is_empty()
                            && standard(created).as_deref() == Some(constructor)
                    })
        });
        if !fresh {
            continue;
        }
        // A position's run in `home`, through straight-line `untrack` layers.
        let in_home = |site: Span| {
            let mut at = site;
            for _ in 0..4 {
                let layer = containing_ast_function(ast, at)?;
                if layer.span == home.span {
                    return Some(at);
                }
                if layer.r#async || layer.generator {
                    return None;
                }
                at = untrack_carrier(file, layer, lookup)
                    .map(|call| call.span)
                    .filter(|step| straight_line(file, *step))?;
            }
            None
        };
        let Some(at) = in_home(store.span).filter(|at| collection.end <= at.start) else {
            continue;
        };
        let stored_key = (constructor == "WeakMapConstructor.construct")
            .then(|| store.arguments.first())
            .flatten()
            .and_then(|key| ast.reference_declaration(ast.peel_ts_sugar_span(key.span)));
        let mut uses_kept = true;
        let mut found = Vec::new();
        for use_site in references(collection) {
            let Some(call) = ast
                .calls
                .iter()
                .find(|call| !call.construct && receiver_of(call) == Some(use_site.span))
            else {
                uses_kept = false;
                break;
            };
            if call.span == store.span {
                continue;
            }
            match standard(call).as_deref() {
                Some("Map.forEach" | "Set.forEach") => {
                    let Some(callback) = call
                        .arguments
                        .first()
                        .filter(|argument| !argument.spread)
                        .and_then(|argument| {
                            ast.function_at(ast.peel_ts_sugar_span(argument.span))
                        })
                    else {
                        uses_kept = false;
                        break;
                    };
                    if callback.parameters.len() > 1 || callback.rest_parameter {
                        uses_kept = false;
                        break;
                    }
                    let runs = !callback.r#async
                        && !callback.generator
                        && straight_line(file, call.span)
                        && in_home(call.span).is_some_and(|run| at.end <= run.start)
                        && callback
                            .parameters
                            .first()
                            .filter(|parameter| {
                                parameter.shape == solid_facts::ast::BindingShape::Identifier
                            })
                            .and_then(|parameter| parameter.names.first())
                            .is_some_and(|parameter| {
                                ast.calls.iter().any(|inner| {
                                    !inner.construct
                                        && ast.reference_declaration(
                                            ast.peel_ts_sugar_span(inner.callee),
                                        ) == Some(parameter.span)
                                        && straight_line(file, inner.span)
                                        && containing_ast_function(ast, inner.span)
                                            .map(|layer| layer.span)
                                            == Some(callback.span)
                                })
                            });
                    if runs {
                        found.push(call.span);
                    }
                }
                Some("WeakMap.get") => {
                    let same_key = stored_key.is_some()
                        && call
                            .arguments
                            .first()
                            .filter(|argument| !argument.spread)
                            .and_then(|key| {
                                ast.reference_declaration(ast.peel_ts_sugar_span(key.span))
                            })
                            == stored_key;
                    let fixed_key = stored_key.is_some_and(|key| {
                        ast.bindings.iter().any(|binding| {
                            binding.immutable && binding.names.iter().any(|name| name.span == key)
                        })
                    });
                    let caller = ast.calls.iter().find(|outer| {
                        !outer.construct && ast.peel_ts_sugar_span(outer.callee) == call.span
                    });
                    if let Some(outer) = caller
                        && same_key
                        && fixed_key
                        && straight_line(file, outer.span)
                        && in_home(outer.span).is_some_and(|run| at.end <= run.start)
                    {
                        found.push(outer.span);
                    }
                }
                _ => {
                    uses_kept = false;
                    break;
                }
            }
        }
        if uses_kept {
            sites.extend(found);
        }
    }
    sites
}

/// ADR 1216 slice 3: the standard constructor of the collection a store call writes its argument `index` into: the
/// value of `Map.set` and `WeakMap.set`, the member of `Set.add`. `None` for any other method or slot: that call stores
/// nothing [`stored_run_sites`] follows, so it gives no step (the site keeps its uncertifiable answer).
fn stored_collection_constructor(method: &str, index: usize) -> Option<&'static str> {
    match (method, index) {
        ("Map.set", 1) => Some("MapConstructor.construct"),
        ("WeakMap.set", 1) => Some("WeakMapConstructor.construct"),
        ("Set.add", 0) => Some("SetConstructor.construct"),
        _ => None,
    }
}

/// ADR 1216 slice 2: the elements that render the component `function` of this file eagerly: a JSX element whose tag
/// resolves to it (no member tag), every enclosing JSX element of which is intrinsic and holds it, or the element between,
/// as a direct child: not in an attribute and not in an expression child (`{c() ? <K /> : null}` is evaluated reactively,
/// possibly long after the creating pass). A component's children and attributes run only when that component reads them; an intrinsic element's
/// children are built with it.
pub(super) fn eager_render_sites(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let ast = &file.ast;
    ast.jsx_elements
        .iter()
        .filter(|element| {
            element.member_object.is_none()
                && !super::intrinsic_element(file, element)
                && lookup
                    .function_called_at(file.path.as_str(), element.name.span)
                    .is_some_and(|(target, called)| {
                        target.path == file.path && called.span == function.span
                    })
                && ast
                    .jsx_containing(element.span)
                    .filter(|outer| outer.span != element.span)
                    .all(|outer| {
                        super::intrinsic_element(file, outer)
                            && outer
                                .children
                                .iter()
                                .any(|child| child.contains(element.span))
                            && (outer.children.contains(&element.span)
                                || ast.jsx_containing(element.span).any(|between| {
                                    between.span != element.span
                                        && outer.span.contains(between.span)
                                        && between.span != outer.span
                                        && outer.children.contains(&between.span)
                                }))
                    })
        })
        .map(|element| element.span)
        .collect()
}

/// ADR 1216: the dialect runs the callback at `index` of `call` before the call returns, on its first run at least
/// (`Execution::Inline`, or `TrackedCallbackTiming::DuringCall`), and no argument stands where a `lazy` option may defer it.
fn runs_during_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
        return false;
    };
    let count = call.arguments.len();
    let dialect = lookup.dialect;
    let during = dialect.callback_execution_at(primitive, index, count)
        == Some(solid_dialect::Execution::Inline)
        || dialect.tracked_callback_timing(primitive, index, count)
            == Some(solid_dialect::TrackedCallbackTiming::DuringCall);
    // A schedule with no `lazy` option has nothing to defer; one with it defers only where that argument is written.
    let lazy_written = dialect
        .lazy_option_argument(primitive)
        .is_some_and(|options| options < count);
    during && !lazy_written
}

/// ADR 1216: `call` is a call its function (or the module) runs on every execution that reaches it, written outside
/// every JSX element: a JSX expression is evaluated when (and as often as) the compiled code reads it, and a component's
/// children only when that component reads them, so `straight_line_calls`, which sees no function there, says nothing.
fn straight_line(file: &solid_facts::FileFacts, call: Span) -> bool {
    file.ast.straight_line_calls.contains(&call) && file.ast.jsx_containing(call).next().is_none()
}

pub(crate) fn runs_on_stack_of(
    file: &solid_facts::FileFacts,
    site: Span,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    entry_on_stack_of(file, site, Some(function), lookup).is_some()
}

/// ADR 0632: `read` runs on the stack of the pass that creates the source
/// declared at `declaration`: nothing has landed and no re-ask has started, so
/// a declared value is served, whatever layers carry the read.
pub(crate) fn served_in_creating_pass(
    file: &solid_facts::FileFacts,
    read: Span,
    declaration: &crate::Location,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return false;
    };
    declaration.path.as_ref() == file.path.as_str()
        && containing_ast_function(&file.ast, Span::new(start, end)).is_some_and(|function| {
            !function.r#async && runs_on_stack_of(file, read, function, lookup)
        })
}

impl UntrackLayers<'_> {
    /// The read is in an unlabelled `untrack` in a strict-read window, so a
    /// pending source its owner created throws there.
    pub(crate) fn keeps_pending_throw(&self) -> bool {
        (self.in_window || self.window_unproven) && !self.labelled
    }

    /// `span` is a call written directly in one of the layers.
    pub(crate) fn holds_call(&self, file: &solid_facts::FileFacts, span: Span) -> bool {
        containing_ast_function(&file.ast, span)
            .is_some_and(|function| self.layers.iter().any(|layer| layer.span == function.span))
    }
}

/// ADR 0619, ADR 0628: where a read of the source declared at `declaration`
/// stands. A labelled `untrack` throws on any pending source; an unlabelled
/// one only runs the check in a strict-read window, for a source the owner
/// running it created.
pub(crate) fn untrack_pending_read(
    file: &solid_facts::FileFacts,
    layers: Option<&UntrackLayers<'_>>,
    span: Span,
    declaration: &crate::Location,
    lookup: &SemanticLookup<'_>,
) -> UntrackPendingRead {
    if !lookup.dialect.untrack_keeps_own_pending_throw() {
        return UntrackPendingRead::NotUnderUntrack;
    }
    let Some(found) = layers.filter(|found| found.in_window || found.window_unproven) else {
        return UntrackPendingRead::NotUnderUntrack;
    };
    if found.labelled {
        return UntrackPendingRead::Throws;
    }
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return UntrackPendingRead::Unproven;
    };
    let declared = Span::new(start, end);
    let Some(source_file) = lookup.file_by_path(declaration.path.as_ref()) else {
        return UntrackPendingRead::Unproven;
    };
    if crate::indexes::at_module_scope(&source_file.ast, declared) {
        return UntrackPendingRead::OtherOwner;
    }
    if found.lexical {
        return UntrackPendingRead::Unproven;
    }
    let own = declaration.path.as_ref() == file.path.as_str()
        && containing_ast_function(&file.ast, declared)
            .is_some_and(|function| found.layers.iter().any(|layer| layer.span == function.span))
        && super::pending_sources::binding_creates_here(file, declared, lookup);
    if own
        || super::pending_sources::read_of_hook_called_in(
            file,
            span,
            &found.layers,
            source_file,
            declared,
            lookup,
        )
    {
        return if found.in_window {
            UntrackPendingRead::Throws
        } else {
            UntrackPendingRead::Unproven
        };
    }
    if super::pending_sources::read_of_module_hook_result(file, span) {
        return UntrackPendingRead::OtherOwner;
    }
    if found.in_window {
        UntrackPendingRead::Unproven
    } else {
        UntrackPendingRead::NotUnderUntrack
    }
}

/// ADR 0623: the read is tracked in a computation's own callback, so a pending
/// source suspends it. A read past an await is tracked by nothing, and a closure
/// the compute names and stores may run anywhere: both stay unproven.
pub(crate) fn computation_read_suspends(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    crate::owners::tracked_computation_read(file, span, entities, symbol_names, lookup)
        && !may_follow_suspension(file, span)
        && file
            .ast
            .arguments_containing(span)
            .any(|(call, index)| direct_callback_contains(file, call.arguments[index].span, span))
}

pub(crate) fn pending_accessor_probe(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        direct_callback_contains(file, call.arguments[index].span, span)
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_handles_pending_accessor_read(primitive, index)
                })
    })
}

pub(crate) fn allowed_callback_spans(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let mut spans = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let Some(primitive) = known_primitive(&primitives.calls[call_index]) else {
            continue;
        };
        // ADR 0628: `untrack(fn, false)` is the unlabelled call.
        if primitive == solid_dialect::Primitive::Untrack
            && untrack_label(file, call) == UntrackLabel::Closed
            && let Some(first) = call.arguments.first()
        {
            spans.push(first.span);
            continue;
        }
        let deferred = deferred_callback_positions(dialect, primitive, call.arguments.len())
            .into_iter()
            .filter(|index| {
                callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
            })
            .collect::<Vec<_>>();
        for index in &deferred {
            if let Some(argument) = call.arguments.get(*index) {
                spans.push(argument.span);
            }
        }
        // ADR 0548: an exact bundle's `error` member runs outside any window
        // and tracking pass, wherever the slot itself is placed.
        for index in (0..call.arguments.len()).filter(|index| !deferred.contains(index)) {
            if let Some(error) = super::bundle_error_member(dialect, primitive, call, index) {
                spans.push(error);
            }
        }
    }
    spans
}
