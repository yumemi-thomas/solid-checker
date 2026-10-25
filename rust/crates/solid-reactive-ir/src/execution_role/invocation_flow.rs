//! Invocation flow (ADR 0319): where a function value can be invoked, followed through closed-program positions.

use super::*;

// ---------------------------------------------------------------------------
// ADR 0319: invocation flow.
//
// Where a function value written in the program can be invoked, followed
// forward through the positions a closed program gives it: bindings,
// object-literal members, spreads, returns to their call sites, arguments to
// exact project callees, component props, and the host's own event
// dispatch. Every invocation site found must be one proven role; anything the
// walk does not understand answers nothing.
// ---------------------------------------------------------------------------

/// How many holder positions one query may visit before it gives up.
pub(super) const FLOW_BUDGET: usize = 800;

/// What object a holder of the tracked value is, when the walk knows: the
/// object literal it was created as, or the props a JSX element renders a
/// component with (inside the component that element is written in, whose
/// own props came from `parent`). It answers whether calling another member
/// with that object as `this` can reach the tracked value.
#[derive(Clone)]
pub(super) enum FlowOrigin<'a> {
    Unknown,
    Object(&'a solid_facts::FileFacts, Span),
    Element(&'a solid_facts::FileFacts, Span, Box<FlowOrigin<'a>>),
}

/// A holder the walk followed: its path and span, the member path it held
/// the value at, the context (calls and rendering elements) it was in, and
/// the identity of its origin, on which receiver proofs depend.
pub(super) type FlowVisit<'a> = (
    &'a str,
    Span,
    Vec<String>,
    Vec<(&'a str, Span)>,
    Vec<(&'a str, Span)>,
);

/// A call of an exact project callee the walk entered through a parameter:
/// the callee's path and span, and the call's file and span.
pub(super) type FlowFrame<'a> = (String, Span, &'a solid_facts::FileFacts, Span);

/// A component the walk entered through its props parameter: the
/// component's path and span, the element that rendered it, and the origin
/// that element gives its props.
pub(super) struct PropsFrame<'a> {
    pub(super) path: String,
    pub(super) component: Span,
    pub(super) element_file: &'a solid_facts::FileFacts,
    pub(super) element: Span,
    /// The member path of the props the walk entered with.
    pub(super) followed: Vec<String>,
    pub(super) origin: FlowOrigin<'a>,
}

/// One way the tracked value is invoked: at a call expression (with the
/// callee activations the walk was inside when it found it), or by the host
/// in a role (an intrinsic element's event dispatch).
#[derive(Clone)]
pub(super) enum Invocation<'a> {
    Call(&'a solid_facts::FileFacts, Span, Vec<FlowFrame<'a>>),
    Role(ExecutionRole),
}

pub(super) struct InvocationFlow<'a, 'l, 'c> {
    pub(super) lookup: &'l SemanticLookup<'a>,
    pub(super) classifying: &'c mut HashSet<(String, Span)>,
    pub(super) budget: usize,
    /// Holders already followed in the current invocation census, with the
    /// call context they were reached in.
    pub(super) seen: HashSet<FlowVisit<'a>>,
    /// Calls of exact project callees the walk entered through a parameter:
    /// a return from that callee goes to that call, not to every call.
    pub(super) frames: Vec<FlowFrame<'a>>,
    /// Components the walk is inside through their props parameter, with
    /// the element that rendered each: what `props.x` reads there.
    pub(super) props_stack: Vec<PropsFrame<'a>>,
    /// The owner question ([`unowned_event_literals`]): a primitive's
    /// callback slot refuses, and a call's role comes only from the walk
    /// itself or a compiler event-handler fact, never from the read role at
    /// the call (which `createRoot` and `runWithOwner` pass through from
    /// their caller although they supply an owner).
    pub(super) owner_mode: bool,
    /// The owner question's other census (ADR 0396): the entries that count
    /// are the function literals a reviewed test runner invokes (a
    /// registration's callback, [`SemanticLookup::is_test_runner_registration`]),
    /// not an event delivery. Meaningful only with `owner_mode`.
    pub(super) test_entries: bool,
    /// ADR 0400: the write question. A call's role is the write classifier's
    /// role at the call under this guard, and the join keeps an owned entry
    /// as the answer (ADR 0378). Never combined with `owner_mode`.
    pub(super) write_root_body: Option<RootBodyGuard>,
    /// The modules this walk read (a holder, a callee, a caller, a rendered
    /// component): an answer that read another module than the literal's own
    /// depends on that module's contents (ADR 0400's digest binding).
    pub(super) touched: HashSet<&'a str>,
}

/// ADR 0319: the one role every invocation of the function literal directly
/// around `span` runs in, followed forward through the program. Reads only:
/// write legality never asks.
pub(super) fn invocation_flow_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    if classifying.contains(&write_classification_marker()) || !lookup.program_closed {
        return None;
    }
    let file = lookup.file_by_path(file.path.as_str())?;
    let literal = containing_ast_function(&file.ast, span)?;
    // An async function runs in its invocation's role only up to its first
    // await; what may follow one is a continuation no invocation places.
    if !literal.body.contains(span) || (literal.r#async && may_follow_suspension(file, span)) {
        return None;
    }
    // Every answer is computed once per build, context-free, and the set of
    // answers is what the cross-file proof digest binds: a cached fragment
    // that consumed one is reused only while every answer is unchanged.
    if COMPUTING_FLOW_ANSWERS.with(std::cell::Cell::get) {
        return literal_flow_role(file, literal, lookup, classifying);
    }
    lookup
        .invocation_flow_answers()
        .get(&(file.path.to_string(), literal.span))
        .copied()
}

thread_local! {
    /// Set while [`invocation_flow_answers`] computes the answers: a nested
    /// role query then computes directly instead of re-entering the memo.
    static COMPUTING_FLOW_ANSWERS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// ADR 0319: the invocation-flow answer of every function literal of a
/// closed program, keyed by path and literal span; only proven roles are
/// kept. Computed once per build, each answer from an empty context.
pub(crate) fn invocation_flow_answers(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), ExecutionRole> {
    let mut answers = HashMap::default();
    if !lookup.program_closed {
        return answers;
    }
    let outer = COMPUTING_FLOW_ANSWERS.with(|computing| computing.replace(true));
    for file in lookup.files() {
        for literal in &file.ast.functions {
            let mut classifying = HashSet::default();
            if let Some(role) = literal_flow_role(file, literal, lookup, &mut classifying) {
                answers.insert((file.path.to_string(), literal.span), role);
            }
        }
    }
    COMPUTING_FLOW_ANSWERS.with(|computing| computing.set(outer));
    answers
}

/// The flow answer for one literal, computed in the context `classifying`.
pub(super) fn literal_flow_role(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let file = lookup.file_by_path(file.path.as_str())?;
    let literal = file.ast.function_at(literal.span)?;
    if literal.generator || lookup.function_component_status(file, literal) != ComponentStatus::No {
        return None;
    }
    if lookup.files().iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
    }) {
        return None;
    }
    let key = (format!("\0flow:{}", file.path.as_str()), literal.span);
    if !classifying.insert(key.clone()) {
        return None;
    }
    let mut flow = InvocationFlow {
        lookup,
        classifying,
        budget: FLOW_BUDGET,
        seen: HashSet::default(),
        frames: Vec::new(),
        props_stack: Vec::new(),
        owner_mode: false,
        test_entries: false,
        write_root_body: None,
        touched: HashSet::default(),
    };
    let role = flow.function_role(file, literal).flatten();
    flow.classifying.remove(&key);
    role
}

thread_local! {
    /// Set while [`write_flow_answers`] computes the table: the write walk,
    /// asked at an invocation site, then skips its own use of the table, so
    /// every answer is computed from an empty context and no order of
    /// computation matters.
    static COMPUTING_WRITE_FLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The key of a write-flow answer: the literal and the guard of the write.
pub(crate) type WriteFlowKey = (String, Span, RootBodyGuard);

/// ADR 0400: the write-flow answer of every function a closed program hands
/// to project code (a component prop value, an argument of a call that is no
/// primitive, a named function), per guard, computed once per build. Only a
/// proven role is kept, with whether the walk read another module than the
/// function's own; see [`literal_write_flow_role`].
pub(crate) fn write_flow_answers(
    lookup: &SemanticLookup<'_>,
) -> HashMap<WriteFlowKey, (ExecutionRole, bool)> {
    let mut answers = HashMap::default();
    let outer = COMPUTING_WRITE_FLOW.with(|computing| computing.replace(true));
    for file in lookup.files() {
        for literal in &file.ast.functions {
            if !write_flow_candidate(file, literal) {
                continue;
            }
            for guard in [RootBodyGuard::Rejects, RootBodyGuard::Exempts] {
                if let Some(answer) = literal_write_flow_role(file, literal, guard, lookup) {
                    answers.insert((file.path.to_string(), literal.span, guard), answer);
                }
            }
        }
    }
    COMPUTING_WRITE_FLOW.with(|computing| computing.set(outer));
    clear_write_site_roles();
    answers
}

/// Whether `literal` is handed to other code where it is written: the value
/// of a non-intrinsic JSX attribute, a whole argument of a call, or a
/// function bound to a name. A function that is none of these has no
/// invocation this walk could follow.
fn write_flow_candidate(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
) -> bool {
    !literal.generator
        && (component_prop_literal(file, literal.span).is_some()
            || file
                .ast
                .arguments_containing(literal.span)
                .any(|(call, index)| {
                    !call.arguments[index].spread
                        && !call.construct
                        && file.ast.peel_ts_sugar_span(call.arguments[index].span) == literal.span
                })
            || named_function_handed_on(file, literal))
}

/// Whether a function bound to a name is handed on somewhere in its own file:
/// a whole argument (`withTx(save)`), a JSX attribute's value
/// (`<Child onPress={save} />`) or the initializer of an alias binding
/// (`const again = save`). A function every use of which is a call or an
/// intrinsic handler is the write walk's own business. A named function
/// expression is held by the binding it initializes as well as by its name.
fn named_function_handed_on(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
) -> bool {
    let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
    let mut holders = Vec::new();
    holders.extend(function_binding_name(file, literal).map(|name| name.span));
    if literal.kind == solid_facts::ast::FunctionKind::Expression && literal.name.is_some() {
        holders.extend(
            file.ast
                .bindings_initializer_containing(literal.span)
                .find(|binding| {
                    binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding
                            .initializer
                            .is_some_and(|initializer| peel(initializer) == literal.span)
                })
                .and_then(|binding| binding.names.first())
                .map(|name| name.span),
        );
    }
    file.ast
        .reference_declarations
        .iter()
        .filter(|(reference, declaration)| {
            holders.contains(declaration) && !holders.contains(reference)
        })
        .any(|(reference, _)| {
            file.ast
                .arguments_containing(*reference)
                .any(|(call, index)| {
                    !call.arguments[index].spread && peel(call.arguments[index].span) == *reference
                })
                || file.ast.jsx_containing(*reference).any(|element| {
                    !intrinsic_element(file, element)
                        && element.attributes.iter().any(|attribute| {
                            attribute
                                .expression
                                .is_some_and(|value| peel(value) == *reference)
                        })
                })
                || file.ast.bindings.iter().any(|binding| {
                    binding
                        .initializer
                        .is_some_and(|initializer| peel(initializer) == *reference)
                })
        })
}

/// The one role every invocation of `literal` runs in, for a write in it
/// (ADR 0400): followed forward as ADR 0319 follows a value, each call's role
/// the write classifier's at the call. An invocation in an owned scope makes
/// the answer that role, as ADR 0378's entries rule has it; every other
/// invocation must be one proven role, and any census gap or unplaced
/// invocation answers nothing. Open or closed program alike: what a callee
/// does with the value does not depend on its other callers, and an exported
/// function holding the value's invocation refuses unless the program is
/// closed ([`InvocationFlow::exported_call_sites`]).
fn literal_write_flow_role(
    file: &solid_facts::FileFacts,
    literal: &solid_facts::ast::FunctionFact,
    root_body: RootBodyGuard,
    lookup: &SemanticLookup<'_>,
) -> Option<(ExecutionRole, bool)> {
    let file = lookup.file_by_path(file.path.as_str())?;
    let literal = file.ast.function_at(literal.span)?;
    if literal.generator
        || lookup.function_component_status(file, literal) != ComponentStatus::No
        || lookup.files().iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        })
    {
        return None;
    }
    let mut classifying = HashSet::default();
    classifying.insert((format!("\0write-flow:{}", file.path.as_str()), literal.span));
    let mut flow = InvocationFlow {
        lookup,
        classifying: &mut classifying,
        budget: FLOW_BUDGET,
        seen: HashSet::default(),
        frames: Vec::new(),
        props_stack: Vec::new(),
        owner_mode: false,
        test_entries: false,
        write_root_body: Some(root_body),
        touched: HashSet::default(),
    };
    let role = flow.function_role(file, literal).flatten()?;
    let across_modules = flow.touched.iter().any(|path| *path != file.path.as_str());
    Some((role, across_modules))
}

/// ADR 0400: the role the invocations of `span`'s function (or of a function
/// around it that runs `span` in place) give a write there, from the per-build
/// table. `None` while the table is computed, in an open program, and for a
/// function the walk proves nothing about.
pub(super) fn write_flow_role(
    file: &solid_facts::FileFacts,
    span: Span,
    root_body: RootBodyGuard,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    if COMPUTING_WRITE_FLOW.with(std::cell::Cell::get) {
        return None;
    }
    let innermost = containing_ast_function(&file.ast, span)?;
    if !innermost.body.contains(span) {
        return None;
    }
    let answers = lookup.write_flow_answers();
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.span.end - function.span.start);
    for function in enclosing {
        if !runs_in_place_within(file, innermost, function, lookup) {
            return None;
        }
        if let Some((role, _)) = answers
            .get(&(file.path.to_string(), function.span, root_body))
            .copied()
        {
            return Some(role);
        }
    }
    None
}
