//! Interprocedural graph discovery: summary nodes, call edges, and the assembly of the result graph.

use super::*;

/// Whether the argument at `position` is handed, exactly, to a function
/// declaration that calls it on every completion. A declaration is
/// initialized when its module is instantiated; a `const` callee may still
/// be in its temporal dead zone (ADR 0218's review round 3).
fn forwarded_answer(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    position: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    crate::execution_role::project_callee_at_parameter(file, call, position, lookup).is_some_and(
        |(callee_file, callee, exact)| {
            exact
                && callee.kind == solid_facts::ast::FunctionKind::Declaration
                && lookup.parameter_called_on_every_completion(
                    callee_file.path.as_str(),
                    callee.span,
                    position,
                )
        },
    )
}

pub(super) fn discover_summary_nodes(
    file: &solid_facts::FileFacts,
    project_indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Vec<SummaryNode> {
    let mut nodes = Vec::new();
    let typescript_file = project_indexes.typescript_file(file.path.as_str());
    for arrow in [false, true] {
        for function in &file.ast.functions {
            let binding_name = function_binding_name(file, function);
            let source_function = typescript_file.and_then(|typescript_file| {
                typescript_file.functions.iter().find(|candidate| {
                    candidate.body.start_byte == u64::from(function.body.start)
                        && (candidate.body.end_byte.saturating_add(1)
                            == u64::from(function.body.end)
                            || (function.expression_body
                                && candidate.body.end_byte == u64::from(function.body.end)))
                })
            });
            // Preserve the checker's finite function universe and ordering:
            // declarations first, then arrows, each in source order. Structural
            // function facts cover named/assigned expressions and
            // expression-bodied arrows, so a missing fact is genuinely outside
            // the project function universe.
            if typescript_file.is_some()
                && source_function.is_none()
                && function.method_name.is_none()
            {
                continue;
            }
            let is_arrow = source_function.map_or(
                function.kind == solid_facts::ast::FunctionKind::Arrow,
                |function| function.arrow,
            );
            if is_arrow != arrow {
                continue;
            }
            let symbol = binding_name
                .as_ref()
                .and_then(|name| {
                    entities
                        .get(&location(file.path.shared(), name.span))
                        .cloned()
                })
                .or_else(|| {
                    crate::owners::own_method_name(file, function).and_then(|name| {
                        // A declaration another file's call resolves to can
                        // name the method (final review R3, plausible 2).
                        lookup.note_declaration_symbol_at(file.path.as_str(), name.span);
                        project_indexes.method_symbol(file.path.as_str(), name.span)
                    })
                });
            let runtime_identity = binding_name
                .as_ref()
                .and_then(|name| {
                    project_indexes
                        .entities_for_path(file.path.as_str())
                        .iter()
                        .find(|entity| {
                            entity.location.start_byte == u64::from(name.span.start)
                                && entity.location.end_byte == u64::from(name.span.end)
                        })
                })
                .map_or_else(String::new, |entity| entity.runtime_identity.to_string());
            let parameters = function
                .parameters
                .iter()
                .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
                .filter_map(|parameter| parameter.names.first())
                .filter_map(|name| {
                    entities
                        .get(&location(file.path.shared(), name.span))
                        .cloned()
                })
                .collect();
            nodes.push(SummaryNode {
                path: file.path.to_string(),
                span: function.span,
                body: function.body,
                name: binding_name
                    .as_ref()
                    .map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
                symbol,
                runtime_identity,
                parameters,
                exported: source_function.map_or_else(
                    || source_function_exported(project_indexes, file, function),
                    |function| function.exported,
                ),
                r#async: source_function.map_or(function.r#async, |function| function.r#async),
                source_hash: file.source_hash.clone(),
            });
        }
    }
    nodes
}

#[derive(Clone, Copy)]
pub(super) struct InterproceduralGraphSymbols<'a> {
    pub(super) entities: &'a EntitySymbols,
    pub(super) names: &'a HashMap<SymbolId, SymbolId>,
}

pub(super) fn discover_interprocedural_graph(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    symbols: InterproceduralGraphSymbols<'_>,
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> InterproceduralGraphContribution {
    let (scope, protocols) =
        GraphScope::new(file, nodes, nodes_by_path, symbols, lookup, accessors);
    let mut contribution = InterproceduralGraphContribution {
        direct_protocol_parameters: protocols,
        ..Default::default()
    };
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        discover_call_graph(&scope, &mut contribution, call_index, call);
    }
    push_unaccounted_parameter_escapes(
        &mut contribution,
        file,
        nodes,
        nodes_by_path,
        lookup,
        symbols.entities,
    );
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = file.ast.call_at(initializer) else {
            continue;
        };
        let Some(target_symbol) = symbols
            .entities
            .get(&location(file.path.shared(), call.callee))
        else {
            continue;
        };
        for name in &binding.names {
            if let Some(binding_symbol) = symbols
                .entities
                .get(&location(file.path.shared(), name.span))
            {
                contribution
                    .returned_bindings
                    .push((binding_symbol.clone(), target_symbol.clone()));
            }
        }
    }
    contribution
}

/// What one file's graph discovery reads, fixed for the file.
struct GraphScope<'a, 'b> {
    file: &'a solid_facts::FileFacts,
    nodes: &'a [SummaryNode],
    nodes_by_path: &'a HashMap<String, Vec<usize>>,
    symbols: InterproceduralGraphSymbols<'a>,
    lookup: &'a SemanticLookup<'b>,
    accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    primitives: &'a crate::indexes::FilePrimitives,
    allowed: Vec<Span>,
    function_lookup: FunctionLookup,
    returned_function_targets: HashMap<SymbolId, usize>,
    ast_parameter_symbols: HashSet<SymbolId>,
    member_calls: HashMap<Span, (Span, usize, Vec<String>)>,
    unwritten_parameters: HashSet<(Span, usize)>,
}

impl<'a, 'b> GraphScope<'a, 'b> {
    /// The scope, and the direct protocol uses it found along the way.
    fn new(
        file: &'a solid_facts::FileFacts,
        nodes: &'a [SummaryNode],
        nodes_by_path: &'a HashMap<String, Vec<usize>>,
        symbols: InterproceduralGraphSymbols<'a>,
        lookup: &'a SemanticLookup<'b>,
        accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    ) -> (
        Self,
        Vec<(Span, crate::summary_semantics::InvokeProtocol, usize)>,
    ) {
        let primitives = lookup.primitives(file);
        let allowed = allowed_callback_spans(file, lookup);
        let function_lookup = function_lookup_for_path(nodes, nodes_by_path, file.path.as_str());
        let returned_function_targets =
            returned_function_targets(file, nodes, &function_lookup, symbols.entities);
        let ast_parameter_symbols = file
            .ast
            .functions
            .iter()
            .flat_map(|function| &function.parameters)
            .flat_map(|parameter| &parameter.names)
            .filter_map(|name| symbols.entities.at(file.path.as_str(), name.span).cloned())
            .collect::<HashSet<_>>();
        let DirectProtocolUses {
            protocols,
            member_calls,
            unwritten_parameters,
        } = direct_protocol_parameters(file, nodes, nodes_by_path);
        let member_calls = member_calls
            .into_iter()
            .map(|(callee, owner, parameter, path)| (callee, (owner, parameter, path)))
            .collect::<HashMap<_, _>>();
        (
            Self {
                file,
                nodes,
                nodes_by_path,
                symbols,
                lookup,
                accessors,
                primitives,
                allowed,
                function_lookup,
                returned_function_targets,
                ast_parameter_symbols,
                member_calls,
                unwritten_parameters,
            },
            protocols,
        )
    }
}

/// One call of the file, as the pieces of its graph discovery see it.
struct GraphCall<'a> {
    call_index: usize,
    call: &'a solid_facts::ast::CallFact,
    owner: usize,
    owner_span: Span,
    ambiguous_dispatch: bool,
    project_function: bool,
}

fn discover_call_graph(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    call_index: usize,
    call: &solid_facts::ast::CallFact,
) {
    let file = scope.file;
    let nodes = scope.nodes;
    let nodes_by_path = scope.nodes_by_path;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    let accessors = scope.accessors;
    let function_lookup = &scope.function_lookup;
    let returned_function_targets = &scope.returned_function_targets;
    let member_calls = &scope.member_calls;
    // ADR 0454: a call in a parameter list is made when that function is
    // called without the argument. A list at module level has no function
    // around it, so placed by body alone the call would drop out of the graph
    // and its reads with it: it belongs to the node of the function whose list
    // it is (reached through an edge that is not a guaranteed one, so a read
    // through it is a possible read). Inside a function the call keeps the node
    // of the function around it, with the rows of the rungs that read it
    // there (`invoked_parameters`), as before.
    let Some(owner) =
        containing_summary_function_indexed(nodes, nodes_by_path, file.path.as_str(), call.span)
            .or_else(|| {
                let function = crate::owners::owning_function(&file.ast, call.span)?;
                nodes_by_path
                    .get(file.path.as_str())?
                    .iter()
                    .copied()
                    .find(|index| nodes[*index].span == function.span)
            })
    else {
        return;
    };
    // A selected property declaration does not establish that this
    // receiver still holds that function (ADR 0299 review). Preserve
    // unknown argument behavior instead of certifying an empty callback
    // summary when the receiver's implementation cannot be followed.
    if crate::object_members::call_withheld(file, call.callee, lookup) {
        push_unresolved_callee_callback_obligations(
            contribution,
            file,
            call,
            nodes,
            function_lookup,
            lookup,
            symbols.entities,
        );
        return;
    }
    let owner_span = nodes[owner].span;
    let call_completes = site_on_every_completion(lookup, file, owner_span, call.span);
    // ADR 0454: a call in the owner's own parameter list, or in a closure's,
    // is no call in the owner's body.
    let in_owner_body = crate::owners::containing_body_function(&file.ast, call.span)
        .is_some_and(|function| function.body == nodes[owner].body);
    // Read propagation is relative to this owner's call. Keep unknown
    // callback timing, but exclude proven tracked/deferred callbacks,
    // retained result closures and compiler-owned getter regions. The
    // dialect proof preserves apply callbacks that inherit the first run.
    let reads_for_owner =
        !read_escapes_synchronous_extent(
            file,
            call.span,
            symbols.entities,
            symbols.names,
            lookup.dialect,
        ) && !read_escapes_runtime_stack(file, call.span, nodes[owner].body, lookup)
            && !runs_in_retained_value_literal(file, call.span, &nodes[owner])
            && !runs_outside_owner_call(file, call.span, nodes[owner].body, lookup);
    record_parameter_member_invocation(
        scope,
        contribution,
        call,
        (owner, owner_span, in_owner_body),
    );
    // Item B of ways-to-improve § 3.3: `handler[0](handler[1], event)`, a
    // call of a literal-keyed member of the owner's own parameter written
    // in its own body. The callee is the caller's code at a path the
    // literal names, so the call runs before the export returns, on the
    // caller's stack, in the caller's tracking context: an `inline` row
    // for that member, exactly as `handler(event)` is one for the
    // parameter. Its arguments are then the caller's callable's business,
    // as a parameter callee's are (see the argument loop below, which
    // skips a callee that is a parameter for the same reason), so the
    // unresolved-callee obligation this call used to raise for them is
    // not raised: it described a callee nothing could name, and this one
    // is named.
    if let Some((callback_owner, parameter, path)) =
        member_calls.get(&file.ast.peel_ts_sugar_span(call.callee))
    {
        contribution.direct_member_callback_parameters.push((
            *callback_owner,
            *parameter,
            path.clone(),
        ));
        contribution.callbacks.push((
            *callback_owner,
            CallbackSummary {
                parameter: *parameter,
                execution: "inline".into(),
                schedule: None,
                clears_tracking: false,
                arguments: accessor_argument_summaries(file, call, symbols.entities, accessors),
                owner: None,
                protocol: crate::summary_semantics::InvokeProtocol::Call,
                path: path.clone(),
            },
        ));
        return;
    }
    let candidate_symbols = lookup.callee_symbols(file, call.callee);
    // Dispatch candidates answer "which analyzed implementation could
    // run"; the callee symbol answers "which declaration is called". A
    // member call whose receiver has no inspectable value -- a DOM
    // method, or a structural parameter with no in-project call site --
    // has no candidates, but it still has an exact identity, and its
    // arguments still have to be classified. Falling back keeps that
    // call in the analysis instead of dropping it silently.
    let Some(symbol) = candidate_symbols
        .first()
        .map(SymbolId::as_str)
        .or_else(|| lookup.callee_symbol(file, call.callee))
    else {
        // No candidates *and* no callee identity: the callee is a member
        // of a value with no inspectable type, which is every parameter
        // of a published JavaScript runtime artifact (`list.map(fn)`
        // where `list` is `any`). Nothing downstream can classify this
        // call's arguments, so dropping it silently lets a caller-supplied
        // callback escape with no recorded behavior -- and an omitted
        // `callbacks` field is a *negative* claim, so silence here
        // certifies "never invoked". Record the obligation instead; the
        // emitted claim becomes `{"status": "unknown"}`.
        push_unresolved_callee_callback_obligations(
            contribution,
            file,
            call,
            nodes,
            function_lookup,
            lookup,
            symbols.entities,
        );
        return;
    };
    let ambiguous_dispatch = candidate_symbols.len() > 1;
    if ambiguous_dispatch {
        // Keep the exact candidate identities, but do not let any one of
        // them contribute reads or callback timing until the fixed point
        // can prove that all candidates have equivalent summaries.
        contribution
            .dispatches
            .push((owner_span, candidate_symbols.clone()));
    }
    let project_function = lookup.function_for_symbol(symbol).is_some();
    if call.direct_callee && !ambiguous_dispatch && reads_for_owner {
        contribution
            .factory_calls
            .push((owner_span, SymbolId::from(symbol)));
    }
    let g = GraphCall {
        call_index,
        call,
        owner,
        owner_span,
        ambiguous_dispatch,
        project_function,
    };
    record_forwarded_parameter_invocations(scope, contribution, &g);
    if !ambiguous_dispatch && reads_for_owner {
        let returned_target = call
            .direct_callee
            .then(|| returned_function_targets.get(symbol).copied());
        if let Some(target) = returned_target.flatten() {
            contribution.edges.push((
                owner_span,
                InterproceduralGraphTarget::LocalSpan(nodes[target].span),
            ));
            if call_completes {
                contribution.guaranteed_edges.push((
                    owner_span,
                    InterproceduralGraphTarget::LocalSpan(nodes[target].span),
                ));
            }
        } else if call.direct_callee || project_function {
            contribution.edges.push((
                owner_span,
                InterproceduralGraphTarget::Symbol(SymbolId::from(symbol)),
            ));
            if call_completes {
                contribution.guaranteed_edges.push((
                    owner_span,
                    InterproceduralGraphTarget::Symbol(SymbolId::from(symbol)),
                ));
            }
        }
    }
    if call.direct_callee
        && !ambiguous_dispatch
        && let Some(&(callback_owner, parameter)) = function_lookup.parameter_owner.get(symbol)
        && record_parameter_callee_invocation(scope, contribution, &g, (callback_owner, parameter))
    {
        return;
    }
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        discover_argument_graph(scope, contribution, &g, symbol, (argument_index, argument));
    }
}

fn record_parameter_member_invocation(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    call: &solid_facts::ast::CallFact,
    (owner, owner_span, in_owner_body): (usize, Span, bool),
) {
    let file = scope.file;
    let nodes = scope.nodes;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    // `reader.read(value)` where `reader` is this function's parameter.
    // Which implementation runs is a property of the *call site*, not of
    // this function, so record the obligation and let each site resolve
    // its own argument. Pooling every site into one summary here is what
    // makes an unambiguous site uncertifiable because a sibling site is
    // not.
    // ADR 0320: the index is the parameter's position in the
    // declaration, which is the argument's at a call. The summary's
    // parameter list skips destructured parameters, so its position
    // would select another argument.
    if let Some((receiver, path)) = lookup.member_callee_receiver(file, call.callee)
        && nodes[owner].parameters.contains(&receiver)
        && let Some(parameter) = file.ast.function_at(owner_span).and_then(|function| {
            function.parameters.iter().position(|parameter| {
                parameter.shape == solid_facts::ast::BindingShape::Identifier
                    && parameter.names.first().is_some_and(|name| {
                        symbols.entities.at(file.path.as_str(), name.span) == Some(&receiver)
                    })
            })
        })
    {
        let primitive_builtin = primitive_builtin_member_call(file, call.callee, lookup);
        let entry = (
            owner_span,
            ParameterMemberInvocation {
                parameter,
                path,
                in_owner_body,
                site: call.callee,
                primitive_builtin,
                dispatch_read_excluded: false,
                unmodeled: false,
            },
        );
        if !contribution.invoked_parameter_members.contains(&entry) {
            contribution.invoked_parameter_members.push(entry);
        }
    }
}

fn record_forwarded_parameter_invocations(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    g: &GraphCall<'_>,
) {
    let file = scope.file;
    let nodes = scope.nodes;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    let GraphCall {
        call,
        owner,
        owner_span,
        ambiguous_dispatch,
        project_function,
        ..
    } = *g;
    // ADR 0285: `function f(cb) { g(cb) }`, where `g` calls that position
    // on every completion, invokes `cb` as `cb()` would. Only the owner's
    // own parameter, passed unchanged at an exactly mapped position.
    if project_function && !ambiguous_dispatch {
        for (position, argument) in call.arguments.iter().enumerate() {
            if argument.spread {
                break;
            }
            let span = file.ast.peel_ts_sugar_span(argument.span);
            if file
                .ast
                .identifiers
                .binary_search_by_key(&span, |identifier| identifier.span)
                .is_err()
            {
                continue;
            }
            let Some(owner_parameter) = symbols
                .entities
                .get(&location(file.path.shared(), span))
                .and_then(|argument_symbol| {
                    nodes[owner]
                        .parameters
                        .iter()
                        .position(|candidate| candidate == argument_symbol)
                })
            else {
                continue;
            };
            // Summary parameter indices skip destructured parameters;
            // the completion table's do not. Admit only where they agree.
            let aligned = file
                .ast
                .function_at(nodes[owner].span)
                .is_some_and(|function| {
                    function
                        .parameters
                        .iter()
                        .take(owner_parameter + 1)
                        .all(|parameter| {
                            parameter.shape == solid_facts::ast::BindingShape::Identifier
                        })
                });
            let forwarded = aligned && forwarded_answer(file, call, position, lookup);
            if forwarded
                && !contribution
                    .invoked_parameters
                    .contains(&(owner_span, owner_parameter))
            {
                contribution
                    .invoked_parameters
                    .push((owner_span, owner_parameter));
            }
        }
    }
}

/// A call of the owner's own parameter. `true` when the call was handed on to a
/// local scheduler, which ends the call's discovery.
fn record_parameter_callee_invocation(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    g: &GraphCall<'_>,
    (callback_owner, parameter): (usize, usize),
) -> bool {
    let file = scope.file;
    let nodes = scope.nodes;
    let lookup = scope.lookup;
    let function_lookup = &scope.function_lookup;
    let call = g.call;
    let owner_span = g.owner_span;
    contribution
        .invoked_parameters
        .push((owner_span, parameter));
    let mut forwarded_to_local_scheduler = false;
    for (scheduler, scheduler_parameter) in
        file.ast
            .arguments_containing(call.callee)
            .filter(|(scheduler, index)| {
                direct_callback_contains(file, scheduler.arguments[*index].span, call.callee)
            })
    {
        let Some(scheduler_symbol) = lookup.callee_symbol(file, scheduler.callee) else {
            continue;
        };
        let Some(target) = function_lookup
            .by_symbol
            .get(scheduler_symbol)
            .map(|index| &nodes[*index])
        else {
            continue;
        };
        contribution.callback_forwardings.push((
            nodes[callback_owner].span,
            target.symbol.clone().map_or(
                InterproceduralGraphTarget::LocalSpan(target.span),
                InterproceduralGraphTarget::Symbol,
            ),
            scheduler_parameter,
            parameter,
            ForwardedAmbientExecution::Callee,
        ));
        forwarded_to_local_scheduler = true;
    }
    if forwarded_to_local_scheduler {
        return true;
    }
    // `parameter()` is synchronous relative to its immediate
    // containing function, but that function may itself be a returned
    // adapter or a timer/Promise callback. Classify the complete
    // execution context before falling back to inline; otherwise a
    // debouncer summary incorrectly promises to invoke its callback
    // before the debouncer factory returns.
    record_invoked_parameter_callback(scope, contribution, g, (callback_owner, parameter));
    false
}

fn record_invoked_parameter_callback(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    g: &GraphCall<'_>,
    (callback_owner, parameter): (usize, usize),
) {
    let file = scope.file;
    let nodes = scope.nodes;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    let accessors = scope.accessors;
    let allowed = &scope.allowed;
    let unwritten_parameters = &scope.unwritten_parameters;
    let call = g.call;
    let invocation_owner = g.owner;
    let runtime_execution = file
        .ast
        .arguments_containing(call.callee)
        .filter(|(scheduler, index)| {
            direct_callback_contains(file, scheduler.arguments[*index].span, call.callee)
        })
        .filter_map(|(scheduler, index)| {
            let resolved = lookup.resolved_callee_call(file, scheduler.callee)?;
            let callability = lookup.smallest_contained_callability(
                file.path.as_str(),
                scheduler.arguments[index].span,
            );
            argument_behavior(resolved, callability, index)
        })
        // The word, and whether the scheduler is proven to run the
        // callback on a fresh stack. A deferral that is not -- an event
        // listener a synchronous `dispatchEvent` can run, a bound
        // argument -- outranks one that is, so the clearing is only
        // claimed when every deferring scheduler here proves it.
        .fold(None, |observed, behavior| match (behavior, observed) {
            (RuntimeArgumentBehavior::DeferredCallback, _) => Some(("deferred", false)),
            (RuntimeArgumentBehavior::FreshStackCallback, Some(("deferred", false))) => observed,
            (RuntimeArgumentBehavior::FreshStackCallback, _) => Some(("deferred", true)),
            (RuntimeArgumentBehavior::InlineCallback, None) => Some(("inline", false)),
            (
                RuntimeArgumentBehavior::InlineCallback
                | RuntimeArgumentBehavior::ValueOnly
                | RuntimeArgumentBehavior::RetainedValue,
                _,
            ) => observed,
        });
    let semantic = semantic_execution_role(
        file,
        call.callee,
        allowed,
        symbols.entities,
        symbols.names,
        lookup,
    );
    // An "inline" row promises the export invokes the callback before
    // it returns, and that promise is about the *export*. Neither rung
    // below establishes it across a closure boundary: `semantic`
    // classifies the lexical region (a capitalized function's body is
    // `UntrackedRendering`, i.e. "inline"), and `direct_callee`
    // classifies the call shape. So
    // `f(props, cb) { helper(props, () => cb(1)) }` -- where `helper`
    // may invoke the arrow later, once, or never -- and
    // `f(cb) { return { g: () => cb(1) } }` both published
    // `execution: "inline"`, a promise the package does not keep.
    //
    // The boundary is read off the AST rather than off
    // `invocation_owner`, so it does not depend on which function
    // shapes the summary-node universe carries.
    let call_in_owner_body = crate::execution_role::functions_around(file, call.span)
        .into_iter()
        .min_by_key(|function| function.span.end - function.span.start)
        // A call in the owner's own parameter list runs only when the caller
        // leaves that argument out, so it is no call "in the body": the body
        // must hold the call, not just belong to the function that does.
        .is_some_and(|innermost| {
            innermost.body == nodes[callback_owner].body
                && nodes[callback_owner].body.contains(call.span)
        });
    // Every primitive callback position between this call and the
    // declaring function's body, composed. This rung outranks the
    // lexical role below it because the lexical role answers a
    // different question: `semantic` reports the *tracking scope* the
    // call is written in, and an execution row states the schedule
    // relative to the export's return. The two disagree in both
    // directions -- `untrack(() => cb())` is written outside tracking
    // and runs during the call, `createEffect(() => untrack(cb))` is
    // written inside a tracked region and runs after it -- and both
    // disagreements were published as claims.
    //
    // The chain is only usable when its outermost wrapping call sits
    // in the declaring function's own body: otherwise the closure
    // holding the chain may itself never run, which is exactly the
    // boundary the rungs below were added for.
    //
    // The outer `Option` is "is there a usable chain"; the inner one is
    // the chain's own answer, and a `None` there is authoritative. A
    // usable chain that composes to an open callback leaf must not fall
    // through to the rungs below: those answer the lexical question,
    // which is the answer this rung exists to replace, so falling
    // through would publish exactly the claim the chain just refused.
    //
    // The wrappers are kept beside the word, not just the word: the
    // word `tracked` carries no schedule column, so the row's schedule
    // has to be recomposed from the same wrappers that produced it (see
    // the push below).
    let chain: Option<ComposedChain> = enclosing_callback_chain(file, call.callee, lookup)
        .filter(|chain| !chain.wrappers.is_empty())
        .filter(|chain| callback_chain_reaches_owner_body(file, chain, &nodes[callback_owner]))
        .map(|chain| (compose_callback_chain(&chain.wrappers), chain.wrappers));
    let chain_execution: Option<Option<&'static str>> = chain
        .as_ref()
        .map(|(composed, _)| composed.map(|(word, _)| word));
    // Only a composed chain can prove a clearing wrapper stood between
    // the export and the callback. A runtime-census word and the
    // lexical rungs below answer the schedule and say nothing about the
    // listener, so they leave this false -- which publishes
    // `ambient-at-execution` rather than an unproven `untracked`.
    let chain_detaches = chain
        .as_ref()
        .and_then(|(composed, _)| composed.map(|(_, detached)| detached))
        .unwrap_or(false);
    // ADR 0100: whether *the site* is a call of the parameter itself,
    // written directly in the body of the function that declares it.
    // That is the one row the implementation census confirms site for
    // site, so it is recorded beside the row
    // (`direct_callback_parameters`) rather than in it: the wire has
    // one word, `inline`, for this and for a primitive's inline
    // position alike.
    //
    // It is read off the call, not set inside whichever rung below
    // happened to name the schedule. Recording it only in the
    // last-resort arm made it a fact about the derivation instead of
    // about the site, and silently under-proposed: a capitalized
    // export whose body calls its own parameter takes
    // `UntrackedRendering` -> `inline` from the lexical rung two arms
    // earlier, publishes the identical row, and never recorded that
    // the site was a direct own call -- so the census could confirm
    // the enumeration and the generator never proposed it. Widening
    // this cannot over-propose, because
    // `callbacks_enumeration_is_confirmable` independently requires
    // the published operation to be an untracked same-stack invoke at
    // the call event: a `deferred` or `tracked` word from an earlier
    // rung still fails there.
    let direct_own_call = call.direct_callee && call_in_owner_body;
    // A fresh-stack host deferral clears the listener by itself,
    // whatever surrounds the scheduling call; any other runtime word
    // leaves the chain's answer (none, for a host scheduler, which is
    // no wrapper) in place.
    let clears_tracking = runtime_execution.map_or(chain_detaches, |(_, fresh)| fresh);
    let runtime_execution = runtime_execution.map(|(word, _)| word);
    let execution = match (runtime_execution, chain_execution) {
        (Some(execution), _) => Some(execution),
        (None, Some(composed)) => composed,
        (None, None) => callback_execution_word(semantic)
            // A lexical role is an answer about where this call is
            // written, not whether an intervening nested helper runs
            // with that role relative to the exported call. This was
            // already enforced for `inline`; real package probes show
            // the same false implication for `tracked` (a nested
            // request starter under mapArray). Only an explicit
            // wrapper chain may cross the function boundary.
            .filter(|_| call_in_owner_body)
            .or_else(|| {
                function_escapes_through_return(
                    file,
                    &nodes[invocation_owner],
                    &nodes[callback_owner],
                    symbols.entities,
                    lookup,
                )
                .then_some("deferred")
            })
            // Last resort, and only for a call written directly in the
            // body of the function that declares the parameter. When no
            // rung can classify the enclosing schedule, no row is
            // written and the unknown-callback obligation opens the
            // sentinel instead.
            .or_else(|| direct_own_call.then_some("inline")),
    };
    // ADR 0183: a tracked row whose one wrapper is an eager owned
    // computation's slot, holding the literal that calls the parameter
    // on every completion of its own body (`createMemo(() => fn())`),
    // runs under the owner that computation creates.
    let owned = runtime_execution.is_none()
        && execution == Some("tracked")
        && call.direct_callee
        && chain
            .as_ref()
            .is_some_and(|(_, wrappers)| wrappers.len() == 1)
        && wrapped_owned_computation_call(file, call, lookup).is_some();
    let guaranteed = owned
        && wrapped_owned_computation_call(file, call, lookup).is_some_and(|computation| {
            solid_facts::ast::completion_call_cover(
                std::path::Path::new(file.path.as_str()),
                &file.source,
                nodes[callback_owner].body,
                &[computation.span],
                &[],
            ) == Some(true)
        });
    if guaranteed {
        contribution
            .guaranteed_callback_parameters
            .push((nodes[callback_owner].span, parameter));
    }
    if let Some(execution) = execution {
        // Recorded only for a binding nothing writes: `cb = other;
        // cb()` is a call of `cb` by syntax and of `other` by value,
        // and the census refuses a described call of a parameter the
        // producer does not state unwritten. The row itself is still
        // written; only the closed enumeration is not proposed.
        if direct_own_call
            && unwritten_parameters.contains(&(nodes[callback_owner].span, parameter))
        {
            contribution
                .direct_callback_parameters
                .push((nodes[callback_owner].span, parameter));
        }
        contribution.callbacks.push((
            nodes[callback_owner].span,
            CallbackSummary {
                parameter,
                // Per word: a composed chain's clearing, or a
                // fresh-stack host deferral's. Every other rung --
                // the returned-closure escape, the lexical role, the
                // last-resort own call, an event listener or bound
                // argument -- answers the schedule alone and proves no
                // clearing, so it publishes `ambient-at-execution`.
                clears_tracking: clears_tracking && execution != "tracked",
                execution: execution.into(),
                // `inline` and `deferred` carry their schedule in the
                // word. `tracked` does not, and it does reach here --
                // an enclosing wrapper chain can compose to it, which
                // the comment that used to stand here denied. Leaving
                // the column empty then took the consumer's `queued`
                // default, publishing "runs after the export returns"
                // for every tracked wrapper the dialect audits as
                // running *during* its own call. With 1.x retired that
                // is every 2.0 tracked primitive but
                // `createTrackedEffect`.
                //
                // Only a word the chain produced gets a schedule, and
                // it is recomposed from that chain's own wrappers. A
                // `tracked` word from the lexical rung below has no
                // wrappers to compose and keeps the historical default:
                // `composed_tracked_schedule(&[])` would answer
                // `same-stack`, which is a claim nothing here proves.
                schedule: (execution == "tracked")
                    .then(|| {
                        chain
                            .as_ref()
                            .map(|(_, wrappers)| composed_tracked_schedule(wrappers))
                    })
                    .flatten(),
                arguments: accessor_argument_summaries(file, call, symbols.entities, accessors),
                owner: owned.then(|| "created".into()),
                protocol: crate::summary_semantics::InvokeProtocol::Call,
                path: Vec::new(),
            },
        ));
    } else {
        // The invocation is real, but its execution relative to the
        // exported call is not classifiable. An omitted callbacks row
        // would be the false negative "never invokes this parameter";
        // open the per-export callbacks sentinel as well as recording
        // the attribution obligation below.
        contribution
            .escaped_parameters
            .push((nodes[callback_owner].span, parameter));
        contribution.unresolved_callback_obligations.push((
            nodes[callback_owner].span,
            unknown_callback_obligation(
                file,
                &nodes[callback_owner],
                call.callee,
                parameter,
                location(file.path.shared(), call.callee),
                lookup,
            ),
        ));
    }
}

/// One argument of a call, when it hands the owner's own parameter on.
struct ArgumentCallback<'a> {
    call: &'a solid_facts::ast::CallFact,
    argument: &'a solid_facts::ast::ArgumentFact,
    argument_index: usize,
    callback_owner: usize,
    parameter: usize,
    callability: Option<typefacts::Callability>,
}

fn discover_argument_graph(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    g: &GraphCall<'_>,
    symbol: &str,
    (argument_index, argument): (usize, &solid_facts::ast::ArgumentFact),
) {
    let file = scope.file;
    let nodes = scope.nodes;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    let primitives = scope.primitives;
    let function_lookup = &scope.function_lookup;
    let returned_function_targets = &scope.returned_function_targets;
    let ast_parameter_symbols = &scope.ast_parameter_symbols;
    let call = g.call;
    let call_index = g.call_index;
    let ambiguous_dispatch = g.ambiguous_dispatch;
    let runtime_argument_callability =
        lookup.smallest_contained_callability(file.path.as_str(), argument.span);
    if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
        return;
    }
    let Some(argument_symbol) = symbols
        .entities
        .get(&location(file.path.shared(), argument.span))
    else {
        return;
    };
    let callback_owner_and_parameter = function_lookup
        .parameter_owner
        .get(argument_symbol)
        .copied();
    let Some((callback_owner, parameter)) = callback_owner_and_parameter else {
        return;
    };
    let site = ArgumentCallback {
        call,
        argument,
        argument_index,
        callback_owner,
        parameter,
        callability: runtime_argument_callability,
    };
    if !ambiguous_dispatch
        && let Some(target) = function_lookup
            .by_symbol
            .get(symbol)
            .copied()
            .or_else(|| returned_function_targets.get(symbol).copied())
    {
        // Local calls are summarized transitively. If the parameter
        // later reaches an unknown external call, that call creates
        // the obligation at the actual escape point.
        let ambient = forwarded_callback_ambient_execution(
            file,
            call,
            argument_index,
            &nodes[callback_owner],
            lookup,
        );
        // A chain that refuses to compose cannot restate the callee's
        // `inline` rows in export-relative terms, so the sentinel opens
        // here rather than in the propagation loop, which has no file
        // or call to build an obligation from. It opens even when the
        // callee turns out to publish no `inline` row for the slot --
        // a conservative precision cost when the enclosing execution
        // is not established.
        if ambient == ForwardedAmbientExecution::Unknown {
            contribution
                .escaped_parameters
                .push((nodes[callback_owner].span, parameter));
            contribution.unresolved_callback_obligations.push((
                nodes[callback_owner].span,
                unknown_callback_obligation(
                    file,
                    &nodes[callback_owner],
                    call.callee,
                    parameter,
                    location(file.path.shared(), call.callee),
                    lookup,
                ),
            ));
        }
        contribution.callback_forwardings.push((
            nodes[callback_owner].span,
            nodes[target].symbol.clone().map_or(
                InterproceduralGraphTarget::LocalSpan(nodes[target].span),
                InterproceduralGraphTarget::Symbol,
            ),
            argument_index,
            parameter,
            ambient,
        ));
        return;
    }
    let primitive = crate::known_primitive(&primitives.calls[call_index]);
    if record_primitive_slot_callback(scope, contribution, &site, primitive) {
        return;
    }
    // A props split only creates property views. Its source and key
    // lists are values even when erased JavaScript types leave their
    // callability unknown. Asked of the dialect because 1.x's
    // `splitProps` is 2.0's `omit`, and naming only the 1.x spelling
    // raised this obligation about 2.0's key lists.
    //
    // Except where the split itself declares the argument a callback:
    // rc.9's `omit(props, hidden)` calls `hidden` on every read of the
    // view it returns. No execution word states that, so the argument
    // falls through to the unknown-callback arm below instead of
    // being published as a value the export never invokes.
    if primitive.is_some_and(|primitive| {
        lookup.dialect.splits_props(primitive)
            && !lookup.dialect.callback_runs_on_result_access(
                primitive,
                argument_index,
                call.arguments.len(),
            )
    }) {
        return;
    }
    if record_runtime_argument_behavior(scope, contribution, &site) {
        return;
    }
    if !potentially_callable(runtime_argument_callability) {
        return;
    }
    if function_lookup.parameter_owner.contains_key(symbol)
        || ast_parameter_symbols.contains(symbol)
    {
        // Calling a function-valued parameter with another parameter
        // invokes the callee parameter, not the value argument. Any
        // nested use belongs to the caller-provided callback.
        return;
    }
    contribution.unresolved_callback_obligations.push((
        nodes[callback_owner].span,
        unknown_callback_obligation(
            file,
            &nodes[callback_owner],
            call.callee,
            parameter,
            location(file.path.shared(), argument.span),
            lookup,
        ),
    ));
}

/// A callback position of a dialect primitive. `true` when the argument is one.
fn record_primitive_slot_callback(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    site: &ArgumentCallback<'_>,
    primitive: Option<Primitive>,
) -> bool {
    let file = scope.file;
    let nodes = scope.nodes;
    let lookup = scope.lookup;
    let ArgumentCallback {
        call,
        argument_index,
        callback_owner,
        parameter,
        callability: runtime_argument_callability,
        ..
    } = *site;
    if let Some(execution) = primitive_callback_execution(
        primitive,
        argument_index,
        call.arguments.len(),
        lookup.dialect,
    ) {
        // A row in that table says how a callback at this slot would run
        // relative to the export. It does not say there *is* a callback
        // at this slot, and this branch used to publish the row as
        // though it did -- see
        // [`primitive_slot_roots_parameter_invoke`], which carries the
        // three premises and the two ecosystem over-claims that came
        // from their absence. When the premises fail the slot is not a
        // callback position for this parameter, so no row is written and
        // no unknown-callback obligation is opened either: this is a
        // proven absence, not an unclassified invocation.
        if !primitive_slot_roots_parameter_invoke(
            lookup.dialect,
            primitive,
            argument_index,
            call.arguments.len(),
            runtime_argument_callability,
        ) {
            return true;
        }
        // The primitive's own slot says how the callback runs relative
        // to *this* call; the row says how it runs relative to the
        // export. `onMount(fn) { createEffect(() => untrack(fn)) }` is
        // the shape where those differ: `untrack`'s slot is inline,
        // and the enclosing `createEffect` has not run it by the time
        // `onMount` returns. Composing the enclosing chain answers the
        // second question; an unclassifiable enclosing position leaves
        // the slot's own answer standing, which is what this branch
        // always published.
        //
        // Two levels of `Option`, and they mean different things. The
        // outer one is "was there a usable chain to compose", and its
        // `None` falls back to the slot's own answer. The inner one is
        // the composition itself, and its `None` is the unknown
        // sentinel -- a usable chain that refuses to answer must open
        // the sentinel rather than fall back, because the slot's answer
        // is relative to the wrapping call and the row is relative to
        // the export.
        let composed: Option<ComposedChain> = enclosing_callback_chain(file, call.span, lookup)
            .filter(|chain| {
                chain.wrappers.is_empty()
                    || callback_chain_reaches_owner_body(file, chain, &nodes[callback_owner])
            })
            .and_then(|chain| {
                let mut wrappers = vec![callback_wrapper_at(file, call, argument_index, lookup)?];
                wrappers.extend(chain.wrappers);
                Some((compose_callback_chain(&wrappers), wrappers))
            });
        // ADR 0183: only an eager owned-computation slot states an owner
        // for the row. With no enclosing callback position -- no chain,
        // or one whose only wrapper is this slot's own -- the slot's
        // answer is the row's; inside literals that synchronous slots
        // (`createRoot`, `untrack`) run during the call, it is too. The
        // inner `bool` is whether every level covers its body, which is
        // what the row's `min: 1` rests on.
        let enclosure: Option<bool> = if !primitive.is_some_and(|slot| {
            lookup
                .dialect
                .eager_owned_computation_slot(slot, argument_index, call.arguments.len())
        }) {
            None
        } else if composed
            .as_ref()
            .is_none_or(|(_, wrappers)| wrappers.len() == 1)
        {
            Some(
                solid_facts::ast::completion_call_cover(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    nodes[callback_owner].body,
                    &[call.span],
                    &[],
                ) == Some(true),
            )
        } else {
            synchronous_enclosure(file, call.span, &nodes[callback_owner], lookup)
        };
        let eager_owned = enclosure.is_some();
        // The wrappers, not just their composed word: `tracked` is an
        // attribution word with no schedule column, and the schedule is
        // the dialect's to state for each wrapper the callback sits
        // under. Without the chain there is one wrapper -- this slot.
        let (word, wrappers) = match composed {
            Some((word, wrappers)) => (word, wrappers),
            None => {
                let slot = primitive.expect("a slot row requires a resolved primitive");
                // No enclosing chain, so the slot's own answer stands --
                // including whether it clears the listener, which is the
                // same question `callback_wrapper_for` asks to call a
                // wrapper `Detaching`. `untrack`'s slot is inline *and*
                // detaching, and publishing it as merely inline is what
                // made every row's `untracked` unfalsifiable.
                //
                // The same dialect answer settles a *deferred* slot:
                // `createReaction`'s invalidation callback runs
                // untracked when it runs. A deferred slot the dialect
                // says nothing of -- `onCleanup`, `onSettled`, an
                // effect's apply -- stays `ambient-at-execution`, and a
                // `tracked` slot's word is its whole claim.
                let detaches = execution != "tracked"
                    && (lookup.dialect.runs_callback_synchronously(slot)
                        || lookup.dialect.reports_untracked_reads_at(
                            slot,
                            argument_index,
                            call.arguments.len(),
                        ));
                (
                    Some((execution, detaches)),
                    vec![CallbackWrapper::Tracked(
                        lookup.dialect.tracked_callback_timing(
                            slot,
                            argument_index,
                            call.arguments.len(),
                        ),
                    )],
                )
            }
        };
        if let Some((execution, detaches)) = word {
            let owned = eager_owned && execution == "tracked";
            // The lower bound is proposed only where the call covers
            // every normal completion of the owner's own body; the
            // census proves both again from the producer's facts.
            if owned && enclosure == Some(true) {
                contribution
                    .guaranteed_callback_parameters
                    .push((nodes[callback_owner].span, parameter));
            }
            contribution.callbacks.push((
                nodes[callback_owner].span,
                CallbackSummary {
                    parameter,
                    execution: execution.into(),
                    schedule: (execution == "tracked")
                        .then(|| composed_tracked_schedule(&wrappers)),
                    clears_tracking: detaches,
                    arguments: Vec::new(),
                    owner: owned.then(|| "created".into()),
                    protocol: crate::summary_semantics::InvokeProtocol::Call,
                    path: Vec::new(),
                },
            ));
        } else {
            contribution.unresolved_callback_obligations.push((
                nodes[callback_owner].span,
                unknown_callback_obligation(
                    file,
                    &nodes[callback_owner],
                    call.callee,
                    parameter,
                    location(file.path.shared(), call.callee),
                    lookup,
                ),
            ));
        }
        return true;
    }
    false
}

/// The runtime behavior of the callee's argument slot. `true` when it decided
/// the argument.
fn record_runtime_argument_behavior(
    scope: &GraphScope<'_, '_>,
    contribution: &mut InterproceduralGraphContribution,
    site: &ArgumentCallback<'_>,
) -> bool {
    let file = scope.file;
    let nodes = scope.nodes;
    let symbols = scope.symbols;
    let lookup = scope.lookup;
    let ArgumentCallback {
        call,
        argument,
        argument_index,
        callback_owner,
        parameter,
        callability: runtime_argument_callability,
    } = *site;
    let resolved_call = lookup.resolved_callee_call(file, call.callee);
    let runtime_behavior = resolved_call
        .and_then(|resolved_call| {
            argument_behavior(resolved_call, runtime_argument_callability, argument_index)
        })
        .or_else(|| {
            proven_array_method(file, call, symbols.entities).and_then(|method| {
                proven_array_method_argument_behavior(method, runtime_argument_callability)
            })
        })
        .or_else(|| {
            resolved_call
                .is_some_and(|resolved_call| {
                    stored_constructor_argument_escapes(
                        file,
                        call,
                        resolved_call,
                        argument_index,
                        lookup,
                    )
                })
                .then_some(RuntimeArgumentBehavior::RetainedValue)
        })
        .or_else(|| {
            // Even when a structurally typed method has no inspectable
            // implementation, invoking it from a callback that itself
            // escapes the exported call proves that any invocation of the
            // forwarded callable cannot happen inline with that export.
            assigned_member_function_contains(file, call.callee, symbols.entities)
                .then_some(RuntimeArgumentBehavior::DeferredCallback)
        });
    if let Some(runtime_behavior) = runtime_behavior {
        match runtime_behavior {
            RuntimeArgumentBehavior::InlineCallback
            | RuntimeArgumentBehavior::DeferredCallback
            | RuntimeArgumentBehavior::FreshStackCallback => {
                contribution.callbacks.push((
                    nodes[callback_owner].span,
                    CallbackSummary {
                        parameter,
                        execution: match runtime_behavior {
                            RuntimeArgumentBehavior::InlineCallback => "inline",
                            RuntimeArgumentBehavior::DeferredCallback
                            | RuntimeArgumentBehavior::FreshStackCallback => "deferred",
                            RuntimeArgumentBehavior::ValueOnly
                            | RuntimeArgumentBehavior::RetainedValue => unreachable!(),
                        }
                        .into(),
                        schedule: None,
                        // The runtime table times the invocation. Only
                        // its reviewed fresh-stack subset also settles
                        // the listener: nothing of any caller's can be
                        // current on an empty stack. An inline runtime
                        // callback, an event listener, a bound argument
                        // or a structurally typed escape proves no
                        // clearing, and publishes
                        // `ambient-at-execution`.
                        clears_tracking: runtime_behavior.runs_on_fresh_stack(),
                        arguments: Vec::new(),
                        owner: None,
                        protocol: crate::summary_semantics::InvokeProtocol::Call,
                        path: Vec::new(),
                    },
                ));
            }
            RuntimeArgumentBehavior::RetainedValue => {
                contribution
                    .escaped_parameters
                    .push((nodes[callback_owner].span, parameter));
                contribution.unresolved_callback_obligations.push((
                    nodes[callback_owner].span,
                    unknown_callback_obligation(
                        file,
                        &nodes[callback_owner],
                        call.callee,
                        parameter,
                        location(file.path.shared(), argument.span),
                        lookup,
                    ),
                ));
            }
            RuntimeArgumentBehavior::ValueOnly => {}
        }
        return true;
    }
    false
}

pub(super) struct InterproceduralGraphAssembly<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) nodes_by_path: &'a HashMap<String, Vec<usize>>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) summaries: &'a mut [SummaryReads],
    pub(super) callback_summaries: &'a mut [Vec<CallbackSummary>],
    pub(super) callback_forwardings:
        &'a mut Vec<(usize, usize, usize, usize, ForwardedAmbientExecution)>,
    pub(super) dispatches: &'a mut Vec<(usize, Vec<usize>)>,
    pub(super) unresolved_callback_obligations: &'a mut [Vec<UnresolvedCallbackObligation>],
    pub(super) edges: &'a mut [Vec<usize>],
    pub(super) guaranteed_edges: &'a mut HashSet<(usize, usize)>,
    pub(super) invoked_parameters: &'a mut [Vec<usize>],
    pub(super) direct_callback_parameters: &'a mut [Vec<usize>],
    pub(super) guaranteed_callback_parameters: &'a mut [Vec<usize>],
    pub(super) direct_protocol_parameters:
        &'a mut [Vec<(crate::summary_semantics::InvokeProtocol, usize)>],
    pub(super) direct_member_callback_parameters: &'a mut [Vec<(usize, Vec<String>)>],
    pub(super) escaped_parameters: &'a mut [Vec<usize>],
    pub(super) invoked_parameter_members: &'a mut [Vec<ParameterMemberInvocation>],
    pub(super) returned_bindings: &'a mut Vec<(SymbolId, SymbolId)>,
    pub(super) factory_calls: &'a mut Vec<(usize, SymbolId)>,
}

impl InterproceduralGraphAssembly<'_> {
    pub(super) fn merge(&mut self, path: &str, contribution: &InterproceduralGraphContribution) {
        // The file's last node at a span (a later duplicate wins).
        let by_span = self
            .nodes_by_path
            .get(path)
            .into_iter()
            .flatten()
            .map(|index| (self.nodes[*index].span, *index))
            .collect::<HashMap<_, _>>();
        let node_index = |span| by_span.get(&span).copied();
        for (owner, read) in &contribution.direct_reads {
            if let Some(owner) = node_index(*owner) {
                self.summaries[owner].push_unique(read.clone());
            }
        }
        for (owner, target) in &contribution.edges {
            let Some(owner) = node_index(*owner) else {
                continue;
            };
            let target = match target {
                InterproceduralGraphTarget::Symbol(symbol) => self.by_symbol.get(symbol).copied(),
                InterproceduralGraphTarget::LocalSpan(span) => node_index(*span),
            };
            if let Some(target) = target {
                self.edges[owner].push(target);
            }
        }
        for (owner, target) in &contribution.guaranteed_edges {
            let Some(owner) = node_index(*owner) else {
                continue;
            };
            let target = match target {
                InterproceduralGraphTarget::Symbol(symbol) => self.by_symbol.get(symbol).copied(),
                InterproceduralGraphTarget::LocalSpan(span) => node_index(*span),
            };
            if let Some(target) = target {
                self.guaranteed_edges.insert((owner, target));
            }
        }
        for (owner, parameter) in &contribution.invoked_parameters {
            if let Some(owner) = node_index(*owner) {
                self.invoked_parameters[owner].push(*parameter);
            }
        }
        for (owner, parameter) in &contribution.direct_callback_parameters {
            if let Some(owner) = node_index(*owner)
                && !self.direct_callback_parameters[owner].contains(parameter)
            {
                self.direct_callback_parameters[owner].push(*parameter);
            }
        }
        for (owner, parameter) in &contribution.guaranteed_callback_parameters {
            if let Some(owner) = node_index(*owner)
                && !self.guaranteed_callback_parameters[owner].contains(parameter)
            {
                self.guaranteed_callback_parameters[owner].push(*parameter);
            }
        }
        for (owner, protocol, parameter) in &contribution.direct_protocol_parameters {
            if let Some(owner) = node_index(*owner)
                && !self.direct_protocol_parameters[owner].contains(&(*protocol, *parameter))
            {
                self.direct_protocol_parameters[owner].push((*protocol, *parameter));
            }
        }
        for (owner, parameter, path) in &contribution.direct_member_callback_parameters {
            if let Some(owner) = node_index(*owner) {
                let entry = (*parameter, path.clone());
                if !self.direct_member_callback_parameters[owner].contains(&entry) {
                    self.direct_member_callback_parameters[owner].push(entry);
                }
            }
        }
        for (owner, parameter) in &contribution.escaped_parameters {
            if let Some(owner) = node_index(*owner)
                && !self.escaped_parameters[owner].contains(parameter)
            {
                self.escaped_parameters[owner].push(*parameter);
            }
        }
        for (owner, invocation) in &contribution.invoked_parameter_members {
            if let Some(owner) = node_index(*owner) {
                let entry = invocation.clone();
                if !self.invoked_parameter_members[owner].contains(&entry) {
                    self.invoked_parameter_members[owner].push(entry);
                }
            }
        }
        for (owner, callback) in &contribution.callbacks {
            if let Some(owner) = node_index(*owner) {
                push_callback_summary(&mut self.callback_summaries[owner], callback.clone());
            }
        }
        for (owner, target, target_parameter, owner_parameter, ambient_execution) in
            &contribution.callback_forwardings
        {
            let target = match target {
                InterproceduralGraphTarget::Symbol(symbol) => self.by_symbol.get(symbol).copied(),
                InterproceduralGraphTarget::LocalSpan(span) => node_index(*span),
            };
            if let (Some(owner), Some(target)) = (node_index(*owner), target) {
                self.callback_forwardings.push((
                    owner,
                    target,
                    *target_parameter,
                    *owner_parameter,
                    ambient_execution.clone(),
                ));
            }
        }
        for (owner, candidates) in &contribution.dispatches {
            let Some(owner) = node_index(*owner) else {
                continue;
            };
            let mut resolved = Vec::with_capacity(candidates.len());
            let mut complete = true;
            for symbol in candidates {
                if let Some(candidate) = self.by_symbol.get(symbol).copied() {
                    resolved.push(candidate);
                } else {
                    // A candidate omitted from the project graph is not
                    // equivalent to the candidates we did resolve. Keep the
                    // dispatch unresolved instead of silently narrowing the
                    // runtime set.
                    complete = false;
                    break;
                }
            }
            resolved.sort_unstable();
            resolved.dedup();
            if complete && !resolved.is_empty() && resolved.len() == candidates.len() {
                self.dispatches.push((owner, resolved));
            }
        }
        for (owner, obligation) in &contribution.unresolved_callback_obligations {
            if let Some(owner) = node_index(*owner)
                && !self.unresolved_callback_obligations[owner].contains(obligation)
            {
                self.unresolved_callback_obligations[owner].push(obligation.clone());
            }
        }
        self.returned_bindings
            .extend(contribution.returned_bindings.iter().cloned());
        for (owner, symbol) in &contribution.factory_calls {
            if let Some(owner) = node_index(*owner) {
                self.factory_calls.push((owner, symbol.clone()));
            }
        }
    }
}
