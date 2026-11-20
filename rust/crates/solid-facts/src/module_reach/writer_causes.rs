//! Writer-cause census (measurement only, ADR 1131's M0): why a file may write a built-in's prototype under any name, in classes finer than the verdict needs.
//!
//! Nothing here is read by a verdict. A class is a string the scan records only where [`crate::package_reach::writer_causes_wanted`] holds (the census or the what-if
//! switch is on); with both off the functions below never run, so the scan's answer and its cost are what they were. The classes name what the syntax at the use shows, so
//! that the mechanism that would decide a cause (follow the global object through a binding, read the `source` of a RegExp, parse a literal function body) can be counted
//! before it is built:
//!
//! - `escape:<head>/<shape>[@<receiver>]`: the global object handed on. `<head>` is the name it was reached through (`globalThis`, `window`, `self`, `top`, `parent`,
//!   `frames`, `global`, `this`, `document`, or the member of a route: `source`, `view`, `defaultView`). `<shape>` is what the parent of the value does with it, see
//!   [`use_shape`]. `<receiver>` is, for a route, what the member was read off (`regexp`, `literal`, `param`, `this`, `member:<name>`, `call`, `binding`, `other`).
//! - `generated:<kind>`: code built at run time, see [`generated_kind`].
//! - `prototype-handed-on/<shape>`, `prototype-key`, `prototype-pattern`: a built-in's prototype handed on whole, written under a key nothing decides, or bound by a pattern
//!   nothing reads closely.

use oxc_ast::AstKind;
use oxc_ast::ast::{Argument, BindingPattern, Expression, VariableDeclarationKind};
use oxc_semantic::{AstNodes, NodeId, Scoping};
use oxc_span::{GetSpan, Span};

use super::{
    constant_declarator, declarator_of, key_name, literal_specifier, names_global, only_inspected,
    parent_past_wrappers,
};

/// How many bindings deep an alias of the global object is followed to its uses (`const a = globalThis; const b = a; b.x`).
const ALIAS_DEPTH: u8 = 3;

/// What the parent of the value at `at` does with it, past the operands that pass it on (`||`, `??`, `&&`, `?:`, a comma):
///
/// - `pattern`: the initializer of an object pattern of static keys and no rest (`const { customElements } = globalThis`): a read of those names;
/// - `pattern-open`: of any other pattern (a rest element, a computed key, a nested pattern, an array pattern);
/// - `alias`: the initializer of a binding nothing reassigns, exports or declares again, every use of which reads a member, a pattern or a computed key, or is judged
///   inert (`typeof`, a comparison); `alias-open`: any other binding;
/// - `member:<name>`: the object of a static member read (`(view.defaultView || window).innerHeight`, `(a || self).document`);
/// - `computed-read`: the object of a computed member read by a key the syntax does not fix;
/// - `argument:<callee>`, `new:<callee>`, `callee`, `spread`: an argument of a call or a construction (named by the callee as it is spelled: `fill`, `Object.assign`,
///   `RegExp`), the function called, an element of an argument spread;
/// - `return`, `assign` (the right of an assignment), `store` (a property value, an array element, a class field), `inspected` (a value only looked at), `coerced` (a
///   template substitution, a `+`, a relational or bitwise operand), `other`.
///
/// A `branch:` prefix says the value reached the parent through one of the pass-on operands.
pub(super) fn use_shape(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    at: NodeId,
    script: bool,
) -> String {
    shape_at(scoping, nodes, at, script, ALIAS_DEPTH)
}

fn shape_at(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    at: NodeId,
    script: bool,
    depth: u8,
) -> String {
    let mut at = at;
    let mut branch = false;
    for _ in 0..6 {
        let (child, parent) = parent_past_wrappers(nodes, at);
        let span = nodes.get_node(child).kind().span();
        let passes = match nodes.get_node(parent).kind() {
            // The left of `&&` is only looked at (an object is truthy, the value is the right operand).
            AstKind::LogicalExpression(logical) => {
                !(logical.operator == oxc_syntax::operator::LogicalOperator::And
                    && logical.left.span() == span)
            }
            AstKind::ConditionalExpression(conditional) => conditional.test.span() != span,
            AstKind::SequenceExpression(sequence) => sequence
                .expressions
                .last()
                .is_some_and(|last| last.span() == span),
            _ => false,
        };
        if passes {
            at = parent;
            branch = true;
            continue;
        }
        let base = consumer(scoping, nodes, parent, span, script, depth);
        return if branch {
            format!("branch:{base}")
        } else {
            base
        };
    }
    "other".to_owned()
}

fn consumer(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    parent: NodeId,
    span: Span,
    script: bool,
    depth: u8,
) -> String {
    if only_inspected(nodes, span, parent) {
        return "inspected".to_owned();
    }
    match nodes.get_node(parent).kind() {
        AstKind::VariableDeclarator(declarator)
            if declarator.init.as_ref().map(GetSpan::span) == Some(span) =>
        {
            match &declarator.id {
                BindingPattern::BindingIdentifier(binding) => {
                    alias_shape(scoping, nodes, binding, parent, script, depth)
                }
                BindingPattern::ObjectPattern(pattern)
                    if pattern.rest.is_none()
                        && pattern.properties.iter().all(|property| {
                            !property.computed
                                && key_name(&property.key).is_some()
                                && matches!(
                                    &property.value,
                                    BindingPattern::BindingIdentifier(_)
                                        | BindingPattern::AssignmentPattern(_)
                                )
                                && match &property.value {
                                    BindingPattern::AssignmentPattern(with_default) => matches!(
                                        &with_default.left,
                                        BindingPattern::BindingIdentifier(_)
                                    ),
                                    _ => true,
                                }
                        }) =>
                {
                    "pattern".to_owned()
                }
                _ => "pattern-open".to_owned(),
            }
        }
        AstKind::AssignmentExpression(assignment) if assignment.right.span() == span => {
            "assign".to_owned()
        }
        AstKind::CallExpression(call) => {
            if is_at(&call.callee, span) {
                "callee".to_owned()
            } else {
                format!("argument:{}", callee_name(&call.callee))
            }
        }
        AstKind::NewExpression(new) => {
            if is_at(&new.callee, span) {
                "callee".to_owned()
            } else {
                format!("new:{}", callee_name(&new.callee))
            }
        }
        AstKind::StaticMemberExpression(member) if is_at(&member.object, span) => {
            format!("member:{}", member.property.name)
        }
        AstKind::ComputedMemberExpression(member) if is_at(&member.object, span) => {
            "computed-read".to_owned()
        }
        AstKind::ReturnStatement(_) => "return".to_owned(),
        AstKind::ExpressionStatement(_) => {
            // The body of an arrow function written as an expression returns its value.
            let body = nodes.parent_id(parent);
            if matches!(nodes.get_node(body).kind(), AstKind::FunctionBody(_))
                && matches!(
                    nodes.parent_kind(body),
                    AstKind::ArrowFunctionExpression(arrow) if arrow.expression
                )
            {
                "return".to_owned()
            } else {
                "inspected".to_owned()
            }
        }
        AstKind::ObjectProperty(_)
        | AstKind::ArrayExpression(_)
        | AstKind::PropertyDefinition(_) => "store".to_owned(),
        AstKind::SpreadElement(_) => "spread".to_owned(),
        AstKind::TemplateLiteral(_)
        | AstKind::BinaryExpression(_)
        | AstKind::UnaryExpression(_) => "coerced".to_owned(),
        _ => "other".to_owned(),
    }
}

/// Whether `expression` is the node at `span`: the child [`parent_past_wrappers`] names is the outermost wrapper, which is the span of the field the parent holds.
fn is_at(expression: &Expression<'_>, span: Span) -> bool {
    expression.span() == span || expression.get_inner_expression().span() == span
}

/// `alias` where the binding the value initializes is followable to its uses and each is one the scan judges (a member, a pattern, a computed key, an inert use);
/// `alias-open` otherwise (see [`use_shape`]).
fn alias_shape(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    binding: &oxc_ast::ast::BindingIdentifier<'_>,
    declarator: NodeId,
    script: bool,
    depth: u8,
) -> String {
    let open = || "alias-open".to_owned();
    let Some(symbol) = binding.symbol_id.get() else {
        return open();
    };
    let declaration = nodes.parent_id(declarator);
    let exported = matches!(
        nodes.parent_kind(declaration),
        AstKind::ExportNamedDeclaration(_)
    );
    let script_global = script
        && scoping.symbol_scope_id(symbol) == scoping.root_scope_id()
        && matches!(nodes.get_node(declaration).kind(), AstKind::VariableDeclaration(held) if held.kind == VariableDeclarationKind::Var);
    if depth == 0
        || exported
        || script_global
        || scoping.symbol_is_mutated(symbol)
        || !scoping.symbol_redeclarations(symbol).is_empty()
    {
        return open();
    }
    let followed = scoping.get_resolved_references(symbol).all(|reference| {
        let shape = shape_at(scoping, nodes, reference.node_id(), script, depth - 1);
        let shape = shape.strip_prefix("branch:").unwrap_or(&shape);
        // `valueOf` hands the global object back (the scan's own rule for a member of it).
        (shape.starts_with("member:") && shape != "member:valueOf")
            || matches!(shape, "pattern" | "alias" | "computed-read" | "inspected")
    });
    if followed { "alias".to_owned() } else { open() }
}

/// The callee of a call as it is spelled, for a class: an identifier, `object.member` of two names, or `?`.
fn callee_name(callee: &Expression<'_>) -> String {
    match callee.get_inner_expression() {
        Expression::Identifier(identifier) => identifier.name.to_string(),
        Expression::StaticMemberExpression(member) => match member.object.get_inner_expression() {
            Expression::Identifier(object) => format!("{}.{}", object.name, member.property.name),
            Expression::ThisExpression(_) => format!("this.{}", member.property.name),
            _ => format!(".{}", member.property.name),
        },
        Expression::ComputedMemberExpression(_) => "[]".to_owned(),
        Expression::CallExpression(_) => "()".to_owned(),
        _ => "?".to_owned(),
    }
}

/// What a route's member (`x.source`, `x.view`, `x.defaultView`) was read off, for a class: `regexp` (a binding nothing reassigns whose initializer is a RegExp literal or
/// `new RegExp(…)`), `literal` (one whose initializer is an object or array literal), `param` (a function's parameter), `binding` (any other binding), `this`,
/// `member:<name>`, `call`, `other`.
pub(super) fn route_receiver(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    object: &Expression<'_>,
) -> String {
    match object.get_inner_expression() {
        Expression::Identifier(identifier) => {
            if let Some((declarator, _)) = constant_declarator(scoping, nodes, identifier)
                && let Some(init) =
                    declarator_of(nodes, declarator).and_then(|declarator| declarator.init.as_ref())
            {
                return match init.get_inner_expression() {
                    Expression::RegExpLiteral(_) => "regexp".to_owned(),
                    Expression::NewExpression(new)
                        if matches!(new.callee.get_inner_expression(), Expression::Identifier(callee)
                            if names_global(scoping, callee, "RegExp")) =>
                    {
                        "regexp".to_owned()
                    }
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
                        "literal".to_owned()
                    }
                    _ => "binding".to_owned(),
                };
            }
            let parameter = identifier
                .reference_id
                .get()
                .and_then(|id| scoping.get_reference(id).symbol_id())
                .is_some_and(|symbol| {
                    matches!(
                        nodes.kind(scoping.symbol_declaration(symbol)),
                        AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_)
                    )
                });
            if parameter { "param" } else { "binding" }.to_owned()
        }
        Expression::ThisExpression(_) => "this".to_owned(),
        Expression::StaticMemberExpression(member) => format!("member:{}", member.property.name),
        Expression::CallExpression(_) => "call".to_owned(),
        _ => "other".to_owned(),
    }
}

/// What kind of generated code a use of `eval`, a timer, `Function` or a `.constructor` is, where `at` is the node whose value is that function (the reference, or the
/// member of the global object that reads it): `eval`, `eval-literal` (called with a string literal), `timer-string`, `function-literal:<body>` (called or constructed with
/// string literals only; `<body>` is `empty`, `this` for `return this`, `const` for a returned literal, `other`), `function-dynamic` (called with anything else),
/// `function-member:<name>` (a member of `Function` read: `Function.call.bind(Object.prototype.hasOwnProperty)`), `function-value` (handed on), `constructor-chain`
/// (`x.constructor.constructor`), `constructor-literal:<body>`, `constructor-other`.
pub(super) fn generated_kind(nodes: &AstNodes<'_>, name: &str, at: NodeId) -> String {
    let (child, parent) = parent_past_wrappers(nodes, at);
    let span = nodes.get_node(child).kind().span();
    match name {
        "eval" => match nodes.get_node(parent).kind() {
            AstKind::CallExpression(call)
                if is_at(&call.callee, span)
                    && call
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .is_some_and(|first| literal_specifier(first).is_some()) =>
            {
                "eval-literal".to_owned()
            }
            _ => "eval".to_owned(),
        },
        "Function" => match nodes.get_node(parent).kind() {
            AstKind::CallExpression(call) if is_at(&call.callee, span) => {
                function_call_kind(&call.arguments)
            }
            AstKind::NewExpression(new) if is_at(&new.callee, span) => {
                function_call_kind(&new.arguments)
            }
            AstKind::StaticMemberExpression(member) if is_at(&member.object, span) => {
                format!("function-member:{}", member.property.name)
            }
            _ => "function-value".to_owned(),
        },
        _ => "timer-string".to_owned(),
    }
}

/// The kind of a `Function(…)` or `new Function(…)` by its arguments.
fn function_call_kind(arguments: &[Argument<'_>]) -> String {
    let literals = arguments
        .iter()
        .map(|argument| argument.as_expression().and_then(literal_specifier))
        .collect::<Vec<_>>();
    if literals.iter().any(Option::is_none) {
        return "function-dynamic".to_owned();
    }
    let body = literals.last().cloned().flatten().unwrap_or_default();
    format!("function-literal:{}", body_class(&body))
}

/// The class of a literal function body: `empty`, `this` (`return this`), `const` (`return` of a literal), `other`.
fn body_class(body: &str) -> &'static str {
    let trimmed = body.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return "empty";
    }
    let Some(returned) = trimmed.strip_prefix("return") else {
        return "other";
    };
    let returned = returned.trim();
    if returned == "this" {
        "this"
    } else if returned.is_empty()
        || matches!(returned, "true" | "false" | "null" | "undefined")
        || returned.parse::<f64>().is_ok()
        || (returned.len() >= 2
            && matches!(returned.as_bytes()[0], b'"' | b'\'')
            && returned.as_bytes()[0] == returned.as_bytes()[returned.len() - 1])
    {
        "const"
    } else {
        "other"
    }
}

/// The kind of a `.constructor` call or chain that generates code (see [`generated_kind`]): `constructor-chain` for `x.constructor.constructor`, else
/// `constructor-literal:<body>` for a call or construction with a string literal first argument, else `constructor-other`.
pub(super) fn constructor_kind(
    nodes: &AstNodes<'_>,
    id: NodeId,
    member: &oxc_ast::ast::StaticMemberExpression<'_>,
) -> String {
    if let Expression::StaticMemberExpression(inner) = member.object.get_inner_expression()
        && inner.property.name == "constructor"
    {
        return "constructor-chain".to_owned();
    }
    let (_, parent) = parent_past_wrappers(nodes, id);
    let arguments = match nodes.get_node(parent).kind() {
        AstKind::CallExpression(call) => Some(&call.arguments),
        AstKind::NewExpression(new) => Some(&new.arguments),
        _ => None,
    };
    match arguments.and_then(|arguments| {
        arguments
            .last()
            .and_then(Argument::as_expression)
            .and_then(literal_specifier)
    }) {
        Some(body) => format!("constructor-literal:{}", body_class(&body)),
        None => "constructor-other".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::scan_module;

    /// The classes a file's scan records for why it may write a built-in's prototype under any name.
    fn causes(source: &str) -> Vec<String> {
        let scan = scan_module(Path::new("/p/node_modules/x/index.js"), source);
        scan.global_writes
            .prototype_any_causes
            .keys()
            .cloned()
            .collect()
    }

    fn has(source: &str, class: &str) {
        let found = causes(source);
        assert!(
            found.iter().any(|found| found == class),
            "{source}: {found:?}"
        );
    }

    #[test]
    fn the_global_object_bound_by_a_pattern_or_followed_through_a_binding() {
        has(
            "export const { customElements } = globalThis;",
            "escape:globalThis/pattern",
        );
        has(
            "export const { a, ...rest } = globalThis;",
            "escape:globalThis/pattern-open",
        );
        has(
            "const g = globalThis; export const x = g.foo, y = g.bar;",
            "escape:globalThis/alias",
        );
        // A binding that is handed on, reassigned or exported is not followed.
        has(
            "const g = globalThis; export const x = h(g);",
            "escape:globalThis/alias-open",
        );
        has(
            "let g = globalThis; g = null; export const x = g.foo;",
            "escape:globalThis/alias-open",
        );
        has(
            "export const g = globalThis;",
            "escape:globalThis/alias-open",
        );
    }

    #[test]
    fn the_global_object_handed_to_a_call_a_return_or_a_store() {
        has("fill(globalThis);", "escape:globalThis/argument:fill");
        has(
            "Object.keys(window); export const f = () => window;",
            "escape:window/return",
        );
        has("new RegExp(self);", "escape:self/new:RegExp");
        has("export const o = { w: window };", "escape:window/store");
        has("export const v = window[k];", "escape:window/computed-read");
    }

    #[test]
    fn a_fallback_is_followed_to_the_member_read_off_it() {
        has(
            "export const h = (v) => (v.ownerDocument.defaultView || window).innerHeight;",
            "escape:window/branch:member:innerHeight",
        );
        has(
            "export const h = (v) => (v.ownerDocument.defaultView || window).innerHeight;",
            "escape:defaultView/branch:member:innerHeight@member:ownerDocument",
        );
    }

    #[test]
    fn a_route_names_what_its_member_was_read_off() {
        has(
            "const lineBreak = /\\r\\n?|\\n/; export const s = new RegExp(lineBreak.source, 'g');",
            "escape:source/new:RegExp@regexp",
        );
        has(
            "export const f = (node) => other(node.source);",
            "escape:source/argument:other@param",
        );
        has(
            "export const f = (e) => { const { view } = e; return other(view); };",
            "escape:view/argument:other@pattern",
        );
    }

    #[test]
    fn generated_code_is_named_by_what_it_is_given() {
        has("eval('1 + 1');", "generated:eval-literal");
        has("export const f = (s) => eval(s);", "generated:eval");
        has("setTimeout('a()', 0);", "generated:timer-string");
        has(
            "export const g = new Function('return this')();",
            "generated:function-literal:this",
        );
        has(
            "export const e = new Function('');",
            "generated:function-literal:empty",
        );
        has(
            "export const e = new Function('a', 'return a + 1');",
            "generated:function-literal:other",
        );
        has(
            "export const f = (b) => new Function('x', b);",
            "generated:function-dynamic",
        );
        has(
            "export const call = Function.call.bind(Object.prototype.hasOwnProperty);",
            "generated:function-member:call",
        );
        has(
            "export const g = (x) => x.constructor.constructor('return this')();",
            "generated:constructor-chain",
        );
        has("export const w = (x) => window.eval(x);", "generated:eval");
    }

    #[test]
    fn a_prototype_handed_on_is_named_by_where_it_goes() {
        has(
            "const p = Array.prototype; fill(p);",
            "prototype-handed-on/argument:fill",
        );
        has(
            "export const p = () => Array.prototype;",
            "prototype-handed-on/return",
        );
        has("let { prototype: p } = Array; p = {};", "prototype-pattern");
    }

    #[test]
    fn a_prototype_member_written_under_a_key_nothing_decides() {
        has(
            "export const f = (k, v) => { Array.prototype[k] = v; };",
            "prototype-key",
        );
    }

    #[test]
    fn a_file_with_no_cause_records_none() {
        assert!(causes("export const a = [1].map((x) => x);").is_empty());
        assert!(causes("export const n = typeof window === 'undefined';").is_empty());
        assert!(causes("export const x = window.innerWidth;").is_empty());
    }
}
