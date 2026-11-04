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
    for file in &facts.files {
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
    for file in &facts.files {
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
    for file in &facts.files {
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
        ExecutionRole::TrackedJsx
    } else {
        execution
    }
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
    if crate::local_access::member_assigned(file, function) {
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
    let in_window = window_label || {
        let allowed = allowed_callback_spans(file, lookup);
        super::read_execution_role(file, outermost, &allowed, entities, symbol_names, lookup)
            == ExecutionRole::UntrackedRendering
    };
    let (kept, body) = held;
    layers.truncate(kept);
    layers.extend(body);
    Some(UntrackLayers {
        layers,
        labelled,
        in_window,
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
        self.in_window && !self.labelled
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
    let Some(found) = layers.filter(|found| found.in_window) else {
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
        return UntrackPendingRead::Throws;
    }
    if super::pending_sources::read_of_module_hook_result(file, span) {
        return UntrackPendingRead::OtherOwner;
    }
    UntrackPendingRead::Unproven
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
