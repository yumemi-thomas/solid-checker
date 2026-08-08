//! `v1/prefer-for`, `v1/prefer-show`, `v1/no-react-deps`, `v1/no-proxy-apis` —
//! four of eslint-plugin-solid's structural-preference rules, each judging a
//! JavaScript-legal but Solid-unidiomatic shape.
//!
//! `prefer-for` and `prefer-show` are intent judgements, not correctness
//! checks: Solid's compiler already handles a plain `.map()` or `&&`/ternary
//! correctly, so these rules exist only to steer style toward the
//! primitives that make list and branch identity explicit. Upstream keeps
//! them conservative for exactly that reason — a wrong "prefer" nag is worse
//! than a missed one — and this port carries upstream's own gates: both
//! rules fire only when the judged expression is itself rendered as JSX
//! children, and `prefer-show` additionally only for the "expensive" branch
//! shapes upstream defines (a JSX element/fragment, or a bare identifier).
//!
//! `no-react-deps` and `no-proxy-apis` resolve their callees through the
//! dialect's own primitive table (`context.lookup.primitives`) instead of
//! matching the callee's source spelling. Upstream's ESLint rules track
//! import aliases by hand (`trackImports`); asking the same resolution the
//! rest of this checker already asks means `import { createEffect as fx }
//! from "solid-js"; fx(fn, [a])` is caught too, while a same-named function
//! from an unrelated module is not.

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, IdentifierRole, LogicalOperatorKind};
use solid_facts::core::Span;

use super::{
    UpstreamCompatContext, binding_initializer, contains, deletion_with_leading_comma,
    is_import_reference, text,
};
use crate::{Fix, StaticViolation, TextEdit, known_primitive, location};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    no_react_deps(file, context, violations);
    no_proxy_apis(file, context, violations);
    prefer_for(file, context, violations);
    prefer_show(file, violations);
}

// ---------------------------------------------------------------------
// SC8010 v1/no-react-deps
// ---------------------------------------------------------------------

/// `createEffect`/`createMemo` do not take a dependency array; Solid finds
/// their dependencies automatically by tracking what the tracked callback
/// reads. A second argument shaped like one is a habit carried over from
/// React and does nothing in Solid except get silently ignored — or, for
/// `createMemo`, get mistaken for the equality comparator it actually is.
///
/// Requires exactly two arguments, matching upstream: a third argument is
/// Solid's own options parameter (`{ equals, name }`), not a dependency
/// array, so a deliberate three-argument call is left alone rather than
/// flagged for looking React-shaped by coincidence.
fn no_react_deps(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        if call.arguments.len() != 2
            || !matches!(
                known_primitive(&primitives.calls[index]),
                Some(Primitive::CreateEffect | Primitive::CreateMemo)
            )
        {
            continue;
        }
        let argument = &call.arguments[1];
        let source = text(file, argument.span).trim();
        let looks_like_deps = source.starts_with('[')
            || binding_initializer(context, file, argument.span)
                .is_some_and(|(_, _, initializer, _)| initializer.trim_start().starts_with('['));
        if !looks_like_deps {
            continue;
        }
        violations.push(StaticViolation {
            id: "SC8010".into(),
            rule: "no-react-deps".into(),
            message: "Solid's reactive primitives do not use a dependency array.".into(),
            hint: "Solid tracks dependencies automatically by reading them; if you really need to override what is tracked, use `on`.".into(),
            location: location(file.path.shared(), argument.span),
            analysis_context: String::new(),
            fixes: vec![Fix {
                message: "Remove the dependency array.".into(),
                applicability: "safe".into(),
                edits: vec![TextEdit {
                    // The deletion swallows the `,` separating the two
                    // arguments too — removing only the argument's own span
                    // would leave `createEffect(fn, )` behind.
                    location: location(
                        file.path.shared(),
                        deletion_with_leading_comma(&file.source, argument.span),
                    ),
                    new_text: String::new(),
                }],
            }],
        });
    }
}

// ---------------------------------------------------------------------
// SC8009 v1/no-proxy-apis
// ---------------------------------------------------------------------

fn no_proxy_apis(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    no_proxy_imports(file, violations);
    no_proxy_calls(file, context, violations);
}

fn proxy_violation(file: &FileFacts, message: &str, hint: &str, span: Span) -> StaticViolation {
    StaticViolation {
        id: "SC8009".into(),
        rule: "no-proxy-apis".into(),
        message: message.into(),
        hint: hint.into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes: vec![],
    }
}

/// `solid-js/store` builds its stores on ES2015 `Proxy`, unavailable on the
/// resource-constrained or legacy engines this rule exists for.
fn no_proxy_imports(file: &FileFacts, violations: &mut Vec<StaticViolation>) {
    for import in &file.ast.imports {
        if import.module.as_str() == "solid-js/store" {
            violations.push(proxy_violation(
                file,
                "The store package relies on JavaScript Proxies.",
                "Proxies are unavailable on engines without ES2015 Proxy support.",
                import.span,
            ));
        }
    }
}

/// The remaining Proxy sources: constructing one directly, passing a
/// non-object-literal to `mergeProps` (which falls back to a Proxy for
/// anything it cannot merge eagerly), and spreading a call or member
/// expression into JSX (Solid cannot tell how many props that produces
/// without evaluating it, so it wraps the result in a Proxy too).
fn no_proxy_calls(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        // `Proxy` is a JS builtin, not a dialect primitive, so there is no
        // resolution table to ask; the source spelling is all there is.
        let callee = call.static_callee(&file.source).unwrap_or_default();
        let call_source = text(file, call.span).trim_start();
        if callee == "Proxy.revocable" || (callee == "Proxy" && call_source.starts_with("new ")) {
            violations.push(proxy_violation(
                file,
                "The Proxy API is unavailable in resource-constrained environments.",
                "Proxies are unavailable on engines without ES2015 Proxy support.",
                call.span,
            ));
        }
        if known_primitive(&primitives.calls[index]) == Some(Primitive::MergeProps) {
            // Upstream judges every argument, not just the first: a spread,
            // a function (written inline or resolved through a binding), or
            // an identifier that neither resolves to an object literal nor
            // names props (`/[pP]rops/`) each makes `mergeProps` fall back
            // to a Proxy. Object literals, calls, and member reads pass.
            for argument in &call.arguments {
                let spread = file
                    .ast
                    .spreads
                    .iter()
                    .any(|spread| spread.span == argument.span);
                let reported = spread
                    || match argument.value {
                        ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction => true,
                        ArgumentValueKind::Identifier => {
                            let name = text(file, argument.span).trim();
                            match binding_initializer(context, file, argument.span) {
                                // Resolved to a function: a Proxy source no
                                // matter what the binding is called.
                                Some((binding_file, initializer_span, _, _))
                                    if binding_file
                                        .ast
                                        .functions
                                        .iter()
                                        .any(|function| function.span == initializer_span) =>
                                {
                                    true
                                }
                                // Resolved to anything else written in this
                                // file — an object literal, a call, a member
                                // read — passes, as upstream's trace does.
                                Some(_) => false,
                                // An imported binding passes: upstream's
                                // trace lands on the import specifier, which
                                // is not an identifier, so it never reaches
                                // the props-name test.
                                None if is_import_reference(context, file, argument.span) => false,
                                // Unresolvable: upstream reports unless the
                                // name reads as props.
                                None => !name.contains("props") && !name.contains("Props"),
                            }
                        }
                        _ => false,
                    };
                if reported {
                    violations.push(proxy_violation(
                        file,
                        "Passing anything but an object literal or a props object to mergeProps may require a Proxy.",
                        "If you pass a function to `mergeProps`, it will create a Proxy, which are incompatible with your target environment.",
                        argument.span,
                    ));
                }
            }
        }
    }
    for element in &file.ast.jsx_elements {
        for spread in &element.spreads {
            // Upstream's selectors are `JSXSpreadAttribute MemberExpression`
            // and `JSXSpreadAttribute CallExpression`: every descendant
            // counts, not just the argument's own node. `{...foo.bar}`,
            // `{...{ a: foo.bar }}`, and `{...make({ a: b() })}` are all
            // Proxy sources, and a call whose callee is a member read
            // reports both nodes, exactly as two selector matches do.
            for member in file
                .ast
                .members
                .iter()
                .filter(|member| contains(spread.argument, member.span))
            {
                violations.push(proxy_violation(
                    file,
                    "Spreading a member expression may require a Proxy.",
                    "Using a property access in JSX spread makes Solid use Proxies, which are incompatible with your target environment.",
                    member.span,
                ));
            }
            for call in file
                .ast
                .calls
                .iter()
                .filter(|call| contains(spread.argument, call.span))
            {
                violations.push(proxy_violation(
                    file,
                    "Spreading a call expression may require a Proxy.",
                    "Using a function call in JSX spread makes Solid use Proxies, which are incompatible with your target environment.",
                    call.span,
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------
// SC8014 v1/prefer-for
// ---------------------------------------------------------------------

/// `Array#map` rendered as JSX children recreates every DOM node on each
/// update; `<For>` keys elements by array identity instead so unchanged
/// items keep their nodes. Position carries the whole judgement, exactly as
/// upstream's `JSXExpressionContainer` parent checks do: the `.map()` call
/// must itself be the expression a JSX element or fragment renders. The
/// callback does not have to build JSX — `<ol>{items.map(x => x.name)}</ol>`
/// still renders a list — and a `.map()` anywhere else (assigned to a
/// variable, inside an attribute) is not rendered as a list and is left
/// alone.
fn prefer_for(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for call in &file.ast.calls {
        if direct_container_position(file, call.span) != Some(JsxExpressionPosition::Child) {
            continue;
        }
        let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.span == call.callee)
        else {
            continue;
        };
        if text(file, member.property) != "map" || call.arguments.len() != 1 {
            continue;
        }
        let argument = &call.arguments[0];
        if !matches!(
            argument.value,
            ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
        ) {
            continue;
        }
        // Upstream's autofix applies only when the callback takes exactly
        // one non-rest parameter (`(item) => ...`); an index parameter, no
        // parameter, or a rest parameter leaves too many candidate
        // rewrites (`<For>` with its own index callback, or `<Index>`) to
        // pick between. `FunctionFact::parameters` already excludes rest
        // parameters, so a rest-only callback also reads as zero
        // parameters here, which correctly falls through to "no fix".
        //
        // The rewrite additionally requires the receiver to be a *proven*
        // array. `.map` is matched by name, as upstream matches it, and the
        // report is safe on a name alone — but `<For each>` iterates arrays,
        // so rewriting an Immutable.js collection or any other `.map`-bearing
        // type would change behaviour. TypeScript's descriptor for the
        // receiver is that proof; without it the report stands and the fix
        // is withheld.
        let receiver_is_array = super::expression_descriptor(context, file, member.object)
            .is_some_and(|descriptor| {
                super::array_like_type(
                    descriptor.text.as_ref(),
                    super::expression_callability(context, file, member.object),
                )
            });
        let one_parameter = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == argument.span)
            .is_some_and(|function| function.parameters.len() == 1);
        let (message, fixes) = if one_parameter {
            let fixes = if receiver_is_array {
                vec![Fix {
                    message: "Replace Array#map with <For>.".into(),
                    applicability: "safe".into(),
                    edits: vec![TextEdit {
                        location: location(file.path.shared(), call.span),
                        new_text: format!(
                            "<For each={{{}}}>{{{}}}</For>",
                            text(file, member.object),
                            text(file, argument.span)
                        ),
                    }],
                }]
            } else {
                vec![]
            };
            (
                "Use Solid's `<For />` component for efficiently rendering lists. Array#map causes DOM elements to be recreated.",
                fixes,
            )
        } else {
            (
                "Use Solid's `<For />` component or `<Index />` component for rendering lists. Array#map causes DOM elements to be recreated.",
                vec![],
            )
        };
        violations.push(StaticViolation {
            id: "SC8014".into(),
            rule: "prefer-for".into(),
            message: message.into(),
            hint: "Pick `<For>` when the callback needs the item value reactively, `<Index>` when it needs the index reactively.".into(),
            location: location(file.path.shared(), call.span),
            analysis_context: String::new(),
            fixes,
        });
    }
}

// ---------------------------------------------------------------------
// SC8015 v1/prefer-show
// ---------------------------------------------------------------------

/// Upstream's `EXPENSIVE_TYPES` gate: a JSX element/fragment, or a bare
/// identifier. A call, a literal, or any other expression shape is left
/// alone — flagging every `cond ? 1 : 2` would be noise, not help, since
/// there is no DOM node identity at stake in those branches.
///
/// Classified from the fact tables, not the branch's first character: a call
/// (`compute()`) and a member read (`user.name`) both *start* with an
/// identifier character, but neither is an `Identifier` node, and upstream
/// leaves both alone. Only a span that is itself a recorded JSX element,
/// JSX fragment, or identifier reference counts.
fn expensive_branch(file: &FileFacts, span: Span) -> bool {
    file.ast
        .jsx_elements
        .iter()
        .any(|element| element.span == span)
        || file.ast.jsx_fragments.contains(&span)
        || file.ast.identifiers.iter().any(|identifier| {
            identifier.span == span && identifier.role == IdentifierRole::Reference
        })
}

/// Where a conditional sits relative to JSX, if it is the immediate
/// expression of a JSX expression container at all.
///
/// Upstream's `prefer-show` matches `JSXExpressionContainer > Logical/
/// ConditionalExpression` — the conditional must *be* the rendered
/// expression, not merely appear somewhere under a JSX span. Containment
/// alone would catch `onClick={() => ready && submit()}` and rewrite the
/// body of an event handler into markup.
#[derive(Clone, Copy, Eq, PartialEq)]
enum JsxExpressionPosition {
    /// A `{...}` child of an element or fragment; a `<Show>` element is a
    /// drop-in replacement here, so the rule fires and the fix applies.
    Child,
    /// An attribute value container. Upstream requires the container's
    /// parent to be a JSX element or fragment, and an attribute container's
    /// parent is the attribute — so the rules consulting this position stay
    /// silent here, exactly as upstream does.
    Attribute,
}

fn jsx_expression_position(file: &FileFacts, span: Span) -> Option<JsxExpressionPosition> {
    if let Some(position) = direct_container_position(file, span) {
        return Some(position);
    }
    // Upstream also fires one level down: when the conditional is the
    // expression body of an arrow that is itself the container's expression —
    // the render-callback shape, `<For>{(item) => item.cond && <span/>}</For>`.
    // The body must *be* the arrow's whole result (parentheses and whitespace
    // aside); a statement inside a block body never qualifies.
    file.ast
        .functions
        .iter()
        .find(|function| {
            function.body.contains(span) && wraps_only_parens(file, function.body, span)
        })
        .and_then(|function| direct_container_position(file, function.span))
}

fn direct_container_position(file: &FileFacts, span: Span) -> Option<JsxExpressionPosition> {
    // The attribute answer must be settled across every element before any
    // child-brace check runs: an attribute container's braces sit inside an
    // element that is itself another element's child span, so an enclosing
    // element visited first would otherwise claim the span as its own child
    // (`<div><button icon={cond ? <A/> : <B/>} /></div>` — the braces the
    // div's child wraps are the button's attribute container).
    if file.ast.jsx_elements.iter().any(|element| {
        element
            .attributes
            .iter()
            .any(|attribute| attribute.expression == Some(span))
    }) {
        return Some(JsxExpressionPosition::Attribute);
    }
    for element in &file.ast.jsx_elements {
        if element
            .children
            .iter()
            .any(|child| container_wraps(file, *child, span))
        {
            return Some(JsxExpressionPosition::Child);
        }
    }
    // Fragments record no child spans, so prove the container from the
    // source instead: the nearest non-whitespace neighbours are the
    // container's own braces, and no function written inside the fragment
    // owns the span (which would make those braces a callback's block or
    // object body, not an expression container).
    file.ast
        .jsx_fragments
        .iter()
        .any(|fragment| {
            fragment.contains(span)
                && !file.ast.functions.iter().any(|function| {
                    fragment.contains(function.span) && function.body.contains(span)
                })
                && container_wraps(file, *fragment, span)
        })
        .then_some(JsxExpressionPosition::Child)
}

/// Whether everything between `outer`'s bounds and `span`, on both sides, is
/// parentheses and whitespace — the test for "this span is the whole
/// expression body of the arrow", tolerant of the wrapping parens JSX
/// formatting conventions add.
fn wraps_only_parens(file: &FileFacts, outer: Span, span: Span) -> bool {
    if !outer.contains(span) {
        return false;
    }
    let before = text(file, Span::new(outer.start, span.start));
    let after = text(file, Span::new(span.end, outer.end));
    before.chars().all(|c| c == '(' || c.is_whitespace())
        && after.chars().all(|c| c == ')' || c.is_whitespace())
}

/// Whether `span`'s nearest non-whitespace neighbours inside `outer` are a
/// `{ ... }` pair. For an element child span this proves the child *is* the
/// expression container holding `span`; for a fragment (whose fact is a bare
/// span) it proves the nearest enclosing braces are a container's. A `${`
/// on the left is a template interpolation, not a container, and is refused.
fn container_wraps(file: &FileFacts, outer: Span, span: Span) -> bool {
    if !outer.contains(span) || outer == span {
        return false;
    }
    braces_wrap(
        text(file, Span::new(outer.start, span.start)),
        text(file, Span::new(span.end, outer.end)),
    )
}

/// The string half of [`container_wraps`], separated so it is testable
/// without building a whole `FileFacts`.
fn braces_wrap(before: &str, after: &str) -> bool {
    let before = before.trim_end();
    before.ends_with('{') && !before.ends_with("${") && after.trim_start().starts_with('}')
}

/// Wraps a branch's source text for use as JSX children: left untouched
/// when it is already a JSX element or fragment, wrapped in `{}` otherwise
/// so the value is evaluated as an expression instead of read as literal
/// text. Upstream's fixer (`putIntoJSX`) makes the same distinction; naively
/// splicing every branch in unwrapped — as the 1.x Rust port's fix does —
/// is only correct for the JSX-element case and turns a bare identifier
/// branch into dead text content instead of the expression it was.
fn as_jsx_child(source: &str) -> String {
    if source.trim_start().starts_with('<') {
        source.to_string()
    } else {
        format!("{{{source}}}")
    }
}

fn as_jsx_attribute_expression(source: &str) -> String {
    format!("{{{source}}}")
}

/// The span a `<Show>` replacement should occupy: the surrounding
/// `{ ... }` expression container when the conditional *is* that container's
/// whole expression, otherwise the conditional itself.
///
/// A `<Show>` element is a JSX child in its own right, so leaving the braces
/// behind would emit `<div>{<Show …></Show>}</div>` — valid, and identical at
/// runtime, but a container that now wraps nothing but an element and will
/// never be reformatted away. Upstream replaces the container for the same
/// reason.
///
/// The braces must be `span`'s immediate neighbours, which is exactly what
/// keeps the render-callback shape (`<For>{(item) => item.cond && …}</For>`)
/// out: there the nearest text on the left is `=>` or `(`, the container
/// belongs to the arrow, and only the conditional inside it is replaced —
/// again as upstream does. Byte-wise and ASCII, so the walk cannot stop
/// inside a multi-byte character.
fn show_replacement_span(source: &str, span: Span) -> Span {
    let bytes = source.as_bytes();
    let (mut start, mut end) = (span.start as usize, span.end as usize);
    if end > bytes.len() {
        return span;
    }
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_whitespace() {
        end += 1;
    }
    // `${` opens a template interpolation, not a JSX container.
    let opens = start > 0 && bytes[start - 1] == b'{' && !(start > 1 && bytes[start - 2] == b'$');
    let closes = end < bytes.len() && bytes[end] == b'}';
    if opens && closes {
        Span::new(
            u32::try_from(start - 1).unwrap_or(span.start),
            u32::try_from(end + 1).unwrap_or(span.end),
        )
    } else {
        span
    }
}

fn show_conditional_replacement(test: &str, consequent: &str, alternate: &str) -> String {
    format!(
        "<Show when={{{test}}} fallback={}>{}</Show>",
        as_jsx_attribute_expression(alternate),
        as_jsx_child(consequent)
    )
}

fn prefer_show(file: &FileFacts, violations: &mut Vec<StaticViolation>) {
    for logical in &file.ast.logical_expressions {
        if logical.operator != LogicalOperatorKind::And || !expensive_branch(file, logical.right) {
            continue;
        }
        // Upstream reports only when the container's parent is a JSX
        // element or fragment. An attribute-value container's parent is the
        // attribute — `fallback={cond ? <A/> : <B/>}` — and upstream stays
        // silent there, so this port does too.
        if jsx_expression_position(file, logical.span) != Some(JsxExpressionPosition::Child) {
            continue;
        }
        let fixes = vec![Fix {
            message: "Replace with <Show>.".into(),
            applicability: "safe".into(),
            edits: vec![TextEdit {
                location: location(
                    file.path.shared(),
                    show_replacement_span(&file.source, logical.span),
                ),
                new_text: format!(
                    "<Show when={{{}}}>{}</Show>",
                    text(file, logical.left),
                    as_jsx_child(text(file, logical.right))
                ),
            }],
        }];
        violations.push(StaticViolation {
            id: "SC8015".into(),
            rule: "prefer-show".into(),
            message: "Use Solid's `<Show />` component for conditionally showing content.".into(),
            hint: "Solid's compiler already covers this case; `<Show>` is a stylistic preference."
                .into(),
            location: location(file.path.shared(), logical.span),
            analysis_context: String::new(),
            fixes,
        });
    }
    for conditional in &file.ast.conditional_expressions {
        if !expensive_branch(file, conditional.consequent)
            && !expensive_branch(file, conditional.alternate)
        {
            continue;
        }
        if jsx_expression_position(file, conditional.span) != Some(JsxExpressionPosition::Child) {
            continue;
        }
        let fixes = vec![Fix {
            message: "Replace with <Show fallback>.".into(),
            applicability: "safe".into(),
            edits: vec![TextEdit {
                location: location(
                    file.path.shared(),
                    show_replacement_span(&file.source, conditional.span),
                ),
                new_text: show_conditional_replacement(
                    text(file, conditional.test),
                    text(file, conditional.consequent),
                    text(file, conditional.alternate),
                ),
            }],
        }];
        violations.push(StaticViolation {
            id: "SC8015".into(),
            rule: "prefer-show".into(),
            message: "Use Solid's `<Show />` component for conditionally showing content with a fallback.".into(),
            hint: "Solid's compiler already covers this case; `<Show>` is a stylistic preference.".into(),
            location: location(file.path.shared(), conditional.span),
            analysis_context: String::new(),
            fixes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        as_jsx_attribute_expression, as_jsx_child, braces_wrap, show_conditional_replacement,
        show_replacement_span,
    };
    use solid_facts::core::Span;

    /// The substring [`show_replacement_span`] chose, given the span of the
    /// conditional written between `[[` and `]]`.
    fn replaced(marked: &str) -> String {
        let start = marked.find("[[").expect("a start marker");
        let end = marked[start + 2..].find("]]").expect("an end marker") + start + 2;
        let source = format!(
            "{}{}{}",
            &marked[..start],
            &marked[start + 2..end],
            &marked[end + 2..]
        );
        let span = Span::new(
            u32::try_from(start).unwrap(),
            u32::try_from(end - 2).unwrap(),
        );
        let widened = show_replacement_span(&source, span);
        source[widened.start as usize..widened.end as usize].to_string()
    }

    #[test]
    fn a_show_replacement_takes_the_container_it_is_the_whole_expression_of() {
        // The braces would otherwise survive as `{<Show …></Show>}`.
        assert_eq!(
            replaced("<div>{[[props.cond && <span/>]]}</div>"),
            "{props.cond && <span/>}"
        );
        // Padding and newlines inside the container go with it.
        assert_eq!(
            replaced("<div>\n  { [[a ? <A/> : <B/>]] }\n</div>"),
            "{ a ? <A/> : <B/> }"
        );
    }

    #[test]
    fn a_show_replacement_leaves_a_container_it_does_not_own() {
        // The render-callback shape: the container belongs to the arrow, and
        // upstream replaces only the conditional inside it too.
        assert_eq!(
            replaced("<For each={xs}>{(x) => [[x.cond && <span/>]]}</For>"),
            "x.cond && <span/>"
        );
        // Parenthesised body: the nearest neighbour is `(`, not `{`.
        assert_eq!(
            replaced("<For each={xs}>{(x) => ([[x.cond && <span/>]])}</For>"),
            "x.cond && <span/>"
        );
        // An operand of a larger expression.
        assert_eq!(replaced("<div>{ready && [[a ? b : c]]}</div>"), "a ? b : c");
        // A template interpolation is not a JSX container.
        assert_eq!(replaced("<div>{`${[[a ? b : c]]}`}</div>"), "a ? b : c");
    }

    #[test]
    fn braces_wrap_accepts_only_an_immediate_expression_container_pair() {
        assert!(braces_wrap("<>{", "}</>"));
        assert!(braces_wrap("{", "}"));
        assert!(braces_wrap("<>{ ", " }</>"));
        // A template interpolation's `${` is not a JSX container.
        assert!(!braces_wrap("<>{`${", "}`}</>"));
        // The span is an operand of a larger expression, not the container's
        // own expression.
        assert!(!braces_wrap("<>{ready && ", "}</>"));
        assert!(!braces_wrap("<>{", " ? a : b}</>"));
        // Block or call syntax between the braces and the span.
        assert!(!braces_wrap("<>{fn(", ")}</>"));
    }

    #[test]
    fn jsx_children_are_left_bare_and_everything_else_is_wrapped() {
        assert_eq!(as_jsx_child("<Content />"), "<Content />");
        assert_eq!(as_jsx_child("content"), "{content}");
        assert_eq!(as_jsx_child("a && b"), "{a && b}");
    }

    #[test]
    fn jsx_attribute_expressions_have_exactly_one_brace_pair() {
        assert_eq!(as_jsx_attribute_expression("fallback"), "{fallback}");
        assert_eq!(
            as_jsx_attribute_expression("<Fallback />"),
            "{<Fallback />}"
        );
    }

    #[test]
    fn show_conditional_fixes_preserve_identifier_and_jsx_fallback_values() {
        assert_eq!(
            show_conditional_replacement("ready", "<strong />", "fallback"),
            "<Show when={ready} fallback={fallback}><strong /></Show>"
        );
        assert_eq!(
            show_conditional_replacement("ready", "content", "<Fallback />"),
            "<Show when={ready} fallback={<Fallback />}>{content}</Show>"
        );
    }
}
