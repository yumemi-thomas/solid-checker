//! Visible synchronous DOM dispatch withholds an ownerless event proof from
//! the handlers it can reach (ADR 0356, 0361).

use super::*;
use solid_facts::ast::ArgumentLiteralFact;
use solid_facts::compiler::CallbackRoleKind;

/// What the project's synchronous DOM dispatches can reach.
#[derive(Default)]
pub(crate) struct EventDelivery {
    /// Some synchronous dispatch site exists (a call or a reference).
    any_site: bool,
    /// Some site is not [`supported_site`], or is a dispatch no site models
    /// ([`unseen_dispatch`]): any handler may be reached.
    unsupported: bool,
    /// The handlers of the supported sites, by file: the span of an event
    /// attribute's value, or of a function a spread holder supplies.
    delivered: HashMap<String, HashSet<Span>>,
}

impl EventDelivery {
    fn build(lookup: &SemanticLookup<'_>) -> Self {
        let mut delivery = Self::default();
        for file in lookup.files() {
            let mut callees = HashSet::default();
            for call in &file.ast.calls {
                callees.insert(file.ast.peel_ts_sugar_span(call.callee));
                let Some(handlers) = call_site(lookup, file, call) else {
                    continue;
                };
                delivery.any_site = true;
                let Some(handlers) = handlers else {
                    delivery.unsupported = true;
                    delivery.delivered.clear();
                    return delivery;
                };
                for (path, span) in handlers {
                    delivery.delivered.entry(path).or_default().insert(span);
                }
            }
            if value_reference_dispatches(lookup, file, &callees) {
                delivery.any_site = true;
                delivery.unsupported = true;
                delivery.delivered.clear();
                return delivery;
            }
        }
        delivery
    }

    fn reaches(&self, path: &str, span: Span) -> bool {
        self.delivered
            .get(path)
            .is_some_and(|spans| spans.contains(&span))
    }
}

impl SemanticLookup<'_> {
    fn event_delivery(&self) -> &EventDelivery {
        self.event_delivery
            .get_or_init(|| EventDelivery::build(self))
    }

    /// ADR 0356: whether any synchronous dispatch is visible. The answer for a
    /// handler no element names (a literal only a helper's flow proves an event
    /// invocation), which no site is joined to.
    pub(crate) fn synchronous_event_dispatch_present(&self) -> bool {
        self.event_delivery().any_site
    }

    /// ADR 0361: whether a synchronous dispatch may run the handler written as
    /// `handler` in `file` (an event attribute's value, or a function a
    /// spread supplies) on its caller's stack, under its owner. True when any
    /// dispatch is unsupported, or when a supported one delivers to it.
    pub(crate) fn event_handler_may_run_on_a_dispatcher_stack(
        &self,
        file: &FileFacts,
        handler: Span,
    ) -> bool {
        let delivery = self.event_delivery();
        delivery.any_site
            && (delivery.unsupported
                || delivery.reaches(file.path.as_str(), file.ast.peel_ts_sugar_span(handler)))
    }

    /// [`Self::event_handler_may_run_on_a_dispatcher_stack`] for code at `site`
    /// that has the event role. Only code written directly in the function
    /// literal of an element's own event attribute names its handler; any other
    /// event role (a forwarded prop, a helper a handler calls, a literal nested
    /// in a handler) is joined to no site, because the flow that made it an
    /// event callback may be one a dispatch delivers to.
    pub(crate) fn event_role_may_run_on_a_dispatcher_stack(
        &self,
        file: &FileFacts,
        site: Span,
    ) -> bool {
        let delivery = self.event_delivery();
        if !delivery.any_site {
            return false;
        }
        if delivery.unsupported {
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
        attributes.next().is_none_or(|callback| {
            delivery.reaches(
                file.path.as_str(),
                file.ast.peel_ts_sugar_span(callback.span),
            )
        })
    }

    /// The cross-file digest input: everything the answers above depend on.
    pub(super) fn bind_event_delivery(&self, hasher: &mut Sha256) {
        let delivery = self.event_delivery();
        hasher.update(b"synchronous-event-dispatch\0");
        hasher.update([u8::from(delivery.any_site), u8::from(delivery.unsupported)]);
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
    }
}

/// What one call is to the dispatch analysis: `None` for a call that cannot
/// dispatch, `Some(None)` for a dispatch that reaches any handler, and
/// `Some(Some(handlers))` for a supported `dispatchEvent`.
///
/// A call whose callee resolves to a declaration is judged by it (ADR 0329's
/// members dispatch; an exact project method of the same spelling does not).
/// A member call that resolves to no declaration, spelled as one of those
/// members, may be any of them on an `any` or unresolved receiver, so it is
/// the program-wide fallback (ADR 0361).
fn call_site(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<Option<Vec<(String, Span)>>> {
    let resolved = lookup.resolved_callee_call(file, call.callee);
    if let Some(resolved) = resolved.filter(|resolved| resolved.declaration.is_some()) {
        return match crate::runtime_semantics::standard_call_name(resolved) {
            Some("EventTarget.dispatchEvent") => Some(supported_site(lookup, file, call)),
            _ if crate::cleanup::dispatches_dom_event(resolved) => Some(None),
            _ => None,
        };
    }
    unseen_dispatch(lookup, file, call.callee).then_some(None)
}

/// Whether the member `callee` calls is spelled as a synchronously dispatching
/// `lib.dom` member and does not resolve to a declaration that is not one: an
/// unresolved receiver (`any`, a failed lookup) keeps the dispatch possible.
fn unseen_dispatch(lookup: &SemanticLookup<'_>, file: &FileFacts, callee: Span) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    if file.ast.computed_members.contains(&callee) {
        return file
            .ast
            .literal_computed_members
            .iter()
            .any(|member| member.span == callee && member_key_dispatches(&member.key));
    }
    let Some(member) = file.ast.member_at(callee) else {
        return false;
    };
    file.source_text(member.property)
        .is_some_and(crate::dispatching_dom_member_spelling)
        && property_dispatches(lookup, file, member.property).unwrap_or(true)
}

fn member_key_dispatches(key: &str) -> bool {
    crate::dispatching_dom_member_spelling(key)
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
    })
}

/// The handlers one `dispatchEvent` call reaches, when that is provable: the
/// event is a non-bubbling, non-composed `new Event(T)`/`new CustomEvent(T,
/// init)` of a type the web runtime does not delegate, dispatched on a local
/// `const` that is exactly one intrinsic element, so the only listeners it runs
/// are that element's own `T` handlers, all of which are known. `None` for
/// anything else (ADR 0361).
fn supported_site(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<Vec<(String, Span)>> {
    // The path of a `window` or `document` dispatch holds no element, whatever
    // the event is: it needs no argument analysis (ADR 0361).
    if global_event_target(lookup, file, call.callee) {
        return lookup
            .dialect
            .global_dispatch_reaches_no_element_handler()
            .then(Vec::new);
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
    if !is_dom_event_constructor(lookup, file, event) {
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
        handlers.extend(targets.iter().cloned());
    }
    Some(handlers)
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

/// Whether `event` constructs the `lib.dom` global `Event` or `CustomEvent`:
/// exact resolution to the standard-library declaration of an unshadowed,
/// unwritten global, never the spelling alone.
fn is_dom_event_constructor(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    event: &solid_facts::ast::CallFact,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(event.callee);
    let Some(name) = file.source_text(callee) else {
        return false;
    };
    if !matches!(name, "Event" | "CustomEvent")
        || file.ast.reference_declaration(callee).is_some()
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
            assert!(member_key_dispatches(name), "{name}");
        }
        for name in ["addEventListener", "scrollTo", "value", "dispatch"] {
            assert!(!crate::dispatching_dom_member_spelling(name), "{name}");
            assert!(!member_key_dispatches(name), "{name}");
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
        // `open` and `close` also name properties and other interfaces' methods.
        for name in ["open", "close", "reset", "send", "show"] {
            assert!(
                !crate::dispatching_dom_member_on_any_interface(name),
                "{name}"
            );
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
