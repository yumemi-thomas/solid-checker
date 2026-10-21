//! Per-file result reads: the reads a file's functions contribute once the graph is solved.

use super::*;

pub(crate) struct InterproceduralResultReadContext<'a, 'b> {
    pub(crate) result: InterproceduralResultView<'a>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) lookup: &'a SemanticLookup<'b>,
    pub(super) transported_calls: &'a HashMap<Location, accessor_provenance::Callable>,
    pub(super) unavailable_calls: &'a HashSet<Location>,
    pub(super) array_member_bodies: &'a HashSet<(String, Span)>,
    pub(super) provenance: &'a accessor_provenance::Resolver<'a, 'b>,
}

/// A local helper can inherit the dispatch rule's existing caller exclusions.
/// Tracked expressions need no untracked-read dispatch proof. A compiler-proven
/// component-property getter runs at its consumer, where reads are projected,
/// rather than in the JSX-building function. Events, deferred callbacks and
/// discarded code share the direct dispatch rule's exclusions. These answers
/// select no member implementation and install no execution role.
pub(super) fn local_dispatch_exclusion_depth(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
    following: &mut HashSet<Span>,
    cache: &mut HashMap<Span, usize>,
) -> Option<usize> {
    use solid_facts::ast::{FunctionKind, IdentifierRole};

    // Parameter initializers run when their function is invoked, not while
    // its surrounding JSX expression or property getter builds that function.
    if file
        .ast
        .functions
        .iter()
        .any(|function| function.span.contains(span) && !function.body.contains(span))
    {
        return None;
    }
    let role = semantic_execution_role(
        file,
        span,
        allowed,
        lookup.entities(),
        lookup.symbol_names(),
        lookup,
    );
    if crate::execution_role::excluded_role_holds(file, span, role, lookup) {
        return Some(0);
    }
    if !matches!(
        role,
        ExecutionRole::Unknown | ExecutionRole::Unreported | ExecutionRole::UntrackedRendering
    ) || file.ast.identifiers.iter().any(|id| {
        // Direct eval can call a local binding without an identifier use.
        // This spelling only refuses proof, including a shadowed eval.
        id.role == IdentifierRole::Reference && file.source_text(id.span) == Some("eval")
    }) {
        return None;
    }
    let helper = containing_ast_function(&file.ast, span)?;
    if !matches!(helper.kind, FunctionKind::Arrow | FunctionKind::Declaration)
        || helper.r#async
        || helper.generator
        || !helper.body.contains(span)
        || containing_ast_function(&file.ast, helper.span).is_none()
        || lookup.function_component_status(file, helper) != ComponentStatus::No
        || !lookup.function_value_is_current(file, helper)
    {
        return None;
    }
    if let Some(depth) = cache.get(&helper.span) {
        return (following.len() + depth <= 32).then_some(*depth);
    }
    if following.len() >= 32 || !following.insert(helper.span) {
        return None;
    }
    let proven = (|| {
        let binding = function_binding_name(file, helper)?;
        let symbol = lookup.entities().at(file.path.as_str(), binding.span)?;
        if file.ast.jsx_elements.iter().any(|element| {
            lookup.entities().at(file.path.as_str(), element.name.span) == Some(symbol)
                || file.ast.reference_declaration(element.name.span) == Some(binding.span)
        }) {
            return None;
        }
        // The AST census includes undemanded references. Union the Type Facts
        // index and refuse any reference that cannot be bound to this exact
        // declaration in this file. A missing reference table proves nothing.
        let mut references = file
            .ast
            .identifiers
            .iter()
            .filter(|id| {
                id.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(binding.span)
            })
            .map(|id| id.span)
            .collect::<HashSet<_>>();
        for reference in lookup.symbol_references(symbol.as_str()) {
            if reference.path.as_ref() != file.path.as_str() {
                return None;
            }
            let reference = Span::new(
                u32::try_from(reference.start_byte).ok()?,
                u32::try_from(reference.end_byte).ok()?,
            );
            if reference != binding.span {
                if file.ast.reference_declaration(reference) != Some(binding.span) {
                    return None;
                }
                references.insert(reference);
            }
        }
        if references.is_empty() {
            return None;
        }
        let mut depth = 1;
        for reference in references {
            // A stored value, alias, JSX tag, .call/.bind use or argument is
            // an escape. Only the exact identifier callee closes this census.
            file.ast
                .calls
                .iter()
                .find(|call| file.ast.peel_ts_sugar_span(call.callee) == reference)?;
            let caller_depth =
                local_dispatch_exclusion_depth(file, reference, allowed, lookup, following, cache)?;
            depth = depth.max(caller_depth + 1);
        }
        Some(depth)
    })();
    following.remove(&helper.span);
    if let Some(depth) = proven {
        // Cache positive proofs with their full depth, not a boolean that
        // could shorten a later traversal and make the bound order-dependent.
        cache.insert(helper.span, depth);
    }
    proven
}

pub(super) fn interprocedural_result_reads_for_file(
    file: &solid_facts::FileFacts,
    context: &InterproceduralResultReadContext<'_, '_>,
) -> (
    Vec<ReactiveRead>,
    Vec<StaticDefect>,
    HashSet<InterproceduralResultDependency>,
) {
    let scope = FileReadScope::new(file, context);
    let mut out = FileReadOutput::default();
    for call in &file.ast.calls {
        call_result_reads(&scope, &mut out, call);
    }
    (out.result, out.dispatch_obligations, out.dependencies)
}

/// What one file's result reads are computed against, fixed for the file.
struct FileReadScope<'a, 'b, 'c> {
    file: &'a solid_facts::FileFacts,
    context: &'a InterproceduralResultReadContext<'b, 'c>,
    allowed: Vec<Span>,
    /// ADR 0283 instantiates only getters this file writes in a literal.
    receives_getters: bool,
}

impl<'a, 'b, 'c> FileReadScope<'a, 'b, 'c> {
    fn new(
        file: &'a solid_facts::FileFacts,
        context: &'a InterproceduralResultReadContext<'b, 'c>,
    ) -> Self {
        let allowed = allowed_callback_spans(file, context.lookup);
        let receives_getters = file.ast.object_get_shapes.iter().any(|shape| {
            shape.closed
                && shape
                    .properties
                    .iter()
                    .any(|property| property.getter.is_some())
        });
        Self {
            file,
            context,
            allowed,
            receives_getters,
        }
    }
}

/// What the calls of one file accumulate.
#[derive(Default)]
struct FileReadOutput {
    result: Vec<ReactiveRead>,
    dispatch_obligations: Vec<StaticDefect>,
    dependencies: HashSet<InterproceduralResultDependency>,
    seen: HashSet<(std::sync::Arc<str>, u64, String)>,
    excluded_dispatch_helpers: HashMap<Span, usize>,
}

/// What a call was written against, before its summary is read.
struct CallSite<'a> {
    obligations_before: usize,
    callee: Location,
    transported: Option<&'a accessor_provenance::Callable>,
    label: String,
    valid_call: bool,
    candidate_symbols: Vec<SymbolId>,
    symbol: String,
}

impl CallSite<'_> {
    fn ambiguous_candidates(&self) -> Option<&Vec<SymbolId>> {
        (self.candidate_symbols.len() > 1).then_some(&self.candidate_symbols)
    }
}

fn call_result_reads(
    scope: &FileReadScope<'_, '_, '_>,
    out: &mut FileReadOutput,
    call: &solid_facts::ast::CallFact,
) {
    let file = scope.file;
    let allowed = &scope.allowed;
    let receives_getters = scope.receives_getters;
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                indexes: result_indexes,
                ..
            },
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup,
        provenance,
        ..
    } = scope.context;
    if !enclosing_render_function(file, call.span, lookup) {
        return;
    }
    if crate::object_members::call_withheld(file, call.callee, lookup) {
        push_withheld_member_obligation(scope, &mut out.dispatch_obligations, call);
        return;
    }
    let Some(site) = resolve_call_target(scope, out, call) else {
        return;
    };
    let Some((label, mut effective, target)) = call_summary(scope, out, call, &site) else {
        return;
    };
    if let Some(target) = target {
        add_invoked_parameter_reads(scope, out, call, &site, &label, target, &mut effective);
        add_parameter_member_reads(scope, out, call, &site, &label, target, &mut effective);
    }
    let obligations_before = site.obligations_before;
    let callee = &site.callee;
    let transported = site.transported;
    let valid_call = site.valid_call;
    let symbol = site.symbol.as_str();
    let ambiguous_candidates = site.ambiguous_candidates();
    let FileReadOutput {
        result,
        dispatch_obligations,
        dependencies,
        seen,
        excluded_dispatch_helpers,
    } = out;

    // A call written inside a helper nested in the component -- `const b =
    // () => a() * 2`, a nested function declaration -- runs when that
    // helper does, not while the component body does. The lexical role
    // below would charge the component body with it, so a derived chain
    // read only from JSX, a handler or a memo was reported as an untracked
    // read it never performs (about 573 of 1,962 measured findings). The
    // read belongs to the helper's own summary and is claimed where the
    // helper is *called*: the call in the body below, or the callback
    // position that runs it. The direct-read path asks the same question
    // (`LocalAccess::discover`), so the two paths agree on which function
    // a read is written in.
    let execution =
        semantic_execution_role(file, call.callee, allowed, entities, symbol_names, lookup);
    // ADR 0202: an unresolved dispatch decides a claim only where the
    // reads it hides could be reported. Reads at an event, a deferred
    // callback, a tracked JSX position or deleted code are reported by no
    // read rule (strict reads report the untracked roles; the conditional
    // return and result-access rules ask about a component body and a
    // predicate), so this call's obligations decide nothing. Asked ahead
    // of the nested-helper skip below, which keeps obligations a nested
    // helper's call raises: an event handler is such a helper.
    // ADR 0320: so is a call after an await its async function reaches
    // on every path, an ownerless continuation on a later task.
    if crate::execution_role::excluded_role_holds(file, call.callee, execution, lookup)
        || crate::execution_role::await_continuation_decides_nothing(file, call.callee, lookup)
        || (dispatch_obligations.len() > obligations_before
            && dispatch_obligations[obligations_before..]
                .iter()
                .all(|defect| {
                    matches!(
                        defect.analysis_context.as_str(),
                        "parameter-member-target-unresolved"
                            | "parameter-member-targets-diverge"
                            | "parameter-member-path-unresolved"
                            | "parameter-default-unresolved"
                            | "parameter-use-unmodeled"
                    )
                })
            && local_dispatch_exclusion_depth(
                file,
                call.callee,
                allowed,
                lookup,
                &mut HashSet::default(),
                excluded_dispatch_helpers,
            )
            .is_some())
    {
        dispatch_obligations.truncate(obligations_before);
    }
    let object_callable = runs_in_unrendered_object_callable(file, call.callee, lookup);
    if !object_callable
        && (inside_non_component_function(file, call.callee, lookup)
            || runs_in_unproven_stored_literal(file, call.callee, entities, symbol_names, lookup))
        && named_callback_execution_role(file, call.callee, lookup).is_none()
    {
        return;
    }
    let mut context = None::<String>;
    let get_context = property_gets::ReadContext {
        file,
        call,
        valid: valid_call && ambiguous_candidates.is_none(),
        execution,
        label: &label,
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup,
    };
    // ADR 0283: a project function's own member Gets of a parameter run
    // the getters of the exact literal this call passes there.
    if receives_getters && transported.is_none() {
        let (mut reads, get_dependencies) =
            property_gets::parameter_reads(&get_context, |path, span| {
                result_indexes.contains_key(&(path.to_owned(), span))
            });
        if object_callable {
            for read in &mut reads {
                read.callee_callback_timing = true;
            }
        }
        result.extend(reads);
        dependencies.extend(get_dependencies);
    }
    // ADR 0201: which of these reads are proven to run while this call
    // does. The callee is one non-generator project function, and the call
    // is a statement-level call outside any JSX (a prop getter runs when
    // the consumer reads it). A read it discovered in its own body, as a
    // call written directly there (not in a nested function, not in a
    // default parameter), runs during the call; so does the accessor
    // argument the callee's own body calls (`invokes_parameter_during_call`
    // proved that row). Every other row, a read propagated from a deeper
    // callee among them, stays attributed unless the entered-call proof
    // below covers it. An async body's reads additionally need a prefix
    // proof: ordinary async calls enter immediately, but resume detached.
    let direct_callee = (ambiguous_candidates.is_none() && !file.ast.any_jsx_containing(call.span))
        .then(|| match transported {
            Some(accessor_provenance::Callable::Function { path, span }) => {
                let file = lookup.file_by_path(path)?;
                Some((file, file.ast.function_at(*span)?))
            }
            _ => lookup.function_for_symbol(symbol),
        })
        .flatten()
        .filter(|(callee_file, callee)| {
            !call.construct
                && !callee.generator
                && (!callee.r#async || call.direct_callee)
                && (matches!(
                    transported,
                    Some(accessor_provenance::Callable::Function { .. })
                ) || lookup.function_value_is_current(callee_file, callee))
        });
    let activated_defaults =
        activated_default_reads(scope, dependencies, call, direct_callee, &label);
    let proof = DirectReadProof::new(scope, call, transported, direct_callee, target);
    let effective = decide_reads_by_arguments(
        scope,
        dependencies,
        call,
        (direct_callee, target),
        effective,
    );
    // One finding per symbol at this call (`seen` below). A symbol read
    // both directly and only through a nested default or closure keeps
    // the direct origin, so the proof is not lost to the order the
    // summary happened to collect them in.
    let mut effective = effective
        .into_iter()
        .map(|read| (proof.holds(&read), read))
        .chain(
            activated_defaults
                .into_iter()
                .map(|read| (!read.local, read)),
        )
        .collect::<Vec<_>>();
    // An undecided row sorts last: a proven row of the same symbol at
    // this call keeps the finding a violation.
    effective.sort_by_key(|(direct, read)| (read.undecided, !*direct, read.transported));
    for (direct, read) in effective {
        let accessor = read.display.to_string();
        if seen.insert((
            callee.path.clone(),
            callee.start_byte,
            read.symbol.to_string(),
        )) {
            result.push(ReactiveRead {
                kind: read
                    .kind
                    .clone()
                    .unwrap_or_else(|| "accessor".into())
                    .into(),
                accessor: accessor.into(),
                location: location(file.path.shared(), call.span),
                declaration: read.declaration,
                execution,
                context: context
                    .get_or_insert_with(|| enclosing_function_label(file, call.span))
                    .clone()
                    .into(),
                via: label.clone().into(),
                origin: Some(read.origin),
                origin_context: read.origin_context.into(),
                uncertain: read.undecided,
                missing_jsx_census: missing_jsx_census(file, call.span, execution),
                host_callback_timing: host_callback_timing(file, call.span, execution, lookup),
                project_consumer_non_strict: false,
                callback_invocation_unproven: false,
                callee_callback_timing: object_callable
                    || callee_callback_timing(file, call.span, execution, lookup),
                summary_attributed: !direct,
                transported: read.transported
                    || transported.is_some()
                    || provenance.has_source_origin(&read.symbol),
            });
        }
    }
}

fn push_withheld_member_obligation(
    scope: &FileReadScope<'_, '_, '_>,
    dispatch_obligations: &mut Vec<StaticDefect>,
    call: &solid_facts::ast::CallFact,
) {
    let file = scope.file;
    let allowed = &scope.allowed;
    let InterproceduralResultReadContext {
        entities,
        symbol_names,
        lookup,
        ..
    } = scope.context;
    // ADR 0320: a member call on an exact literal binding whose
    // receiver census does not hold (a member written, the object
    // handed on) runs an implementation nothing selects. It is a
    // dispatch obligation wherever its reads could be reported, not
    // a call that reads nothing.
    let execution =
        semantic_execution_role(file, call.callee, allowed, entities, symbol_names, lookup);
    if !crate::execution_role::excluded_role_holds(file, call.callee, execution, lookup)
        && !crate::execution_role::await_continuation_decides_nothing(file, call.callee, lookup)
    {
        dispatch_obligations.push(StaticDefect {
            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                callee: call
                    .static_callee(&file.source)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "dynamic call".into()),
                member: None,
            },
            location: location(file.path.shared(), call.callee),
            analysis_context: "withheld-object-member".into(),
            fixes: vec![],
            uncertain: true,
        });
    }
}

fn resolve_call_target<'b>(
    scope: &FileReadScope<'_, 'b, '_>,
    out: &mut FileReadOutput,
    call: &solid_facts::ast::CallFact,
) -> Option<CallSite<'b>> {
    let file = scope.file;
    let InterproceduralResultReadContext {
        result: InterproceduralResultView { nodes, .. },
        lookup,
        transported_calls,
        unavailable_calls,
        ..
    } = scope.context;
    let dispatch_obligations = &mut out.dispatch_obligations;
    let dependencies = &mut out.dependencies;
    let obligations_before = dispatch_obligations.len();
    let callee = location(file.path.shared(), call.callee);
    let transported = transported_calls.get(&callee);
    let label = call
        .static_callee(&file.source)
        .map(str::to_owned)
        .unwrap_or_else(|| "dynamic call".into());
    let valid_call = lookup
        .resolved_callee_call(file, call.callee)
        .is_some_and(|resolved| resolved.validity == ResolvedCallValidity::Valid);
    if unavailable_calls.contains(&callee)
        && !lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == ResolvedCallValidity::Recovery)
    {
        dispatch_obligations.push(StaticDefect {
            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                callee: label.clone(),
                member: None,
            },
            location: callee.clone(),
            analysis_context: "returned-callable-outside-source-universe".into(),
            fixes: vec![],
            uncertain: true,
        });
    }
    if !matches!(
        transported,
        Some(accessor_provenance::Callable::ArrayMembers(_))
    ) && lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| resolved.declaration.as_ref())
        .is_some_and(|declaration| declaration.standard_library)
    {
        // Type Facts tied this exact call target to the compiler's
        // standard/platform library declaration. Its implementation is
        // outside the package and cannot contribute package-owned
        // reactive reads; callback bodies were already connected by the
        // graph's exact argument-behavior proof. Exact returned array
        // members additionally project their reads at this iteration call.
        return None;
    }
    // A member invoked on the innermost function's parameter is not a
    // local runtime-dispatch choice. The graph records it in
    // `invoked_parameter_members`, and each caller either selects an
    // exact implementation or receives one call-site obligation. Also
    // reporting the unresolved member here duplicates that same proof
    // obligation at the helper definition.
    let parameter_member_call = lookup
        .member_callee_receiver(file, call.callee)
        .is_some_and(|(receiver, _)| {
            nodes
                .iter()
                .filter(|node| node.path == file.path.as_str() && node.body.contains(call.span))
                .min_by_key(|node| node.body.end - node.body.start)
                .is_some_and(|owner| owner.parameters.contains(&receiver))
        });
    if parameter_member_call {
        return None;
    }
    let candidate_symbols = lookup.callee_symbols(file, call.callee);
    if candidate_symbols.is_empty()
        && file
            .ast
            .computed_members
            .binary_search(&file.ast.peel_ts_sugar_span(call.callee))
            .is_ok()
    {
        if valid_call {
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: label,
                    member: None,
                },
                location: callee,
                analysis_context: "computed-call-target-unresolved".into(),
                fixes: vec![],
                uncertain: true,
            });
        }
        return None;
    }
    let transported_symbol = match transported {
        Some(accessor_provenance::Callable::Accessor(symbol)) => Some(symbol.to_string()),
        Some(accessor_provenance::Callable::Function { path, span }) => Some(format!(
            "project-callable:{path}:{}:{}",
            span.start, span.end
        )),
        Some(accessor_provenance::Callable::ArrayMembers(_)) => {
            Some("project-array-members".into())
        }
        None => None,
    };
    let symbol = transported_symbol.as_deref().or_else(|| {
        candidate_symbols
            .first()
            .map(SymbolId::as_str)
            .or_else(|| lookup.callee_symbol(file, call.callee))
    })?;
    if let Some(accessor_provenance::Callable::ArrayMembers(targets)) = transported {
        dependencies.extend(targets.iter().map(|(path, span)| {
            InterproceduralResultDependency::InlineFunction(path.clone(), *span)
        }));
    }
    match transported {
        Some(accessor_provenance::Callable::Function { path, span }) => {
            dependencies.insert(InterproceduralResultDependency::InlineFunction(
                path.clone(),
                *span,
            ));
        }
        Some(accessor_provenance::Callable::ArrayMembers(_)) => {}
        _ => {
            dependencies.insert(InterproceduralResultDependency::Symbol(SymbolId::from(
                symbol,
            )));
        }
    }
    let symbol = symbol.to_owned();
    Some(CallSite {
        obligations_before,
        callee,
        transported,
        label,
        valid_call,
        candidate_symbols,
        symbol,
    })
}

fn call_summary(
    scope: &FileReadScope<'_, '_, '_>,
    out: &mut FileReadOutput,
    call: &solid_facts::ast::CallFact,
    site: &CallSite<'_>,
) -> Option<(String, Vec<SummaryRead>, Option<usize>)> {
    let file = scope.file;
    let context = scope.context;
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                nodes,
                by_symbol,
                summaries,
                returned_bindings,
                ..
            },
        provenance,
        ..
    } = context;
    let dispatch_obligations = &mut out.dispatch_obligations;
    let dependencies = &mut out.dependencies;
    let label = site.label.clone();
    let callee = site.callee.clone();
    let transported = site.transported;
    let symbol = site.symbol.as_str();
    let ambiguous_candidates = site.ambiguous_candidates();
    let summary = if let Some(transported) = transported {
        match transported {
            accessor_provenance::Callable::Accessor(symbol) => (
                label,
                provenance
                    .read(symbol, file, call.span)
                    .into_iter()
                    .collect(),
                None,
            ),
            accessor_provenance::Callable::Function { path, span } => {
                let target = context
                    .result
                    .indexes
                    .get(&(path.clone(), *span))
                    .copied()?;
                (label, summaries[target].to_vec(), Some(target))
            }
            accessor_provenance::Callable::ArrayMembers(targets) => {
                let mut reads = Vec::new();
                for target in targets {
                    let Some(index) = context.result.indexes.get(target) else {
                        continue;
                    };
                    for read in summaries[*index].iter() {
                        push_unique_summary_read(&mut reads, read.clone());
                    }
                }
                (label, reads, None)
            }
        }
    } else if let Some(candidates) = ambiguous_candidates {
        let mut candidate_summaries = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            dependencies.insert(InterproceduralResultDependency::Symbol(candidate.clone()));
            if let Some(index) = by_symbol.get(candidate) {
                candidate_summaries.push(&summaries[*index]);
            }
        }
        let equivalent = candidate_summaries.len() == candidates.len()
            && candidate_summaries
                .split_first()
                .is_some_and(|(first, rest)| {
                    rest.iter()
                        .all(|candidate| equivalent_summary_reads(candidate, first))
                });
        if !equivalent {
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: label,
                    member: None,
                },
                location: callee,
                analysis_context: "runtime-call-targets-diverge".into(),
                fixes: vec![],
                uncertain: true,
            });
            return None;
        }
        (
            label,
            candidate_summaries[0].to_vec(),
            by_symbol.get(&candidates[0]).copied(),
        )
    } else if let Some(target) = by_symbol.get(symbol).copied() {
        (
            nodes[target].name.clone().unwrap_or(label),
            summaries[target].to_vec(),
            Some(target),
        )
    } else {
        let summary = returned_bindings.get(symbol)?;
        (label, summary.clone(), None)
    };
    Some(summary)
}

fn add_invoked_parameter_reads(
    scope: &FileReadScope<'_, '_, '_>,
    out: &mut FileReadOutput,
    call: &solid_facts::ast::CallFact,
    site: &CallSite<'_>,
    label: &str,
    target: usize,
    effective: &mut Vec<SummaryRead>,
) {
    let FileReadOutput {
        dependencies,
        dispatch_obligations,
        ..
    } = out;
    let file = scope.file;
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                by_symbol,
                summaries,
                invoked_parameters,
                returned_bindings,
                ..
            },
        source_kinds,
        accessors,
        entities,
        lookup,
        ..
    } = scope.context;
    let ambiguous_candidates = site.ambiguous_candidates();
    for parameter in &invoked_parameters[target] {
        let Some(argument) = call.arguments.get(*parameter) else {
            continue;
        };
        // ADR 0338: an argument that is an expression rather than a symbol
        // (`[get][0]`, `cond ? a : b`, `(() => get)()`) has no entity, but
        // the callee still calls it: it takes the same proof below.
        let argument_symbol = entities.get(&location(file.path.shared(), argument.span));
        let mut summarized = false;
        let mut function_value = false;
        if let Some(argument_symbol) = argument_symbol {
            dependencies.insert(InterproceduralResultDependency::Symbol(
                argument_symbol.clone(),
            ));
            let argument_summary = by_symbol
                .get(argument_symbol)
                .map(|index| &summaries[*index][..])
                .or_else(|| returned_bindings.get(argument_symbol).map(Vec::as_slice));
            if let Some(argument_summary) = argument_summary {
                for read in argument_summary {
                    push_unique_summary_read(effective, read.clone());
                }
                summarized = true;
            }
            // A project function passed by its own name: the function value
            // is what is passed, so the callee calling it reads what the
            // summary says and no getter hides behind it. A call result held
            // in a binding (`returned_bindings`) is not that: it is whatever
            // the callee returned.
            // `binding_declaration` is set for a bare identifier only: the
            // entity of a call span names its callee, not the call's result.
            function_value = argument.binding_declaration.is_some()
                && by_symbol.contains_key(argument_symbol)
                && lookup
                    .function_for_symbol(argument_symbol.as_str())
                    .is_some_and(|(callee_file, callee)| {
                        lookup.function_value_is_current(callee_file, callee)
                    });
        }
        // An accessor passed by reference, `run(count)`: the callee
        // calling the parameter reads the accessor at that moment.
        // `invoked_parameters` alone does not place that call inside
        // this call -- its owner is the nearest summary node, which
        // may enclose a timer callback or a returned closure -- so
        // the read is claimed only when the callee's own synchronous
        // body calls the parameter. One callee, not a dispatch.
        // ADR 0277: a body call of the parameter off the callee's
        // every-completion path still reads on some calls, so the row
        // stays, as one this call is not proven to perform.
        // ADR 0278: an inexact mapping (`f.call`, a replaced binding,
        // an earlier spread) proves the invocation on no path.
        let mut decided_source = None;
        let invocation =
            crate::execution_role::project_callee_at_parameter(file, call, *parameter, lookup)
                .and_then(|(callee_file, callee, exact)| {
                    if !exact {
                        Some(false)
                    } else if crate::execution_role::invokes_parameter_during_call(
                        callee_file,
                        callee,
                        *parameter,
                        lookup,
                    ) {
                        Some(true)
                    } else if crate::execution_role::calls_parameter_in_body(
                        callee_file,
                        callee,
                        *parameter,
                        lookup,
                    ) {
                        // ADR 0280: this call's arguments may decide the
                        // branch the parameter call sits in.
                        let standing = crate::execution_role::parameter_call_standing(
                            file,
                            call,
                            callee_file,
                            callee,
                            *parameter,
                            lookup,
                        );
                        if standing.is_some() {
                            decided_source = Some((callee_file.path.to_string(), callee.span));
                        }
                        match standing {
                            Some(solid_facts::ast::SiteStanding::Guaranteed) => Some(true),
                            Some(solid_facts::ast::SiteStanding::Unreachable) => None,
                            _ => Some(false),
                        }
                    } else {
                        None
                    }
                });
        if let Some((path, span)) = decided_source {
            dependencies.insert(InterproceduralResultDependency::FunctionSource(path, span));
        }
        let mut proven_read = false;
        if !summarized
            && ambiguous_candidates.is_none()
            && !argument.spread
            && let Some(argument_symbol) = argument_symbol
            && source_kinds.get(argument_symbol.as_str()) == Some(&ReactiveSourceKind::Accessor)
            && let Some((display, declaration)) = accessors.get(argument_symbol.as_str())
            && let Some(guaranteed) = invocation
        {
            proven_read = true;
            push_unique_summary_read(
                effective,
                SummaryRead {
                    transported: false,
                    symbol: argument_symbol.clone(),
                    display: display.clone(),
                    kind: Some("accessor".into()),
                    declaration: declaration.clone(),
                    origin: location(file.path.shared(), call.span),
                    origin_context: label.to_owned(),
                    owner: None,
                    local: !guaranteed,
                    conditional: false,
                    undecided: false,
                },
            );
        }
        // ADR 0338 (inverted): the callee may call this parameter, and the
        // argument is not an accessor the analysis proved. "Not a proven
        // getter" is not "not a function": a getter that reached here through
        // a spread, a rest pattern, a held alias, a helper's return or
        // `Reflect.apply` is called all the same. Only a value proven not to
        // be a getter settles it; everything else keeps an SC9012 obligation.
        if !proven_read
            && invocation.is_some()
            && site.valid_call
            && !function_value
            && !argument_proven_not_a_getter(file, argument, entities, source_kinds, lookup)
        {
            let defect = StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: label.to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), argument.span),
                analysis_context: "parameter-call-argument-unproven".into(),
                fixes: vec![],
                uncertain: true,
            };
            if !dispatch_obligations.contains(&defect) {
                dispatch_obligations.push(defect);
            }
        }
    }
}

fn add_parameter_member_reads(
    scope: &FileReadScope<'_, '_, '_>,
    out: &mut FileReadOutput,
    call: &solid_facts::ast::CallFact,
    site: &CallSite<'_>,
    label: &str,
    target: usize,
    effective: &mut Vec<SummaryRead>,
) {
    let file = scope.file;
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                nodes,
                invoked_parameter_members,
                ..
            },
        source_kinds,
        accessors,
        entities,
        lookup,
        ..
    } = scope.context;
    let dispatch_obligations = &mut out.dispatch_obligations;
    let valid_call = site.valid_call;
    // The callee invokes a member of one of its parameters. Which
    // implementation that is belongs to this call site, not to the
    // callee: resolve it from the argument actually passed here. An
    // argument that is exactly one object proves what runs; anything
    // else -- unresolved, or a conditional over two objects -- proves
    // nothing and contributes no read.
    for ParameterMemberInvocation {
        parameter,
        path,
        primitive_builtin,
        dispatch_read_excluded,
        unmodeled,
        ..
    } in &invoked_parameter_members[target]
    {
        if *primitive_builtin || *dispatch_read_excluded {
            continue;
        }
        // `undefined`, written or omitted, selects the parameter's
        // default initializer, whose value is the callee's: nothing
        // at this call settles which member runs.
        let defaulted = parameter_has_default(&nodes[target], *parameter, lookup);
        let Some(argument) = call.arguments.get(*parameter) else {
            // An omission a default covers is no type error; Type
            // Facts resolves no signature for a call with no
            // arguments, which is not evidence of one either.
            if defaulted
                && (!path.is_empty() || *unmodeled)
                && (valid_call || lookup.resolved_callee_call(file, call.callee).is_none())
                && !call.arguments.iter().any(|argument| argument.spread)
            {
                let defect = StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label.to_owned(),
                        member: (!path.is_empty()).then(|| path.join(".")),
                    },
                    location: location(file.path.shared(), call.span),
                    analysis_context: "parameter-default-unresolved".into(),
                    fixes: vec![],
                    uncertain: true,
                };
                if !dispatch_obligations.contains(&defect) {
                    dispatch_obligations.push(defect);
                }
            }
            continue;
        };
        let undefined_selects_default = defaulted
            && (argument.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Nullish
                && file.source_text(file.ast.peel_ts_sugar_span(argument.span)) != Some("null"));
        if undefined_selects_default && !argument.spread && (!path.is_empty() || *unmodeled) {
            if valid_call {
                let defect = StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label.to_owned(),
                        member: (!path.is_empty()).then(|| path.join(".")),
                    },
                    location: location(file.path.shared(), argument.span),
                    analysis_context: "parameter-default-unresolved".into(),
                    fixes: vec![],
                    uncertain: true,
                };
                if !dispatch_obligations.contains(&defect) {
                    dispatch_obligations.push(defect);
                }
            }
            continue;
        }
        // ADR 0320: the callee uses the parameter in a way no member
        // invocation describes; only a primitive or nullish argument,
        // which carries no project member, settles it.
        if *unmodeled {
            // A function argument's invocation is the callback
            // rules' question, answered at this call already.
            let function_argument = file
                .ast
                .functions
                .iter()
                .any(|function| function.span == file.ast.peel_ts_sugar_span(argument.span))
                || lookup
                    .entity_at(file.path.as_str(), argument.span)
                    .and_then(|entity| entity.callability)
                    == Some(typefacts::Callability::Callable);
            if !argument.spread
                && !undefined_selects_default
                && (function_argument
                    || matches!(
                        argument.runtime_value_kind,
                        solid_facts::ast::RuntimeValueKind::Primitive
                            | solid_facts::ast::RuntimeValueKind::Nullish
                    )
                    || argument_is_proven_primitive(file, argument, lookup))
            {
                continue;
            }
            if valid_call {
                let defect = StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label.to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), argument.span),
                    analysis_context: "parameter-use-unmodeled".into(),
                    fixes: vec![],
                    uncertain: true,
                };
                if !dispatch_obligations.contains(&defect) {
                    dispatch_obligations.push(defect);
                }
            }
            continue;
        }
        // A literal argument is a fresh primitive (or RegExp) value,
        // so every member reached through it is its built-in
        // prototype's, and no built-in reads reactive state. A
        // callback the callee hands that built-in (`replace`'s
        // replacer) is the callee's own call, decided in its body by
        // the audited standard-library rows of `runtime_semantics`.
        if !path.is_empty()
            && !argument.spread
            && (argument.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Primitive
                || argument_is_proven_primitive(file, argument, lookup))
        {
            continue;
        }
        if path.is_empty() {
            let argument_location = location(file.path.shared(), argument.span);
            let argument_symbol = entities.get(&argument_location);
            if let Some(argument_symbol) = argument_symbol
                && source_kinds.get(argument_symbol.as_str()) == Some(&ReactiveSourceKind::Store)
            {
                push_unique_summary_read(
                    effective,
                    SummaryRead {
                        transported: false,
                        symbol: argument_symbol.clone(),
                        display: SymbolId::from(file.source_text(argument.span).unwrap_or("store")),
                        kind: Some("store-path".into()),
                        declaration: accessors
                            .get(argument_symbol.as_str())
                            .map(|(_, location)| location.clone())
                            .unwrap_or_else(|| argument_location.clone()),
                        origin: location(file.path.shared(), call.span),
                        origin_context: label.to_owned(),
                        owner: None,
                        local: false,
                        conditional: false,
                        undecided: false,
                    },
                );
            } else if !crate::local_access::argument_proves_non_reactive(
                file,
                argument,
                entities,
                source_kinds,
                lookup,
            ) && valid_call
            {
                dispatch_obligations.push(StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label.to_owned(),
                        member: None,
                    },
                    location: argument_location,
                    analysis_context: "parameter-member-argument-unresolved".into(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
            continue;
        }
        add_nested_member_reads(
            scope,
            dispatch_obligations,
            &mut out.dependencies,
            call,
            &MemberCall {
                argument,
                parameter: *parameter,
                path,
                valid_call,
                label,
                undefined_selects_default,
            },
            target,
            effective,
        );
    }
}

/// One member invocation of a callee's parameter, as this call passes it.
struct MemberCall<'a> {
    argument: &'a solid_facts::ast::ArgumentFact,
    parameter: usize,
    path: &'a [String],
    valid_call: bool,
    label: &'a str,
    undefined_selects_default: bool,
}

fn add_nested_member_reads(
    scope: &FileReadScope<'_, '_, '_>,
    dispatch_obligations: &mut Vec<StaticDefect>,
    dependencies: &mut HashSet<InterproceduralResultDependency>,
    call: &solid_facts::ast::CallFact,
    member: &MemberCall<'_>,
    target: usize,
    effective: &mut Vec<SummaryRead>,
) {
    let file = scope.file;
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                nodes,
                by_symbol,
                summaries,
                ..
            },
        lookup,
        ..
    } = scope.context;
    let MemberCall {
        argument,
        path,
        valid_call,
        label,
        undefined_selects_default,
        ..
    } = member;
    let (valid_call, undefined_selects_default) = (*valid_call, *undefined_selects_default);
    let parameter = &member.parameter;
    let label = *label;
    // A nested path requires an exact, stable value at every
    // intermediate Get, not a lookup of its last name on the root.
    let property = &path[0];
    let nested = path.len() > 1;
    if nested {
        // Receiver closure inspects every use of the callee's
        // parameter, including escapes that leave reads unchanged.
        // Keep this dependency even when the current proof refuses.
        dependencies.insert(InterproceduralResultDependency::FunctionSource(
            nodes[target].path.clone(),
            nodes[target].span,
        ));
    }
    // ADR 0320: the callee hands the object to nothing that could
    // replace the member before it runs.
    let mut consulted = Vec::new();
    let closed_callee =
        lookup.parameter_only_member_accessed_consulting(file, call, *parameter, &mut consulted);
    let mut implementations = if nested {
        lookup.nested_member_value_symbols_at(file, call, *parameter, path)
    } else if closed_callee {
        lookup.member_value_symbols_at_tracked(file, argument.span, property, &mut consulted)
    } else {
        Vec::new()
    };
    // The answer read these functions' source: a cached result
    // here is reused only while they are unchanged.
    for (path, span) in consulted {
        dependencies.insert(InterproceduralResultDependency::FunctionSource(path, span));
    }
    // ADR 0211: an argument whose origin fixes its class fixes the
    // member that runs -- a fresh built-in value's is its
    // prototype's, which reads nothing reactive, and an exact
    // project class's is the method that class declares. Unless
    // the program rewrites that member or a prototype.
    // ADR 0314: a write through a prototype vetoes only the
    // objects whose chain holds that prototype.
    if implementations.is_empty()
        && !nested
        && !argument.spread
        && closed_callee
        && !undefined_selects_default
    {
        match lookup.value_origin(file, argument.span, argument.runtime_value_kind, 6) {
            Some(crate::indexes::ValueOrigin::Builtin)
                if !lookup.member_may_be_reassigned_on_builtin(property)
                    && !lookup.member_may_be_reassigned_on_builtin("__proto__") =>
            {
                return;
            }
            Some(crate::indexes::ValueOrigin::ProjectClass(class_file, class))
                if !lookup.member_may_be_reassigned_on_class(property, class_file, class)
                    && !lookup.member_may_be_reassigned_on_class(
                        "__proto__",
                        class_file,
                        class,
                    ) =>
            {
                implementations.extend(
                    lookup
                        .class_method_symbol(class_file, class, property)
                        .cloned(),
                );
            }
            _ => {}
        }
    }
    // ADR 0321: a function literal that is exactly an argument
    // literal's property value is its own implementation. Its
    // reads stay attributed where it is written (they are claimed
    // there, as a callback's), so it adds none at this dispatch.
    let literal_property_summary = SummaryReads::default();
    let mut member_summaries = Vec::with_capacity(implementations.len());
    for implementation in &implementations {
        dependencies.insert(InterproceduralResultDependency::Symbol(
            implementation.clone(),
        ));
        if let Some(index) = by_symbol.get(implementation) {
            member_summaries.push(&summaries[*index]);
        } else if is_literal_property_function_symbol(implementation) {
            member_summaries.push(&literal_property_summary);
        }
    }
    let equivalent = member_summaries.len() == implementations.len()
        && member_summaries.split_first().is_some_and(|(first, rest)| {
            rest.iter()
                .all(|candidate| equivalent_summary_reads(candidate, first))
        });
    if !equivalent {
        if valid_call {
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: label.to_owned(),
                    member: Some(path.join(".")),
                },
                location: location(file.path.shared(), argument.span),
                analysis_context: if nested {
                    "parameter-member-path-unresolved".into()
                } else if implementations.is_empty() {
                    "parameter-member-target-unresolved".into()
                } else {
                    "parameter-member-targets-diverge".into()
                },
                fixes: vec![],
                uncertain: true,
            });
        }
        return;
    }
    for read in member_summaries[0].iter() {
        push_unique_summary_read(effective, read.clone());
    }
}

fn activated_default_reads(
    scope: &FileReadScope<'_, '_, '_>,
    dependencies: &mut HashSet<InterproceduralResultDependency>,
    call: &solid_facts::ast::CallFact,
    direct_callee: Option<(&solid_facts::FileFacts, &solid_facts::ast::FunctionFact)>,
    label: &str,
) -> Vec<SummaryRead> {
    let InterproceduralResultReadContext {
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup,
        ..
    } = scope.context;
    // ADR 0204: a default-parameter initializer of an ordinary callee runs
    // during this call exactly when the call omits that argument. An
    // accessor called directly in it (not in a function it creates) is
    // read then, as one the body calls is. This includes nested async
    // helpers and exact store Gets. Keep activation separate from summary
    // ownership: a default is not a read in the enclosing function's body.
    // The authored default remains the origin, not the invocation span.
    let mut activated_defaults = Vec::new();
    if let Some((callee_file, callee)) = direct_callee
        && call.direct_callee
        && !call.arguments.iter().any(|argument| argument.spread)
    {
        for parameter in callee.parameters.iter().skip(call.arguments.len()) {
            let Some(default) = parameter.initializer else {
                continue;
            };
            let evaluated_here = |span: Span| {
                default.contains(span)
                    && !callee_file.ast.any_jsx_containing(span)
                    && !callee_file
                        .ast
                        .functions_within(default)
                        .any(|nested| nested.span.contains(span))
                    && !callee_file
                        .ast
                        .classes
                        .iter()
                        .any(|class| class.span.contains(span))
            };
            for read_call in callee_file
                .ast
                .calls
                .iter()
                .filter(|candidate| evaluated_here(candidate.span))
            {
                let Some(accessor) =
                    entities.get(&location(callee_file.path.shared(), read_call.callee))
                else {
                    continue;
                };
                if source_kinds.get(accessor.as_str()) != Some(&ReactiveSourceKind::Accessor)
                    || !source_has_runtime_witness(
                        accessor,
                        accessors,
                        entities,
                        symbol_names,
                        lookup,
                    )
                {
                    continue;
                }
                let Some((display, declaration)) = accessors.get(accessor.as_str()) else {
                    continue;
                };
                dependencies.insert(InterproceduralResultDependency::Symbol(accessor.clone()));
                push_unique_summary_read(
                    &mut activated_defaults,
                    SummaryRead {
                        transported: false,
                        symbol: accessor.clone(),
                        display: display.clone(),
                        kind: Some("accessor".into()),
                        declaration: declaration.clone(),
                        origin: location(callee_file.path.shared(), read_call.span),
                        origin_context: label.to_owned(),
                        owner: None,
                        // A branch inside the default itself (ADR 0276).
                        local: callee_file
                            .ast
                            .completion_sites
                            .binary_search(&read_call.span)
                            .is_err(),
                        conditional: false,
                        undecided: false,
                    },
                );
            }
            // The immediate receiver must be an exact store source. A
            // prefix Get is enough (`state.range`), even when a longer
            // suffix cannot be resolved. No parameter is globally tainted
            // from the value this one invocation's default provides.
            for member in callee_file.ast.members.iter().filter(|member| {
                evaluated_here(member.span) && member_is_get(&callee_file.ast, member.span)
            }) {
                let object = callee_file.ast.peel_ts_sugar_span(member.object);
                let Some(store) = entities.get(&location(callee_file.path.shared(), object)) else {
                    continue;
                };
                if source_kinds.get(store.as_str()) != Some(&ReactiveSourceKind::Store)
                    || !source_has_runtime_witness(store, accessors, entities, symbol_names, lookup)
                    || !store_get_has_strict_witness(
                        callee_file,
                        member.span,
                        store,
                        entities,
                        lookup.dialect,
                    )
                {
                    continue;
                }
                let Some((_, declaration)) = accessors.get(store.as_str()) else {
                    continue;
                };
                dependencies.insert(InterproceduralResultDependency::Symbol(store.clone()));
                push_unique_summary_read(
                    &mut activated_defaults,
                    SummaryRead {
                        transported: false,
                        symbol: store.clone(),
                        display: SymbolId::from(
                            callee_file.source_text(member.span).unwrap_or("store"),
                        ),
                        kind: Some("store-path".into()),
                        declaration: declaration.clone(),
                        origin: location(callee_file.path.shared(), member.span),
                        origin_context: label.to_owned(),
                        owner: None,
                        local: callee_file
                            .ast
                            .completion_sites
                            .binary_search(&member.span)
                            .is_err(),
                        conditional: false,
                        undecided: false,
                    },
                );
            }
        }
    }
    activated_defaults
}

/// Whether a summary read is proven to run while its call does (ADR 0201).
struct DirectReadProof<'s, 'a, 'b, 'c> {
    scope: &'s FileReadScope<'a, 'b, 'c>,
    call: &'s solid_facts::ast::CallFact,
    transported: Option<&'s accessor_provenance::Callable>,
    direct_callee: Option<(
        &'s solid_facts::FileFacts,
        &'s solid_facts::ast::FunctionFact,
    )>,
    target: Option<usize>,
    array_entry: bool,
    entered: std::cell::OnceCell<Vec<(String, Span)>>,
}

impl<'s, 'a, 'b, 'c> DirectReadProof<'s, 'a, 'b, 'c> {
    fn new(
        scope: &'s FileReadScope<'a, 'b, 'c>,
        call: &'s solid_facts::ast::CallFact,
        transported: Option<&'s accessor_provenance::Callable>,
        direct_callee: Option<(
            &'s solid_facts::FileFacts,
            &'s solid_facts::ast::FunctionFact,
        )>,
        target: Option<usize>,
    ) -> Self {
        let InterproceduralResultReadContext {
            array_member_bodies,
            lookup,
            transported_calls,
            ..
        } = scope.context;
        // ADR 0204: the functions this call enters synchronously, through
        // calls written directly in each one's own body, outside JSX, each
        // resolving to one ordinary project function. Every async hop must
        // itself be in its caller's synchronous prefix; the leaf read must
        // pass the same test. Entry alone never proves the whole async body.
        let entered = std::cell::OnceCell::new();
        let array_entry = matches!(
            transported,
            Some(accessor_provenance::Callable::ArrayMembers(_))
        ) || (!array_member_bodies.is_empty()
            && direct_callee.is_some_and(|(callee_file, callee)| {
                array_member_bodies.contains(&(callee_file.path.to_string(), callee.span))
                    || entered
                        .get_or_init(|| {
                            directly_entered_functions(
                                callee_file,
                                callee,
                                lookup,
                                transported_calls,
                            )
                        })
                        .iter()
                        .any(|key| array_member_bodies.contains(key))
            }));
        Self {
            scope,
            call,
            transported,
            direct_callee,
            target,
            array_entry,
            entered,
        }
    }

    fn holds(&self, read: &SummaryRead) -> bool {
        let Self {
            scope,
            call,
            transported,
            direct_callee,
            target,
            array_entry,
            entered,
        } = self;
        let (call, transported, direct_callee, target, array_entry) =
            (*call, *transported, *direct_callee, *target, *array_entry);
        let file = scope.file;
        let InterproceduralResultReadContext {
            result: InterproceduralResultView { nodes, .. },
            source_kinds,
            accessors,
            entities,
            symbol_names,
            lookup,
            transported_calls,
            provenance,
            ..
        } = scope.context;
        // ADR 0276: a read the callee, or a hop below it, performs only
        // on some of its completions is not performed by this call.
        if read.local || read.conditional {
            return false;
        }
        let proven_origin = || {
            provenance.has_runtime_source_origin(&read.symbol)
                || provenance.has_source_origin(&read.symbol)
        };
        if array_entry && !proven_origin() {
            return false;
        }
        if let Some(accessor_provenance::Callable::ArrayMembers(targets)) = transported {
            // The callback invokes each exact element member inline at
            // this map call. Its definition's execution role is irrelevant.
            return !file.ast.any_jsx_containing(call.span)
                && targets.iter().any(|(path, span)| {
                    let Some(target_file) = lookup.file_by_path(path) else {
                        return false;
                    };
                    let Some(function) = target_file.ast.function_at(*span) else {
                        return false;
                    };
                    captured_body_read_runs_during_call(target_file, function, read)
                });
        }
        if matches!(
            transported,
            Some(accessor_provenance::Callable::Accessor(_))
        ) {
            return read.origin == location(file.path.shared(), call.span);
        }
        let Some((callee_file, callee)) = direct_callee else {
            return false;
        };
        if read.owner.is_none() {
            if matches!(
                transported,
                Some(accessor_provenance::Callable::Function { .. })
            ) {
                return captured_body_read_runs_during_call(callee_file, callee, read);
            }
            // ADR 0299: an anonymous object member has no declaration
            // symbol for the ordinary entered-function join. Its exact
            // source location is still on this synchronous call chain.
            // Require a runtime source witness; typed summary candidates
            // and detached async origins cannot acquire this proof.
            if !callee.r#async
                && source_has_runtime_witness(
                    &read.symbol,
                    accessors,
                    entities,
                    symbol_names,
                    lookup,
                )
            {
                let entered = entered.get_or_init(|| {
                    directly_entered_functions(callee_file, callee, lookup, transported_calls)
                });
                let all_sync = entered.iter().all(|(path, span)| {
                    lookup
                        .file_by_path(path)
                        .and_then(|file| file.ast.function_at(*span))
                        .is_some_and(|function| !function.r#async)
                });
                if all_sync
                    && entered.iter().any(|(path, span)| {
                        lookup
                            .object_member_targets()
                            .values()
                            .any(|(target_path, target_span)| {
                                target_path == path && target_span == span
                            })
                            && lookup.file_by_path(path).is_some_and(|file| {
                                file.ast.function_at(*span).is_some_and(|function| {
                                    captured_body_read_runs_during_call(file, function, read)
                                })
                            })
                    })
                {
                    return true;
                }
            }
            return read.kind.as_deref() == Some("accessor")
                && read.origin == location(file.path.shared(), call.span);
        }
        let owned_here = target.is_some_and(|target| {
            nodes[target].symbol.is_some() && nodes[target].symbol == read.owner
        });
        let (owner_file, owner) = if owned_here {
            (callee_file, callee)
        } else {
            let Some(owner) = read
                .owner
                .as_ref()
                .and_then(|owner| lookup.function_for_symbol(owner.as_str()))
            else {
                return false;
            };
            let entered: &Vec<(String, Span)> = entered.get_or_init(|| {
                directly_entered_functions(callee_file, callee, lookup, transported_calls)
            });
            if !entered
                .iter()
                .any(|(path, span)| path == owner.0.path.as_str() && *span == owner.1.span)
            {
                return false;
            }
            owner
        };
        if read.origin.path.as_ref() != owner_file.path.as_str() {
            return false;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(read.origin.start_byte),
            u32::try_from(read.origin.end_byte),
        ) else {
            return false;
        };
        let origin = Span::new(start, end);
        // Only the new async proof needs this narrower source premise.
        // A published Accessor/Store type alone does not prove runtime
        // reactivity. Existing all-sync directness is left unchanged.
        let through_async = callee.r#async
            || owner.r#async
            || entered.get().is_some_and(|functions| {
                functions.iter().any(|(path, span)| {
                    lookup.file_by_path(path).is_some_and(|file| {
                        file.ast
                            .functions
                            .iter()
                            .any(|function| function.span == *span && function.r#async)
                    })
                })
            });
        (!through_async
            || ((source_has_runtime_witness(
                &read.symbol,
                accessors,
                entities,
                symbol_names,
                lookup,
            ) || provenance.has_source_origin(&read.symbol))
                && (source_kinds.get(&read.symbol) != Some(&ReactiveSourceKind::Store)
                    || store_get_has_strict_witness(
                        owner_file,
                        origin,
                        &read.symbol,
                        entities,
                        lookup.dialect,
                    ))))
            && body_site_runs_during_call(owner_file, owner, origin)
    }
}

fn decide_reads_by_arguments(
    scope: &FileReadScope<'_, '_, '_>,
    dependencies: &mut HashSet<InterproceduralResultDependency>,
    call: &solid_facts::ast::CallFact,
    (direct_callee, target): (
        Option<(&solid_facts::FileFacts, &solid_facts::ast::FunctionFact)>,
        Option<usize>,
    ),
    mut effective: Vec<SummaryRead>,
) -> Vec<SummaryRead> {
    let file = scope.file;
    let InterproceduralResultReadContext {
        result: InterproceduralResultView { nodes, .. },
        lookup,
        ..
    } = scope.context;
    // ADR 0280: a read the callee performs on some of its completions is
    // decided for this call when its arguments decide the branch: dropped
    // where it cannot run, proven where it always does. Only rows the
    // callee itself discovered, with no hop below it, are decided.
    if let (Some((callee_file, callee)), Some(target)) = (direct_callee, target)
        && let Some(owner) = nodes[target].symbol.as_ref()
    {
        let candidates = effective
            .iter()
            .enumerate()
            .filter(|(_, read)| {
                read.local
                    && !read.conditional
                    && read.owner.as_ref() == Some(owner)
                    && read.origin.path.as_ref() == callee_file.path.as_str()
            })
            .filter_map(|(index, read)| {
                Some((
                    index,
                    Span::new(
                        u32::try_from(read.origin.start_byte).ok()?,
                        u32::try_from(read.origin.end_byte).ok()?,
                    ),
                ))
            })
            .collect::<Vec<_>>();
        if !candidates.is_empty()
            && let Some(values) =
                crate::argument_values::parameter_values(file, call, callee_file, callee, lookup)
                    .filter(|values| !values.is_empty())
            && let Some(standings) = solid_facts::ast::argument_site_standings(
                std::path::Path::new(callee_file.path.as_str()),
                &callee_file.source,
                callee.span,
                &values,
                &lookup.never_returning_calls_within(callee_file, callee),
                &candidates.iter().map(|(_, span)| *span).collect::<Vec<_>>(),
            )
        {
            dependencies.insert(InterproceduralResultDependency::FunctionSource(
                callee_file.path.to_string(),
                callee.span,
            ));
            let mut unreachable = HashSet::default();
            for ((index, _), standing) in candidates.iter().zip(standings) {
                match standing {
                    solid_facts::ast::SiteStanding::Unreachable => {
                        unreachable.insert(*index);
                    }
                    solid_facts::ast::SiteStanding::Guaranteed => {
                        effective[*index].local = false;
                    }
                    solid_facts::ast::SiteStanding::Possible => {}
                }
            }
            if !unreachable.is_empty() {
                effective = effective
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| !unreachable.contains(index))
                    .map(|(_, read)| read)
                    .collect();
            }
        }
    }
    effective
}

pub(super) fn captured_body_read_runs_during_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    read: &SummaryRead,
) -> bool {
    if read.origin.path.as_ref() != file.path.as_str() {
        return false;
    }
    let (Ok(start), Ok(end)) = (
        u32::try_from(read.origin.start_byte),
        u32::try_from(read.origin.end_byte),
    ) else {
        return false;
    };
    body_site_runs_during_call(file, function, Span::new(start, end))
}

/// Narrow source premise for newly promoted defaults/async origins.
/// Require an exact, unwritten binding created by the selected dialect's
/// source primitive. Type-only Accessor/Store roots and external/returned
/// sources need a separate provenance proof; declaration spelling is not one.
pub(super) fn source_has_runtime_witness(
    symbol: &SymbolId,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some((_, declaration)) = accessors.get(symbol) else {
        return false;
    };
    let Some(file) = lookup.file_by_path(declaration.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return false;
    };
    if entities.get(declaration) != Some(symbol)
        || crate::value_identity::binding_has_write(file, entities, symbol)
        || crate::indexes::binding_written(file, Span::new(start, end))
    {
        return false;
    }
    file.ast.bindings.iter().any(|binding| {
        binding
            .names
            .iter()
            .any(|name| location(file.path.shared(), name.span) == *declaration)
            && binding.call_initializer.is_some_and(|initializer| {
                file.ast.call_at(initializer).is_some_and(|call| {
                    crate::call_primitive_name_cached(
                        lookup,
                        file,
                        call,
                        entities,
                        symbol_names,
                        lookup.dialect,
                    )
                    .as_ref()
                    .and_then(crate::PrimitiveName::primitive)
                    .is_some_and(|primitive| lookup.dialect.creates_reactive_source(primitive))
                })
            })
    })
}

/// A new strict store-read proof needs an exact identifier root and a
/// known string head key. Symbol/dynamic/protocol keys are not witnesses.
pub(super) fn store_get_has_strict_witness(
    file: &solid_facts::FileFacts,
    span: Span,
    symbol: &SymbolId,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
) -> bool {
    let Some(mut head) = file.ast.member_at(span) else {
        return false;
    };
    while let Some(inner) = file.ast.member_at(file.ast.peel_ts_sugar_span(head.object)) {
        head = inner;
    }
    let root = file.ast.peel_ts_sugar_span(head.object);
    if !file.ast.identifiers.iter().any(|identifier| {
        identifier.span == root && identifier.role == solid_facts::ast::IdentifierRole::Reference
    }) || entities.at(file.path.as_str(), root) != Some(symbol)
        || !member_is_get(&file.ast, head.span)
    {
        return false;
    }
    let key = if file.ast.computed_members.binary_search(&head.span).is_ok() {
        file.ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == head.span)
            .map(|literal| literal.key.as_ref())
    } else {
        file.source_text(head.property)
    };
    key.is_some_and(|key| dialect.store_key_warns_strict_read(key))
}

/// Whether a member occurrence performs Get, rather than only writing or
/// deleting that exact member. Receivers and keys inside a target still run.
pub(super) fn member_is_get(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    !ast.is_plain_assignment_target(span)
        && !ast
            .deleted_targets
            .iter()
            .any(|target| ast.peel_ts_sugar_span(*target) == span)
        && !ast
            .iteration_targets
            .iter()
            .any(|target| target.contains(span))
        // A destructuring target has no per-leaf Get/write census here.
        && !ast.assignments.iter().any(|assignment| {
            !assignment.reads_target
                && assignment.target.contains(span)
                && !ast
                    .members
                    .iter()
                    .any(|member| member.span == assignment.target)
        })
}

/// A direct body site that can execute only before this call first suspends.
/// This is a may-run proof, as in ADR 0204: a branch need not be taken, but
/// whenever the site is evaluated it must still be on the caller's stack.
///
/// All own awaits matter, not just `unconditional_awaits`. Source ordering is
/// deliberately conservative: even an await operand is refused. Loops need
/// a second check because a lexically earlier site can run again after an
/// await on a back edge (including a for-loop's test or update). Implicit
/// suspensions include for-await and await-using; the latter is recorded at
/// its declaration, earlier than disposal, so it can only withhold proof.
pub(crate) fn body_site_runs_during_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    site: Span,
) -> bool {
    if function.generator || !crate::owners::written_directly_in(&file.ast, function, site) {
        return false;
    }
    // JSX may lower to a getter; class fields may wait for construction.
    // Neither lexical containment nor source order proves evaluation then.
    // A method's own body lies inside its class; only a class written in
    // this function defers the site.
    if file.ast.any_jsx_containing(site)
        || file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(site) && function.body.contains(class.span))
        // Binding-pattern defaults/keys run after the initializer, even
        // though written before it. Assignment patterns have the same
        // reversal. Withhold these sites rather than infer evaluation order.
        || file.ast.bindings.iter().any(|binding| binding.pattern.contains(site))
        || file.ast.assignments.iter().any(|assignment| assignment.target.contains(site))
        || (file.ast.members.iter().any(|member| member.span == site)
            && !member_is_get(&file.ast, site))
    {
        return false;
    }
    if !function.r#async {
        return true;
    }
    let suspensions = file
        .ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .copied()
        .filter(|span| crate::owners::written_directly_in(&file.ast, function, *span))
        .collect::<Vec<_>>();
    // `default: read(); case await value:` evaluates the later case test
    // before the default body. The conditional-test census contains if,
    // ternary and switch tests; only the first two have an ordering model
    // here. Refuse a suspending function with any other own conditional
    // test, including future kinds, rather than invent a switch CFG.
    if !suspensions.is_empty()
        && file.ast.conditional_tests.iter().any(|test| {
            crate::owners::written_directly_in(&file.ast, function, *test)
                && !file
                    .ast
                    .if_regions
                    .iter()
                    .any(|region| region.test == *test)
                && !file
                    .ast
                    .conditional_expressions
                    .iter()
                    .any(|expression| expression.test == *test)
        })
    {
        return false;
    }
    !suspensions.iter().any(|span| span.start < site.end)
        && !file.ast.loop_statements.iter().any(|looped| {
            looped.contains(site)
                && function.body.contains(*looped)
                && suspensions.iter().any(|span| looped.contains(*span))
        })
}

/// ADR 0276: whether `site`, written directly in the function at
/// `function_span`, runs on every normal completion of a call of it, before
/// any suspension. A site outside the completion fact, or one
/// [`body_site_runs_during_call`] withholds, is not proven.
pub(crate) fn site_on_every_completion(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function_span: Span,
    site: Span,
) -> bool {
    file.ast.completion_sites.binary_search(&site).is_ok()
        && containing_ast_function(&file.ast, site)
            .filter(|function| function.span == function_span)
            .is_some_and(|function| {
                body_site_runs_during_call(file, function, site)
                    && !lookup.never_returning_call_before(file, function, site)
            })
}

/// Whether `file` holds syntax that a caller's call-role proof reads beyond
/// the summaries the result cache compares: an async function's suspensions,
/// a parameter default, or a written binding that may hold a function
/// (`function_value_is_current`, ADR 0221), or a bare identifier or `new Proxy` return whose
/// source identity discovery follows (ADR 0222). A file without any of these moves
/// no such proof.
/// ADR 0289: a project Get, in a role that reports an untracked read, of a
/// member a package analyzed as source (ADR 0284) defines as a getter that
/// reads an accessor. ADR 0300 proves a Get only when every visible provider
/// supplies the getter and the visible member/descriptor vetoes permit it.
/// The may-derivation remains an obligation when that proof refuses.
pub(super) fn dependency_getter_obligations(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    provenance: &accessor_provenance::Resolver<'_, '_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> (Vec<ReactiveRead>, Vec<StaticDefect>) {
    let mut reads = Vec::new();
    let mut obligations = Vec::new();
    for file in &facts.files {
        if facts.dependency_source_root(file.path.as_str()).is_some() {
            continue;
        }
        let mut allowed = None;
        let mut untracked = |span: Span| {
            let allowed = allowed.get_or_insert_with(|| allowed_callback_spans(file, lookup));
            semantic_execution_role(file, span, allowed, entities, symbol_names, lookup)
                .reports_untracked_read()
        };
        let targets = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .chain(&file.ast.iteration_targets)
            .chain(file.ast.assignments.iter().map(|a| &a.target))
            .map(|span| file.ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>();
        let callees = file
            .ast
            .calls
            .iter()
            .map(|call| file.ast.peel_ts_sugar_span(call.callee))
            .collect::<HashSet<_>>();
        let mut push = |span: Span, getters: &[Location], proven: bool| {
            let Some(getter) = getters.first() else {
                return;
            };
            if proven {
                let allowed = allowed_callback_spans(file, lookup);
                let execution =
                    semantic_execution_role(file, span, &allowed, entities, symbol_names, lookup);
                reads.push(ReactiveRead {
                    summary_attributed: false,
                    transported: true,
                    kind: "reactive-getter".into(),
                    accessor: file.source_text(span).unwrap_or("returned getter").into(),
                    location: location(file.path.shared(), span),
                    declaration: getter.clone(),
                    execution,
                    context: read_analysis_context(file, span, execution, lookup).into(),
                    via: "returned object Get".into(),
                    origin: Some(getter.clone()),
                    origin_context: "selected package getter (package integrity premise, ADR 0300)"
                        .into(),
                    uncertain: lookup.inside_possible_component(file, span),
                    missing_jsx_census: missing_jsx_census(file, span, execution),
                    host_callback_timing: host_callback_timing(file, span, execution, lookup),
                    callee_callback_timing: callee_callback_timing(file, span, execution, lookup),
                    project_consumer_non_strict: false,
                    callback_invocation_unproven: false,
                });
                return;
            }
            let package =
                facts
                    .dependency_source_root(getter.path.as_ref())
                    .map_or("a dependency", |root| {
                        root.rsplit_once("node_modules/")
                            .map_or(root, |(_, name)| name)
                    });
            obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: file.source_text(span).unwrap_or("this Get").to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), span),
                analysis_context: format!(
                    "{}{package} defines the member as a getter that reads an accessor, line {} of {}",
                    crate::DEPENDENCY_SOURCE_GETTER,
                    getter_line(lookup, getter),
                    getter
                        .path
                        .rsplit_once("node_modules/")
                        .map_or(getter.path.as_ref(), |(_, rest)| rest),
                ),
                fixes: vec![],
                uncertain: true,
            });
        };
        for member in &file.ast.members {
            if targets.contains(&member.span)
                || callees.contains(&member.span)
                || !enclosing_render_function(file, member.span, lookup)
            {
                continue;
            }
            // A literal computed key names its member as a dotted one does.
            let name = if file.ast.computed_members.contains(&member.span) {
                file.ast
                    .literal_computed_members
                    .iter()
                    .find(|key| key.span == member.span)
                    .map(|key| key.key.as_str())
            } else {
                file.source_text(member.property)
            };
            let Some(name) = name else {
                continue;
            };
            let getters = provenance.dependency_getters(file, member.object, name);
            if !getters.is_empty() && untracked(member.span) {
                let proven = provenance.proven_dependency_getters(file, member.object, name);
                push(
                    member.span,
                    proven.as_deref().unwrap_or(&getters),
                    proven.is_some(),
                );
            }
        }
        for binding in &file.ast.bindings {
            let Some(initializer) = binding.initializer.filter(|_| {
                binding.shape == solid_facts::ast::BindingShape::Object
                    && enclosing_render_function(file, binding.pattern, lookup)
            }) else {
                continue;
            };
            for slot in &binding.object_slots {
                let getters =
                    provenance.dependency_getters(file, initializer, slot.property.as_str());
                if !getters.is_empty() && untracked(binding.pattern) {
                    // Defaults, rest, nested patterns and computed keys have
                    // execution beyond the simple Get this proof describes.
                    let proven = (binding.object_slots.len() == binding.names.len()
                        && slot.plain_identifier_key)
                        .then(|| {
                            provenance.proven_dependency_getters(
                                file,
                                initializer,
                                slot.property.as_str(),
                            )
                        })
                        .flatten();
                    push(
                        binding.pattern,
                        proven.as_deref().unwrap_or(&getters),
                        proven.is_some(),
                    );
                }
            }
        }
    }
    (reads, obligations)
}

/// The one-based line of `site` in its analyzed file, for an obligation's
/// context.
pub(super) fn getter_line(lookup: &SemanticLookup<'_>, site: &Location) -> usize {
    lookup
        .file_by_path(site.path.as_ref())
        .and_then(|file| {
            let start = usize::try_from(site.start_byte).ok()?;
            file.source
                .get(..start)
                .map(|prefix| prefix.matches('\n').count() + 1)
        })
        .unwrap_or(0)
}

pub(super) fn call_role_syntax(file: &solid_facts::FileFacts) -> bool {
    crate::indexes::returns_source_root(file)
        // Nested literal receivers require a project-wide eval veto. Adding
        // or removing it must refresh unchanged callers in both directions.
        || file.ast.module_hazards.iter().any(|hazard| {
            hazard.kind == solid_facts::ast::ModuleHazardKind::Eval
        })
        || !file.ast.object_get_shapes.is_empty()
        // Provider multiplicity and returned literal identities are project
        // syntax inputs, even when their read summaries stay byte-identical.
        || !file.ast.jsx_elements.is_empty()
        || file.ast.returns.iter().any(|returned| returned.value == solid_facts::ast::ReturnValueKind::Function)
        || file.ast.functions.iter().any(|function| {
            function.r#async
                || function.expression_return.is_some()
                || function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.initializer.is_some())
                || (function.kind == solid_facts::ast::FunctionKind::Declaration
                    && function
                        .name
                        .as_ref()
                        .is_some_and(|name| crate::indexes::binding_written(file, name.span)))
        })
        || file.ast.bindings.iter().any(|binding| {
            !binding.immutable
                && binding.initializer.is_some_and(|initializer| {
                    let initializer = file.ast.peel_ts_sugar_span(initializer);
                    file.ast
                        .functions
                        .iter()
                        .any(|function| function.span == initializer)
                })
        })
}

/// ADR 0204: every project function a call of `function` enters
/// directly, `function` excluded: through a plain call written in its own body
/// (not in a nested function, not in a default parameter, not in JSX) whose
/// callee resolves to one ordinary project function, and so on from each.
/// For async functions only calls in their synchronous prefix are followed.
///
/// A cycle makes the answer empty: whether a recursive call reaches a read
/// depends on the values it is called with (`readA(2)` may never get to the
/// branch that reads). The walk is bounded; past the bound it stops, which
/// only proves less.
pub(super) fn directly_entered_functions(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    transported_calls: &HashMap<Location, accessor_provenance::Callable>,
) -> Vec<(String, Span)> {
    let mut entered = Vec::new();
    let mut path = vec![(file.path.to_string(), function.span)];
    if enter_directly_called(
        file,
        function,
        lookup,
        transported_calls,
        &mut path,
        &mut entered,
    ) {
        entered
    } else {
        Vec::new()
    }
}

/// One step of [`directly_entered_functions`]: false on a cycle.
pub(super) fn enter_directly_called(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    transported_calls: &HashMap<Location, accessor_provenance::Callable>,
    path: &mut Vec<(String, Span)>,
    entered: &mut Vec<(String, Span)>,
) -> bool {
    const MAX_DEPTH: usize = 8;
    for call in &file.ast.calls {
        // A plain call: a method call dispatches on its receiver, which a
        // subclass can override.
        let transported = transported_calls.get(&location(file.path.shared(), call.callee));
        if (!call.direct_callee && transported.is_none())
            || !body_site_runs_during_call(file, function, call.span)
            || file.ast.any_jsx_containing(call.span)
        {
            continue;
        }
        if let Some(accessor_provenance::Callable::ArrayMembers(targets)) = transported {
            for (target_path, target_span) in targets {
                let Some(callee_file) = lookup.file_by_path(target_path) else {
                    continue;
                };
                let Some(callee) = callee_file.ast.function_at(*target_span) else {
                    continue;
                };
                let key = (target_path.clone(), *target_span);
                if path.contains(&key) {
                    return false;
                }
                if entered.contains(&key) {
                    continue;
                }
                entered.push(key.clone());
                if path.len() < MAX_DEPTH {
                    path.push(key);
                    if !enter_directly_called(
                        callee_file,
                        callee,
                        lookup,
                        transported_calls,
                        path,
                        entered,
                    ) {
                        return false;
                    }
                    path.pop();
                }
            }
            continue;
        }
        let candidates = lookup.callee_symbols(file, call.callee);
        if candidates.len() > 1 {
            continue;
        }
        let Some(symbol) = candidates
            .first()
            .map(SymbolId::as_str)
            .or_else(|| lookup.callee_symbol(file, call.callee))
        else {
            continue;
        };
        let resolved = match transported {
            Some(accessor_provenance::Callable::Function { path, span }) => lookup
                .file_by_path(path)
                .and_then(|f| f.ast.function_at(*span).map(|fun| (f, fun))),
            _ => lookup.function_for_symbol(symbol),
        };
        let Some((callee_file, callee)) = resolved else {
            continue;
        };
        if call.construct
            || callee.generator
            || (transported.is_none() && !lookup.function_value_is_current(callee_file, callee))
        {
            continue;
        }
        let key = (callee_file.path.to_string(), callee.span);
        if path.contains(&key) {
            return false;
        }
        if entered.contains(&key) {
            continue;
        }
        entered.push(key.clone());
        if path.len() < MAX_DEPTH {
            path.push(key);
            if !enter_directly_called(
                callee_file,
                callee,
                lookup,
                transported_calls,
                path,
                entered,
            ) {
                return false;
            }
            path.pop();
        }
    }
    true
}

/// ADR 0338: whether `argument` is proven not to be a Solid getter, so a callee
/// calling the parameter it lands in reads nothing reactive through it: a
/// literal (function, primitive, object, array, nullish), an unwritten
/// project function (a declaration, or a `const` bound to a function
/// literal), a store (never callable), a value Type Facts proves a primitive,
///. A getter is only ever produced by a primitive, so a project function is
/// never one; a standard-library call's result may be (`slice`, `bind`).
fn argument_proven_not_a_getter(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    entities: &EntitySymbols,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if argument.spread {
        return false;
    }
    if argument.runtime_value_kind != solid_facts::ast::RuntimeValueKind::Unknown
        || crate::cleanup::callback_argument_literal(file, argument.span).is_some()
        || argument_is_proven_primitive(file, argument, lookup)
    {
        return true;
    }
    // A store proxy is never callable, but a property read off one may hold a
    // getter (`createStore({ value: get })`, then `store.value`), and a view
    // alias of such a read is recorded as a store too. Only the proxy itself
    // settles it: not a member read, and not a binding initialized by one.
    if let Some(symbol) = entities.get(&location(file.path.shared(), argument.span))
        && source_kinds.get(symbol.as_str()) == Some(&ReactiveSourceKind::Store)
        && file.ast.member_at(argument.span).is_none()
        && !argument.binding_declaration.is_some_and(|declaration| {
            file.ast.bindings.iter().any(|binding| {
                binding.names.iter().any(|name| name.span == declaration)
                    && binding
                        .initializer
                        .is_some_and(|initializer| member_initializer(file, initializer))
            })
        })
    {
        return true;
    }
    if let Some(declaration) = argument.binding_declaration
        && !crate::indexes::binding_written(file, declaration)
        && (file.ast.functions.iter().any(|function| {
            function.kind == solid_facts::ast::FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
        }) || file.ast.bindings.iter().any(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.initializer_function
                && binding.names.iter().any(|name| name.span == declaration)
        }))
    {
        return true;
    }
    // A call of a primitive wrapper's own method (`String(1).slice(0)`) or a
    // const bound to one returns a primitive or an array of primitives.
    // ADR 0338: the resolved declaration is a type fact; what runs is the
    // receiver's own method unless the receiver is proven primitive at
    // runtime (a literal, a template, a `String`/`Number` call result). Any
    // TS assertion on the way voids it: `({ slice: () => get } as unknown as
    // string).slice(0)` resolves to `String.prototype.slice` and returns a getter.
    let primitive_method_call = |call_span: Span| {
        file.ast.call_at(call_span).is_some_and(|call| {
            !call.construct
                && primitive_builtin_member_call(file, call.callee, lookup)
                && file
                    .ast
                    .member_at(call.callee)
                    .is_some_and(|member| lookup.proven_primitive(file, member.object, 6))
        })
    };
    if primitive_method_call(file.ast.peel_ts_sugar_span(argument.span)) {
        return true;
    }
    if let Some(declaration) = argument.binding_declaration
        && !crate::indexes::binding_written(file, declaration)
        && file.ast.bindings.iter().any(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.immutable
                && binding.names.iter().any(|name| name.span == declaration)
                && binding.call_initializer.is_some_and(primitive_method_call)
        })
    {
        return true;
    }
    // A local bound to a standard-library call's result is otherwise not
    // here: `slice`, `bind`, `Array.from`, `Reflect.apply` and `at` all hand a
    // getter back.
    false
}

/// Whether the expression at `initializer` is a member or element read (under
/// parentheses and TS wrappers).
fn member_initializer(file: &solid_facts::FileFacts, initializer: Span) -> bool {
    file.ast
        .member_at(file.ast.peel_ts_sugar_span(initializer))
        .is_some()
}
