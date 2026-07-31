//! `v1/prefer-for`, `v1/prefer-show`, `v1/no-react-deps`, `v1/no-proxy-apis` —
//! four of eslint-plugin-solid's structural-preference rules, each judging a
//! JavaScript-legal but Solid-unidiomatic shape.
//!
//! `prefer-for` and `prefer-show` are intent judgements, not correctness
//! checks: Solid's compiler already handles a plain `.map()` or `&&`/ternary
//! correctly, so these rules exist only to steer style toward the
//! primitives that make list and branch identity explicit. Upstream and the
//! 1.x port both keep them conservative for exactly that reason — a wrong
//! "prefer" nag is worse than a missed one. This port keeps the same
//! restraint: `prefer-for` fires only for a `.map()` whose own callback
//! builds JSX, and `prefer-show` only for the "expensive" branch shapes
//! upstream defines (a JSX element/fragment, or a bare identifier).
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
use solid_facts::ast::{ArgumentValueKind, LogicalOperatorKind};
use solid_facts::core::Span;

use super::UpstreamCompatContext;
use crate::{Fix, StaticViolation, TextEdit, known_primitive, location};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    no_react_deps(file, context, violations);
    no_proxy_apis(file, context, violations);
    prefer_for(file, violations);
    prefer_show(file, violations);
}

/// The UTF-8 source text a span covers, or `""` for a span outside the file.
/// Never expected in practice, but a rule that panics explaining its own
/// finding is worse than one that quietly declines to.
fn text(file: &FileFacts, span: Span) -> &str {
    file.source_text(span).unwrap_or_default()
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
            || binding_initializer(file, source)
                .is_some_and(|(_, initializer)| initializer.trim_start().starts_with('['));
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
                    location: location(file.path.shared(), argument.span),
                    new_text: String::new(),
                }],
            }],
        });
    }
}

/// Traces a same-file `const name = ...` binding to its initializer text.
///
/// A narrow, single-hop trace: it does not resolve reassignment, shadowing,
/// or which of several same-named bindings is actually in scope at the call
/// site. That is acceptable here because the result only ever loosens a
/// stylistic nag (does the value this name was initialized with *look
/// like* an array or object literal) — it is never used to decide whether a
/// name is defined at all, which is exactly the judgement `undef.rs` refuses
/// to make by hand, asking TypeScript facts instead.
fn binding_initializer<'a>(file: &'a FileFacts, name: &str) -> Option<(Span, &'a str)> {
    file.ast.bindings.iter().find_map(|binding| {
        binding
            .names
            .iter()
            .any(|candidate| text(file, candidate.span) == name)
            .then(|| binding.initializer.map(|span| (span, text(file, span))))
            .flatten()
    })
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
        if known_primitive(&primitives.calls[index]) == Some(Primitive::MergeProps)
            && call.arguments.first().is_some_and(|argument| {
                let source = text(file, argument.span).trim();
                !source.starts_with('{')
                    && binding_initializer(file, source)
                        .is_none_or(|(_, initializer)| !initializer.trim_start().starts_with('{'))
            })
        {
            violations.push(proxy_violation(
                file,
                "The first argument to mergeProps should be an object literal.",
                "If you pass a function to `mergeProps`, it will create a Proxy, which are incompatible with your target environment.",
                call.span,
            ));
        }
    }
    for element in &file.ast.jsx_elements {
        for spread in &element.spreads {
            let is_call = file
                .ast
                .calls
                .iter()
                .any(|call| call.span == spread.argument);
            let is_member = file
                .ast
                .members
                .iter()
                .any(|member| member.span == spread.argument);
            if is_call {
                violations.push(proxy_violation(
                    file,
                    "Spreading a call expression may require a Proxy.",
                    "Using a function call in JSX spread makes Solid use Proxies, which are incompatible with your target environment.",
                    spread.span,
                ));
            } else if is_member {
                violations.push(proxy_violation(
                    file,
                    "Spreading a member expression may require a Proxy.",
                    "Using a property access in JSX spread makes Solid use Proxies, which are incompatible with your target environment.",
                    spread.span,
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------
// SC8014 v1/prefer-for
// ---------------------------------------------------------------------

/// `Array#map` returning JSX recreates every DOM node on each update;
/// `<For>` keys elements by array identity instead so unchanged items keep
/// their nodes. Restricted to a `.map()` whose own callback builds JSX —
/// not every `.map()` in a component file — the same restraint the 1.x port
/// applies.
fn prefer_for(file: &FileFacts, violations: &mut Vec<StaticViolation>) {
    for call in &file.ast.calls {
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
        ) || !file
            .ast
            .jsx_elements
            .iter()
            .any(|element| argument.span.contains(element.span))
        {
            continue;
        }
        // Upstream's autofix applies only when the callback takes exactly
        // one non-rest parameter (`(item) => ...`); an index parameter, no
        // parameter, or a rest parameter leaves too many candidate
        // rewrites (`<For>` with its own index callback, or `<Index>`) to
        // pick between. `FunctionFact::parameters` already excludes rest
        // parameters, so a rest-only callback also reads as zero
        // parameters here, which correctly falls through to "no fix".
        let one_parameter = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == argument.span)
            .is_some_and(|function| function.parameters.len() == 1);
        let (message, fixes) = if one_parameter {
            (
                "Use Solid's `<For />` component for efficiently rendering lists. Array#map causes DOM elements to be recreated.",
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
                }],
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
fn expensive_branch(source: &str) -> bool {
    let source = source.trim();
    source.starts_with('<')
        || source
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
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

fn prefer_show(file: &FileFacts, violations: &mut Vec<StaticViolation>) {
    for logical in &file.ast.logical_expressions {
        if logical.operator != LogicalOperatorKind::And {
            continue;
        }
        let right = text(file, logical.right);
        if !expensive_branch(right) {
            continue;
        }
        violations.push(StaticViolation {
            id: "SC8015".into(),
            rule: "prefer-show".into(),
            message: "Use Solid's `<Show />` component for conditionally showing content.".into(),
            hint: "Solid's compiler already covers this case; `<Show>` is a stylistic preference."
                .into(),
            location: location(file.path.shared(), logical.span),
            analysis_context: String::new(),
            fixes: vec![Fix {
                message: "Replace with <Show>.".into(),
                applicability: "safe".into(),
                edits: vec![TextEdit {
                    location: location(file.path.shared(), logical.span),
                    new_text: format!(
                        "<Show when={{{}}}>{}</Show>",
                        text(file, logical.left),
                        as_jsx_child(right)
                    ),
                }],
            }],
        });
    }
    for conditional in &file.ast.conditional_expressions {
        let consequent = text(file, conditional.consequent);
        let alternate = text(file, conditional.alternate);
        if !expensive_branch(consequent) && !expensive_branch(alternate) {
            continue;
        }
        violations.push(StaticViolation {
            id: "SC8015".into(),
            rule: "prefer-show".into(),
            message: "Use Solid's `<Show />` component for conditionally showing content with a fallback.".into(),
            hint: "Solid's compiler already covers this case; `<Show>` is a stylistic preference.".into(),
            location: location(file.path.shared(), conditional.span),
            analysis_context: String::new(),
            fixes: vec![Fix {
                message: "Replace with <Show fallback>.".into(),
                applicability: "safe".into(),
                edits: vec![TextEdit {
                    location: location(file.path.shared(), conditional.span),
                    new_text: format!(
                        "<Show when={{{}}} fallback={{{}}}>{}</Show>",
                        text(file, conditional.test),
                        as_jsx_child(alternate),
                        as_jsx_child(consequent)
                    ),
                }],
            }],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{as_jsx_child, expensive_branch};

    #[test]
    fn expensive_branch_matches_jsx_and_identifiers_only() {
        assert!(expensive_branch("<Content />"));
        assert!(expensive_branch("content"));
        assert!(expensive_branch("_private"));
        assert!(expensive_branch("$signal"));
        assert!(expensive_branch("  <Padded />  "));
        assert!(!expensive_branch("42"));
        assert!(!expensive_branch("\"literal\""));
        assert!(!expensive_branch(""));
    }

    #[test]
    fn jsx_children_are_left_bare_and_everything_else_is_wrapped() {
        assert_eq!(as_jsx_child("<Content />"), "<Content />");
        assert_eq!(as_jsx_child("content"), "{content}");
        assert_eq!(as_jsx_child("a && b"), "{a && b}");
    }
}
