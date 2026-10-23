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
    let mut literals = HashSet::default();
    if !lookup.program_closed
        || lookup.files().iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        })
    {
        return literals;
    }
    for file in lookup.files() {
        for literal in &file.ast.functions {
            if literal.generator
                || lookup.function_component_status(file, literal) != ComponentStatus::No
            {
                continue;
            }
            let mut classifying = HashSet::default();
            let mut flow = InvocationFlow {
                lookup,
                classifying: &mut classifying,
                budget: FLOW_BUDGET,
                seen: HashSet::default(),
                frames: Vec::new(),
                props_stack: Vec::new(),
                owner_mode: true,
                mixed_mode: false,
                refused_mix: false,
            };
            if flow.function_role(file, literal) == Some(Some(ExecutionRole::EventCallback)) {
                literals.insert((file.path.to_string(), literal.span));
            }
        }
    }
    literals
}

/// ADR 0363: every function literal of a closed program that has a proven
/// event edge beside invocations that are not event runs, with the edge it is
/// named by. The literal is run by the host as an intrinsic element's handler
/// (or by a call written directly in one that is), and also from somewhere that
/// may supply an owner: a `runWithOwner` callback slot, a plain call in a
/// component's setup. [`event_invoked_literals`] takes the literals with no such
/// invocation, so the two sets are disjoint. Whether the event run reaches the
/// literal with no owner is the owner graph's question, asked with ADR 0356's
/// dispatch veto.
pub(crate) fn mixed_event_literals(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), EventEdge> {
    let mut literals = HashMap::default();
    if !lookup.program_closed
        || lookup.files().iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        })
    {
        return literals;
    }
    let event_only = lookup.event_invoked_literals();
    for file in lookup.files() {
        for literal in &file.ast.functions {
            if literal.generator
                || lookup.function_component_status(file, literal) != ComponentStatus::No
                || event_only.contains(&(file.path.to_string(), literal.span))
            {
                continue;
            }
            let mut classifying = HashSet::default();
            let mut flow = InvocationFlow {
                lookup,
                classifying: &mut classifying,
                budget: FLOW_BUDGET,
                seen: HashSet::default(),
                frames: Vec::new(),
                props_stack: Vec::new(),
                owner_mode: true,
                mixed_mode: true,
                refused_mix: false,
            };
            if let Some(edge) = flow.function_event_edge(file, literal) {
                literals.insert((file.path.to_string(), literal.span), edge);
            }
        }
    }
    literals
}

/// ADR 0361 (amended): where the host's event runs of a function literal end.
/// Each is an intrinsic element's `on…` attribute or spread key the literal
/// reaches through ADR 0319's walk, or the attribute of a compiler event
/// handler that calls it; the listener type is the attribute's lowercased tail.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EventTerminals {
    /// The lowercase listener types of the attributes and keys the runs end at.
    pub(crate) types: std::collections::BTreeSet<String>,
    /// The intrinsic elements those attributes and keys are written on.
    pub(crate) elements: std::collections::BTreeSet<(String, Span)>,
    /// Some run ends at an attribute whose type is not read (non-ASCII, a
    /// handler no attribute names exactly, a cycle or refused walk on the way).
    pub(crate) unreadable: bool,
    /// Every invocation is an event run ([`event_invoked_literals`]): the
    /// terminals are all of the literal's runs, not only its event runs.
    pub(crate) event_only: bool,
}

impl EventTerminals {
    fn note(&mut self, attribute: &str, element: Option<(String, Span)>) {
        match crate::indexes::listener_type(attribute) {
            Some(event_type) => {
                self.types.insert(event_type);
            }
            None => self.unreadable = true,
        }
        match element {
            Some(element) => {
                self.elements.insert(element);
            }
            None => self.unreadable = true,
        }
    }

    fn merge(&mut self, other: Self) {
        self.types.extend(other.types);
        self.elements.extend(other.elements);
        self.unreadable |= other.unreadable;
    }
}

/// ADR 0361 (amended): the event terminals of every literal the owner censuses
/// prove event-run ([`event_invoked_literals`], [`mixed_event_literals`]), so a
/// synchronous dispatch withholds a forwarded handler only when it fires the
/// type of an attribute the handler ends at, as it does a handler written on
/// the element. A literal whose walk the terminal census cannot follow has no
/// entry: it stays conditional while any dispatch exists.
pub(crate) fn event_terminals(
    lookup: &SemanticLookup<'_>,
) -> HashMap<(String, Span), EventTerminals> {
    let mut terminals = HashMap::default();
    let event_only = lookup.event_invoked_literals();
    let literals = event_only
        .iter()
        .map(|key| (key, true))
        .chain(lookup.mixed_event_literals().keys().map(|key| (key, false)));
    for ((path, span), only) in literals {
        let Some(file) = lookup.file_by_path(path) else {
            continue;
        };
        let Some(literal) = file.ast.function_at(*span) else {
            continue;
        };
        let mut classifying = HashSet::default();
        let mut flow = InvocationFlow {
            lookup,
            classifying: &mut classifying,
            budget: FLOW_BUDGET,
            seen: HashSet::default(),
            frames: Vec::new(),
            props_stack: Vec::new(),
            owner_mode: true,
            mixed_mode: true,
            refused_mix: false,
        };
        if let Some(mut found) = flow.function_event_terminals(file, literal) {
            found.event_only = only;
            terminals.insert((path.clone(), *span), found);
        }
    }
    terminals
}

/// The attribute and intrinsic element a compiler event handler's value is
/// written on; empty when no attribute names it exactly.
fn handler_edge(file: &solid_facts::FileFacts, handler: Span) -> EventEdge {
    let handler = file.ast.peel_ts_sugar_span(handler);
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| intrinsic_element(file, element))
        .find_map(|element| {
            element
                .attributes
                .iter()
                .find(|attribute| {
                    attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == handler)
                })
                .map(|attribute| EventEdge {
                    attribute: file
                        .source_text(attribute.name)
                        .unwrap_or_default()
                        .to_owned(),
                    element: file
                        .source_text(element.name.span)
                        .unwrap_or_default()
                        .to_owned(),
                })
        })
        .unwrap_or_default()
}

/// The attribute name and the intrinsic element a compiler event handler's
/// value is written on, when an attribute names it exactly.
fn handler_attribute(file: &solid_facts::FileFacts, handler: Span) -> Option<(&str, Span)> {
    let handler = file.ast.peel_ts_sugar_span(handler);
    file.ast
        .jsx_elements
        .iter()
        .filter(|element| intrinsic_element(file, element))
        .find_map(|element| {
            element
                .attributes
                .iter()
                .find(|attribute| {
                    attribute.namespace.is_none()
                        && attribute
                            .expression
                            .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == handler)
                })
                .and_then(|attribute| file.source_text(attribute.name))
                .map(|name| (name, element.span))
        })
}

impl<'a> InvocationFlow<'a, '_, '_> {
    /// The one safe role of every invocation of `function`; `Some(None)`
    /// when the (complete) census finds no invocation at all.
    pub(super) fn function_role(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Option<ExecutionRole>> {
        let invocations = self.function_invocations(file, function)?;
        let mut role = None::<ExecutionRole>;
        for invocation in invocations {
            let here = match invocation {
                Invocation::Role(role) => role,
                Invocation::Event(..) => ExecutionRole::EventCallback,
                // Only the mixed-invocation census produces these, and it asks
                // for event edges, not for the one role of every invocation.
                Invocation::Other => return None,
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
                Some(_) => {
                    self.refused_mix = true;
                    return None;
                }
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
        let callee = file.ast.call_at(call)?.callee;
        if self.owner_mode {
            return self.owner_site_role(file, call, frames);
        }
        let here = semantic_execution_role_within(
            file,
            callee,
            &[],
            self.lookup.entities(),
            self.lookup.symbol_names(),
            self.lookup,
            self.classifying,
        );
        // ADR 0320: a role read off a named function's callback references
        // ignores its direct calls (`press()` beside `onClick={press}`); the
        // walk below asks for every invocation of that function instead.
        if flow_safe_role(here)
            && named_callbacks_within_component(file, call, self.lookup)
                .into_iter()
                .all(|function| named_callback_invocations_hold(file, function, self.lookup, false))
        {
            return Some(Some(here));
        }
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
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
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
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
        if let Some(callback) = file.compiler.callback_roles.iter().find(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == owner.span
        }) {
            // ADR 0364: a handler a later attribute or spread may replace is
            // not proven to run.
            if self
                .lookup
                .event_attribute_may_be_replaced(file, callback.span)
            {
                return None;
            }
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

    /// The host's event run of the tracked value, as the walk reports it.
    ///
    /// ADR 0364: `None` when a later attribute or spread of the element may
    /// replace the key defined at `defined` (`event_key_may_be_replaced`): the
    /// host may never run the value, so the walk cannot name the invocation and
    /// the claim is withheld, in both censuses.
    fn event_invocation(
        &self,
        file: &'a solid_facts::FileFacts,
        element: &solid_facts::ast::JsxElementFact,
        defined: Span,
        attribute: &str,
    ) -> Option<Invocation<'a>> {
        if crate::object_members::event_key_may_be_replaced(
            self.lookup,
            file,
            element,
            defined,
            attribute,
        ) {
            return None;
        }
        if !self.mixed_mode {
            return Some(Invocation::Role(ExecutionRole::EventCallback));
        }
        Some(Invocation::Event(
            EventEdge {
                attribute: attribute.to_owned(),
                element: file
                    .source_text(element.name.span)
                    .unwrap_or_default()
                    .to_owned(),
            },
            file,
            element.span,
        ))
    }

    /// The invocation a returning function has when it is the callback of an
    /// owner-supplying call whose callback runs, and returns its value, inside
    /// the call (`createRoot`, `runWithOwner`): the call itself. The value
    /// the callback returns is the call's value, and that is what the walk
    /// follows; who owns the callback's own body is not asked here. Owner
    /// walks only: the read walk reaches the same call through the primitive's
    /// callback slot.
    fn root_pass_through(
        &self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<Invocation<'a>> {
        if !self.owner_mode {
            return None;
        }
        let (call, index) =
            file.ast
                .arguments_containing(function.span)
                .find(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
                })?;
        let primitive = self.lookup.primitive_at_call(file, call.span)?;
        (!call.construct
            && call
                .arguments
                .iter()
                .take(index + 1)
                .all(|argument| !argument.spread)
            && self
                .lookup
                .dialect
                .callback_inherits_calling_window(primitive, index)
            && callback_execution_at_call(file, call, primitive, index, self.lookup).is_some())
        .then(|| Invocation::Call(file, call.span, self.frames.clone()))
    }

    /// ADR 0363: the proven event edge of the function the walk finds, if any
    /// invocation of it is the host's event run, or a call written directly in
    /// a function that has one. `None` when the walk cannot follow every holder
    /// of the value (an unresolved holder withholds the whole claim).
    pub(super) fn function_event_edge(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<EventEdge> {
        let invocations = self.function_invocations(file, function)?;
        let mut found = None;
        for invocation in invocations {
            let edge = match invocation {
                Invocation::Event(edge, ..) => Some(edge),
                Invocation::Call(call_file, call, frames) => {
                    self.call_event_edge(call_file, call, frames)
                }
                Invocation::Role(_) | Invocation::Other => None,
            };
            if found.is_none() {
                found = edge;
            }
        }
        found
    }

    /// The event terminals of every event run of `function`: each
    /// [`Invocation::Event`], and each call written directly in a compiler event
    /// handler (its attribute) or in a plain function (that function's
    /// terminals), with [`Self::call_event_edge`]'s eligibility. Other
    /// invocations are not event runs and add nothing. `None` when the walk
    /// cannot follow every holder.
    pub(super) fn function_event_terminals(
        &mut self,
        file: &'a solid_facts::FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Option<EventTerminals> {
        let invocations = self.function_invocations(file, function)?;
        let mut terminals = EventTerminals::default();
        for invocation in invocations {
            match invocation {
                Invocation::Event(edge, element_file, element) => terminals.note(
                    &edge.attribute,
                    Some((element_file.path.to_string(), element)),
                ),
                Invocation::Call(call_file, call, frames) => {
                    terminals.merge(self.call_event_terminals(call_file, call, frames));
                }
                Invocation::Role(_) | Invocation::Other => {}
            }
        }
        Some(terminals)
    }

    /// [`Self::function_event_terminals`] for one call that runs the tracked
    /// value. A handler a later attribute may replace still names the type it
    /// would run as; a call whose function cannot be followed is unreadable.
    fn call_event_terminals(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> EventTerminals {
        let mut terminals = EventTerminals::default();
        let Some(owner) = containing_ast_function(&file.ast, call) else {
            return terminals;
        };
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
            || !crate::owners::written_directly_in(&file.ast, owner, call)
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
            return terminals;
        }
        if let Some(callback) = file.compiler.callback_roles.iter().find(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == owner.span
        }) {
            match handler_attribute(file, callback.span) {
                Some((attribute, element)) => {
                    terminals.note(attribute, Some((file.path.to_string(), element)))
                }
                None => terminals.unreadable = true,
            }
            return terminals;
        }
        let key = (
            format!("\0terminal-flow:{}", file.path.as_str()),
            owner.span,
        );
        if !self.classifying.insert(key.clone()) {
            terminals.unreadable = true;
            return terminals;
        }
        let outer = std::mem::replace(&mut self.frames, frames);
        match self.function_event_terminals(file, owner) {
            Some(found) => terminals.merge(found),
            None => terminals.unreadable = true,
        }
        self.frames = outer;
        self.classifying.remove(&key);
        terminals
    }

    /// The event edge of a call that runs the tracked value: the call is written
    /// directly in a compiler event handler, or in a plain function that has an
    /// event edge itself. The eligibility is [`Self::owner_site_role`]'s.
    fn call_event_edge(
        &mut self,
        file: &'a solid_facts::FileFacts,
        call: Span,
        frames: Vec<FlowFrame<'a>>,
    ) -> Option<EventEdge> {
        let owner = containing_ast_function(&file.ast, call)?;
        if !owner.body.contains(call)
            || (owner.r#async && may_follow_suspension(file, call))
            || owner.generator
            || self.lookup.function_component_status(file, owner) != ComponentStatus::No
            || !crate::owners::written_directly_in(&file.ast, owner, call)
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
        if let Some(callback) = file.compiler.callback_roles.iter().find(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == owner.span
        }) {
            if self
                .lookup
                .event_attribute_may_be_replaced(file, callback.span)
            {
                return None;
            }
            return Some(handler_edge(file, callback.span));
        }
        let key = (format!("\0mixed-flow:{}", file.path.as_str()), owner.span);
        if !self.classifying.insert(key.clone()) {
            return None;
        }
        let outer = std::mem::replace(&mut self.frames, frames);
        let edge = self.function_event_edge(file, owner);
        self.frames = outer;
        self.classifying.remove(&key);
        edge
    }

    /// The context the walk is in, as a `seen` key component: the callee
    /// calls it entered through a parameter and the elements whose props it
    /// is inside. Receiver proofs depend on the elements.
    fn context(&self) -> Vec<(String, Span)> {
        self.frames
            .iter()
            .map(|(_, _, file, call)| (file.path.to_string(), *call))
            .chain(
                self.props_stack
                    .iter()
                    .map(|frame| (frame.element_file.path.to_string(), frame.element)),
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
            if flow_binding_exported(file, name.span)
                || crate::indexes::binding_written(file, name.span)
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
        let expr = file.ast.peel_ts_sugar_span(expr);
        if !self.seen.insert((
            file.path.to_string(),
            expr,
            path.clone(),
            self.context(),
            origin_identity(&origin),
        )) {
            return Some(());
        }
        let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
        // Called.
        if let Some(call) = file.ast.calls.iter().find(|call| peel(call.callee) == expr) {
            if call.construct || !path.is_empty() {
                return None;
            }
            out.push(Invocation::Call(file, call.span, self.frames.clone()));
            return Some(());
        }
        // The object of a member access.
        if let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| peel(member.object) == expr)
        {
            if file
                .ast
                .assignments
                .iter()
                .any(|assignment| assignment.target.contains(member.span))
            {
                return None;
            }
            // A computed member counts only with a literal key.
            let key = member_static_key(file, member)?;
            let key = key.as_str();
            let called = file
                .ast
                .calls
                .iter()
                .any(|call| peel(call.callee) == member.span)
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
                        origin_member_ignores_receiver(&origin, key, self.lookup, 6).then_some(())
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
            if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
                // ADR 0363: the function itself in a primitive's slot is one more
                // invocation, not an event edge and not a reason to refuse the
                // others; a container holding it still refuses.
                if self.mixed_mode && path.is_empty() {
                    out.push(Invocation::Other);
                    return Some(());
                }
                if !path.is_empty() || self.owner_mode {
                    return None;
                }
                // A callback that runs synchronously in the call's own window
                // (`createRoot`, `runWithOwner`) runs wherever the call does:
                // its invocation is the call.
                if self
                    .lookup
                    .dialect
                    .callback_inherits_calling_window(primitive, index)
                    && callback_execution_at_call(file, call, primitive, index, self.lookup)
                        .is_some()
                {
                    out.push(Invocation::Call(file, call.span, self.frames.clone()));
                    return Some(());
                }
                let role = primitive_callback_slot_role(file, call, index, self.lookup)?;
                out.push(Invocation::Role(role));
                return Some(());
            }
            let (callee_file, callee, exact) =
                project_callee_at_parameter(file, call, index, self.lookup)?;
            if !exact
                || self.lookup.function_component_status(callee_file, callee) != ComponentStatus::No
            {
                return None;
            }
            let callee_file = self.lookup.file_by_path(callee_file.path.as_str())?;
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
                    if !path.is_empty()
                        || !self.lookup.dialect.compiler_lowers_event_attribute(name)
                    {
                        return None;
                    }
                    out.push(self.event_invocation(file, element, attribute.span, name)?);
                    return Some(());
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
            if let Some(spread) = element
                .spreads
                .iter()
                .find(|spread| peel(spread.argument) == expr)
            {
                if intrinsic_element(file, element) {
                    return match path.as_slice() {
                        [] => Some(()),
                        [key] if self.lookup.dialect.spread_key_is_event(key) == Some(true) => {
                            out.push(self.event_invocation(file, element, spread.span, key)?);
                            Some(())
                        }
                        _ => None,
                    };
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
            let invocations = match self.root_pass_through(file, function) {
                Some(invocation) => vec![invocation],
                None => self.function_invocations(file, function)?,
            };
            for invocation in invocations {
                match invocation {
                    Invocation::Call(call_file, call, frames) => {
                        let outer = std::mem::replace(&mut self.frames, frames);
                        let result = self.uses(call_file, call, path.clone(), origin.clone(), out);
                        self.frames = outer;
                        result?;
                    }
                    // The value this function returns goes into a primitive's
                    // slot: the walk does not follow it there.
                    Invocation::Other => return None,
                    Invocation::Role(_) | Invocation::Event(..) => {}
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
        let (component_file, component) = self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)?;
        if component.r#async || component.generator {
            return None;
        }
        // The tag must run that declaration: its binding is current, and a
        // member tag (`Menu.Item`) reaches exactly the declared member.
        let tag = file.ast.peel_ts_sugar_span(element.name.span);
        let member_tag = file.ast.members.iter().any(|member| member.span == tag)
            || file.ast.computed_members.binary_search(&tag).is_ok();
        if !self
            .lookup
            .function_value_is_current(component_file, component)
            || (member_tag
                && !member_runs_declared_method(file, tag, component_file, component, self.lookup))
        {
            return None;
        }
        let component_file = self.lookup.file_by_path(component_file.path.as_str())?;
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
            }) || value_ignores_receiver(file, value, &FlowOrigin::Unknown, self.lookup, 6)
        };
        // Members the rendered component calls on its props: each must be
        // forwarded, given a value that ignores `this`, or undefined.
        let Some((component_file, component)) = self
            .lookup
            .function_called_at(file.path.as_str(), element.name.span)
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
                None => {
                    !self.lookup.member_may_be_reassigned_on_builtin(key)
                        && !self.lookup.member_may_be_reassigned_on_builtin("__proto__")
                        && !self.lookup.property_may_be_defined_reflectively_as(key)
                }
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
