//! Event-only invocation census; delivery ownership is a separate proof.

use super::*;

/// Every function literal of a closed program whose invocations, followed
/// forward as ADR 0319 follows them, all run through event delivery: an
/// intrinsic element's `on…` attribute or spread key, or a function the
/// compiler classifies as an event handler. A synchronous dispatch can inherit
/// its caller's owner; the owner graph applies that delivery veto. Any
/// invocation through a primitive's callback slot refuses: who owns that run
/// is the owner graph's question, not this walk's.
pub(crate) fn event_invoked_literals(lookup: &SemanticLookup<'_>) -> HashSet<(String, Span)> {
    invoked_literals(lookup, false)
}

/// ADR 0438: the census of [`event_invoked_literals`] with a reviewed test
/// runner as the only entry. Every function literal of a closed program whose
/// invocations, followed forward the same way, are all calls written directly
/// in a function literal handed to a test runner's registration (or in a
/// literal of this set, transitively and within the same bounds): each runs
/// on the runner's stack or on a continuation of it, with no owner and no
/// strict-read window. A reference that is an export, an alias, a value passed
/// on, or a call from anywhere else (a component body, an event handler, a
/// primitive's callback) refuses, as it does for the event census.
pub(crate) fn test_invoked_literals(lookup: &SemanticLookup<'_>) -> HashSet<(String, Span)> {
    invoked_literals(lookup, true)
}

fn invoked_literals(lookup: &SemanticLookup<'_>, test_entries: bool) -> HashSet<(String, Span)> {
    let mut literals = HashSet::default();
    if !lookup.program_closed || lookup.program_has_eval() {
        return literals;
    }
    if test_entries {
        // The test census is the lookup's own whole-program computation (not a
        // memo): its only reader is the dispatch veto, whose section binds
        // its value ([`crate::base::reads::Section::SynchronousDispatch`]).
        let world = LookupFlow { lookup };
        for file in lookup.files() {
            for literal in &file.ast.functions {
                if literal_test_invoked(&world, file, literal) {
                    literals.insert((file.path.to_string(), literal.span));
                }
            }
        }
        return literals;
    }
    let files = super::flow_memos::file_keys(lookup);
    // Each file's census is a memo, each literal's computed from an empty
    // context ([`super::flow_memos::EventInvokedLiterals`]). In parallel only
    // once the flow answers exist and this thread is not computing one, as
    // before.
    let world = lookup.memo_world();
    let answer = |(id, _): &(crate::base::FileId, String)| {
        world.answer::<super::flow_memos::EventInvokedLiterals>(id)
    };
    let invoked = if lookup.invocation_flow_answers_ready() && !computing_flow_answers() {
        crate::pipeline::parallel_balanced_results(&files, answer)
    } else {
        files.iter().map(answer).collect()
    };
    for ((_, path), invoked) in files.into_iter().zip(invoked) {
        for span in invoked.iter() {
            literals.insert((path.clone(), *span));
        }
    }
    literals
}

/// ADR 0437: whether the call at `call` is written directly in a function
/// whose every invocation the census ([`event_invoked_literals`]) finds is an
/// event delivery. Such a call runs on the stack of a delivery, so a dispatch
/// there hands its handler no owner and no strict-read window the delivery did
/// not have. Code that runs elsewhere than the function's own flow (a class
/// field, JSX the compiler wraps in a getter) is left out, as the census's own
/// call placement leaves it out, and so is a file whose compiler facts were
/// withheld (ADR 0360). The census reads no dispatch veto.
pub(crate) fn dispatch_in_event_only_function(
    file: &solid_facts::FileFacts,
    call: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    dispatch_in_census_function(file, call, lookup.event_invoked_literals())
}

/// ADR 0438: [`dispatch_in_event_only_function`] for the test-only census
/// ([`test_invoked_literals`]): the call is written directly in a helper every
/// invocation of which is a call in a test runner's callback (or in another
/// such helper), so it runs on the runner's stack or a continuation of it.
pub(crate) fn dispatch_in_test_only_function(
    file: &solid_facts::FileFacts,
    call: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    dispatch_in_census_function(file, call, lookup.test_invoked_literals())
}

fn dispatch_in_census_function(
    file: &solid_facts::FileFacts,
    call: Span,
    census: &HashSet<(String, Span)>,
) -> bool {
    if file.compiler.withheld() {
        return false;
    }
    let Some(function) = containing_ast_function(&file.ast, call) else {
        return false;
    };
    crate::owners::written_directly_in(&file.ast, function, call)
        && !class_within(file, function.span, call)
        && !file
            .ast
            .jsx_containing(call)
            .any(|element| function.body.contains(element.span))
        && !file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| function.body.contains(*fragment) && fragment.contains(call))
        && census.contains(&(file.path.to_string(), function.span))
}

/// Whether `function` is a literal written directly as an argument of a
/// reviewed test runner's registration ([`SemanticLookup::test_runner_callback`]) that
/// the runner invokes from its own scheduler, later. A literal behind a wrapper
/// call (`it("x", wrap(fn))`) is not the argument, and a spread before it
/// makes the position unknown.
fn registered_test_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.ast
        .arguments_containing(function.span)
        .any(|(call, index)| {
            matches!(
                call.arguments[index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                && !call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                && lookup.test_runner_callback(file, call)
                    == Some(crate::indexes::TestRunnerCallback::Scheduled)
        })
}

thread_local! {
    /// The write classifier's role at each call the write flow has asked of
    /// during one table computation (empty otherwise).
    static WRITE_SITE_ROLES: std::cell::RefCell<HashMap<(String, Span, RootBodyGuard), ExecutionRole>> =
        std::cell::RefCell::new(HashMap::default());
}

pub(super) fn clear_write_site_roles() {
    WRITE_SITE_ROLES.with(|cache| cache.borrow_mut().clear());
}

/// Whether every invocation of the function literal runs through event
/// delivery ([`event_invoked_literals`]), from an empty context.
pub(super) fn literal_event_invoked<'a, W: FlowWorld<'a>>(
    world: &W,
    file: &'a solid_facts::FileFacts,
    literal: &'a solid_facts::ast::FunctionFact,
) -> bool {
    if literal.generator || world.component_status(file, literal) != ComponentStatus::No {
        return false;
    }
    let mut classifying = HashSet::default();
    let mut flow = InvocationFlow {
        world,
        classifying: &mut classifying,
        budget: FLOW_BUDGET,
        seen: HashSet::default(),
        frames: Vec::new(),
        props_stack: Vec::new(),
        owner_mode: true,
        test_entries: false,
        write_root_body: None,
        touched: HashSet::default(),
    };
    flow.function_role(file, literal) == Some(Some(ExecutionRole::EventCallback))
}

/// Whether every invocation of the function literal is a call written in a
/// reviewed test runner's callback ([`test_invoked_literals`]), from an empty
/// context.
fn literal_test_invoked<'a, W: FlowWorld<'a>>(
    world: &W,
    file: &'a solid_facts::FileFacts,
    literal: &'a solid_facts::ast::FunctionFact,
) -> bool {
    if literal.generator || world.component_status(file, literal) != ComponentStatus::No {
        return false;
    }
    let mut classifying = HashSet::default();
    let mut flow = InvocationFlow {
        world,
        classifying: &mut classifying,
        budget: FLOW_BUDGET,
        seen: HashSet::default(),
        frames: Vec::new(),
        props_stack: Vec::new(),
        owner_mode: true,
        test_entries: true,
        write_root_body: None,
        touched: HashSet::default(),
    };
    flow.function_role(file, literal) == Some(Some(ExecutionRole::DeferredCallback))
}

impl<'a, W: FlowWorld<'a>> InvocationFlow<'a, '_, '_, W> {
    /// The one safe role of every invocation of `function`; `Some(None)`
    /// when the (complete) census finds no invocation at all.
    pub(super) fn function_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Option<ExecutionRole>> {
        if let Some(root_body) = self.write_root_body {
            return self.write_function_role(file, function, root_body);
        }
        let invocations = self.function_invocations(file, function)?;
        let mut role = None::<ExecutionRole>;
        for invocation in invocations {
            let here = match invocation {
                Invocation::Role(role) => role,
                // A call written in a function nothing invokes never runs.
                Invocation::Call(call_file, call, frames) => {
                    match self.site_role(call_file, call, frames)? {
                        Some(here) => here,
                        None => continue,
                    }
                }
            };
            role = Some(match role {
                None => here,
                Some(role) if role == here => role,
                // ADR 0323: invocations in different roles that all keep a
                // read out of the render window (an event, a deferred
                // callback, a tracked computation, an explicit `untrack`) say
                // one thing for read classification: no invocation reports
                // the read. No single role describes the mix without
                // overclaiming for some other consumer (a tracked role claims
                // a subscription the `untrack` run does not make), so the
                // answer is the no-claim role (`Unreported`), which every read rule
                // suppresses and which a caller merging it treats as the mix
                // it is. A reporting role among them refuses, as before.
                Some(role) if non_reporting_mix(role) && non_reporting_mix(here) => {
                    if role == ExecutionRole::Unknown || here == ExecutionRole::Unknown {
                        ExecutionRole::Unknown
                    } else {
                        ExecutionRole::Unreported
                    }
                }
                Some(_) => return None,
            });
        }
        Some(role)
    }

    /// The role of the call at `call`: a proven role at its position, or the
    /// one role of every invocation of the plain function it is written in;
    /// `Some(None)` when nothing invokes that function, so the call never
    /// runs.
    fn site_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> Option<Option<ExecutionRole>> {
        // The call must be one the facts hold (the site role reads its
        // callee).
        file.ast.call_at(call)?;
        if self.owner_mode {
            return self.owner_site_role(file, call, frames);
        }
        if let Some(root_body) = self.write_root_body {
            return self.write_site_role(
                file,
                call,
                file.ast.call_at(call)?.callee,
                frames,
                root_body,
            );
        }
        if let Some(here) = self.world.site_role(file, call, self.classifying) {
            return Some(Some(here));
        }
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || self.world.component_status(file, owner) != ComponentStatus::No
        {
            return None;
        }
        let key = (format!("\0flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        // The function runs in the callee activations the call was found in:
        // a value it returns goes back to those calls.
        let outer = std::mem::replace(&mut self.frames, frames);
        let role = self.function_role(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        role
    }

    /// ADR 0442: [`Self::function_role`] for a write. An invocation in an
    /// owned scope is the answer whatever the others are (ADR 0378: one entry
    /// that throws is a violation); otherwise every invocation must be placed,
    /// in one role or, when they differ, in none (`Unreported`, as for reads).
    fn write_function_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        root_body: RootBodyGuard,
    ) -> Option<Option<ExecutionRole>> {
        let invocations = self.function_invocations(file, function)?;
        let mut owned = None::<ExecutionRole>;
        let mut unplaced = false;
        let mut role = None::<ExecutionRole>;
        for invocation in invocations {
            let here = match invocation {
                // A host event dispatch hands the function the delivery's
                // owner, which a synchronous dispatch of the program's own
                // may make the dispatcher's (ADR 0356).
                Invocation::Role(ExecutionRole::EventCallback)
                    if self
                        .world
                        .lookup()
                        .is_none_or(SemanticLookup::synchronous_event_dispatch_present) =>
                {
                    unplaced = true;
                    continue;
                }
                Invocation::Role(role) => role,
                Invocation::Call(call_file, call, frames) => {
                    match self.write_site_role(
                        call_file,
                        call,
                        call_file.ast.call_at(call)?.callee,
                        frames,
                        root_body,
                    ) {
                        None => {
                            unplaced = true;
                            continue;
                        }
                        // A call written in a function nothing invokes never runs.
                        Some(None) => continue,
                        Some(Some(here)) => here,
                    }
                }
            };
            if here.reports_disallowed_write() {
                owned.get_or_insert(here);
            } else if matches!(
                here,
                ExecutionRole::Unknown | ExecutionRole::DiscardedRendering
            ) {
                unplaced = true;
            } else {
                role = Some(match role {
                    Some(role) if role == here => role,
                    Some(_) => ExecutionRole::Unreported,
                    None => here,
                });
            }
        }
        if let Some(owned) = owned {
            return Some(Some(owned));
        }
        if unplaced {
            return None;
        }
        Some(role)
    }

    /// ADR 0442: the role of the call at `call` for a write, as the write
    /// classifier places it where it is written, or the one role of every
    /// invocation of the plain function it is written in when no fact places
    /// it there.
    fn write_site_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        callee: Span,
        frames: Vec<FlowFrame<'a>>,
        root_body: RootBodyGuard,
    ) -> Option<Option<ExecutionRole>> {
        // The write question runs over the lookup only ([`LookupFlow`]).
        let lookup = self.world.lookup()?;
        self.touched.insert(file.path.as_str());
        // The classifier's answer at a call depends on where the call is
        // written, not on the activation the walk found it in; the table
        // computation asks it of the same call from many functions.
        let key = (file.path.to_string(), callee, root_body);
        let cached = WRITE_SITE_ROLES.with(|cache| cache.borrow().get(&key).copied());
        let here = cached.unwrap_or_else(|| {
            let allowed = write_allowed_spans(file, &allowed_callback_spans(file, lookup), lookup);
            // Only what the position proves: another entry of the function
            // the call is written in is not an entry of this activation.
            let here = semantic_write_role_walk(
                file,
                callee,
                &allowed,
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
                root_body,
                &mut HashSet::from_iter([lexical_only_marker()]),
            );
            WRITE_SITE_ROLES.with(|cache| cache.borrow_mut().insert(key, here));
            here
        });
        if here != ExecutionRole::Unknown {
            return Some(Some(here));
        }
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || lookup.function_component_status(file, owner) != ComponentStatus::No
        {
            return None;
        }
        // The call runs in the activation that handed the function its
        // parameter: that call alone is the owner's invocation, whatever its
        // other callers do (a helper called from a handler and from a body).
        if let Some((callee_path, callee_span, caller_file, caller_call)) = frames.last().cloned()
            && callee_path == file.path.as_str()
            && callee_span == owner.span
        {
            let mut rest = frames;
            rest.pop();
            let caller_callee = caller_file.ast.call_at(caller_call)?.callee;
            return self.write_site_role(caller_file, caller_call, caller_callee, rest, root_body);
        }
        let key = (format!("\0write-flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        let outer = std::mem::replace(&mut self.frames, frames);
        let role = self.function_role(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        role
    }

    /// [`Self::site_role`] for the owner question: the call is written
    /// directly in a function the compiler classifies as an event handler,
    /// or it takes the one role of every invocation of the plain function it
    /// is written in.
    fn owner_site_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> Option<Option<ExecutionRole>> {
        let owner = containing_ast_function(&file.ast, call)?;
        // ADR 0438: a function literal handed directly to a reviewed test
        // runner's registration. Both its prefix up to the first await and
        // every continuation after one run with no owner and no strict-read
        // window (the runner's stack, then no owner at any resumption, ADR
        // 0333), so a call written in it is a satisfied entry past an await
        // as well; an event handler's census refuses past one.
        let registered = self.test_entries
            && self
                .world
                .lookup()
                .is_some_and(|lookup| registered_test_callback(file, owner, lookup));
        // ADR 0442: in the test census an async helper's call past a
        // suspension is a satisfied entry as a registered callback's is: the
        // resumption runs with no owner and no window (ADR 0333) whoever
        // called the helper, and the part before the first await runs on its
        // caller's stack, which the walk below requires to be a runner's.
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call) && !self.test_entries)
            || owner.generator
            || self.world.component_status(file, owner) != ComponentStatus::No
        {
            return None;
        }
        // Only a call in the function's own flow runs when it does: a nested
        // function's default parameter runs when that function is called, a
        // class field initializer at construction, and a JSX expression in
        // the getter the compiler makes of it, wherever it is read.
        if !crate::owners::written_directly_in(&file.ast, owner, call)
            || class_within(file, owner.span, call)
            || file
                .ast
                .jsx_containing(call)
                .any(|element| owner.body.contains(element.span))
            || file
                .ast
                .jsx_fragments
                .iter()
                .any(|fragment| owner.body.contains(*fragment) && fragment.contains(call))
        {
            return None;
        }
        if self.test_entries {
            if registered {
                return Some(Some(ExecutionRole::DeferredCallback));
            }
        } else if file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == owner.span
        }) {
            return Some(Some(ExecutionRole::EventCallback));
        }
        let key = (format!("\0owner-flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        let outer = std::mem::replace(&mut self.frames, frames);
        let role = self.function_role(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        role
    }

    /// The context the walk is in, as a `seen` key component: the callee
    /// calls it entered through a parameter and the elements whose props it
    /// is inside. Receiver proofs depend on the elements.
    fn context(&self) -> Vec<(&'a str, Span)> {
        self.frames
            .iter()
            .map(|(_, _, file, call)| (file.path.as_str(), *call))
            .chain(
                self.props_stack
                    .iter()
                    .map(|frame| (frame.element_file.path.as_str(), frame.element)),
            )
            .collect()
    }

    /// Every invocation of the function value `function` denotes.
    fn function_invocations(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<Invocation<'a>>> {
        // A script's top-level declarations are globals any file can reach
        // by name; no census of this file's references is complete.
        if !file_is_module(file) {
            return None;
        }
        // Each census is complete on its own: a holder another census already
        // followed still contributes its invocations here.
        let outer = std::mem::take(&mut self.seen);
        let result = self.function_invocations_inner(file, function);
        self.seen = outer;
        result
    }

    fn function_invocations_inner(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Vec<Invocation<'a>>> {
        let mut out = Vec::new();
        if let Some(method) = crate::owners::own_method_name(file, function) {
            // An object literal's own method: the object holds it. (A function
            // nested in a method's body carries the method's name in the facts
            // but is not the method.)
            let property = file
                .ast
                .object_properties
                .iter()
                .find(|property| property.key == method.span && !property.computed)?;
            let object = innermost_object_literal(file, property.key)?;
            if receiver_used_within(file, object.span) {
                return None;
            }
            let name = static_key_text(file.source_text(method.span)?, false)?;
            self.uses(
                file,
                object.span,
                vec![name],
                FlowOrigin::Object(file, object.span),
                &mut out,
            )?;
            return Some(out);
        }
        if function.kind == solid_facts::ast::FunctionKind::Declaration {
            let name = function.name.as_ref()?;
            if crate::indexes::binding_written(file, name.span) {
                return None;
            }
            if flow_binding_exported(file, name.span) {
                self.exported_call_sites(file, function, &mut out)?;
            }
            for (reference, _) in
                file.ast
                    .reference_declarations
                    .iter()
                    .filter(|(reference, declaration)| {
                        *declaration == name.span && *reference != name.span
                    })
            {
                self.uses(file, *reference, Vec::new(), FlowOrigin::Unknown, &mut out)?;
            }
            return Some(out);
        }
        // A named function expression can also hand itself on by its own
        // name (`function me() { slot.fn = me; }`).
        if let Some(name) = function.name.as_ref()
            && file
                .ast
                .reference_declarations
                .iter()
                .any(|(reference, declaration)| {
                    *declaration == name.span && *reference != name.span
                })
        {
            return None;
        }
        self.uses(
            file,
            function.span,
            Vec::new(),
            FlowOrigin::Unknown,
            &mut out,
        )?;
        Some(out)
    }

    /// ADR 0442: the invocations of an exported function declaration in
    /// another module of a closed program. Every other module's reference to
    /// the declaration's symbol must be a call site the call graph accounts
    /// for (`function_call_site_references`: no alias, re-export, argument,
    /// type query or namespace hand-out), and each site must be a plain call
    /// expression; any other reference refuses, as an open program does. The
    /// module's own references are followed by the caller, as for a function
    /// that is not exported.
    fn exported_call_sites(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        for (caller_file, call) in self.world.exported_call_sites(file, function)? {
            self.touched.insert(caller_file.path.as_str());
            out.push(Invocation::Call(caller_file, call, self.frames.clone()));
        }
        Some(())
    }

    /// Follow the value the expression at `expr` evaluates to, holding the
    /// tracked function at member `path` (empty: the function itself).
    /// `origin` is the object literal that value is, when known.
    #[allow(clippy::too_many_lines)]
    fn uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        expr: Span,
        path: Vec<String>,
        origin: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        if self.budget == 0 || !file_is_module(file) {
            return None;
        }
        self.budget -= 1;
        self.touched.insert(file.path.as_str());
        let expr = file.ast.peel_ts_sugar_span(expr);
        if !self.seen.insert((
            file.path.as_str(),
            expr,
            path.clone(),
            self.context(),
            origin_identity(&origin),
        )) {
            return Some(());
        }
        let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
        // Called.
        if let Some(call) = file.ast.call_with_peeled_callee(expr) {
            if call.construct || !path.is_empty() {
                return None;
            }
            out.push(Invocation::Call(file, call.span, self.frames.clone()));
            return Some(());
        }
        // The object of a member access.
        if let Some(member) = file.ast.member_with_peeled_object(expr) {
            if file.ast.any_assignment_target_containing(member.span) {
                return None;
            }
            // A computed member counts only with a literal key.
            let key = member_static_key(file, member)?;
            let key = key.as_str();
            let called = file.ast.any_call_with_peeled_callee(member.span)
                || flow_tagged_or_chained(file, member.span);
            return match path.split_first() {
                // A property of the function itself: `length`, `name`. A
                // call through one (`bind`, `call`) runs it.
                None => (!called).then_some(()),
                Some((first, rest)) if first == key => {
                    self.uses(file, member.span, rest.to_vec(), FlowOrigin::Unknown, out)
                }
                // Another property: read, or called with this object as
                // `this`, which must be a function that cannot use it.
                Some(_) => {
                    if called {
                        origin_member_ignores_receiver(&origin, key, self.world, 6).then_some(())
                    } else {
                        Some(())
                    }
                }
            };
        }
        // An argument.
        if let Some((call, index)) = file
            .ast
            .arguments_containing(expr)
            .find(|(call, index)| peel(call.arguments[*index].span) == expr)
        {
            if call.construct
                || call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
            {
                return None;
            }
            // ADR 0323: the function itself handed to a primitive's callback
            // slot runs in that slot's role, when the dialect states one that
            // keeps a read out of the render window (`untrack(options.key)`,
            // `createMemo(options.key)`). A container holding it, an apply
            // slot, a slot that stores the function or runs it on result
            // access, and every unmodelled slot refuse.
            if self.world.primitive_at_call(file, call.span).is_some() {
                if !path.is_empty() || self.owner_mode || self.write_root_body.is_some() {
                    return None;
                }
                // A callback that runs synchronously in the call's own window
                // (`createRoot`, `runWithOwner`) runs wherever the call does:
                // its invocation is the call.
                let (inherits, role) = self.world.primitive_slot(file, call, index);
                if inherits {
                    out.push(Invocation::Call(file, call.span, self.frames.clone()));
                    return Some(());
                }
                out.push(Invocation::Role(role?));
                return Some(());
            }
            // ADR 0442: a reviewed scheduler's callback (`setTimeout(fn)`) is
            // run from a fresh stack with no owner and no window: the
            // scheduled role, wherever the call was written. The selected
            // standard-library declaration says so, never the spelling.
            if self.write_root_body.is_some() && path.is_empty() {
                let lookup = self.world.lookup()?;
                let resolved = lookup.resolved_callee_call(file, call.callee);
                let callability = lookup
                    .entity_at(file.path.as_str(), call.arguments[index].span)
                    .and_then(|entity| entity.callability);
                if resolved.is_some_and(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                        && matches!(
                            crate::runtime_semantics::argument_behavior(resolved, callability, index),
                            Some(crate::runtime_semantics::RuntimeArgumentBehavior::FreshStackCallback)
                        )
                }) {
                    out.push(Invocation::Role(ExecutionRole::DeferredCallback));
                    return Some(());
                }
            }
            let (callee_file, callee, exact) = self.world.project_callee(file, call, index)?;
            if !exact || self.world.component_status(callee_file, callee) != ComponentStatus::No {
                return None;
            }
            let callee_file = self.world.canonical(callee_file)?;
            self.frames
                .push((callee_file.path.to_string(), callee.span, file, call.span));
            let result = self.parameter_uses(callee_file, callee, index, path, origin, out);
            self.frames.pop();
            return result;
        }
        // A JSX attribute value or spread.
        for element in file.ast.jsx_containing(expr) {
            if let Some(attribute) = element.attributes.iter().find(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| peel(value) == expr)
            }) {
                if attribute.namespace.is_some() {
                    return None;
                }
                let name = file.source_text(attribute.name)?;
                if intrinsic_element(file, element) {
                    return (path.is_empty()
                        && self.world.dialect().compiler_lowers_event_attribute(name))
                    .then(|| {
                        out.push(Invocation::Role(ExecutionRole::EventCallback));
                    });
                }
                let mut prop_path = vec![name.to_owned()];
                prop_path.extend(path);
                return self.component_props_uses(
                    file,
                    element,
                    prop_path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
            if element
                .spreads
                .iter()
                .any(|spread| peel(spread.argument) == expr)
            {
                if intrinsic_element(file, element) {
                    return match path.as_slice() {
                        [] => Some(()),
                        [key] if self.world.dialect().spread_key_is_event(key) == Some(true) => {
                            out.push(Invocation::Role(ExecutionRole::EventCallback));
                            Some(())
                        }
                        _ => None,
                    };
                }
                // A spread hands over every prop of its source, in an order
                // later attributes can replace, so which function the
                // component calls is not this value's to claim (ADR 0442).
                if self.write_root_body.is_some() {
                    return None;
                }
                return self.component_props_uses(
                    file,
                    element,
                    path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
            // A component's sole child expression is its `children` prop,
            // set after every attribute and spread. Several children make an
            // array, and a `children` attribute competes: both refuse.
            if element
                .children
                .iter()
                .any(|child| jsx_child_expression(file, *child) == Some(expr))
            {
                if intrinsic_element(file, element)
                    || jsx_children(file, element).len() != 1
                    || element
                        .attributes
                        .iter()
                        .any(|attribute| file.source_text(attribute.name) == Some("children"))
                {
                    return None;
                }
                let mut prop_path = vec!["children".to_owned()];
                prop_path.extend(path);
                return self.component_props_uses(
                    file,
                    element,
                    prop_path,
                    self.enclosing_props(file, element.span),
                    out,
                );
            }
        }
        // A property value of an object literal.
        if let Some(property) = file
            .ast
            .object_properties
            .iter()
            .find(|property| !property.computed && peel(property.value) == expr)
        {
            let object = innermost_object_literal(file, property.key)?;
            let key = property_static_key(file, property)?;
            if object
                .properties
                .iter()
                .any(|candidate| candidate.key.as_str() == key && candidate.getter.is_some())
                || receiver_used_within(file, object.span)
            {
                return None;
            }
            let mut member_path = vec![key];
            member_path.extend(path);
            return self.uses(
                file,
                object.span,
                member_path,
                FlowOrigin::Object(file, object.span),
                out,
            );
        }
        // Spread into an object literal.
        if let Some(spread) = file
            .ast
            .spreads
            .iter()
            .find(|spread| peel(spread.argument) == expr)
        {
            let object = file
                .ast
                .object_get_shapes
                .iter()
                .filter(|shape| shape.span.contains(spread.span) && shape.span != spread.span)
                .min_by_key(|shape| shape.span.end - shape.span.start)?;
            if path.is_empty() {
                return Some(());
            }
            // A later own property of the same key replaces it.
            if file.ast.object_properties.iter().any(|property| {
                object.span.contains(property.span)
                    && property.span.start > spread.span.start
                    && property_static_key(file, property).as_deref() == Some(path[0].as_str())
                    && innermost_object_literal(file, property.key)
                        .is_some_and(|inner| inner.span == object.span)
            }) {
                return Some(());
            }
            // The spread must be directly in that literal.
            if file.ast.object_get_shapes.iter().any(|inner| {
                inner.span != object.span
                    && object.span.contains(inner.span)
                    && inner.span.contains(spread.span)
            }) {
                return None;
            }
            // As for an own property: no getter of that key, and no function
            // in the literal that may reach the copied member through `this`.
            if object
                .properties
                .iter()
                .any(|candidate| candidate.key.as_str() == path[0] && candidate.getter.is_some())
                || receiver_used_within(file, object.span)
            {
                return None;
            }
            return self.uses(
                file,
                object.span,
                path,
                FlowOrigin::Object(file, object.span),
                out,
            );
        }
        // Returned.
        let returned_from = file
            .ast
            .returns
            .iter()
            .find(|returned| {
                returned
                    .argument
                    .is_some_and(|argument| peel(argument) == expr)
            })
            .and_then(|returned| containing_ast_function(&file.ast, returned.span))
            .or_else(|| {
                file.ast.functions.iter().find(|function| {
                    function.kind == solid_facts::ast::FunctionKind::Arrow
                        && peel(function.body) == expr
                })
            });
        if let Some(function) = returned_from {
            if function.r#async || function.generator {
                return None;
            }
            if let Some(index) = self.frames.iter().rposition(|(path, span, _, _)| {
                path == file.path.as_str() && *span == function.span
            }) {
                let (_, _, call_file, call) = self.frames[index];
                let saved = self.frames.split_off(index);
                let result = self.uses(call_file, call, path, origin.clone(), out);
                self.frames.extend(saved);
                return result;
            }
            for invocation in self.function_invocations(file, function)? {
                if let Invocation::Call(call_file, call, frames) = invocation {
                    let outer = std::mem::replace(&mut self.frames, frames);
                    let result = self.uses(call_file, call, path.clone(), origin.clone(), out);
                    self.frames = outer;
                    result?;
                }
            }
            return Some(());
        }
        // Bound to a `const`.
        if let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| peel(initializer) == expr)
        }) {
            // ADR 0442: an object pattern holding the object the value is a
            // member of (`const { add } = renderPanel()`): the tracked member
            // is the one binding the pattern names for its key. A pattern
            // with a rest element or a nested pattern (names and slots
            // differ), or any key that is not a plain identifier (a quoted
            // or computed key may name the member without its spelling),
            // refuses.
            if binding.shape == solid_facts::ast::BindingShape::Object {
                if binding.names.len() != binding.object_slots.len()
                    || binding
                        .object_slots
                        .iter()
                        .any(|slot| !slot.plain_identifier_key)
                {
                    return None;
                }
                let (first, rest) = path.split_first()?;
                let Some(slot) = binding
                    .object_slots
                    .iter()
                    .find(|slot| slot.property.as_str() == first)
                else {
                    // The object is read for other members only.
                    return Some(());
                };
                if !(binding.immutable || !crate::indexes::binding_written(file, slot.local.span))
                    || flow_binding_exported(file, slot.local.span)
                {
                    return None;
                }
                for (reference, _) in
                    file.ast
                        .reference_declarations
                        .iter()
                        .filter(|(reference, declaration)| {
                            *declaration == slot.local.span && *reference != slot.local.span
                        })
                {
                    self.uses(file, *reference, rest.to_vec(), FlowOrigin::Unknown, out)?;
                }
                return Some(());
            }
            let [name] = binding.names.as_slice() else {
                return None;
            };
            if binding.shape != solid_facts::ast::BindingShape::Identifier
                || !(binding.immutable || !crate::indexes::binding_written(file, name.span))
                || flow_binding_exported(file, name.span)
            {
                return None;
            }
            for (reference, _) in
                file.ast
                    .reference_declarations
                    .iter()
                    .filter(|(reference, declaration)| {
                        *declaration == name.span && *reference != name.span
                    })
            {
                self.uses(file, *reference, path.clone(), origin.clone(), out)?;
            }
            return Some(());
        }
        // A conditional's arm, or its test.
        if let Some(conditional) = file.ast.conditional_expressions.iter().find(|conditional| {
            peel(conditional.consequent) == expr || peel(conditional.alternate) == expr
        }) {
            return self.uses(file, conditional.span, path, FlowOrigin::Unknown, out);
        }
        if file
            .ast
            .conditional_expressions
            .iter()
            .any(|conditional| peel(conditional.test) == expr)
        {
            return Some(());
        }
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| peel(logical.left) == expr || peel(logical.right) == expr)
        {
            if peel(logical.left) == expr
                && logical.operator == solid_facts::ast::LogicalOperatorKind::And
            {
                // Its value is the left operand only when that is falsy.
                return Some(());
            }
            return self.uses(file, logical.span, path, FlowOrigin::Unknown, out);
        }
        None
    }

    /// The uses of `function`'s parameter `index`, holding the value at
    /// `path`.
    fn parameter_uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        index: usize,
        path: Vec<String>,
        origin: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        if !file_is_module(file) {
            return None;
        }
        self.touched.insert(file.path.as_str());
        let parameter = function.parameters.get(index)?;
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.shape != solid_facts::ast::BindingShape::Identifier
            || parameter.initializer.is_some()
            || (function.rest_parameter && index + 1 == function.parameters.len())
            || crate::indexes::binding_written(file, name.span)
            || file.ast.identifiers.iter().any(|identifier| {
                function.span.contains(identifier.span)
                    && matches!(
                        file.source_text(identifier.span),
                        Some("arguments" | "eval")
                    )
            })
        {
            return None;
        }
        for (reference, _) in file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
        {
            if !function.body.contains(*reference) {
                return None;
            }
            self.uses(file, *reference, path.clone(), origin.clone(), out)?;
        }
        Some(())
    }

    /// The uses of the props a project component element renders with,
    /// holding the value at `path`.
    fn component_props_uses(
        &mut self,
        file: &'a solid_facts::FileFacts,
        element: &solid_facts::ast::JsxElementFact,
        path: Vec<String>,
        enclosing: FlowOrigin<'a>,
        out: &mut Vec<Invocation<'a>>,
    ) -> Option<()> {
        let (component_file, component) = self.world.function_called_at(file, element.name.span)?;
        if component.r#async || component.generator {
            return None;
        }
        // ADR 0442: the prop must be the one the component receives under its
        // name: a spread may carry another function under it, or replace it
        // (`<Child onPress={a} {...rest} />`), and a repeated attribute
        // competes with it. The owned-scope answer is a violation, so a prop
        // another value may have replaced proves nothing.
        if self.write_root_body.is_some()
            && (!element.spreads.is_empty()
                || path.first().is_some_and(|prop| {
                    element
                        .attributes
                        .iter()
                        .filter(|attribute| {
                            attribute.namespace.is_none()
                                && file.source_text(attribute.name) == Some(prop.as_str())
                        })
                        .count()
                        > 1
                }))
        {
            return None;
        }
        // The tag must run that declaration: its binding is current, and a
        // member tag (`Menu.Item`) reaches exactly the declared member.
        let tag = file.ast.peel_ts_sugar_span(element.name.span);
        let member_tag = file.ast.members.iter().any(|member| member.span == tag)
            || file.ast.computed_members.binary_search(&tag).is_ok();
        if !crate::indexes::function_value_is_current_in(component_file, component)
            || (member_tag
                && !self
                    .world
                    .member_runs_declared_method(file, tag, component_file, component))
        {
            return None;
        }
        let component_file = self.world.canonical(component_file)?;
        self.touched.insert(component_file.path.as_str());
        // A component rendering itself through this element again, holding
        // the value under the same member path (a tree of branches): the
        // recursive activation holds it at the same positions with the same
        // call context, so its invocations are the ones this activation
        // finds. Only its receiver proofs could differ, and they resolve
        // through the elements of the cycle back to the ones already proven,
        // provided every element in the cycle forwards each prop unchanged or
        // gives it a value that ignores `this`. A cycle that moves the value
        // to another prop (`b={props.a}`) is a different path and is followed
        // again, until it repeats or the budget runs out.
        if let Some(index) = self.props_stack.iter().position(|frame| {
            frame.element_file.path == file.path
                && frame.element == element.span
                && frame.followed == path
        }) {
            return self.props_stack[index..]
                .iter()
                .all(|frame| self.cycle_element_forwards(frame))
                .then_some(());
        }
        let origin = FlowOrigin::Element(file, element.span, Box::new(enclosing));
        self.props_stack.push(PropsFrame {
            path: component_file.path.to_string(),
            component: component.span,
            element_file: file,
            element: element.span,
            followed: path.clone(),
            origin: origin.clone(),
        });
        let result = self.parameter_uses(component_file, component, 0, path, origin, out);
        self.props_stack.pop();
        result
    }

    /// For an element on a rendering cycle: it has no spread, and each
    /// member the rendered component calls on its props is the same-named
    /// prop of the component the element is written in, forwarded unchanged,
    /// a value that ignores `this`, or undefined.
    fn cycle_element_forwards(&self, frame: &PropsFrame<'a>) -> bool {
        let file = frame.element_file;
        let Some(element) = file.ast.jsx_element_at(frame.element) else {
            return false;
        };
        let children = jsx_children(file, element);
        if !element.spreads.is_empty() || children.len() > 1 {
            return false;
        }
        // The props parameter of the component the element is written in.
        let Some(writer) = self.props_stack.iter().rev().find(|outer| {
            outer.path == file.path.as_str() && outer.component.contains(element.span)
        }) else {
            return false;
        };
        let Some(writer) = file.ast.function_at(writer.component) else {
            return false;
        };
        let Some(writer_props) = props_parameter(file, writer) else {
            return false;
        };
        let forwards = |value: Span, key: &str| {
            let value = jsx_value_expression(file, value);
            file.ast.members.iter().any(|member| {
                member.span == value
                    && member_static_key(file, member).as_deref() == Some(key)
                    && file
                        .ast
                        .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
                        == Some(writer_props)
            }) || value_ignores_receiver(file, value, &FlowOrigin::Unknown, self.world, 6)
        };
        // Members the rendered component calls on its props: each must be
        // forwarded, given a value that ignores `this`, or undefined.
        let Some((component_file, component)) =
            self.world.function_called_at(file, element.name.span)
        else {
            return false;
        };
        let Some(component_props) = props_parameter(component_file, component) else {
            return false;
        };
        component_file.ast.calls.iter().all(|call| {
            let callee = component_file.ast.peel_ts_sugar_span(call.callee);
            let Some(member) = component_file.ast.members.iter().find(|member| {
                member.span == callee
                    && component.span.contains(member.span)
                    && component_file
                        .ast
                        .reference_declaration(component_file.ast.peel_ts_sugar_span(member.object))
                        == Some(component_props)
            }) else {
                return true;
            };
            let Some(key) = member_static_key(component_file, member) else {
                return false;
            };
            let key = key.as_str();
            if key == "children" && !children.is_empty() {
                return children
                    .first()
                    .is_some_and(|child| forwards(*child, "children"));
            }
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(key))
            {
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.namespace.is_none()
                            && attribute
                                .expression
                                .is_some_and(|value| forwards(value, key))
                    }
                    _ => false,
                },
                None => self.world.absent_member_inert(key),
            }
        })
    }

    /// The props of the component activation an element at `element` is
    /// written in, when the walk entered that component through its props.
    fn enclosing_props(&self, file: &solid_facts::FileFacts, element: Span) -> FlowOrigin<'a> {
        self.props_stack
            .iter()
            .rev()
            .find(|frame| frame.path == file.path.as_str() && frame.component.contains(element))
            .map_or(FlowOrigin::Unknown, |frame| frame.origin.clone())
    }
}
