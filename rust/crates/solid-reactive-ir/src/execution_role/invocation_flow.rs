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

/// Whether this thread is computing [`invocation_flow_answers`].
pub(super) fn computing_flow_answers() -> bool {
    COMPUTING_FLOW_ANSWERS.with(std::cell::Cell::get)
}

/// ADR 0319: the invocation-flow answer of every function literal of a
/// closed program, keyed by path and literal span; only proven roles are
/// kept. Computed once per build, each answer from an empty context.
///
/// The answers are independent, so they are computed in parallel, each on a
/// thread that is computing the answers (a nested role query there computes
/// directly, as on the caller's thread), and inserted in file and literal
/// order. A table one of them first forces is forced inside this
/// computation in any order, and its initializer receives no context from
/// the literal that forced it.
pub(crate) fn invocation_flow_answers(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), ExecutionRole> {
    let mut answers = HashMap::default();
    if !lookup.program_closed {
        return answers;
    }
    let outer = COMPUTING_FLOW_ANSWERS.with(|computing| computing.replace(true));
    let literals = lookup
        .files()
        .iter()
        .flat_map(|file| {
            file.ast
                .functions
                .iter()
                .map(move |literal| (file, literal))
        })
        .collect::<Vec<_>>();
    let roles = crate::pipeline::parallel_balanced_results(&literals, |(file, literal)| {
        let outer = COMPUTING_FLOW_ANSWERS.with(|computing| computing.replace(true));
        let mut classifying = HashSet::default();
        let role = literal_flow_role(file, literal, lookup, &mut classifying);
        COMPUTING_FLOW_ANSWERS.with(|computing| computing.set(outer));
        role
    });
    for ((file, literal), role) in literals.into_iter().zip(roles) {
        if let Some(role) = role {
            answers.insert((file.path.to_string(), literal.span), role);
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
    if lookup.program_has_eval() {
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
    };
    let role = flow.function_role(file, literal).flatten();
    flow.classifying.remove(&key);
    role
}
