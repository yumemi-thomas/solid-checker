//! Visible synchronous DOM dispatch withholds an ownerless event proof from
//! the handlers it can reach (ADR 0356, 0361).

use super::*;
use crate::dispatch_events::Fires;
use crate::execution_role::EventTerminals;
use solid_facts::ast::ArgumentLiteralFact;
use solid_facts::compiler::CallbackRoleKind;

/// What the project's synchronous DOM dispatches can reach.
///
/// A synchronous dispatch can only run the handlers of the event types it
/// fires, so each site is a set of types ([`Fires`]); a handler is reached when
/// its own type is in the union of every site's set. A site the analysis can
/// join to one element's handlers (the supported `dispatchEvent`) is narrower
/// and fires no type program-wide.
#[derive(Default)]
pub(crate) struct EventDelivery {
    /// Some synchronous dispatch site exists (a call or a reference).
    any_site: bool,
    /// Some site may fire any type, or is a dispatch no site models
    /// ([`value_reference_dispatches`]): any handler may be reached.
    fired_any: bool,
    /// The lowercase event types the sites fire.
    fired: HashSet<String>,
    /// The handlers of the narrowly joined sites, by file: the span of an event
    /// attribute's value, or of a function a spread holder supplies.
    delivered: HashMap<String, HashSet<Span>>,
    /// The elements the narrowly joined sites dispatch on, by file and span: a
    /// forwarded handler that ends at one of their attributes is reached.
    delivered_elements: HashSet<(String, Span)>,
    /// The event type of each handler written on an element or supplied by a
    /// spread holder, by file and span. Built only while no site fires any type.
    handler_types: HashMap<(String, Span), HandlerTypes>,
}

/// The types one handler span listens for: the types its attributes and spread
/// keys name, or unreadable when one cannot be read.
#[derive(Default)]
struct HandlerTypes {
    types: HashSet<String>,
    unreadable: bool,
}

impl EventDelivery {
    fn build(lookup: &SemanticLookup<'_>) -> Self {
        let mut delivery = Self::default();
        let continuations = crate::owners::AwaitContinuations::new(lookup.facts, lookup);
        for file in lookup.files() {
            let mut callees = HashSet::default();
            for call in &file.ast.calls {
                callees.insert(file.ast.peel_ts_sugar_span(call.callee));
                // ADR 0366: a dispatch whose own stack has no owner runs its
                // handlers with none, as the host does: it is no site.
                let ownerless = || ownerless_dispatch_stack(lookup, &continuations, file, call);
                delivery.visit_call(lookup, file, call, &ownerless);
                if delivery.fired_any {
                    return delivery.every_handler();
                }
            }
            if value_reference_dispatches(lookup, file, &callees) {
                delivery.any_site = true;
                return delivery.every_handler();
            }
        }
        // The type join needs the release's handler grammar; a release whose web
        // runtime was not read has every dispatch reach every handler.
        if delivery.any_site && !lookup.dialect.event_handler_runs_only_for_its_type() {
            return delivery.every_handler();
        }
        delivery.read_handler_types(lookup);
        delivery
    }

    /// The state in which any handler may be reached.
    fn every_handler(mut self) -> Self {
        self.fired_any = true;
        self.fired.clear();
        self.delivered.clear();
        self.delivered_elements.clear();
        self
    }

    fn fire(&mut self, fires: Fires) {
        match fires {
            Fires::AnyType => self.fired_any = true,
            Fires::Types(types) => self.fired.extend(types.iter().map(|t| (*t).to_owned())),
        }
    }

    /// A `dispatchEvent` of a literal type fires that type; an event whose type
    /// is not read fires any.
    fn fire_event_type(&mut self, event_type: Option<&str>) {
        match event_type {
            Some(event_type) if event_type.is_ascii() => {
                self.fired.insert(event_type.to_ascii_lowercase());
            }
            _ => self.fired_any = true,
        }
    }

    /// A `dispatchEvent` of an inline `lib.dom` event: its type, and with it the
    /// activation behavior a synthetic `click` runs ([`DispatchedEvent`]).
    fn fire_dispatched_event(&mut self, event: Option<DispatchedEvent<'_>>) {
        match event {
            Some(event) if event.activation => {
                self.fire(crate::dispatch_events::synthetic_click_fires())
            }
            Some(event) => self.fire_event_type(Some(event.event_type)),
            None => self.fire_event_type(None),
        }
    }

    /// `ownerless` answers ADR 0366 for the call; it is asked only once the call
    /// is known to dispatch.
    fn visit_call(
        &mut self,
        lookup: &SemanticLookup<'_>,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        ownerless: &dyn Fn() -> bool,
    ) {
        let resolved = lookup.resolved_callee_call(file, call.callee);
        if let Some(resolved) = resolved.filter(|resolved| resolved.declaration.is_some()) {
            // A call whose callee resolves to a declaration is judged by it
            // (ADR 0329's members dispatch; an exact project method of the same
            // spelling does not).
            if crate::runtime_semantics::standard_call_name(resolved)
                == Some("EventTarget.dispatchEvent")
            {
                if ownerless() {
                    return;
                }
                self.any_site = true;
                match supported_site(lookup, file, call) {
                    Some((element, handlers)) => {
                        if let Some(element) = element {
                            self.delivered_elements
                                .insert((file.path.to_string(), element));
                        }
                        for (path, span) in handlers {
                            self.delivered.entry(path).or_default().insert(span);
                        }
                    }
                    None => self.fire_dispatched_event(dispatched_event_type(lookup, file, call)),
                }
                return;
            }
            if let Some(fires) = crate::cleanup::resolved_dom_dispatch_fires(resolved) {
                if ownerless() {
                    return;
                }
                self.any_site = true;
                if crate::cleanup::is_dispatch_event_declaration(resolved) {
                    self.fire_dispatched_event(dispatched_event_type(lookup, file, call));
                } else {
                    self.fire(fires);
                }
            }
            return;
        }
        self.visit_unresolved_call(lookup, file, call, ownerless);
    }

    /// A member call that resolves to no declaration, spelled as one of ADR
    /// 0329's members, may be any of them on an `any` or unresolved receiver
    /// (ADR 0361), and fires that member's set. A computed key that is not a
    /// literal names no member: unless [`computed_key_names_no_member`] proves
    /// the call is no member call, it may be any, so it fires any type.
    fn visit_unresolved_call(
        &mut self,
        lookup: &SemanticLookup<'_>,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        ownerless: &dyn Fn() -> bool,
    ) {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let name = if file.ast.computed_members.contains(&callee) {
            let Some(member) = file
                .ast
                .literal_computed_members
                .iter()
                .find(|member| member.span == callee)
            else {
                if !computed_key_names_no_member(lookup, file, call, callee) && !ownerless() {
                    self.any_site = true;
                    self.fired_any = true;
                }
                return;
            };
            member.key.as_str()
        } else {
            let Some(member) = file.ast.member_at(callee) else {
                return;
            };
            let Some(name) = file.source_text(member.property) else {
                return;
            };
            if !crate::dispatching_dom_member_spelling(name)
                || !property_dispatches(lookup, file, member.property).unwrap_or(true)
            {
                return;
            }
            name
        };
        if !crate::dispatching_dom_member_spelling(name) || ownerless() {
            return;
        }
        self.any_site = true;
        if name == "dispatchEvent" {
            self.fire_dispatched_event(dispatched_event_type(lookup, file, call));
        } else {
            for fires in crate::dispatch_events::spelled_member_fires(name) {
                self.fire(fires);
            }
        }
    }

    /// The type of every handler an intrinsic element's attributes and spread
    /// holders supply (ADR 0358): the `on`+uppercase attribute's tail, lowercased
    /// as the compiler names the listener (`name.slice(2).toLowerCase()`), or the
    /// spread key's.
    fn read_handler_types(&mut self, lookup: &SemanticLookup<'_>) {
        if self.fired.is_empty() {
            return;
        }
        for file in lookup.files() {
            for element in &file.ast.jsx_elements {
                if !crate::object_members::intrinsic_element(file, element) {
                    continue;
                }
                for attribute in &element.attributes {
                    if attribute.namespace.is_some() {
                        continue;
                    }
                    let (Some(name), Some(expression)) =
                        (file.source_text(attribute.name), attribute.expression)
                    else {
                        continue;
                    };
                    if !lookup.dialect.compiler_lowers_event_attribute(name) {
                        continue;
                    }
                    let expression = file.ast.peel_ts_sugar_span(expression);
                    let event_type = listener_type(name);
                    for span in [
                        expression,
                        crate::owners::handler_value_span(file, expression),
                    ] {
                        self.note_handler(file.path.as_str(), span, event_type.as_deref());
                    }
                }
                for spread in &element.spreads {
                    let key = crate::location(file.path.shared(), spread.argument);
                    let Some(Some(targets)) = lookup.spread_handler_targets().get(&key) else {
                        continue;
                    };
                    for held in targets {
                        self.note_handler(
                            &held.path,
                            held.span,
                            listener_type(&held.key).as_deref(),
                        );
                    }
                }
            }
        }
    }

    fn note_handler(&mut self, path: &str, span: Span, event_type: Option<&str>) {
        let types = self
            .handler_types
            .entry((path.to_owned(), span))
            .or_default();
        match event_type {
            Some(event_type) => {
                types.types.insert(event_type.to_owned());
            }
            None => types.unreadable = true,
        }
    }

    /// Whether the handler at `span` may be reached: the narrow site delivering
    /// to it, or a fired type that is one of its own (a handler whose type is
    /// not read may be any).
    fn reaches(&self, path: &str, span: Span) -> bool {
        if self.fired_any
            || self
                .delivered
                .get(path)
                .is_some_and(|spans| spans.contains(&span))
        {
            return true;
        }
        if self.fired.is_empty() {
            return false;
        }
        self.handler_types
            .get(&(path.to_owned(), span))
            .is_none_or(|handler| {
                handler.unreadable || handler.types.iter().any(|ty| self.fired.contains(ty))
            })
    }

    /// Whether an event run that ends at `terminals` may be reached by some
    /// site (ADR 0361, amended): as [`Self::reaches`], by the terminal
    /// attributes' types, or by a narrowly joined site on one of their elements.
    /// A walk the census could not follow (no terminals, none at all, or an
    /// unreadable one) is reached by any site, as before the join.
    fn reaches_terminals(&self, terminals: Option<&EventTerminals>) -> bool {
        let Some(terminals) = terminals else {
            return true;
        };
        self.fired_any
            || terminals.unreadable
            || terminals.types.is_empty()
            || terminals
                .elements
                .iter()
                .any(|element| self.delivered_elements.contains(element))
            || terminals.types.iter().any(|ty| self.fired.contains(ty))
    }
}

/// ADR 0366: whether the dispatch `call` provably runs on a stack no owner is
/// current on, so the handlers it runs have none, as when the host runs them.
/// Three shapes, each syntactic and none read off a computed owner context
/// (that would be circular with the handlers' own contexts):
///
/// - the call follows an `await` that dominates it in its own async function
///   (the producer's `calls_after_await`, ADR 0176: a promise continuation)
///   and is written directly in that function's flow: the producer also lists
///   calls in getters, setters, constructors, class bodies and JSX, which run
///   when they are read, constructed or rendered, not on the continuation;
/// - it is written directly in a function literal handed to a fresh-stack host
///   scheduler (`setTimeout`, `queueMicrotask`, `Promise.then`, ...), the
///   argument [`crate::owners::fresh_stack_scheduler_edges`] gives an unowned
///   edge; a function the literal calls or stores does not count;
/// - it is written directly in a module-local function declaration whose every
///   reference is the callee of a call of the first shape (the closed census
///   ADR 0319's walk takes for a declaration).
///
/// Both premises are the dialect's [`solid_dialect::Dialect::fresh_stack_callback_owner`]
/// (`AwaitContinuations` and the scheduler edges are empty without it).
fn ownerless_dispatch_stack(
    lookup: &SemanticLookup<'_>,
    continuations: &crate::owners::AwaitContinuations<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    if after_dominating_await(continuations, file, call) {
        return true;
    }
    let Some(function) = crate::owners::containing_ast_function(&file.ast, call.span) else {
        return false;
    };
    if !crate::owners::written_directly_in_flow(&file.ast, function, call.span) {
        return false;
    }
    fresh_stack_scheduler_literal(lookup, file, function)
        || called_only_after_awaits(lookup, continuations, file, function)
}

/// Whether `function` is a literal argument a fresh-stack host scheduler runs
/// from a task or microtask queue with no owner (the owner graph's own edge).
/// A `then`, `catch` or `finally` handler counts only on a receiver proven to
/// be a native promise (ADR 0340, 0349): a subclass or a thenable typed
/// `Promise<T>` may call it on the caller's stack.
fn fresh_stack_scheduler_literal(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast
        .arguments_containing(function.span)
        .filter(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
        })
        .any(|(call, index)| {
            crate::owners::fresh_stack_scheduler_edges(file, call, lookup)
                .iter()
                .any(|edge| {
                    edge.argument == index && edge.kind == crate::owners::OwnerEdgeKind::Unowned
                })
                && (!promise_handler_call(lookup, file, call)
                    || promise_receiver(file, call).is_some_and(|receiver| {
                        crate::static_rules::native_promise_in(lookup, file, receiver, 0)
                    }))
        })
}

/// Whether `call` resolves to `Promise.prototype.then`, `catch` or `finally`.
fn promise_handler_call(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    lookup
        .resolved_callee_call(file, call.callee)
        .and_then(crate::runtime_semantics::standard_call_name)
        .is_some_and(|name| matches!(name, "Promise.then" | "Promise.catch" | "Promise.finally"))
}

/// The receiver of a member call through a static key.
fn promise_receiver(file: &FileFacts, call: &solid_facts::ast::CallFact) -> Option<Span> {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.computed_members.contains(&callee) {
        return None;
    }
    file.ast.member_at(callee).map(|member| member.object)
}

/// Whether `function` is a module-local, unwritten function declaration of a
/// program with no `eval`, every reference to which is the callee of a call
/// that follows a dominating `await` ([`crate::owners::AwaitContinuations`]).
fn called_only_after_awaits(
    lookup: &SemanticLookup<'_>,
    continuations: &crate::owners::AwaitContinuations<'_>,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    if function.kind != solid_facts::ast::FunctionKind::Declaration || function.generator {
        return false;
    }
    let Some(name) = function.name.as_ref() else {
        return false;
    };
    if !crate::execution_role::file_is_module(file)
        || crate::execution_role::flow_binding_exported(file, name.span)
        || binding_written(file, name.span)
        || lookup.files().iter().any(|file| {
            file.ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
        })
    {
        return false;
    }
    let mut references = file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
        .peekable();
    references.peek().is_some()
        && references.all(|(reference, _)| {
            file.ast.calls.iter().any(|call| {
                !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == *reference
                    && after_dominating_await(continuations, file, call)
            })
        })
}

/// Whether `call` follows a dominating `await` ([`crate::owners::AwaitContinuations`],
/// keyed by callee start, written directly in its async function's flow). A
/// callee start another call shares (`a.b(c).d(await e)`: `a.b(c)` runs before
/// the await) is ambiguous and answers no.
fn after_dominating_await(
    continuations: &crate::owners::AwaitContinuations<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    continuations.contains(file.path.as_str(), call.callee.start)
        && file
            .ast
            .calls
            .iter()
            .filter(|other| other.callee.start == call.callee.start)
            .count()
            == 1
}

/// Whether a call through a computed key that is not a literal (`target[key]()`)
/// is provably no call of a named member, so it fires nothing: the key is a
/// well-known symbol (`x[Symbol.asyncIterator]()`) or typed `number`/`symbol`
/// (no member of `lib.dom` has a numeric or symbol name), or the callee
/// resolves to a typed signature (a return type that is not `any`: an untyped
/// or `any` callee resolves to the `any` signature) that no `lib.dom`
/// declaration is a target of. Anything else may name `focus`, `click`, ...
fn computed_key_names_no_member(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    callee: Span,
) -> bool {
    let Some(key) = file
        .ast
        .members
        .iter()
        .find(|member| member.span == callee)
        .map(|member| file.ast.peel_ts_sugar_span(member.property))
    else {
        return false;
    };
    if well_known_symbol(lookup, file, key) {
        return true;
    }
    if lookup
        .entity_at(file.path.as_str(), key)
        .and_then(|entity| entity.type_descriptor.as_deref())
        .is_some_and(|descriptor| {
            matches!(
                descriptor.text.as_ref(),
                "number" | "symbol" | "unique symbol" | "bigint"
            )
        })
    {
        return true;
    }
    lookup
        .resolved_callee_call(file, call.callee)
        .is_some_and(|resolved| {
            resolved.validity == ResolvedCallValidity::Valid
                && resolved.kind == typefacts::CallKind::Call
                && resolved.declaration.is_none()
                && resolved.return_type_text.as_ref() != "any"
                // A missing target set is no evidence (a union or generic key
                // type, `k: "click" | "focus"`, resolves to no target set).
                && resolved.targets.as_ref().is_some_and(|targets| {
                    targets.exhaustive
                        && !targets
                            .candidates
                            .iter()
                            .any(|candidate| declaration_in_lib_dom(candidate.source_file.as_ref()))
                })
        })
}

/// Whether `key` is `Symbol.<name>` of the unshadowed, unwritten global: a
/// well-known symbol, which no string-named member is.
fn well_known_symbol(lookup: &SemanticLookup<'_>, file: &FileFacts, key: Span) -> bool {
    let Some(member) = file.ast.member_at(key) else {
        return false;
    };
    let object = file.ast.peel_ts_sugar_span(member.object);
    file.source_text(object) == Some("Symbol")
        && file.ast.reference_declaration(object).is_none()
        && !global_identifier_written(lookup.facts, "Symbol")
}

/// The lowercase listener type the compiler (or a spread's `assignProp`) gives
/// the attribute or key `name`: `name.slice(2).toLowerCase()`. `None` for a
/// name this does not read (non-ASCII).
pub(crate) fn listener_type(name: &str) -> Option<String> {
    (name.is_ascii() && name.len() > 2).then(|| name[2..].to_ascii_lowercase())
}

impl SemanticLookup<'_> {
    fn event_delivery(&self) -> &EventDelivery {
        self.event_delivery
            .get_or_init(|| EventDelivery::build(self))
    }

    /// ADR 0361 (amended): where the event runs of the literals the owner
    /// censuses prove event-run end; computed only while a dispatch site exists
    /// and none fires every type.
    pub(crate) fn event_terminals(&self) -> &HashMap<(String, Span), EventTerminals> {
        self.event_terminals.get_or_init(|| {
            let delivery = self.event_delivery();
            if delivery.any_site && !delivery.fired_any {
                crate::execution_role::event_terminals(self)
            } else {
                HashMap::default()
            }
        })
    }

    /// ADR 0356, 0361 (amended): whether a synchronous dispatch may run the
    /// function literal at `literal` in `path`, which the owner censuses prove
    /// event-run (a forwarded handler, a literal a handler calls), on its
    /// caller's stack: some site fires the type of an attribute its event runs
    /// end at ([`EventTerminals`]), a narrowly joined site dispatches on that
    /// attribute's element, or the census could not follow it and any site
    /// exists.
    pub(crate) fn event_literal_may_run_on_a_dispatcher_stack(
        &self,
        path: &str,
        literal: Span,
    ) -> bool {
        let delivery = self.event_delivery();
        delivery.any_site
            && delivery.reaches_terminals(self.event_terminals().get(&(path.to_owned(), literal)))
    }

    /// ADR 0361: whether a synchronous dispatch may run the handler written as
    /// `handler` in `file` (an event attribute's value, or a function a
    /// spread supplies) on its caller's stack, under its owner. True when a
    /// dispatch may fire any type, when one fires a type the handler listens
    /// for, or when a narrowly joined one delivers to it.
    pub(crate) fn event_handler_may_run_on_a_dispatcher_stack(
        &self,
        file: &FileFacts,
        handler: Span,
    ) -> bool {
        let delivery = self.event_delivery();
        delivery.any_site
            && delivery.reaches(file.path.as_str(), file.ast.peel_ts_sugar_span(handler))
    }

    /// ADR 0364: whether the event attribute whose value is `handler` may be
    /// replaced by a later attribute or spread of its element, so that the host
    /// never runs the value as the handler (`event_key_may_be_replaced`).
    pub(crate) fn event_attribute_may_be_replaced(&self, file: &FileFacts, handler: Span) -> bool {
        let handler = file.ast.peel_ts_sugar_span(handler);
        file.ast
            .jsx_containing(handler)
            .filter(|element| crate::object_members::intrinsic_element(file, element))
            .any(|element| {
                element.attributes.iter().any(|attribute| {
                    attribute.namespace.is_none()
                        && attribute
                            .expression
                            .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == handler)
                        && file.source_text(attribute.name).is_some_and(|name| {
                            self.dialect.compiler_lowers_event_attribute(name)
                                && crate::object_members::event_key_may_be_replaced(
                                    self,
                                    file,
                                    element,
                                    attribute.span,
                                    name,
                                )
                        })
                })
            })
    }

    /// Whether the host's run of the handler written as `handler` is unproven:
    /// a synchronous dispatch may run it on its caller's stack (ADR 0361), or a
    /// later attribute or spread may replace it (ADR 0364).
    pub(crate) fn event_handler_unowned_run_unproven(
        &self,
        file: &FileFacts,
        handler: Span,
    ) -> bool {
        self.event_handler_may_run_on_a_dispatcher_stack(file, handler)
            || self.event_attribute_may_be_replaced(file, handler)
    }

    /// [`Self::event_handler_may_run_on_a_dispatcher_stack`] for code at `site`
    /// that has the event role. Code written directly in the function literal
    /// of an element's own event attribute is joined by that attribute; code
    /// written directly in a literal whose every invocation is an event run
    /// (a forwarded prop, a literal a handler calls) by the attributes those
    /// runs end at ([`EventTerminals`]). Any other event role (a literal nested
    /// in a handler and called by code the census does not follow, a mixed
    /// literal, a helper) is joined to no site, because the flow that made it
    /// an event callback may be one a dispatch delivers to.
    pub(crate) fn event_role_may_run_on_a_dispatcher_stack(
        &self,
        file: &FileFacts,
        site: Span,
    ) -> bool {
        let delivery = self.event_delivery();
        if !delivery.any_site {
            return false;
        }
        if delivery.fired_any {
            return true;
        }
        let Some(innermost) = file
            .ast
            .functions_body_containing(site)
            .min_by_key(|function| function.body.end - function.body.start)
        else {
            return true;
        };
        let mut attributes = file.compiler.callback_roles.iter().filter(|callback| {
            callback.role == CallbackRoleKind::EventHandler
                && callback.span.contains(site)
                && crate::owners::callback_literal_span(file, callback.span, true)
                    == Some(innermost.span)
        });
        match attributes.next() {
            Some(callback) => delivery.reaches(
                file.path.as_str(),
                file.ast.peel_ts_sugar_span(callback.span),
            ),
            None => delivery.reaches_terminals(
                self.event_terminals()
                    .get(&(file.path.to_string(), innermost.span))
                    .filter(|terminals| terminals.event_only),
            ),
        }
    }

    /// The cross-file digest input: everything the answers above depend on.
    pub(super) fn bind_event_delivery(&self, hasher: &mut Sha256) {
        let delivery = self.event_delivery();
        hasher.update(b"synchronous-event-dispatch\0");
        hasher.update([u8::from(delivery.any_site), u8::from(delivery.fired_any)]);
        let mut fired = delivery.fired.iter().collect::<Vec<_>>();
        fired.sort_unstable();
        hasher.update(b"fired-event-types\0");
        for event_type in fired {
            hasher.update(
                u64::try_from(event_type.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(event_type.as_bytes());
        }
        let mut delivered = delivery
            .delivered
            .iter()
            .flat_map(|(path, spans)| spans.iter().map(move |span| (path.as_str(), *span)))
            .collect::<Vec<_>>();
        delivered.sort_unstable();
        for (path, span) in delivered {
            hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update(span.start.to_le_bytes());
            hasher.update(span.end.to_le_bytes());
        }
        let mut elements = delivery.delivered_elements.iter().collect::<Vec<_>>();
        elements.sort_unstable();
        hasher.update(b"delivered-elements\0");
        for (path, span) in elements {
            hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update(span.start.to_le_bytes());
            hasher.update(span.end.to_le_bytes());
        }
        // ADR 0361 (amended): a forwarded handler's answer reads the attributes
        // its event runs end at, in any file of the closed program.
        let mut terminals = self.event_terminals().iter().collect::<Vec<_>>();
        terminals.sort_unstable_by(|left, right| left.0.cmp(right.0));
        hasher.update(b"event-terminals\0");
        for ((path, span), terminal) in terminals {
            hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update(span.start.to_le_bytes());
            hasher.update(span.end.to_le_bytes());
            hasher.update([u8::from(terminal.unreadable), u8::from(terminal.event_only)]);
            for event_type in &terminal.types {
                hasher.update(
                    u64::try_from(event_type.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                hasher.update(event_type.as_bytes());
            }
            hasher.update(b"\0");
            for (element_path, element) in &terminal.elements {
                hasher.update(
                    u64::try_from(element_path.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                hasher.update(element_path.as_bytes());
                hasher.update(element.start.to_le_bytes());
                hasher.update(element.end.to_le_bytes());
            }
            hasher.update(b"\0");
        }
    }
}

/// What an inline event construction passed to `dispatchEvent` fires.
struct DispatchedEvent<'f> {
    event_type: &'f str,
    /// The event is a `click` of a `MouseEvent` (or any other `…Event` that is
    /// not exactly `Event` or `CustomEvent`): the DOM dispatch algorithm runs
    /// the target's activation behavior for it (DOM Standard,
    /// `#concept-event-dispatch`: `isActivationEvent` is "event is a
    /// `MouseEvent` object and event's `type` attribute is `click`"), which is
    /// what `click()` runs. Over-approximated to every `…Event` constructor:
    /// the class hierarchy of the interface is not read.
    activation: bool,
}

/// The type a `dispatchEvent` call fires when the event is an inline
/// construction of a `lib.dom` event with a literal type; `None` for any other
/// argument (an identifier, a helper, a computed type, a subclass), which may
/// fire any type. Bubbling and `composed` are irrelevant here: an event runs
/// only the handlers of its own type.
fn dispatched_event_type<'f>(
    lookup: &SemanticLookup<'_>,
    file: &'f FileFacts,
    call: &'f solid_facts::ast::CallFact,
) -> Option<DispatchedEvent<'f>> {
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    if argument.spread {
        return None;
    }
    let event = file
        .ast
        .call_at(file.ast.peel_ts_sugar_span(argument.span))
        .filter(|event| event.construct)?;
    if !is_dom_event_constructor(lookup, file, event, true) {
        return None;
    }
    let event_type = event_literal_type(event)?;
    let constructor = file.source_text(file.ast.peel_ts_sugar_span(event.callee))?;
    Some(DispatchedEvent {
        event_type,
        activation: event_type.eq_ignore_ascii_case("click")
            && !matches!(constructor, "Event" | "CustomEvent"),
    })
}

/// The string literal first argument of an event construction, whatever its
/// init: `new Event(T)`, `new CustomEvent(T, init)`.
fn event_literal_type(event: &solid_facts::ast::CallFact) -> Option<&str> {
    if event.arguments.iter().any(|argument| argument.spread) || event.arguments.len() > 2 {
        return None;
    }
    match &event.arguments.first()?.literal_value {
        ArgumentLiteralFact::String(event_type) => Some(event_type.as_str()),
        _ => None,
    }
}

/// Whether `name` is the spelling of a `lib.dom` event constructor: `Event`
/// itself or an interface of the `…Event` family, each taking its type first.
fn event_constructor_name(name: &str) -> bool {
    name == "Event"
        || (name.ends_with("Event")
            && name.starts_with(|c: char| c.is_ascii_uppercase())
            && name.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

/// Whether a member that is never called resolves to a `lib.dom` declaration of
/// a member that dispatches on every interface declaring it (`dispatchEvent`,
/// `click`, `focus`, ...; the owner-specific ones, `open`, `reset`, `close`, also
/// name properties and stay unread here): `Reflect.apply(EventTarget.prototype.dispatchEvent, el, [event])`,
/// `el.dispatchEvent.call(el, event)`, `const send = el.dispatchEvent`. The
/// value runs wherever it is called, so the dispatch is no site's.
fn value_reference_dispatches(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    callees: &HashSet<Span>,
) -> bool {
    file.ast.members.iter().any(|member| {
        !callees.contains(&member.span)
            && !file.ast.computed_members.contains(&member.span)
            && file
                .source_text(member.property)
                .is_some_and(crate::dispatching_dom_member_on_any_interface)
            && property_dispatches(lookup, file, member.property) == Some(true)
    }) || owned_member_invoked_as_value(lookup, file, callees)
}

/// Whether a member spelled as an owner-specific dispatching one
/// (`showModal`, `reset`, `close`, ...) is invoked as a value: the object of a
/// `.call`/`.apply`/`.bind`, or the function argument of `Reflect.apply`
/// (`d.showModal.call(d)`). These spellings also name properties and other
/// interfaces' methods, so a plain read (`d.open`) is no dispatch, but a
/// value that is called through `Function.prototype` runs the member, whichever
/// interface owns it. The declaration decides where it resolves (a project
/// member that shares the spelling does not); an unresolved receiver is read by
/// the spelling.
fn owned_member_invoked_as_value(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    callees: &HashSet<Span>,
) -> bool {
    let reflect_applied = |member: Span| {
        file.ast.calls.iter().any(|call| {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            !file.ast.computed_members.contains(&callee)
                && file.ast.member_at(callee).is_some_and(|callee| {
                    let object = file.ast.peel_ts_sugar_span(callee.object);
                    file.source_text(object) == Some("Reflect")
                        && file.source_text(callee.property) == Some("apply")
                        && file.ast.reference_declaration(object).is_none()
                        && !global_identifier_written(lookup.facts, "Reflect")
                })
                && call.arguments.first().is_some_and(|argument| {
                    !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == member
                })
        })
    };
    file.ast.members.iter().any(|member| {
        !callees.contains(&member.span)
            && !file.ast.computed_members.contains(&member.span)
            && file
                .source_text(member.property)
                .is_some_and(crate::dispatching_dom_member_on_exact_owner)
            && property_dispatches(lookup, file, member.property).unwrap_or(true)
            && (file.ast.members.iter().any(|parent| {
                file.ast.peel_ts_sugar_span(parent.object) == member.span
                    && !file.ast.computed_members.contains(&parent.span)
                    && file
                        .source_text(parent.property)
                        .is_some_and(|name| matches!(name, "call" | "apply" | "bind"))
            }) || reflect_applied(member.span))
    })
}

/// Whether the property at `property` (spelled as a dispatching member)
/// resolves to a `lib.dom` declaration (`Some(true)`), to declarations outside
/// it (`Some(false)`: a project or package member that shares the spelling), or
/// to none (`None`). The owner and the member's kind are not read: Type Facts
/// gives declarations no method kind, so the spelling decides which `lib.dom`
/// declarations count.
fn property_dispatches(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    property: Span,
) -> Option<bool> {
    let entity = lookup.entity_at(file.path.as_str(), property)?;
    if entity.symbol.is_empty() {
        return None;
    }
    let symbol = lookup.facts.typescript.symbol(&entity.symbol)?;
    let declarations = symbol.declarations();
    if declarations.is_empty() {
        return None;
    }
    Some(
        declarations
            .iter()
            .any(|declaration| declaration_in_lib_dom(declaration.location.path.as_ref())),
    )
}

fn declaration_in_lib_dom(path: &str) -> bool {
    std::path::Path::new(path)
        .file_name()
        .is_some_and(|name| name == "lib.dom.d.ts" || name == "lib.dom.iterable.d.ts")
}

/// A narrowly joined site: the element it dispatches on (none for `window` or
/// `document`) and the handlers it reaches, by file and span.
type SupportedSite = (Option<Span>, Vec<(String, Span)>);

/// The handlers one `dispatchEvent` call reaches, when that is provable: the
/// event is a non-bubbling, non-composed `new Event(T)`/`new CustomEvent(T,
/// init)` of a type the web runtime does not delegate, dispatched on a local
/// `const` that is exactly one intrinsic element, so the only listeners it runs
/// are that element's own `T` handlers, all of which are known. `None` for
/// anything else (ADR 0361): the call then fires its type program-wide. With
/// the handlers, the span of the element dispatched on (none for `window` or
/// `document`).
fn supported_site(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<SupportedSite> {
    // The path of a `window` or `document` dispatch holds no element, whatever
    // the event is: it needs no argument analysis (ADR 0361).
    if global_event_target(lookup, file, call.callee) {
        return lookup
            .dialect
            .global_dispatch_reaches_no_element_handler()
            .then(|| (None, Vec::new()));
    }
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    if argument.spread {
        return None;
    }
    let event = file
        .ast
        .call_at(file.ast.peel_ts_sugar_span(argument.span))
        .filter(|event| event.construct)?;
    if !is_dom_event_constructor(lookup, file, event, false) {
        return None;
    }
    let event_type = bubbling_free_event_type(event)?;
    if !event_type.is_ascii() || lookup.dialect.event_type_is_delegated(event_type) {
        return None;
    }
    let element = receiver_element(lookup, file, call.callee)?;
    let mut handlers = Vec::new();
    for attribute in &element.attributes {
        if attribute.namespace.is_some() {
            continue;
        }
        let name = file.source_text(attribute.name)?;
        if !lookup.dialect.compiler_lowers_event_attribute(name) {
            continue;
        }
        // The compiler names the listener `name.slice(2).toLowerCase()`.
        if !name.is_ascii() || !name[2..].eq_ignore_ascii_case(event_type) {
            continue;
        }
        if let Some(expression) = attribute.expression {
            handlers.push((
                file.path.to_string(),
                file.ast.peel_ts_sugar_span(expression),
            ));
        }
    }
    // A spread may supply a listener of any type; its exact holder names the
    // functions (ADR 0358), an unresolved one names nothing.
    for spread in &element.spreads {
        let key = crate::location(file.path.shared(), spread.argument);
        let targets = lookup.spread_handler_targets().get(&key)?.as_ref()?;
        handlers.extend(targets.iter().map(|held| (held.path.clone(), held.span)));
    }
    Some((Some(element.span), handlers))
}

/// Whether the receiver of `callee` is the `lib.dom` global `window` or
/// `document`: an unshadowed, unwritten identifier that resolves to that
/// declaration alone, never the spelling.
fn global_event_target(lookup: &SemanticLookup<'_>, file: &FileFacts, callee: Span) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    if file.ast.computed_members.contains(&callee) {
        return false;
    }
    let Some(member) = file.ast.member_at(callee) else {
        return false;
    };
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let Some(name) = file
        .source_text(receiver)
        .filter(|name| matches!(*name, "window" | "document"))
    else {
        return false;
    };
    if file.ast.reference_declaration(receiver).is_some()
        || global_identifier_written(lookup.facts, name)
    {
        return false;
    }
    let Some(symbol) = lookup
        .entity_at(file.path.as_str(), receiver)
        .filter(|entity| !entity.symbol.is_empty())
        .and_then(|entity| lookup.facts.typescript.symbol(&entity.symbol))
    else {
        return false;
    };
    let declarations = symbol.declarations();
    !declarations.is_empty()
        && declarations.iter().all(|declaration| {
            declaration.name.as_ref() == name
                && declaration_in_lib_dom(declaration.location.path.as_ref())
        })
}

/// Whether `event` constructs a `lib.dom` event global: `Event` or `CustomEvent`
/// (`any_event` false, the shape the narrow join reads the init of), or any
/// interface of the `…Event` family (`any_event`, for the type alone). Exact
/// resolution to the standard-library declaration of an unshadowed, unwritten
/// global, never the spelling alone.
fn is_dom_event_constructor(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    event: &solid_facts::ast::CallFact,
    any_event: bool,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(event.callee);
    let Some(name) = file.source_text(callee) else {
        return false;
    };
    if !(if any_event {
        event_constructor_name(name)
    } else {
        matches!(name, "Event" | "CustomEvent")
    }) || file.ast.reference_declaration(callee).is_some()
        || global_identifier_written(lookup.facts, name)
    {
        return false;
    }
    lookup
        .resolved_callee_call(file, event.callee)
        .is_some_and(|resolved| {
            resolved.validity == ResolvedCallValidity::Valid
                && resolved.kind == typefacts::CallKind::Construct
                && resolved.declaration.as_ref().is_some_and(|declaration| {
                    declaration.standard_library
                        && declaration.qualified_name.strip_suffix(".construct") == Some(name)
                        && std::path::Path::new(declaration.source_file.as_ref())
                            .file_name()
                            .is_some_and(|file_name| file_name == "lib.dom.d.ts")
                })
        })
}

/// The string-literal type of an event that cannot bubble or cross a shadow
/// boundary: written with no init, or with an object literal whose `bubbles`
/// and `composed`, when named, are the literal `false`. An init that is not a
/// static object (a spread, a computed key, a variable) is refused.
fn bubbling_free_event_type(event: &solid_facts::ast::CallFact) -> Option<&str> {
    if event.arguments.iter().any(|argument| argument.spread) {
        return None;
    }
    let (event_type, init) = match event.arguments.as_slice() {
        [event_type] => (event_type, None),
        [event_type, init] => (event_type, Some(init)),
        _ => return None,
    };
    let ArgumentLiteralFact::String(event_type) = &event_type.literal_value else {
        return None;
    };
    if let Some(init) = init {
        let ArgumentLiteralFact::Object(properties) = &init.literal_value else {
            return None;
        };
        if properties.iter().any(|property| {
            matches!(property.name.as_str(), "bubbles" | "composed")
                && property.value != ArgumentLiteralFact::Boolean(false)
        }) {
            return None;
        }
    }
    Some(event_type.as_str())
}

/// The intrinsic element a receiver names exactly: a `const` local to a
/// function, initialized to one element (through parentheses, `as`,
/// `satisfies` and `!`), and read only through members.
fn receiver_element<'f>(
    lookup: &SemanticLookup<'_>,
    file: &'f FileFacts,
    callee: Span,
) -> Option<&'f solid_facts::ast::JsxElementFact> {
    let callee = file.ast.peel_ts_sugar_span(callee);
    if file.ast.computed_members.contains(&callee) {
        return None;
    }
    let receiver = file
        .ast
        .peel_ts_sugar_span(file.ast.member_at(callee)?.object);
    let (owner, binding, _) = lookup.binding_at_reference(file.path.as_str(), receiver)?;
    if owner.path != file.path
        || !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
    {
        return None;
    }
    let [name] = binding.names.as_slice() else {
        return None;
    };
    let declaration = name.span;
    if !file
        .ast
        .functions
        .iter()
        .any(|function| function.span.contains(declaration))
        || binding_written(file, declaration)
    {
        return None;
    }
    let element = file
        .ast
        .jsx_element_at(file.ast.peel_ts_sugar_span(binding.initializer?))?;
    let tag = file.source_text(element.name.span)?;
    if !crate::object_members::intrinsic_element(file, element) || tag.contains('-') {
        return None;
    }
    // Any other use may hand the element to code that gives it listeners the
    // element's attributes do not show.
    let every_reference_a_member_read = file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, declared)| *declared == declaration)
        .all(|(reference, _)| {
            file.ast
                .members
                .iter()
                .any(|member| file.ast.peel_ts_sugar_span(member.object) == *reference)
        });
    every_reference_a_member_read.then_some(element)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::ast::{ArgumentFact, CallFact};

    fn argument(literal: ArgumentLiteralFact) -> ArgumentFact {
        ArgumentFact {
            span: Span::new(0, 0),
            spread: false,
            value: solid_facts::ast::ArgumentValueKind::Other,
            boolean_properties: vec![],
            identifier_properties: vec![],
            string_properties: vec![],
            property_names: vec![],
            exact_object_literal: false,
            closed_object_literal: false,
            runtime_value_kind: solid_facts::ast::RuntimeValueKind::Unknown,
            binding_declaration: None,
            value_span: None,
            runtime_type_escape: false,
            literal_members: vec![],
            literal_value: literal,
        }
    }

    fn construct(arguments: Vec<ArgumentFact>) -> CallFact {
        CallFact {
            span: Span::new(0, 0),
            callee: Span::new(0, 0),
            direct_callee: true,
            type_arguments: false,
            arguments,
            static_callee: true,
            owned_write_option: false,
            result_discarded: false,
            construct: true,
        }
    }

    fn text(value: &str) -> ArgumentLiteralFact {
        ArgumentLiteralFact::String(value.into())
    }

    fn init(properties: &[(&str, ArgumentLiteralFact)]) -> ArgumentLiteralFact {
        ArgumentLiteralFact::Object(
            properties
                .iter()
                .map(
                    |(name, value)| solid_facts::ast::ArgumentLiteralPropertyFact {
                        name: (*name).into(),
                        value: value.clone(),
                    },
                )
                .collect(),
        )
    }

    fn event_type(arguments: Vec<ArgumentFact>) -> Option<String> {
        bubbling_free_event_type(&construct(arguments)).map(str::to_owned)
    }

    #[test]
    fn a_plain_event_type_is_read() {
        assert_eq!(
            event_type(vec![argument(text("scroll"))]).as_deref(),
            Some("scroll")
        );
    }

    #[test]
    fn an_init_that_cannot_bubble_is_read() {
        for properties in [
            vec![],
            vec![("detail", ArgumentLiteralFact::Unknown)],
            vec![("bubbles", ArgumentLiteralFact::Boolean(false))],
            vec![
                ("bubbles", ArgumentLiteralFact::Boolean(false)),
                ("composed", ArgumentLiteralFact::Boolean(false)),
            ],
        ] {
            assert_eq!(
                event_type(vec![argument(text("x")), argument(init(&properties))]).as_deref(),
                Some("x"),
                "{properties:?}"
            );
        }
    }

    #[test]
    fn an_init_that_may_bubble_or_cross_a_boundary_is_refused() {
        for properties in [
            vec![("bubbles", ArgumentLiteralFact::Boolean(true))],
            vec![("composed", ArgumentLiteralFact::Boolean(true))],
            vec![("bubbles", ArgumentLiteralFact::Unknown)],
            vec![("composed", ArgumentLiteralFact::Function)],
        ] {
            assert_eq!(
                event_type(vec![argument(text("x")), argument(init(&properties))]),
                None,
                "{properties:?}"
            );
        }
        // A spread, a computed key, an accessor or `__proto__` makes the
        // literal fact unknown.
        assert_eq!(
            event_type(vec![
                argument(text("x")),
                argument(ArgumentLiteralFact::Unknown)
            ]),
            None
        );
    }

    #[test]
    fn only_a_literal_type_is_read() {
        assert_eq!(
            event_type(vec![argument(ArgumentLiteralFact::Unknown)]),
            None
        );
        assert_eq!(event_type(vec![]), None);
        assert_eq!(
            event_type(vec![
                argument(text("x")),
                argument(init(&[])),
                argument(init(&[]))
            ]),
            None
        );
        let mut spread = argument(text("x"));
        spread.spread = true;
        assert_eq!(event_type(vec![spread]), None);
    }

    #[test]
    fn a_dispatched_event_type_is_read_whatever_the_init() {
        // Bubbling and `composed` matter to the narrow join, not to the type.
        let bubbling = init(&[("bubbles", ArgumentLiteralFact::Boolean(true))]);
        assert_eq!(
            event_literal_type(&construct(vec![argument(text("sync")), argument(bubbling)])),
            Some("sync")
        );
        assert_eq!(
            event_literal_type(&construct(vec![argument(text("sync"))])),
            Some("sync")
        );
        for arguments in [
            vec![argument(ArgumentLiteralFact::Unknown)],
            vec![],
            vec![
                argument(text("a")),
                argument(init(&[])),
                argument(init(&[])),
            ],
        ] {
            assert_eq!(event_literal_type(&construct(arguments)), None);
        }
        let mut spread = argument(text("sync"));
        spread.spread = true;
        assert_eq!(event_literal_type(&construct(vec![spread])), None);
    }

    #[test]
    fn an_event_interface_is_spelled_event_or_ends_in_it() {
        for name in [
            "Event",
            "CustomEvent",
            "MouseEvent",
            "FocusEvent",
            "InputEvent",
            "PointerEvent",
        ] {
            assert!(event_constructor_name(name), "{name}");
        }
        for name in [
            "Request",
            "event",
            "Events",
            "MyEvent$",
            "XMLHttpRequest",
            "EventTarget",
        ] {
            assert!(!event_constructor_name(name), "{name}");
        }
    }

    #[test]
    fn a_synthetic_click_fires_the_activation_set_but_event_and_custom_event_do_not() {
        let mut delivery = EventDelivery::default();
        delivery.fire_dispatched_event(Some(DispatchedEvent {
            event_type: "click",
            activation: false,
        }));
        assert_eq!(delivery.fired.len(), 1);
        assert!(delivery.fired.contains("click"));
        delivery.fire_dispatched_event(Some(DispatchedEvent {
            event_type: "click",
            activation: true,
        }));
        for fired in [
            "click",
            "input",
            "change",
            "submit",
            "beforetoggle",
            "beforematch",
        ] {
            assert!(delivery.fired.contains(fired), "{fired}");
        }
        assert!(!delivery.fired_any);
        delivery.fire_dispatched_event(None);
        assert!(delivery.fired_any);
    }

    #[test]
    fn a_listener_type_is_the_lowercased_tail() {
        assert_eq!(listener_type("onFocusIn").as_deref(), Some("focusin"));
        assert_eq!(listener_type("onScroll").as_deref(), Some("scroll"));
        assert_eq!(listener_type("onÉvent"), None);
        assert_eq!(listener_type("on"), None);
    }

    fn delivery(fired: &[&str]) -> EventDelivery {
        let mut delivery = EventDelivery {
            any_site: true,
            ..EventDelivery::default()
        };
        delivery.fired.extend(fired.iter().map(|t| (*t).to_owned()));
        delivery
    }

    fn handler(delivery: &mut EventDelivery, start: u32, event_type: Option<&str>) {
        delivery.note_handler("a.tsx", Span::new(start, start + 1), event_type);
    }

    #[test]
    fn a_handler_is_reached_when_its_type_is_fired() {
        let mut delivery = delivery(&["focus", "blur"]);
        handler(&mut delivery, 1, Some("focus"));
        handler(&mut delivery, 2, Some("scroll"));
        handler(&mut delivery, 3, None);
        // The same function held under two keys listens for both.
        handler(&mut delivery, 4, Some("scroll"));
        handler(&mut delivery, 4, Some("blur"));
        assert!(delivery.reaches("a.tsx", Span::new(1, 2)));
        assert!(!delivery.reaches("a.tsx", Span::new(2, 3)));
        // An unreadable type may be any fired one.
        assert!(delivery.reaches("a.tsx", Span::new(3, 4)));
        assert!(delivery.reaches("a.tsx", Span::new(4, 5)));
        // A handler the index never saw is conditional while a type fires.
        assert!(delivery.reaches("a.tsx", Span::new(9, 10)));
    }

    #[test]
    fn a_site_that_fires_no_element_event_reaches_no_handler() {
        let mut delivery = delivery(&[]);
        handler(&mut delivery, 1, None);
        assert!(!delivery.reaches("a.tsx", Span::new(1, 2)));
        assert!(!delivery.reaches("a.tsx", Span::new(9, 10)));
        delivery
            .delivered
            .entry("a.tsx".into())
            .or_default()
            .insert(Span::new(9, 10));
        assert!(delivery.reaches("a.tsx", Span::new(9, 10)));
        delivery.fired_any = true;
        assert!(delivery.reaches("b.tsx", Span::new(0, 1)));
    }

    fn terminals(types: &[&str], element: Option<u32>) -> EventTerminals {
        let mut terminals = EventTerminals::default();
        terminals
            .types
            .extend(types.iter().map(|t| (*t).to_owned()));
        if let Some(start) = element {
            terminals
                .elements
                .insert(("a.tsx".into(), Span::new(start, start + 1)));
        }
        terminals
    }

    #[test]
    fn a_forwarded_handler_is_reached_by_its_terminal_types() {
        let delivery = delivery(&["focus", "blur"]);
        // Ends at an `onScroll`: no fired type is its.
        assert!(!delivery.reaches_terminals(Some(&terminals(&["scroll"], Some(1)))));
        // Ends at an `onScroll` and an `onFocus`: the union is reached.
        assert!(delivery.reaches_terminals(Some(&terminals(&["scroll", "focus"], Some(1)))));
        // No terminal at all, an unreadable one, or no census entry: reached.
        assert!(delivery.reaches_terminals(Some(&terminals(&[], Some(1)))));
        let mut unreadable = terminals(&["scroll"], Some(1));
        unreadable.unreadable = true;
        assert!(delivery.reaches_terminals(Some(&unreadable)));
        assert!(delivery.reaches_terminals(None));
    }

    #[test]
    fn a_forwarded_handler_on_a_narrowly_joined_element_is_reached() {
        // A narrow site fires no type program-wide but dispatches on element 1.
        let mut delivery = delivery(&[]);
        delivery
            .delivered_elements
            .insert(("a.tsx".into(), Span::new(1, 2)));
        assert!(delivery.reaches_terminals(Some(&terminals(&["sync"], Some(1)))));
        assert!(!delivery.reaches_terminals(Some(&terminals(&["sync"], Some(5)))));
        // Any type fired reaches every terminal.
        delivery.fired_any = true;
        assert!(delivery.reaches_terminals(Some(&terminals(&["sync"], Some(5)))));
    }

    #[test]
    fn firing_accumulates_by_union() {
        let mut delivery = EventDelivery::default();
        delivery.fire(Fires::Types(&["focus", "blur"]));
        delivery.fire(Fires::Types(&["blur", "invalid"]));
        delivery.fire(Fires::Types(&[]));
        delivery.fire_event_type(Some("Sync"));
        assert!(!delivery.fired_any);
        let mut fired = delivery.fired.iter().cloned().collect::<Vec<_>>();
        fired.sort();
        assert_eq!(fired, ["blur", "focus", "invalid", "sync"]);
        delivery.fire_event_type(None);
        assert!(delivery.fired_any);
        let mut other = EventDelivery::default();
        other.fire_event_type(Some("é"));
        assert!(other.fired_any);
        let mut other = EventDelivery::default();
        other.fire(Fires::AnyType);
        assert!(other.fired_any);
    }

    #[test]
    fn a_dispatching_member_is_known_by_its_spelling() {
        for name in [
            "dispatchEvent",
            "click",
            "focus",
            "checkValidity",
            "showModal",
            "reset",
        ] {
            assert!(crate::dispatching_dom_member_spelling(name), "{name}");
        }
        for name in ["addEventListener", "scrollTo", "value", "dispatch"] {
            assert!(!crate::dispatching_dom_member_spelling(name), "{name}");
        }
    }

    #[test]
    fn a_value_reference_reads_only_the_names_every_interface_dispatches() {
        for name in ["dispatchEvent", "click", "focus", "blur", "reportValidity"] {
            assert!(
                crate::dispatching_dom_member_on_any_interface(name),
                "{name}"
            );
        }
        // `open` and `close` also name properties and other interfaces' methods;
        // a value reference to one counts only when it is invoked through
        // `.call`/`.apply`/`.bind` or `Reflect.apply`.
        for name in ["open", "close", "reset", "send", "show", "showModal"] {
            assert!(
                !crate::dispatching_dom_member_on_any_interface(name),
                "{name}"
            );
            assert!(crate::dispatching_dom_member_on_exact_owner(name), "{name}");
        }
    }

    #[test]
    fn only_the_dom_library_declares_a_dispatching_member() {
        assert!(declaration_in_lib_dom("bundled:/libs/lib.dom.d.ts"));
        assert!(declaration_in_lib_dom("/ts/lib/lib.dom.iterable.d.ts"));
        assert!(!declaration_in_lib_dom("/project/src/store.ts"));
        assert!(!declaration_in_lib_dom(
            "/node_modules/@types/react/index.d.ts"
        ));
    }
}
