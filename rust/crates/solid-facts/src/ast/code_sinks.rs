//! The places a file runs code it builds (facts schema 95, the second re-check of #232).
//!
//! Code a program builds as a string and runs is code nobody here reads: it may give `Object.prototype` an accessor that reads any key through `this`, and nothing in the
//! file's own syntax shows the read. A consumer that asks "can this program give a shared prototype an accessor that reads a member the program stores" has to treat a file
//! with such a place as one whose receivers are unreadable. This pass lists the places, from the syntax alone (never a type, never a name a binding of the file shadows):
//!
//! - `eval`, and the `Function` constructor, used as anything but an operand nothing runs (`typeof`, the right side of `instanceof`, `Function.prototype`);
//! - a call of a `.constructor` member (`(() => {}).constructor("code")`: the `Function` constructor of any function value; a `new x.constructor()` makes an instance and is none);
//! - a timer (`setTimeout`, `setInterval`, `setImmediate`, `execScript` of the global or of a global object) whose callback is not proven a function: a string runs as code;
//! - a `<script>` element made at run time (`createElement("script")`, or a tag the program does not name), whose text or source runs when it is inserted;
//! - an event attribute set as text: `setAttribute` of a name that starts `on` or that the program does not name, and a JSX attribute written `onclick`, `onerror`, … (all
//!   lower case, a native handler attribute) with a value that is no function written there;
//! - markup written as a string that is not proven inert: an assignment to `innerHTML` or `outerHTML`, `insertAdjacentHTML`, `document.write`, `createContextualFragment` and
//!   `setHTMLUnsafe` given anything but a literal that holds no script, no event attribute and no `javascript:` URL;
//! - a load the program does not name as a module path: `import(x)` of a computed specifier, and one of a `data:`, `blob:`, `javascript:` or network URL;
//! - a string literal that is a `javascript:` URL (tabs and line breaks inside it, and leading spaces and controls, ignored as a URL parser does);
//! - ADR 1133 (F5): a URL a navigation or a frame loads that is not proven to be no `javascript:` URL, and `srcdoc` markup not proven inert, by
//!   every route the syntax names: a member write (`a.href = …`, `iframe.srcdoc = …`, `location = …`), a JSX attribute, `setAttribute`, an
//!   object literal's key (a spread or `Object.assign` may set it), `location.assign`/`replace` and `open`. A value is proven when its leading
//!   text is known and already differs from `javascript:` (`"/path/" + id`, `` `https://${host}` ``), or it is a literal, a `const` bound to
//!   one of these, or `URL.createObjectURL(…)`. A class field so named is a key too (a spread copies an instance's own fields).
//!
//! A name is the global it names only when no binding of the file declares it; a callback is proven a function when it is a function or arrow written there, or a name that
//! a function declaration or a `const` bound to one declares.

use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, CallExpression, Expression, IdentifierReference,
    JSXAttribute, JSXAttributeName, JSXAttributeValue, JSXExpression, TemplateLiteral,
    VariableDeclarationKind,
};
use oxc_semantic::{NodeId, Semantic, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::UnaryOperator;

use super::extract_helpers::{literal_property_key, peel_ts_sugar};
use super::{CodeSinkFact, CodeSinkKind};
use crate::core::Span;

/// The timers that run a string as code.
const TIMERS: [&str; 4] = ["setTimeout", "setInterval", "setImmediate", "execScript"];

/// The names of the global objects: a member of one is the global of that name.
const GLOBAL_OBJECTS: [&str; 7] = [
    "globalThis",
    "window",
    "self",
    "global",
    "top",
    "parent",
    "frames",
];

/// ADR 1133 (F5): the names (attribute names and members, compared without case and after a namespace prefix such as `attr:`) whose value a
/// navigation loads as a URL: a link's or a base's `href`, a frame's `src`, a form's `action` and a button's `formAction`, the
/// window's `location`. A `javascript:` URL runs only when navigated to: `<object data>`, `<embed src>`, `<img src>` and `<script src>`
/// fetch their resource, and fetching a `javascript:` URL is a network error (Fetch, scheme fetch), so `data` is no such name.
const URL_NAMES: [&str; 5] = ["href", "src", "action", "formaction", "location"];

/// Whether `name` (an attribute, a member or a key, with any `attr:`/`prop:`/`xlink:` prefix) names a URL slot or the `srcdoc` markup slot.
fn slot_of(name: &str) -> Option<Slot> {
    let local = name.rsplit(':').next().unwrap_or(name);
    if local.eq_ignore_ascii_case("srcdoc") {
        Some(Slot::Markup)
    } else if URL_NAMES.iter().any(|url| local.eq_ignore_ascii_case(url)) {
        Some(Slot::Url)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum Slot {
    Url,
    Markup,
}

struct Sinks<'s, 'a> {
    semantic: &'s Semantic<'a>,
    resolve: bool,
    found: Vec<CodeSinkFact>,
}

/// The places a file runs code it builds, in source order (see the module's text), and whether it holds a `this` or `super` expression: code that may read a member through
/// the object a function was called on. `resolve` is whether names are followed to their declarations (false when the binder reported errors).
pub(super) fn code_sinks(semantic: &Semantic<'_>, resolve: bool) -> (Vec<CodeSinkFact>, bool) {
    let mut sinks = Sinks {
        semantic,
        resolve,
        found: Vec::new(),
    };
    let mut uses_receiver = false;
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::ThisExpression(_) | AstKind::Super(_) => uses_receiver = true,
            AstKind::IdentifierReference(identifier) => sinks.reference(node.id(), identifier),
            AstKind::CallExpression(call) => sinks.call(call),
            AstKind::AssignmentExpression(assignment) => {
                if writes_markup_property(&assignment.left)
                    && !sinks.inert_markup(&assignment.right)
                {
                    sinks.add(assignment.span, CodeSinkKind::MarkupWrite);
                }
                // ADR 1133 (F5): `a.href = …`, `iframe.srcdoc = …`, `location = …`. Only `=` and the logical assignments store the
                // right side alone; `+=` appends to a value nothing here reads.
                let name = match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member) => {
                        Some(member.property.name.to_string())
                    }
                    AssignmentTarget::ComputedMemberExpression(member) => {
                        literal_property_key(&member.expression).map(|key| key.to_string())
                    }
                    AssignmentTarget::AssignmentTargetIdentifier(identifier)
                        if identifier.name == "location"
                            && sinks.symbol_of(identifier).is_none() =>
                    {
                        Some("location".to_string())
                    }
                    _ => None,
                };
                if let Some(slot) = name.as_deref().and_then(slot_of) {
                    let stores = matches!(
                        assignment.operator,
                        oxc_syntax::operator::AssignmentOperator::Assign
                            | oxc_syntax::operator::AssignmentOperator::LogicalOr
                            | oxc_syntax::operator::AssignmentOperator::LogicalAnd
                            | oxc_syntax::operator::AssignmentOperator::LogicalNullish
                    );
                    if !stores || !sinks.slot_value_proven(slot, &assignment.right) {
                        sinks.add(assignment.span, slot.kind());
                    }
                }
            }
            AstKind::PropertyDefinition(property) => {
                if let Some(slot) = crate::kernel::property_key_value(&property.key)
                    .as_deref()
                    .and_then(slot_of)
                {
                    let proven = property
                        .value
                        .as_ref()
                        .is_none_or(|value| sinks.slot_value_proven(slot, value));
                    if !proven {
                        sinks.add(property.span, slot.kind());
                    }
                }
            }
            // ADR 1133 (F5): an object literal's key may reach an element (a JSX spread, `Object.assign(el, …)`, `spread`).
            AstKind::ObjectProperty(property) => {
                if let Some(slot) = crate::kernel::property_key_value(&property.key)
                    .as_deref()
                    .and_then(slot_of)
                {
                    let proven = property.kind == oxc_ast::ast::PropertyKind::Init
                        && !property.method
                        && sinks.slot_value_proven(slot, &property.value);
                    if !proven {
                        sinks.add(property.span, slot.kind());
                    }
                }
            }
            // ADR 1133 (F5): a navigation method read anywhere but as a call's callee (`const go = location.assign`, `window.open.call(…)`,
            // `el.setAttribute.call(el, "href", url)`) may be handed a built URL; `open` is read as a value only where its value may be
            // called or kept (`if (props.open)` reads a flag).
            AstKind::StaticMemberExpression(member)
                if !matches!(semantic.nodes().parent_kind(node.id()), AstKind::CallExpression(call) if call.callee.span() == member.span)
                    && !sinks.is_write_target(node.id(), member.span)
                    && match member.property.name.as_str() {
                        "assign" | "replace" => sinks.is_location(&member.object),
                        "setAttribute" | "setAttributeNS" => true,
                        "open" => !sinks.value_unused(node.id(), member.span),
                        _ => false,
                    } =>
            {
                sinks.add(member.span, CodeSinkKind::NavigationUrl);
            }
            // ADR 1125 (G5): a timer read as a member anywhere but as a call's callee (`const t = window.setTimeout`) may be handed a string.
            AstKind::StaticMemberExpression(member)
                if TIMERS.contains(&member.property.name.as_str())
                    && !matches!(semantic.nodes().parent_kind(node.id()), AstKind::CallExpression(call) if call.callee.span() == member.span) =>
            {
                sinks.add(member.span, CodeSinkKind::StringTimer);
            }
            // `window.eval(..)`, `top.Function(..)`: the global read as a member, by its name.
            AstKind::StaticMemberExpression(member)
                if matches!(member.property.name.as_str(), "eval" | "Function") =>
            {
                sinks.member_of_code_constructor(node.id(), member.span);
            }
            AstKind::ComputedMemberExpression(member) => {
                match literal_property_key(&member.expression).as_deref() {
                    Some("eval" | "Function") => {
                        sinks.member_of_code_constructor(node.id(), member.span);
                    }
                    // A global looked up by a name the program does not fix (`window[name]`): it may be `eval` or `Function`.
                    None if sinks.is_global_object(&member.object) => {
                        sinks.add(member.span, CodeSinkKind::GlobalLookup);
                    }
                    _ => {}
                }
            }
            AstKind::ImportExpression(import) => {
                if !static_module_path(&import.source) {
                    sinks.add(import.span, CodeSinkKind::DynamicImport);
                }
            }
            AstKind::JSXAttribute(attribute) => sinks.jsx_attribute(attribute),
            AstKind::StringLiteral(literal) if is_javascript_url(literal.value.as_str()) => {
                sinks.add(literal.span, CodeSinkKind::JavascriptUrl);
            }
            _ => {}
        }
    }
    sinks.found.sort_by_key(|sink| (sink.at, sink.kind as u8));
    sinks.found.dedup();
    (sinks.found, uses_receiver)
}

impl<'a> Sinks<'_, 'a> {
    fn add(&mut self, at: oxc_span::Span, kind: CodeSinkKind) {
        self.found.push(CodeSinkFact {
            at: Span::new(at.start, at.end),
            kind,
        });
    }

    fn symbol_of(&self, identifier: &IdentifierReference<'_>) -> Option<SymbolId> {
        identifier
            .reference_id
            .get()
            .and_then(|reference| self.semantic.scoping().get_reference(reference).symbol_id())
    }

    /// Whether `object` is a `const` binding of an object literal that defines `name` itself (`const timers = { setTimeout() {} }`): the
    /// member called is that literal's own, never a window's.
    fn own_literal_member(&self, object: &'a Expression<'a>, name: &str) -> bool {
        if !self.resolve {
            return false;
        }
        let Expression::Identifier(identifier) = peel_ts_sugar(object) else {
            return false;
        };
        let Some(symbol) = self.symbol_of(identifier) else {
            return false;
        };
        let nodes = self.semantic.nodes();
        let declaration = self.semantic.scoping().symbol_declaration(symbol);
        let declarator = match nodes.kind(declaration) {
            AstKind::VariableDeclarator(declarator) => declarator,
            _ => match nodes.parent_kind(declaration) {
                AstKind::VariableDeclarator(declarator) => declarator,
                _ => return false,
            },
        };
        declarator.kind == VariableDeclarationKind::Const
            && declarator.id.is_binding_identifier()
            && matches!(
                declarator.init.as_ref().map(peel_ts_sugar),
                Some(Expression::ObjectExpression(literal)) if literal.properties.iter().any(|property| matches!(
                    property,
                    oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property)
                        if !property.computed && property.key.static_name().is_some_and(|key| key == name)
                ))
            )
    }

    /// Whether the expression is a global object (`window`, `globalThis`, …) that no binding of the file declares.
    fn is_global_object(&self, expression: &'a Expression<'a>) -> bool {
        matches!(
            peel_ts_sugar(expression),
            Expression::Identifier(identifier)
                if GLOBAL_OBJECTS.contains(&identifier.name.as_str()) && self.symbol_of(identifier).is_none()
        )
    }

    /// `eval` and `Function`, by the reference to the global.
    fn reference(&mut self, id: NodeId, identifier: &'a IdentifierReference<'a>) {
        // ADR 1133 (F5): the global `location` handed on (`const loc = location; loc.assign(url)`) and the global `open` read as a value.
        if matches!(identifier.name.as_str(), "location" | "open")
            && (!self.resolve || self.symbol_of(identifier).is_none())
            && !identifier
                .reference_id
                .get()
                .is_some_and(|reference| self.semantic.scoping().get_reference(reference).is_type())
        {
            let in_sight = if identifier.name == "open" {
                matches!(self.semantic.nodes().parent_kind(id), AstKind::CallExpression(call) if call.callee.span() == identifier.span)
                    || self.value_unused(id, identifier.span)
            } else {
                self.location_in_sight(id, identifier.span)
            };
            if !in_sight {
                self.add(identifier.span, CodeSinkKind::NavigationUrl);
            }
        }
        // ADR 1125 (G5): a global timer named anywhere but as the callee of a call (`const later = setTimeout`,
        // `setTimeout.call(null, "code")`, `Reflect.apply(setTimeout, …)`) may be handed a string.
        if TIMERS.contains(&identifier.name.as_str())
            && (!self.resolve || self.symbol_of(identifier).is_none())
            // A type position (`ReturnType<typeof setTimeout>`) and `typeof setTimeout` run nothing.
            && !identifier
                .reference_id
                .get()
                .is_some_and(|reference| self.semantic.scoping().get_reference(reference).is_type())
            && !matches!(
                self.semantic.nodes().parent_kind(id),
                AstKind::CallExpression(call) if call.callee.span() == identifier.span
            )
            && !matches!(
                self.semantic.nodes().parent_kind(id),
                AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof
            )
            && !matches!(self.semantic.nodes().parent_kind(id), AstKind::TSTypeQuery(_))
        {
            self.add(identifier.span, CodeSinkKind::StringTimer);
        }
        let kind = match identifier.name.as_str() {
            "eval" => CodeSinkKind::Eval,
            "Function" => CodeSinkKind::FunctionConstructor,
            _ => return,
        };
        if self.symbol_of(identifier).is_some() {
            return;
        }
        let reference = identifier
            .reference_id
            .get()
            .map(|reference| self.semantic.scoping().get_reference(reference));
        if reference.is_none_or(|reference| reference.is_type()) {
            return;
        }
        let nodes = self.semantic.nodes();
        let span = identifier.span;
        let parent = nodes.parent_kind(id);
        let inert = match parent {
            // `typeof eval`, `x instanceof Function`: nothing runs.
            AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Typeof,
            AstKind::BinaryExpression(binary) => {
                binary.operator == oxc_syntax::operator::BinaryOperator::Instanceof
                    && binary.right.span() == span
            }
            // `Function.prototype`: the object, not the constructor.
            AstKind::StaticMemberExpression(member) => {
                member.object.span() == span && member.property.name == "prototype"
            }
            _ => false,
        };
        if !inert {
            self.add(span, kind);
        }
    }

    fn call(&mut self, call: &'a CallExpression<'a>) {
        let callee = peel_ts_sugar(&call.callee);
        // The name the callee calls, and the object it is a member of (a global object or any other).
        let (name, object): (Option<String>, Option<&'a Expression<'a>>) = match callee {
            Expression::Identifier(identifier) if self.symbol_of(identifier).is_none() => {
                (Some(identifier.name.to_string()), None)
            }
            Expression::StaticMemberExpression(member) => {
                (Some(member.property.name.to_string()), Some(&member.object))
            }
            Expression::ComputedMemberExpression(member) => (
                literal_property_key(&member.expression).map(|key| key.to_string()),
                Some(&member.object),
            ),
            _ => (None, None),
        };
        let Some(name) = name else {
            return;
        };
        match name.as_str() {
            // A timer given anything but a proven function runs a string. ADR 1125 (G5): on any receiver, not only a global object
            // spelled (`document.defaultView?.setTimeout("…")`): a member so named may be the window's.
            "setTimeout" | "setInterval" | "setImmediate" | "execScript"
                if !object.is_some_and(|object| self.own_literal_member(object, &name)) =>
            {
                let proven = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .is_some_and(|callback| self.proven_function(callback));
                if !proven {
                    self.add(call.span, CodeSinkKind::StringTimer);
                }
            }
            // `(() => {}).constructor("code")`: the `Function` constructor of a function value.
            "constructor" if object.is_some() => self.add(call.span, CodeSinkKind::ConstructorCall),
            "createElement" if object.is_some() => {
                if !self.argument_is_benign_text(call, 0, |tag| !tag.eq_ignore_ascii_case("script"))
                {
                    self.add(call.span, CodeSinkKind::ScriptElement);
                }
            }
            "createElementNS" if object.is_some() => {
                if !self.argument_is_benign_text(call, 1, |tag| !tag.eq_ignore_ascii_case("script"))
                {
                    self.add(call.span, CodeSinkKind::ScriptElement);
                }
            }
            "setAttribute" | "setAttributeNS" | "setAttributeNode" if object.is_some() => {
                let at = if name == "setAttributeNS" { 1 } else { 0 };
                // An attribute set as text is code when it is an event handler; a name the program does not name may be one.
                if !self.argument_is_benign_text(call, at, |attribute| {
                    !attribute
                        .get(..2)
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
                }) {
                    self.add(call.span, CodeSinkKind::EventAttribute);
                }
                // ADR 1133 (F5): `setAttribute("href", url)`, `setAttribute("srcdoc", markup)`.
                if let Some(slot) = call
                    .arguments
                    .get(at)
                    .and_then(Argument::as_expression)
                    .and_then(literal_text)
                    .as_deref()
                    .and_then(slot_of)
                    && !call
                        .arguments
                        .get(at + 1)
                        .and_then(Argument::as_expression)
                        .is_some_and(|value| self.slot_value_proven(slot, value))
                {
                    self.add(call.span, slot.kind());
                }
            }
            // ADR 1133 (F5): `location.assign(url)`, `location.replace(url)`: a navigation.
            "assign" | "replace" if object.is_some_and(|object| self.is_location(object)) => {
                self.navigation_argument(call, 0);
            }
            // `open(url)`, `window.open(url)`, on any receiver (a window may have any name); a method's first argument proven
            // no `javascript:` URL (`xhr.open("GET", …)`) is none.
            "open" => self.navigation_argument(call, 0),
            "insertAdjacentHTML" if object.is_some() => {
                if !call
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .is_some_and(|markup| self.inert_markup(markup))
                {
                    self.add(call.span, CodeSinkKind::MarkupWrite);
                }
            }
            "write" | "writeln" | "createContextualFragment" | "setHTMLUnsafe"
                if object.is_some() =>
            {
                let inert = !call.arguments.is_empty()
                    && call.arguments.iter().all(|argument| {
                        argument
                            .as_expression()
                            .is_some_and(|markup| self.inert_markup(markup))
                    });
                if !inert {
                    self.add(call.span, CodeSinkKind::MarkupWrite);
                }
            }
            _ => {}
        }
    }

    /// Whether argument `index` is a literal text that satisfies `benign` (the tag is not `script`, the attribute is no event handler); false for any other argument.
    fn argument_is_benign_text(
        &self,
        call: &'a CallExpression<'a>,
        index: usize,
        benign: impl Fn(&str) -> bool,
    ) -> bool {
        call.arguments
            .get(index)
            .and_then(Argument::as_expression)
            .and_then(literal_text)
            .is_some_and(|text| benign(&text))
    }

    /// A read of the member `eval` or `Function` of anything: the global of that name where the object is a window, and a function a lookup gives out anywhere else. No
    /// sink where nothing runs it (`typeof window.eval`, the right side of `instanceof`, `x.Function.prototype`) or where it is written.
    fn member_of_code_constructor(&mut self, id: NodeId, span: oxc_span::Span) {
        let nodes = self.semantic.nodes();
        let inert = match nodes.parent_kind(id) {
            AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Typeof,
            AstKind::BinaryExpression(binary) => {
                binary.operator == oxc_syntax::operator::BinaryOperator::Instanceof
                    && binary.right.span() == span
            }
            AstKind::StaticMemberExpression(parent) => {
                parent.object.span() == span && parent.property.name == "prototype"
            }
            // The target of a write (`x.eval = f`) reads nothing out.
            AstKind::AssignmentExpression(assignment) => assignment.left.span() == span,
            _ => false,
        };
        if !inert {
            self.add(span, CodeSinkKind::Eval);
        }
    }

    /// A JSX attribute written as a native event handler (`onclick`, `onerror`, all lower case) whose value is no function written there.
    fn jsx_attribute(&mut self, attribute: &'a JSXAttribute<'a>) {
        // ADR 1133 (F5): `<a href={…}>`, `<iframe srcdoc={…}>`, `attr:href`, `xlink:href`, on any element (a component may forward it).
        let full = match &attribute.name {
            JSXAttributeName::Identifier(name) => name.name.to_string(),
            JSXAttributeName::NamespacedName(name) => {
                format!("{}:{}", name.namespace.name, name.name.name)
            }
        };
        if let Some(slot) = slot_of(&full) {
            let proven = match &attribute.value {
                None => true,
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    slot_text_proven(slot, literal.value.as_str())
                }
                Some(JSXAttributeValue::ExpressionContainer(container)) => container
                    .expression
                    .as_expression()
                    .is_some_and(|value| self.slot_value_proven(slot, value)),
                Some(_) => false,
            };
            if !proven {
                self.add(attribute.span, slot.kind());
            }
        }
        let JSXAttributeName::Identifier(name) = &attribute.name else {
            return;
        };
        let name = name.name.as_str();
        let native_handler = name.len() > 2
            && name.starts_with("on")
            && name.bytes().all(|byte| byte.is_ascii_lowercase());
        if !native_handler {
            return;
        }
        let function = match &attribute.value {
            Some(JSXAttributeValue::ExpressionContainer(container)) => {
                match &container.expression {
                    JSXExpression::EmptyExpression(_) => true,
                    expression => expression
                        .as_expression()
                        .is_some_and(|expression| self.proven_function(expression)),
                }
            }
            _ => false,
        };
        if !function {
            self.add(attribute.span, CodeSinkKind::EventAttribute);
        }
    }

    /// Whether the expression is a function written there, or a name a function declaration or a `const` bound to a function declares.
    fn proven_function(&self, expression: &'a Expression<'a>) -> bool {
        match peel_ts_sugar(expression) {
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
            Expression::Identifier(identifier) if self.resolve => {
                let Some(symbol) = self.symbol_of(identifier) else {
                    return false;
                };
                let scoping = self.semantic.scoping();
                if !scoping.symbol_redeclarations(symbol).is_empty()
                    || scoping
                        .get_resolved_references(symbol)
                        .any(oxc_semantic::Reference::is_write)
                {
                    return false;
                }
                let nodes = self.semantic.nodes();
                match nodes.kind(scoping.symbol_declaration(symbol)) {
                    AstKind::Function(function) => function.is_declaration(),
                    AstKind::VariableDeclarator(declarator) => {
                        matches!(&declarator.id, BindingPattern::BindingIdentifier(_))
                            && matches!(
                                nodes.parent_kind(scoping.symbol_declaration(symbol)),
                                AstKind::VariableDeclaration(declaration)
                                    if declaration.kind == VariableDeclarationKind::Const
                            )
                            && declarator.init.as_ref().is_some_and(|init| {
                                matches!(
                                    peel_ts_sugar(init),
                                    Expression::ArrowFunctionExpression(_)
                                        | Expression::FunctionExpression(_)
                                )
                            })
                    }
                    // ADR 1125: the first or second parameter of an executor written as the first argument of `new Promise(..)`, the
                    // global, is the resolving function the constructor hands it (a replaced global `Promise` is a delivery hazard).
                    AstKind::FormalParameter(parameter) => {
                        parameter.initializer.is_none()
                            && matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
                            && self.promise_executor_parameter(
                                scoping.symbol_declaration(symbol),
                                parameter.span,
                            )
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Whether the parameter at `id` (span `at`) is the first or second of a function written as the first argument of `new Promise(..)`, where
    /// `Promise` is the global (no binding of the program's).
    fn promise_executor_parameter(&self, id: NodeId, at: oxc_span::Span) -> bool {
        let nodes = self.semantic.nodes();
        let parameters = nodes.parent_id(id);
        let function = nodes.parent_id(parameters);
        let (span, params) = match nodes.kind(function) {
            AstKind::ArrowFunctionExpression(arrow) => (arrow.span, &arrow.params),
            AstKind::Function(function) if !function.is_declaration() => {
                (function.span, &function.params)
            }
            _ => return false,
        };
        if !params.items.iter().take(2).any(|item| item.span == at) {
            return false;
        }
        nodes.ancestor_kinds(function).take(4).any(|kind| {
            matches!(kind, AstKind::NewExpression(new)
                if new.arguments.first().is_some_and(|first| first.span() == span)
                    && matches!(peel_ts_sugar(&new.callee), Expression::Identifier(callee)
                        if callee.name == "Promise" && self.symbol_of(callee).is_none()))
        })
    }

    /// Whether the expression is a literal whose markup holds no script, event attribute or `javascript:` URL.
    fn inert_markup(&self, expression: &'a Expression<'a>) -> bool {
        literal_text(expression).is_some_and(|text| markup_is_inert(&text))
    }

    /// ADR 1133 (F5): whether the value stored in a slot runs no code: a URL proven no `javascript:` URL, or markup proven inert.
    fn slot_value_proven(&self, slot: Slot, value: &'a Expression<'a>) -> bool {
        match slot {
            Slot::Url => self.url_proven(value, 0),
            Slot::Markup => self.inert_markup(value),
        }
    }

    /// Whether a URL value is proven to be no `javascript:` URL: its leading text is known and differs from `javascript:` (as a URL parser
    /// reads it), a non-text literal, a branch of which every arm is proven, a `const` bound to one of these, or `URL.createObjectURL(…)`
    /// (a `blob:` URL).
    fn url_proven(&self, value: &'a Expression<'a>, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match peel_ts_sugar(value) {
            Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_) => true,
            Expression::Identifier(identifier)
                if identifier.name == "undefined" && self.symbol_of(identifier).is_none() =>
            {
                true
            }
            Expression::ConditionalExpression(conditional) => {
                self.url_proven(&conditional.consequent, depth + 1)
                    && self.url_proven(&conditional.alternate, depth + 1)
            }
            Expression::LogicalExpression(logical) => {
                self.url_proven(&logical.left, depth + 1)
                    && self.url_proven(&logical.right, depth + 1)
            }
            Expression::Identifier(identifier) => self
                .const_init(identifier)
                .is_some_and(|init| self.url_proven(init, depth + 1)),
            Expression::CallExpression(call) => matches!(
                peel_ts_sugar(&call.callee),
                Expression::StaticMemberExpression(member)
                    if member.property.name == "createObjectURL"
                        && matches!(peel_ts_sugar(&member.object), Expression::Identifier(url)
                            if url.name == "URL" && self.symbol_of(url).is_none())
            ),
            _ => self.leading_text(value, depth).is_some_and(|text| {
                if self.whole_text(value) {
                    !is_javascript_url(&text)
                } else {
                    url_text_differs(&text)
                }
            }),
        }
    }

    /// The text a string value is known to start with: a literal's whole text, a template's first part, the left operand's of a `+`.
    fn leading_text(&self, value: &'a Expression<'a>, depth: usize) -> Option<String> {
        if depth > 8 {
            return None;
        }
        match peel_ts_sugar(value) {
            Expression::StringLiteral(literal) if !literal.lone_surrogates => {
                Some(literal.value.to_string())
            }
            Expression::TemplateLiteral(template) => template
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_ref())
                .map(|text| text.to_string()),
            Expression::BinaryExpression(binary)
                if binary.operator == oxc_syntax::operator::BinaryOperator::Addition =>
            {
                // Text known in full on the left runs on into the right: `"java" + "script:"`.
                let left = self.leading_text(&binary.left, depth + 1)?;
                if self.whole_text(&binary.left) {
                    let right = self
                        .leading_text(&binary.right, depth + 1)
                        .unwrap_or_default();
                    Some(left + &right)
                } else {
                    Some(left)
                }
            }
            Expression::Identifier(identifier) => self
                .const_init(identifier)
                .and_then(|init| self.leading_text(init, depth + 1)),
            _ => None,
        }
    }

    /// Whether the value's text is known in full (a literal, a template with no substitution, a `+` of such texts).
    fn whole_text(&self, value: &'a Expression<'a>) -> bool {
        match peel_ts_sugar(value) {
            Expression::StringLiteral(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            Expression::BinaryExpression(binary)
                if binary.operator == oxc_syntax::operator::BinaryOperator::Addition =>
            {
                self.whole_text(&binary.left) && self.whole_text(&binary.right)
            }
            _ => false,
        }
    }

    /// The initializer of a `const` binding identifier that nothing redeclares.
    fn const_init(&self, identifier: &IdentifierReference<'_>) -> Option<&'a Expression<'a>> {
        if !self.resolve {
            return None;
        }
        let symbol = self.symbol_of(identifier)?;
        let scoping = self.semantic.scoping();
        if !scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let nodes = self.semantic.nodes();
        let declaration = scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = nodes.kind(declaration) else {
            return None;
        };
        if declarator.kind != VariableDeclarationKind::Const
            || !declarator.id.is_binding_identifier()
        {
            return None;
        }
        declarator.init.as_ref()
    }

    /// Whether the expression is the window's `location`: the global, or a member so named of anything.
    fn is_location(&self, object: &'a Expression<'a>) -> bool {
        match peel_ts_sugar(object) {
            Expression::Identifier(identifier) => {
                identifier.name == "location" && self.symbol_of(identifier).is_none()
            }
            Expression::StaticMemberExpression(member) => member.property.name == "location",
            Expression::ComputedMemberExpression(member) => {
                literal_property_key(&member.expression).is_some_and(|key| key == "location")
            }
            _ => false,
        }
    }

    /// A navigation to argument `index` of a call: a sink unless the URL is proven no `javascript:` URL (no argument navigates nowhere).
    fn navigation_argument(&mut self, call: &'a CallExpression<'a>, index: usize) {
        let proven = match call.arguments.get(index) {
            None => true,
            Some(argument) => argument
                .as_expression()
                .is_some_and(|value| self.url_proven(value, 0)),
        };
        if !proven {
            self.add(call.span, CodeSinkKind::NavigationUrl);
        }
    }

    /// Whether the global `location` at `span` is used where nothing else can call its `assign` or `replace`: the object of a member, a
    /// destructuring's source (a detached method throws), the target of a write (the write's own test reads the value), `typeof`, an
    /// operand of an operator or a template (they read it as text).
    fn location_in_sight(&self, id: NodeId, span: oxc_span::Span) -> bool {
        match self.semantic.nodes().parent_kind(id) {
            AstKind::StaticMemberExpression(member) => member.object.span() == span,
            AstKind::ComputedMemberExpression(member) => member.object.span() == span,
            AstKind::VariableDeclarator(declarator) => {
                matches!(declarator.id, BindingPattern::ObjectPattern(_))
            }
            AstKind::AssignmentExpression(assignment) => assignment.left.span() == span,
            AstKind::UnaryExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::TemplateLiteral(_) => true,
            _ => false,
        }
    }

    /// Whether the value at `span` is only tested or compared: a condition, `!`, `typeof`, a comparison, a template, or the left of `&&`/`||`
    /// in a condition is read as a flag or as text, never called or kept.
    fn value_unused(&self, id: NodeId, span: oxc_span::Span) -> bool {
        let nodes = self.semantic.nodes();
        match nodes.parent_kind(id) {
            AstKind::IfStatement(statement) => statement.test.span() == span,
            AstKind::WhileStatement(statement) => statement.test.span() == span,
            AstKind::ConditionalExpression(conditional) => conditional.test.span() == span,
            AstKind::UnaryExpression(unary) => matches!(
                unary.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
            ),
            AstKind::BinaryExpression(binary) => {
                binary.operator.is_equality() || binary.operator.is_compare()
            }
            AstKind::TemplateLiteral(_) => true,
            // `String(x.open)`, `Boolean(x.open)`, `Number(x.open)`: the global converts the value, it calls no function it is given.
            AstKind::CallExpression(call) => {
                call.arguments.len() == 1
                    && call.arguments[0].span() == span
                    && matches!(peel_ts_sugar(&call.callee), Expression::Identifier(callee)
                        if matches!(callee.name.as_str(), "String" | "Boolean" | "Number") && self.symbol_of(callee).is_none())
            }
            _ => false,
        }
    }

    /// Whether the member at `span` is written (`x.open = …`): the write stores a value, it reads none.
    fn is_write_target(&self, id: NodeId, span: oxc_span::Span) -> bool {
        matches!(self.semantic.nodes().parent_kind(id), AstKind::AssignmentExpression(assignment) if assignment.left.span() == span)
    }
}

/// The text a string literal or a template with no substitution is, under TypeScript's wrappers.
fn literal_text(expression: &Expression<'_>) -> Option<String> {
    match peel_ts_sugar(expression) {
        Expression::StringLiteral(literal) if !literal.lone_surrogates => {
            Some(literal.value.to_string())
        }
        Expression::TemplateLiteral(template) => template_text(template),
        _ => None,
    }
}

fn template_text(template: &TemplateLiteral<'_>) -> Option<String> {
    if !template.expressions.is_empty() {
        return None;
    }
    template
        .quasis
        .first()
        .and_then(|quasi| quasi.value.cooked.as_ref())
        .map(|text| text.to_string())
}

/// Whether the target of an assignment is the member `innerHTML` or `outerHTML`.
fn writes_markup_property(target: &AssignmentTarget<'_>) -> bool {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            matches!(member.property.name.as_str(), "innerHTML" | "outerHTML")
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            literal_property_key(&member.expression)
                .is_some_and(|key| matches!(key.as_str(), "innerHTML" | "outerHTML"))
        }
        _ => false,
    }
}

/// Whether a load names a module path the program fixes: a string literal (or a template with no substitution) that is no URL of code made at run time.
fn static_module_path(source: &Expression<'_>) -> bool {
    literal_text(source).is_some_and(|specifier| {
        let lower = specifier.trim_start().to_ascii_lowercase();
        !["data:", "blob:", "javascript:", "http:", "https:"]
            .iter()
            .any(|scheme| lower.starts_with(scheme))
    })
}

/// The text a URL parser reads: leading spaces and C0 controls dropped, and every tab and line break removed.
fn url_text(text: &str) -> String {
    text.trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

fn is_javascript_url(text: &str) -> bool {
    url_text(text)
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
}

/// ADR 1133 (F5): whether a URL's known leading text already differs from `javascript:` (so no text after it can make one).
fn url_text_differs(leading: &str) -> bool {
    let text = url_text(leading);
    let scheme = b"javascript:";
    // Every known byte agrees with the scheme (or none is known): a whole `javascript:` prefix runs, a shorter one may be finished by
    // what follows.
    text.bytes()
        .zip(scheme.iter())
        .any(|(byte, expected)| !byte.eq_ignore_ascii_case(expected))
}

/// Whether a slot's literal text runs no code.
fn slot_text_proven(slot: Slot, text: &str) -> bool {
    match slot {
        Slot::Url => !is_javascript_url(text),
        Slot::Markup => markup_is_inert(text),
    }
}

impl Slot {
    fn kind(self) -> CodeSinkKind {
        match self {
            Slot::Url => CodeSinkKind::NavigationUrl,
            Slot::Markup => CodeSinkKind::MarkupWrite,
        }
    }
}

/// Whether markup written as a string holds no script, no event attribute and no `javascript:` URL. Text with no tag in it is inert.
fn markup_is_inert(markup: &str) -> bool {
    if !markup.contains('<') {
        return true;
    }
    let lower = markup.to_ascii_lowercase();
    if [
        "<script",
        "javascript:",
        "<iframe",
        "<object",
        "<embed",
        "srcdoc",
        "<base",
        "<meta",
        "<link",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return false;
    }
    // An attribute named `on…`: `on`, letters, optional space, `=`, after a space, a quote or a slash.
    let bytes = lower.as_bytes();
    for at in 0..bytes.len().saturating_sub(2) {
        if &bytes[at..at + 2] != b"on" {
            continue;
        }
        if at == 0
            || !matches!(
                bytes[at - 1],
                b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'"' | b'\''
            )
        {
            continue;
        }
        let mut end = at + 2;
        while end < bytes.len() && bytes[end].is_ascii_lowercase() {
            end += 1;
        }
        while end < bytes.len() && bytes[end].is_ascii_whitespace() {
            end += 1;
        }
        if end > at + 2 && bytes.get(end) == Some(&b'=') {
            return false;
        }
    }
    true
}
