//! Unknown-callback obligations, unaccounted parameter escapes, and direct protocol parameters.

use super::*;

pub(super) fn unknown_callback_execution_message(
    file: &solid_facts::FileFacts,
    callee: Span,
    parameter: usize,
    lookup: &SemanticLookup<'_>,
) -> String {
    let target = file.source_text(callee).unwrap_or("<unknown>");
    let Some(resolved) = lookup.resolved_callee_call(file, callee) else {
        return format!(
            "parameter {parameter} is passed to {target}, but no resolved call fact or dependency source proves when it executes"
        );
    };
    let declaration = resolved.declaration.as_ref();
    let callable_type = resolved_parameter(resolved, parameter)
        .and_then(|parameter| parameter.type_descriptor.as_ref())
        .map(|descriptor| descriptor.text.as_ref())
        .filter(|text| !text.is_empty())
        .unwrap_or("callable value");
    let qualified = declaration
        .and_then(|declaration| {
            (!declaration.qualified_name.is_empty()).then_some(declaration.qualified_name.as_ref())
        })
        .or_else(|| {
            declaration.and_then(|declaration| {
                (!declaration.name.is_empty()).then_some(declaration.name.as_ref())
            })
        })
        .unwrap_or(target);
    let package = declaration
        .and_then(|declaration| {
            (!declaration.origin_module.is_empty()).then_some(declaration.origin_module.as_ref())
        })
        .or_else(|| {
            declaration.and_then(|declaration| {
                (!declaration.source_file.is_empty()).then_some(declaration.source_file.as_ref())
            })
        })
        .unwrap_or("the current project");
    format!(
        "parameter {parameter} ({callable_type}) is passed to resolved {qualified} from {package}, but no dependency source proves when it executes"
    )
}

pub(super) fn package_entrypoint(module: &str) -> (String, String) {
    let mut parts = module.split('/');
    let first = parts.next().filter(|part| !part.is_empty());
    let Some(first) = first else {
        return ("current project".into(), ".".into());
    };
    let package = if first.starts_with('@') {
        let Some(scope_package) = parts.next() else {
            return (module.into(), ".".into());
        };
        format!("{first}/{scope_package}")
    } else {
        first.to_owned()
    };
    let entrypoint = parts.collect::<Vec<_>>().join("/");
    if entrypoint.is_empty() {
        (package, ".".into())
    } else {
        (package, format!("./{entrypoint}"))
    }
}

pub(super) fn unknown_callback_obligation(
    file: &solid_facts::FileFacts,
    node: &SummaryNode,
    callee: Span,
    parameter: usize,
    location: Location,
    lookup: &SemanticLookup<'_>,
) -> UnresolvedCallbackObligation {
    let resolved = lookup.resolved_callee_call(file, callee);
    let declaration = resolved.and_then(|call| call.declaration.as_ref());
    let module = declaration
        .and_then(|declaration| {
            (!declaration.origin_module.is_empty()).then_some(declaration.origin_module.as_ref())
        })
        .unwrap_or_default();
    let (package, entrypoint) = package_entrypoint(module);
    let parameter_type = resolved
        .and_then(|call| resolved_parameter(call, parameter))
        .and_then(|parameter| parameter.type_descriptor.as_ref())
        .map(|descriptor| descriptor.text.to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "callable value".into());
    let function = node.name.clone().unwrap_or_else(|| "<anonymous>".into());
    UnresolvedCallbackObligation {
        function,
        function_symbol: node
            .symbol
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        function_identity: node.runtime_identity.clone(),
        parameter,
        package,
        entrypoint,
        parameter_type,
        location,
        message: format!(
            "{}; this callback cannot be certified until its callee's source is available to the analysis",
            unknown_callback_execution_message(file, callee, parameter, lookup)
        ),
    }
}

pub(super) fn accessor_argument_summaries(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> Vec<Option<ReturnSummary>> {
    let mut arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let symbol = entities.get(&location(file.path.shared(), argument.span))?;
            let (display, _) = super::transport_reads::accessor(accessors, symbol)?;
            Some(ReturnSummary {
                kind: "accessor".into(),
                label: display.to_string(),
                ..ReturnSummary::default()
            })
        })
        .collect::<Vec<_>>();
    while arguments.last().is_some_and(Option::is_none) {
        arguments.pop();
    }
    arguments
}

/// Parameters whose caller-supplied value this function *retains*.
///
/// The call loop below classifies every *argument* position, and a call into a
/// local function is summarized transitively — on the assumption that the
/// callee's own summary accounts for what it does with that parameter. That
/// assumption fails the moment the callee merely **stores** the value:
///
/// ```js
/// function createComputation(fn, init) {          // solid-js 1.9.14
///   const c = { fn, value: init, /* … */ };       // fn is retained, never called
///   return c;
/// }
/// ```
///
/// `createComputation` invokes nothing, so its callback summary is empty, so
/// every export that forwards a callback into it — `createMemo`,
/// `createEffect`, `children`, `createSelector`, `createDeferred`,
/// `createRenderEffect`, `createComputed` — recorded *no* callback summary. An
/// omitted callback summary is a **negative** claim, so those summaries
/// would certify "invokes no caller-supplied function" for the seven
/// primitives whose whole purpose is to invoke one.
///
/// Retention is stated as a closed list of positions, never as "everything the
/// analysis did not recognize". The difference is the whole precision budget:
/// a published runtime artifact is dense with references that *observe* a
/// parameter — `typeof value === "string"`, `prev && …`, `for (const key in
/// props)`, `value[HREF]`, `node[name] = value`, a reassignment of the
/// parameter itself — and treating those as escapes turns a third of a DOM
/// package's exports into sentinels while proving nothing. The positions that
/// do put the value somewhere this analysis stops following are:
///
/// - an **object-literal property value** (`{ fn }`, `{ fn: p }`) — the
///   literal outlives the call, and whoever receives it may invoke the
///   property. This is exactly `createComputation`;
/// - an **assignment value** (`source = pSource`) — stored into a binding or a
///   container whose later use this summary does not track. Storing into a
///   member chain rooted at one of the *caller's own* parameters is excluded:
///   `node.className = value` writes into a container the caller supplied and
///   the caller's code is analyzed too;
/// - a **computed read of a rest parameter** (`sources[index]`) — a rest
///   parameter absorbs an unbounded argument tail that no `callbacks` row can
///   name, so anything at all happening to one of its elements is unstatable
///   as a closed callback claim.
///
/// Only *potentially callable* parameters can escape into a callback claim, so
/// a parameter the type facts prove non-callable never opens the sentinel.
pub(super) fn push_unaccounted_parameter_escapes(
    contribution: &mut InterproceduralGraphContribution,
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
) {
    let owners = parameter_declaration_owners(file, nodes, nodes_by_path, entities);
    if owners.is_empty() {
        return;
    }
    let mut retained = HashSet::default();
    for property in file
        .ast
        .object_properties
        .iter()
        .filter(|property| property.data && !property.computed)
    {
        retained.insert(file.ast.peel_ts_sugar_span(property.value));
        retained.insert(property.value);
    }
    for assignment in &file.ast.assignments {
        if member_root_is_parameter(file, assignment.target, &owners) {
            continue;
        }
        retained.insert(file.ast.peel_ts_sugar_span(assignment.value_span));
        retained.insert(assignment.value_span);
    }
    // A returned conditional can expose the same caller-supplied callable in
    // two incompatible forms: `fn.length ? () => fn(value) : fn`. The nested
    // branch contains an invocation, while the sibling branch returns `fn`
    // itself. Treating the nested call as the helper's own callback timing is
    // unsound; which callable escapes is chosen only when the helper runs.
    //
    // Keep this deliberately narrower than "a returned function closes over a
    // parameter". Returned schedulers such as `() => setTimeout(fn)` have an
    // exact deferred summary, and a plain `return fn` is a relational return,
    // not evidence that the exporting function invokes `fn`.
    for returned in file.ast.returns.iter().chain(
        file.ast
            .functions
            .iter()
            .filter_map(|function| function.expression_return.as_ref()),
    ) {
        if let Some(argument) = returned.argument {
            let nested = if file.ast.call_at(argument).is_none() {
                file.ast.functions_within(argument).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            for identifier in file.ast.identifiers.iter().filter(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && nested
                        .iter()
                        .any(|function| function.span.contains(identifier.span))
            }) {
                let Some(declaration) = file.ast.reference_declaration(identifier.span) else {
                    continue;
                };
                let also_returned_directly = file.ast.identifiers.iter().any(|candidate| {
                    candidate.role == solid_facts::ast::IdentifierRole::Reference
                        && argument.contains(candidate.span)
                        && !nested
                            .iter()
                            .any(|function| function.span.contains(candidate.span))
                        && file.ast.reference_declaration(candidate.span) == Some(declaration)
                });
                if also_returned_directly {
                    retained.insert(identifier.span);
                }
            }
        }
    }
    for member in file.ast.members.iter().filter(|member| {
        file.ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
    }) {
        if owners_rest_parameter(file, member.object, &owners) {
            retained.insert(member.object);
        }
    }
    let mut seen = HashSet::default();
    for identifier in &file.ast.identifiers {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference
            || !retained.contains(&identifier.span)
        {
            continue;
        }
        let Some(declaration) = file.ast.reference_declaration(identifier.span) else {
            continue;
        };
        let Some(&(owner, parameter)) = owners.get(&declaration) else {
            continue;
        };
        if !seen.insert((owner, parameter)) {
            continue;
        }
        if !potentially_callable(
            lookup.smallest_contained_callability(file.path.as_str(), identifier.span),
        ) {
            continue;
        }
        contribution
            .escaped_parameters
            .push((nodes[owner].span, parameter));
    }
}

/// Whether `object` is a reference to a rest parameter of a summarized
/// function.
pub(super) fn owners_rest_parameter(
    file: &solid_facts::FileFacts,
    object: Span,
    owners: &HashMap<Span, (usize, usize)>,
) -> bool {
    file.ast
        .reference_declaration(object)
        .and_then(|declaration| owners.get(&declaration))
        .is_some_and(|(_, parameter)| *parameter == REST_PARAMETER)
}

/// Whether `target` is a member chain rooted at one of this file's summarized
/// parameters — a container the caller handed in.
pub(super) fn member_root_is_parameter(
    file: &solid_facts::FileFacts,
    target: Span,
    owners: &HashMap<Span, (usize, usize)>,
) -> bool {
    let mut current = target;
    for _ in 0..32 {
        let Some(member) = file.ast.member_at(current) else {
            break;
        };
        current = member.object;
    }
    if current == target {
        return false;
    }
    file.ast
        .reference_declaration(current)
        .is_some_and(|declaration| owners.contains_key(&declaration))
}

/// The slot a rest parameter's escape is recorded under.
///
/// A rest parameter absorbs an unbounded tail of argument positions and has no
/// single index, so it is deliberately absent from
/// [`SummaryNode::parameters`]. An escape through it still has to reach the
/// callbacks domain, and this index — which no real slot can take — is how it
/// travels without ever being mistaken for a stated parameter.
pub(super) const REST_PARAMETER: usize = usize::MAX;

/// Every summarized parameter's declaration span, and the slot it fills.
///
/// The slot indices have to be the ones the rest of the pipeline uses, so the
/// filter chain here is exactly [`function_nodes`]': identifier-shaped
/// parameters whose binding name carries a compiler entity, in declaration
/// order. A rest parameter fills no single slot and is recorded under
/// [`REST_PARAMETER`].
pub(super) fn parameter_declaration_owners(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    entities: &EntitySymbols,
) -> HashMap<Span, (usize, usize)> {
    let mut owners = HashMap::default();
    for (index, node) in functions_for_path(nodes, nodes_by_path, file.path.as_str()) {
        let Some(function) = file.ast.function_at(node.span) else {
            continue;
        };
        let spans = function
            .parameters
            .iter()
            .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
            .filter_map(|parameter| parameter.names.first())
            .filter(|name| {
                entities
                    .get(&location(file.path.shared(), name.span))
                    .is_some()
            })
            .map(|name| name.span);
        for (parameter, span) in spans.enumerate() {
            owners.entry(span).or_insert((index, parameter));
        }
        for name in &function.rest_parameter_names {
            owners.entry(name.span).or_insert((index, REST_PARAMETER));
        }
    }
    owners
}

/// Record an unknown-callback obligation for every caller-supplied callable
/// forwarded into a call whose callee has no resolvable identity at all.
///
/// The obligation is only meaningful for an argument that is a parameter of
/// an enclosing analyzed function: that is the value the *caller* supplies,
/// and therefore the one an export's summary has to describe. A local value or
/// an inline literal invoked by an unresolved callee is the callee's business,
/// not the exported surface's.
pub(super) fn push_unresolved_callee_callback_obligations(
    contribution: &mut InterproceduralGraphContribution,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    nodes: &[SummaryNode],
    function_lookup: &FunctionLookup,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
) {
    for argument in &call.arguments {
        if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
            continue;
        }
        if !potentially_callable(
            lookup.smallest_contained_callability(file.path.as_str(), argument.span),
        ) || literal_argument_is_not_callable(argument.runtime_value_kind)
        {
            continue;
        }
        let Some(argument_symbol) = entities.get(&location(file.path.shared(), argument.span))
        else {
            continue;
        };
        let Some(&(callback_owner, parameter)) =
            function_lookup.parameter_owner.get(argument_symbol)
        else {
            continue;
        };
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
}

/// The parameters each function of `file` reads a property of, or coerces,
/// directly in its own body (item A of ways-to-improve § 3.3): the proposal
/// input from which the generator describes a non-call `callbacks` item
/// (ADR 0006: the generator derives the item itself, and the implementation
/// census confirms it site for site).
///
/// Syntactic and exact, from Oxc's own scope resolution, and deliberately
/// narrow: the parameter is a plain identifier with no default and is written
/// nowhere in the file; the use sits in the function's own body, outside any
/// nested callable; for a `get`, the member's object -- after transparent
/// wrappers only -- *is* the parameter's identifier, the member is not the
/// callee of a call (`v.x()` is a member invocation, not a described read),
/// and it is not in write position; for a `coerce`, the coercing operand is
/// the parameter's identifier. Anything wider (an alias, a chain root, a
/// destructured name) derives nothing, and the census then refuses the
/// enumeration as understating -- the fail-closed direction.
///
/// Item B of the same section rides on the same premises: a member that *is*
/// the callee of a call (not a `new`), whose key is a literal naming one
/// property (`handler[0]`, `h["run"]`, `solid_facts::ast::AstFacts::
/// literal_computed_members`), and whose object is directly such a
/// parameter, is a call of the caller's member at that path -- the
/// [`DirectProtocolUses::member_calls`] entry the pass writes a member-path
/// `inline` row for. Reading the member runs the caller's getter or trap
/// first, so the same site also derives that parameter's `get`. A dotted
/// member callee (`props.onClick()`) derives neither: that is ADR 0101's
/// `parameter-member` read, unchanged.
///
/// The same premise bounds ADR 0100's bare call: the pass records a direct
/// own call of a parameter as describable only when that parameter is in
/// [`DirectProtocolUses::unwritten_parameters`], since `cb = other; cb()` is
/// a call of `cb` by syntax and of `other` by value.
pub(super) fn direct_protocol_parameters(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
) -> DirectProtocolUses {
    use crate::summary_semantics::InvokeProtocol;
    let ast = &file.ast;
    let Some(indices) = nodes_by_path.get(file.path.as_str()) else {
        return DirectProtocolUses::default();
    };
    let functions_by_body = ast
        .functions
        .iter()
        .map(|function| (function.body, function))
        .collect::<HashMap<_, _>>();
    // The candidate parameters first: a plain identifier with no default, of a
    // function a summary node names. Everything below is keyed by the
    // declaration span, so each table is walked once per file however many
    // functions it holds.
    let mut parameters = HashMap::<Span, (Span, usize, Span)>::default();
    for node in indices.iter().map(|index| &nodes[*index]) {
        let Some(function) = functions_by_body.get(&node.body) else {
            continue;
        };
        for (index, parameter) in function.parameters.iter().enumerate() {
            if let [name] = parameter.names.as_slice()
                && parameter.shape == solid_facts::ast::BindingShape::Identifier
                && parameter.initializer.is_none()
            {
                parameters.insert(name.span, (node.span, index, function.body));
            }
        }
    }
    if parameters.is_empty() {
        return DirectProtocolUses::default();
    }
    // Every write position, sorted by start with the running maximum end, so
    // "does some write contain this span" is a binary search and a short walk
    // back rather than a scan of every assignment.
    let mut written = ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(ast.iteration_targets.iter().copied())
        .collect::<Vec<_>>();
    written.sort_unstable_by_key(|target| (target.start, target.end));
    let reach = written
        .iter()
        .scan(0, |end, target| {
            *end = (*end).max(target.end);
            Some(*end)
        })
        .collect::<Vec<_>>();
    let in_write = |span: Span| {
        let mut index = written.partition_point(|target| target.start <= span.start);
        while index > 0 {
            index -= 1;
            if reach[index] < span.end {
                return false;
            }
            if written[index].contains(span) {
                return true;
            }
        }
        false
    };
    let written_parameters = ast
        .reference_declarations
        .iter()
        .filter(|(reference, target)| parameters.contains_key(target) && in_write(*reference))
        .map(|(_, target)| *target)
        .collect::<HashSet<_>>();
    let unwritten = parameters
        .keys()
        .copied()
        .filter(|declaration| !written_parameters.contains(declaration))
        .collect::<HashSet<_>>();
    let callees = ast
        .calls
        .iter()
        .flat_map(|call| [call.callee, ast.peel_ts_sugar_span(call.callee)])
        .collect::<HashSet<_>>();
    // The parameter a span names directly, after transparent wrappers only,
    // when the use sits in that parameter's own function body and outside any
    // callable nested in it.
    let parameter_at = |span: Span, used_at: Span| {
        let declaration = ast.reference_declaration(ast.peel_ts_sugar_span(span))?;
        if !unwritten.contains(&declaration) {
            return None;
        }
        let (owner, index, body) = parameters[&declaration];
        crate::owners::containing_ast_function(ast, used_at)
            .is_some_and(|innermost| innermost.body == body)
            .then_some((owner, index))
    };
    let call_callees = ast
        .calls
        .iter()
        .filter(|call| !call.construct)
        .flat_map(|call| [call.callee, ast.peel_ts_sugar_span(call.callee)])
        .collect::<HashSet<_>>();
    let mut uses = std::collections::BTreeSet::new();
    let mut member_calls = std::collections::BTreeSet::new();
    for member in &ast.members {
        if in_write(member.span) {
            continue;
        }
        if callees.contains(&member.span) {
            if call_callees.contains(&member.span)
                && let Ok(position) = ast
                    .literal_computed_members
                    .binary_search_by_key(&member.span, |fact| fact.span)
                && let Some((owner, index)) = parameter_at(member.object, member.span)
            {
                member_calls.insert((
                    member.span,
                    owner,
                    index,
                    vec![ast.literal_computed_members[position].key.to_string()],
                ));
                uses.insert((owner, InvokeProtocol::Get, index));
            }
            continue;
        }
        if let Some((owner, index)) = parameter_at(member.object, member.span) {
            uses.insert((owner, InvokeProtocol::Get, index));
        }
    }
    for operand in &ast.coercing_operands {
        if let Some((owner, index)) = parameter_at(*operand, *operand) {
            uses.insert((owner, InvokeProtocol::Coerce, index));
        }
    }
    // An iteration of a parameter's value, or of a value reached through its
    // members (`const [x, y] = point`, `const [xi] = polygon[i]`, `for (const
    // item of options.items)`), anywhere in the function, nested callables
    // included, and an array pattern in parameter position. The generator
    // derives no `iterate` item, so any of these leaves a `callbacks`
    // enumeration it could not describe whole; recorded so it declines to
    // propose rather than publish a closure the census must refuse. Unlike the
    // two tables above this asks nothing of writes or of the innermost frame:
    // it only ever withholds a proposal.
    let member_objects = ast
        .members
        .iter()
        .map(|member| (member.span, member.object))
        .collect::<HashMap<_, _>>();
    let all_parameters = indices
        .iter()
        .map(|index| &nodes[*index])
        .filter_map(|node| Some((node, *functions_by_body.get(&node.body)?)))
        .flat_map(|(node, function)| {
            function
                .parameters
                .iter()
                .enumerate()
                .flat_map(move |(index, parameter)| {
                    parameter
                        .names
                        .iter()
                        .map(move |name| (name.span, (node.span, index, function.body)))
                })
        })
        .collect::<HashMap<_, _>>();
    for operand in &ast.iterated_operands {
        let mut root = ast.peel_ts_sugar_span(*operand);
        let mut hops = 0;
        while let Some(object) = member_objects.get(&root) {
            root = ast.peel_ts_sugar_span(*object);
            hops += 1;
            if hops > 64 {
                break;
            }
        }
        if let Some(&(owner, index, body)) = ast
            .reference_declaration(root)
            .and_then(|declaration| all_parameters.get(&declaration))
            && body.contains(*operand)
            && parameters_by_shape_is_whole(ast, body, index)
        {
            uses.insert((owner, InvokeProtocol::Iterate, index));
        }
    }
    for node in indices.iter().map(|index| &nodes[*index]) {
        let Some(function) = functions_by_body.get(&node.body) else {
            continue;
        };
        for (index, parameter) in function.parameters.iter().enumerate() {
            if parameter.shape == solid_facts::ast::BindingShape::Array {
                uses.insert((node.span, InvokeProtocol::Iterate, index));
            }
        }
    }
    DirectProtocolUses {
        protocols: uses.into_iter().collect(),
        member_calls: member_calls.into_iter().collect(),
        unwritten_parameters: unwritten
            .iter()
            .map(|declaration| {
                let (owner, index, _) = parameters[declaration];
                (owner, index)
            })
            .collect(),
    }
}

/// What [`direct_protocol_parameters`] derives for one file.
#[derive(Default)]
pub(super) struct DirectProtocolUses {
    /// `(owner, protocol, parameter index)`: item A's `get` and `coerce`
    /// uses, and the iterations that forbid describing an enumeration whole.
    pub(super) protocols: Vec<(Span, crate::summary_semantics::InvokeProtocol, usize)>,
    /// `(callee member span, owner, parameter index, path)`: item B's calls
    /// of a literal-keyed member of the parameter's own value.
    pub(super) member_calls: Vec<(Span, Span, usize, Vec<String>)>,
    /// `(owner, parameter index)` of every plain, undefaulted parameter
    /// written nowhere in the file: the only parameters whose bare call is
    /// the caller's value, and so the only ones a described call item is
    /// proposed for (`direct_callback_parameters`).
    pub(super) unwritten_parameters: HashSet<(Span, usize)>,
}

/// Whether the parameter at `index` of the function whose body is `body` binds
/// the argument itself (a plain identifier), so a reference to one of its
/// names is a reference to the caller's value rather than to one slot of it.
pub(super) fn parameters_by_shape_is_whole(
    ast: &solid_facts::ast::AstFacts,
    body: Span,
    index: usize,
) -> bool {
    ast.functions
        .iter()
        .find(|function| function.body == body)
        .and_then(|function| function.parameters.get(index))
        .is_some_and(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
}
