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
//! - a string literal that is a `javascript:` URL.
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
        let global_call = object.is_none_or(|object| self.is_global_object(object));
        match name.as_str() {
            // A timer given anything but a proven function runs a string.
            "setTimeout" | "setInterval" | "setImmediate" | "execScript" if global_call => {
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
            }
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
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Whether the expression is a literal whose markup holds no script, event attribute or `javascript:` URL.
    fn inert_markup(&self, expression: &'a Expression<'a>) -> bool {
        literal_text(expression).is_some_and(|text| markup_is_inert(&text))
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

fn is_javascript_url(text: &str) -> bool {
    text.trim_start()
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
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
