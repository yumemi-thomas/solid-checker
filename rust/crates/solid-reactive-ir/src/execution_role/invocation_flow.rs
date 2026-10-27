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
    /// The call sites in other modules of the exported function declaration
    /// `function` of `file` (ADR 0442): `(caller file, call span)`; `None`
    /// when they cannot all be accounted for ([`exported_call_sites_in`]).
    fn exported_call_sites(
        &self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<(&'a solid_facts::FileFacts, Span)>>;
    /// The build's lookup, when the world is the lookup ([`LookupFlow`]). The
    /// write question (ADR 0442) and the test-runner census (ADR 0438) are
    /// whole-lookup computations, never memos: they ask it for the tables the
    /// memo world does not record, and a world without it answers nothing.
    fn lookup(&self) -> Option<&SemanticLookup<'a>> {
        None
    }
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

    fn exported_call_sites(
        &self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<(&'a solid_facts::FileFacts, Span)>> {
        exported_call_sites_in(self.lookup, file, function)
    }

    fn lookup(&self) -> Option<&SemanticLookup<'a>> {
        Some(self.lookup)
    }
}

/// ADR 0442: the invocations of an exported function declaration in other
/// modules of a closed program. Every other module's reference to the
/// declaration's symbol must be a call site the call graph accounts for
/// (`function_call_site_references`: no alias, re-export, argument, type query
/// or namespace hand-out), and each site must be a plain call expression; any
/// other reference refuses, as an open program does. The module's own
/// references are followed by the walk, as for a function that is not
/// exported.
pub(super) fn exported_call_sites_in<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<Vec<(&'a solid_facts::FileFacts, Span)>> {
    let path = file.path.as_str();
    let name = function.name.as_ref()?;
    if !lookup.program_closed
        || lookup.escapes_unnamed(path)
        || lookup.hands_out_by_name(path, function.span)
    {
        return None;
    }
    let symbol = lookup.entities().at(path, name.span)?;
    let references = lookup.symbol_references_if_present(symbol.as_str())?;
    let accounted: HashSet<(&str, u32, u32)> = lookup
        .function_call_site_references(path, function.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.as_str(), callee.start, callee.end))
        .collect();
    for reference in &references {
        if reference.path.as_ref() == path {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return None;
        };
        if !accounted.contains(&(reference.path.as_ref(), start, end)) {
            return None;
        }
    }
    let mut sites = Vec::new();
    for (caller_file, callee) in lookup.function_call_sites(path, function.span) {
        if caller_file.path == file.path {
            continue;
        }
        let call = caller_file.ast.calls.iter().find(|call| {
            !call.construct && caller_file.ast.peel_ts_sugar_span(call.callee) == callee
        })?;
        sites.push((caller_file, call.span));
    }
    Some(sites)
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
    /// The owner question's other census (ADR 0438): the entries that count
    /// are the function literals a reviewed test runner invokes (a
    /// registration's callback, [`SemanticLookup::test_runner_callback`]),
    /// not an event delivery. Meaningful only with `owner_mode`.
    pub(super) test_entries: bool,
    /// ADR 0442: the write question. A call's role is the write classifier's
    /// role at the call under this guard, and the join keeps an owned entry
    /// as the answer (ADR 0378). Never combined with `owner_mode`.
    pub(super) write_root_body: Option<RootBodyGuard>,
    /// The modules this walk read (a holder, a callee, a caller, a rendered
    /// component): an answer that read another module than the literal's own
    /// depends on that module's contents (ADR 0442's digest binding).
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
    // Every top-level answer is a memo, computed context-free; a cached
    // chunk that consumed one records the answers' section
    // ([`crate::base::reads::Section::FlowAnswers`]) and is reused only
    // while every answer is unchanged. A
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
        test_entries: false,
        write_root_body: None,
        touched: HashSet::default(),
    };
    let role = flow.function_role(file, literal).flatten();
    flow.classifying.remove(&key);
    role
}

thread_local! {
    /// Set while [`write_flow_answer`] computes an answer: the write walk,
    /// asked at an invocation site, then skips its own use of the answers, so
    /// every answer is computed from an empty context and no order of
    /// computation matters.
    static COMPUTING_WRITE_FLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// ADR 0442, per function and guard (ADR 0450): the write-flow answer of a
/// function a closed program hands to project code (a component prop value, an
/// argument of a call that is no primitive, a named function). Only a proven
/// role is an answer, with whether the walk read another module than the
/// function's own; see [`literal_write_flow_role`]. `None` for a function that
/// is no candidate and for one the walk proves nothing about.
///
/// The walk's cache of the classifier's role at each call lives for one answer
/// only: what the classifier says at a call depends on the build.
pub(crate) fn write_flow_answer(
    lookup: &SemanticLookup<'_>,
    path: &str,
    span: Span,
    exempts_root: bool,
) -> Option<(ExecutionRole, bool)> {
    let file = lookup.file_by_path(path)?;
    let literal = file.ast.function_at(span)?;
    if !write_flow_candidate(file, literal) {
        return None;
    }
    let guard = if exempts_root {
        RootBodyGuard::Exempts
    } else {
        RootBodyGuard::Rejects
    };
    let outer = COMPUTING_WRITE_FLOW.with(|computing| computing.replace(true));
    let answer = literal_write_flow_role(file, literal, guard, lookup);
    COMPUTING_WRITE_FLOW.with(|computing| computing.set(outer));
    clear_write_site_roles();
    answer
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
/// (ADR 0442): followed forward as ADR 0319 follows a value, each call's role
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
        || lookup.program_has_eval()
    {
        return None;
    }
    let mut classifying = HashSet::default();
    classifying.insert((format!("\0write-flow:{}", file.path.as_str()), literal.span));
    let mut flow = InvocationFlow {
        world: &LookupFlow { lookup },
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

/// ADR 0442: the role the invocations of `span`'s function (or of a function
/// around it that runs `span` in place) give a write there, from the
/// function's own answer ([`write_flow_answer`]). `None` while an answer is computed, in an open program, and for a
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
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.span.end - function.span.start);
    // Where the write, then each layer around it, sits in the next function
    // out. An await in any of those functions that precedes the position
    // suspends the run: it resumes later with no owner (ADR 0333), whatever
    // invokes the function.
    let mut position = span;
    for function in enclosing {
        if !runs_in_place_within(file, innermost, function, lookup) {
            return None;
        }
        if let Some(suspended) = suspended_write_role(file, position, lookup) {
            return Some(suspended);
        }
        position = function.span;
        if write_flow_candidate(file, function)
            && let Some((role, _)) = lookup.write_flow_answer_at(
                file.path.as_str(),
                function.span,
                root_body == RootBodyGuard::Exempts,
            )
        {
            return Some(role);
        }
    }
    None
}
