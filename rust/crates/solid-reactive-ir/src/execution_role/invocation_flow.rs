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

/// What the walk asks of the program beyond the syntax of the files it holds.
///
/// Two worlds answer it. [`LookupFlow`] asks the build's lookup directly: a
/// nested evaluation inside another computation (a role query the walk's own
/// site classification makes), whose reads that computation covers. The memo
/// world ([`super::flow_memos`]) answers each question through a recorded
/// read, so a top-level answer is a memo whose record lists every file the
/// walk visited and every table it consulted. Every file the walk holds comes
/// from one of these answers (or from the key), so the syntax it reads is a
/// recorded read too.
pub(super) trait FlowWorld<'a> {
    fn dialect(&self) -> &'a dyn solid_dialect::Dialect;
    /// The facts the world holds for `file`'s path (the lookup's last slot).
    fn canonical(&self, file: &'a solid_facts::FileFacts) -> Option<&'a solid_facts::FileFacts>;
    /// Whether some module calls direct `eval`.
    fn program_has_eval(&self) -> bool;
    fn component_status(
        &self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> ComponentStatus;
    /// The primitive resolved for the call occupying exactly `call`.
    fn primitive_at_call(&self, file: &'a solid_facts::FileFacts, call: Span) -> Option<Primitive>;
    /// For the primitive call `call`, argument `index`: whether its callback
    /// runs in the call's own window (its invocation is the call), and
    /// otherwise the role its slot gives it ([`primitive_callback_slot_role`]).
    fn primitive_slot(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &'a solid_facts::ast::CallFact,
        index: usize,
    ) -> (bool, Option<ExecutionRole>);
    /// [`project_callee_at_parameter`].
    fn project_callee(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &'a solid_facts::ast::CallFact,
        index: usize,
    ) -> Option<(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
        bool,
    )>;
    /// [`SemanticLookup::function_called_at`].
    fn function_called_at(
        &self,
        file: &'a solid_facts::FileFacts,
        callee: Span,
    ) -> Option<(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
    )>;
    /// [`member_runs_declared_method`].
    fn member_runs_declared_method(
        &self,
        file: &'a solid_facts::FileFacts,
        tag: Span,
        component_file: &'a solid_facts::FileFacts,
        component: &'a solid_facts::ast::FunctionFact,
    ) -> bool;
    /// Whether a member `key` that no attribute gives a props object reads
    /// `undefined` or an inert `Object.prototype` built-in: no assignment or
    /// reflective definition anywhere may give it one.
    fn absent_member_inert(&self, key: &str) -> bool;
    /// The proven role at the call at `call` ([`semantic_execution_role_within`]
    /// at its callee), when it keeps a read out of every reporting role and
    /// holds for every invocation of the named callbacks around the call;
    /// `classifying` as the role query leaves it.
    fn site_role(
        &self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        classifying: &mut HashSet<(String, Span)>,
    ) -> Option<ExecutionRole>;
}

/// The walk over the build's lookup, unrecorded: for a nested evaluation
/// whose reads the enclosing computation covers.
pub(super) struct LookupFlow<'l, 'a> {
    pub(super) lookup: &'l SemanticLookup<'a>,
}

impl<'a> FlowWorld<'a> for LookupFlow<'_, 'a> {
    fn dialect(&self) -> &'a dyn solid_dialect::Dialect {
        self.lookup.dialect
    }

    fn canonical(&self, file: &'a solid_facts::FileFacts) -> Option<&'a solid_facts::FileFacts> {
        self.lookup.file_by_path(file.path.as_str())
    }

    fn program_has_eval(&self) -> bool {
        self.lookup.program_has_eval()
    }

    fn component_status(
        &self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> ComponentStatus {
        self.lookup.function_component_status(file, function)
    }

    fn primitive_at_call(&self, file: &'a solid_facts::FileFacts, call: Span) -> Option<Primitive> {
        self.lookup.primitive_at_call(file, call)
    }

    fn primitive_slot(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &'a solid_facts::ast::CallFact,
        index: usize,
    ) -> (bool, Option<ExecutionRole>) {
        primitive_slot_in(self.lookup, file, call, index)
    }

    fn project_callee(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &'a solid_facts::ast::CallFact,
        index: usize,
    ) -> Option<(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
        bool,
    )> {
        project_callee_at_parameter(file, call, index, self.lookup)
    }

    fn function_called_at(
        &self,
        file: &'a solid_facts::FileFacts,
        callee: Span,
    ) -> Option<(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
    )> {
        self.lookup.function_called_at(file.path.as_str(), callee)
    }

    fn member_runs_declared_method(
        &self,
        file: &'a solid_facts::FileFacts,
        tag: Span,
        component_file: &'a solid_facts::FileFacts,
        component: &'a solid_facts::ast::FunctionFact,
    ) -> bool {
        member_runs_declared_method(file, tag, component_file, component, self.lookup)
    }

    fn absent_member_inert(&self, key: &str) -> bool {
        absent_member_inert_in(self.lookup, key)
    }

    fn site_role(
        &self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        classifying: &mut HashSet<(String, Span)>,
    ) -> Option<ExecutionRole> {
        site_role_in(self.lookup, file, call, classifying)
    }
}

/// [`FlowWorld::primitive_slot`] over the lookup.
pub(super) fn primitive_slot_in(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
) -> (bool, Option<ExecutionRole>) {
    let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
        return (false, None);
    };
    // A callback that runs synchronously in the call's own window
    // (`createRoot`, `runWithOwner`) runs wherever the call does.
    if lookup
        .dialect
        .callback_inherits_calling_window(primitive, index)
        && callback_execution_at_call(file, call, primitive, index, lookup).is_some()
    {
        return (true, None);
    }
    (
        false,
        primitive_callback_slot_role(file, call, index, lookup),
    )
}

/// [`FlowWorld::absent_member_inert`] over the lookup.
pub(super) fn absent_member_inert_in(lookup: &SemanticLookup<'_>, key: &str) -> bool {
    !lookup.member_may_be_reassigned_on_builtin(key)
        && !lookup.member_may_be_reassigned_on_builtin("__proto__")
        && !lookup.property_may_be_defined_reflectively_as(key)
}

/// [`FlowWorld::site_role`] over the lookup.
pub(super) fn site_role_in(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: Span,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let callee = file.ast.call_at(call)?.callee;
    let here = semantic_execution_role_within(
        file,
        callee,
        &[],
        lookup.entities(),
        lookup.symbol_names(),
        lookup,
        classifying,
    );
    // ADR 0320: a role read off a named function's callback references
    // ignores its direct calls (`press()` beside `onClick={press}`); the
    // walk asks for every invocation of that function instead.
    (flow_safe_role(here)
        && named_callbacks_within_component(file, call, lookup)
            .into_iter()
            .all(|function| named_callback_invocations_hold(file, function, lookup, false)))
    .then_some(here)
}

pub(super) struct InvocationFlow<'a, 'w, 'c, W: FlowWorld<'a>> {
    pub(super) world: &'w W,
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
    /// The owner question ([`event_invoked_literals`]): a primitive's
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
    // Every top-level answer is a memo, computed context-free; the set of
    // answers is what the cross-file proof digest binds: a cached fragment
    // that consumed one is reused only while every answer is unchanged. A
    // query nested in an answer's computation stays inside it, in its
    // context.
    if COMPUTING_FLOW_ANSWERS.with(std::cell::Cell::get) {
        return literal_flow_role(&LookupFlow { lookup }, file, literal, classifying);
    }
    lookup
        .invocation_flow_answers()
        .get(&(file.path.to_string(), literal.span))
        .copied()
}

thread_local! {
    /// Set while a flow memo computes ([`super::flow_memos`]): a nested role
    /// query then computes directly, in the answer's context, instead of
    /// reading the answers.
    static COMPUTING_FLOW_ANSWERS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this thread is computing a flow answer.
pub(super) fn computing_flow_answers() -> bool {
    COMPUTING_FLOW_ANSWERS.with(std::cell::Cell::get)
}

/// Runs `f` as a flow memo's computation: a nested role query on this thread
/// computes directly. Every flow memo runs this way, whoever asks, so its
/// answer does not depend on the asking thread's state.
pub(super) fn computing_flow<R>(f: impl FnOnce() -> R) -> R {
    let outer = COMPUTING_FLOW_ANSWERS.with(|computing| computing.replace(true));
    let result = f();
    COMPUTING_FLOW_ANSWERS.with(|computing| computing.set(outer));
    result
}

/// ADR 0319: the invocation-flow answer of every function literal of a
/// closed program, keyed by path and literal span; only proven roles are
/// kept. Each file's answers are a memo, each computed from an empty context
/// ([`super::flow_memos::InvocationFlowAnswers`]); files are asked in
/// parallel and inserted in file order. A path listed twice is one source:
/// its first slot answers.
pub(crate) fn invocation_flow_answers(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), ExecutionRole> {
    let mut answers = HashMap::default();
    if !lookup.program_closed {
        return answers;
    }
    let files = super::flow_memos::file_keys(lookup);
    let world = lookup.memo_world();
    let roles = crate::pipeline::parallel_balanced_results(&files, |(id, _)| {
        world.answer::<super::flow_memos::InvocationFlowAnswers>(id)
    });
    for ((_, path), roles) in files.into_iter().zip(roles) {
        for (span, role) in roles.iter() {
            answers.insert((path.clone(), *span), *role);
        }
    }
    answers
}

/// The flow answer for one literal, computed in the context `classifying`.
pub(super) fn literal_flow_role<'a, W: FlowWorld<'a>>(
    world: &W,
    file: &'a solid_facts::FileFacts,
    literal: &'a solid_facts::ast::FunctionFact,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    let file = world.canonical(file)?;
    let literal = file.ast.function_at(literal.span)?;
    if literal.generator || world.component_status(file, literal) != ComponentStatus::No {
        return None;
    }
    if world.program_has_eval() {
        return None;
    }
    let key = (format!("\0flow:{}", file.path.as_str()), literal.span);
    if !classifying.insert(key.clone()) {
        return None;
    }
    let mut flow = InvocationFlow {
        world,
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
