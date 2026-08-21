//! Shared `prefer-for` and `prefer-show` control-flow preferences, narrowed
//! from eslint-plugin-solid's Solid 1.x structural rules. Each judges a
//! JavaScript-legal but Solid-unidiomatic shape only when its governing input
//! has a proven reactive dependency at the rendered JSX position.
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

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, IdentifierRole, ImportKind, LogicalOperatorKind};
use solid_facts::core::Span;

use super::{UpstreamCompatContext, text};
use crate::{Fix, StaticViolation, TextEdit, location};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    if context.prefer_for_enabled {
        prefer_for(file, context, violations);
    }
    if context.prefer_show_enabled {
        prefer_show(file, context, violations);
    }
}

// ---------------------------------------------------------------------
// SC8014 v1/prefer-for
// ---------------------------------------------------------------------

/// A reactive `Array#map` rendered as JSX children recreates every DOM node on each
/// update; `<For>` keys elements by array identity instead so unchanged
/// items keep their nodes. Position carries the whole judgement, exactly as
/// upstream's `JSXExpressionContainer` parent checks do: the `.map()` call
/// must itself be the expression a JSX element or fragment renders. The
/// callback does not have to build JSX — `<ol>{items().map(x => x.name)}</ol>`
/// still renders a list — and a `.map()` anywhere else (assigned to a
/// variable, inside an attribute) is not rendered as a list and is left
/// alone.
fn prefer_for(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let mut may_add_import = true;
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
        let standard_array_map = context
            .lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library && declaration.name.as_ref() == "map"
            });
        if !standard_array_map {
            continue;
        }
        if !context
            .reactive_reads
            .has_proven_read(context, file, member.object)
        {
            continue;
        }
        let argument = &call.arguments[0];
        let solid_one = context.dialect.carries_eslint_era_rules();
        if !matches!(
            argument.value,
            ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
        ) || solid_one && argument.value == ArgumentValueKind::AsyncFunction
        {
            continue;
        }
        // The autofix applies only to an arrow with exactly one non-rest
        // parameter (`(item) => ...`). A regular function can observe
        // Array#map's three callback arguments through `arguments`, while
        // `<For>` supplies only its declared parameter. An index parameter,
        // no parameter, or a rest parameter also leaves too many candidate
        // rewrites (`<For>` with its own index callback, or `<Index>`) to
        // pick between. `FunctionFact::parameters` already excludes rest
        // parameters, so a rest-only callback also reads as zero
        // parameters here, which correctly falls through to "no fix".
        //
        // The judgement itself requires a proven array/tuple. A reactive
        // collection can expose a completely unrelated `.map`; recommending
        // `<For each>` for it would be a type and runtime error, not merely an
        // unsafe autofix. Async callbacks are also left to TypeScript because
        // the published Solid 1.x types reject their Promise-valued children.
        let receiver_is_array = super::expression_array_shape(context, file, member.object)
            .is_some_and(typefacts::ArrayShape::is_array_or_tuple);
        if !receiver_is_array {
            continue;
        }
        let one_parameter_arrow = argument.value == ArgumentValueKind::Function
            && file
                .ast
                .functions
                .iter()
                .find(|function| function.span == file.ast.peel_ts_sugar_span(argument.span))
                .is_some_and(|function| {
                    function.kind == solid_facts::ast::FunctionKind::Arrow
                        && function.parameters.len() == 1
                });
        let fix_target = one_parameter_arrow
            .then(|| solid_component_fix_target(file, "For", call.span, may_add_import))
            .flatten();
        if fix_target
            .as_ref()
            .is_some_and(|target| target.added_import)
        {
            may_add_import = false;
        }
        let (message, fixes) = if one_parameter_arrow {
            let fixes = if let Some(target) = fix_target {
                let mut edits = target.import_edit.into_iter().collect::<Vec<_>>();
                edits.push(TextEdit {
                    location: location(file.path.shared(), call.span),
                    new_text: format!(
                        "<{name} each={{{receiver}}}>{{{callback}}}</{name}>",
                        name = target.name,
                        receiver = text(file, member.object),
                        callback = text(file, argument.span),
                    ),
                });
                vec![Fix {
                    message: "Replace Array#map with <For>.".into(),
                    applicability: "safe".into(),
                    edits,
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
            hint: if solid_one {
                "Pick `<For>` when the callback needs the item value reactively, `<Index>` when it needs the index reactively.".into()
            } else {
                "Solid 2.0's default `<For>` preserves item identity; `keyed={false}` instead passes an accessor and is not a semantics-preserving rewrite of an Array#map callback.".into()
            },
            location: location(file.path.shared(), call.span),
            analysis_context: String::new(),
            fixes,
            uncertain: false,
        });
    }
}

struct SolidComponentFixTarget {
    name: String,
    import_edit: Option<TextEdit>,
    added_import: bool,
}

/// Names a runtime Solid control-flow component without guessing that a JSX
/// identifier exists. Existing named/namespace imports are used only when no
/// second binding can shadow them anywhere in the file. Otherwise the first
/// fix of that component kind adds a collision-free named import; later
/// findings wait for the normal lint fix pass to rerun, avoiding overlapping
/// import edits when a client applies every safe fix in one batch.
fn solid_component_fix_target(
    file: &FileFacts,
    imported: &str,
    candidate: Span,
    may_add_import: bool,
) -> Option<SolidComponentFixTarget> {
    for import in &file.ast.imports {
        if import.module.as_str() != "solid-js" || import.type_only {
            continue;
        }
        for binding in &import.bindings {
            if binding.type_only {
                continue;
            }
            let matches = match binding.kind {
                ImportKind::Named => binding.imported.as_deref() == Some(imported),
                ImportKind::Namespace => true,
                ImportKind::SideEffect | ImportKind::Default => false,
            };
            if !matches {
                continue;
            }
            let local = text(file, binding.local.span);
            let binding_count = file
                .ast
                .identifiers
                .iter()
                .filter(|identifier| {
                    identifier.role == IdentifierRole::Binding
                        && text(file, identifier.span) == local
                })
                .count();
            if binding_count == 1 {
                return Some(SolidComponentFixTarget {
                    name: if binding.kind == ImportKind::Namespace {
                        format!("{local}.{imported}")
                    } else {
                        local.to_string()
                    },
                    import_edit: None,
                    added_import: false,
                });
            }
        }
    }
    if !may_add_import {
        return None;
    }

    let stem = format!("__SolidChecker{imported}{}", candidate.start);
    let mut name = stem.clone();
    let mut suffix = 2;
    while file
        .ast
        .identifiers
        .iter()
        .any(|identifier| text(file, identifier.span) == name)
    {
        name = format!("{stem}_{suffix}");
        suffix += 1;
    }
    let insertion = file
        .ast
        .imports
        .last()
        .map(|import| import.span.end)
        .or_else(|| {
            file.ast
                .module_directives
                .last()
                .map(|directive| directive.span.end)
        })
        .or_else(|| {
            file.source
                .starts_with("#!")
                .then(|| file.source.find('\n').unwrap_or(file.source.len()))
                .and_then(|offset| u32::try_from(offset).ok())
        });
    let (offset, prefix, suffix_text) = match insertion {
        Some(offset) => (offset, "\n", ""),
        None => (0, "", "\n"),
    };
    Some(SolidComponentFixTarget {
        name: name.clone(),
        import_edit: Some(TextEdit {
            location: location(file.path.shared(), Span::new(offset, offset)),
            new_text: format!(
                "{prefix}import {{ {imported} as {name} }} from \"solid-js\";{suffix_text}"
            ),
        }),
        added_import: true,
    })
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

#[cfg(test)]
fn show_conditional_replacement(test: &str, consequent: &str, alternate: &str) -> String {
    show_conditional_replacement_with_name("Show", test, consequent, alternate)
}

fn show_conditional_replacement_with_name(
    name: &str,
    test: &str,
    consequent: &str,
    alternate: &str,
) -> String {
    format!(
        "<{name} when={{{test}}} fallback={}>{}</{name}>",
        as_jsx_attribute_expression(alternate),
        as_jsx_child(consequent)
    )
}

fn prefer_show(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let mut may_add_import = true;
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
        if !context
            .reactive_reads
            .has_proven_read(context, file, logical.left)
        {
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
            // `0 && child` renders the text `0`, while `<Show when={0}>`
            // renders nothing. Type Facts do not currently prove a Boolean
            // value domain, so this rewrite cannot honestly be called safe.
            fixes: vec![],
            uncertain: false,
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
        if !context
            .reactive_reads
            .has_proven_read(context, file, conditional.test)
        {
            continue;
        }
        let fixes = solid_component_fix_target(file, "Show", conditional.span, may_add_import)
            .map(|target| {
                if target.added_import {
                    may_add_import = false;
                }
                let mut edits = target.import_edit.into_iter().collect::<Vec<_>>();
                edits.push(TextEdit {
                    location: location(
                        file.path.shared(),
                        show_replacement_span(&file.source, conditional.span),
                    ),
                    new_text: show_conditional_replacement_with_name(
                        &target.name,
                        text(file, conditional.test),
                        text(file, conditional.consequent),
                        text(file, conditional.alternate),
                    ),
                });
                vec![Fix {
                    message: "Replace with <Show fallback>.".into(),
                    applicability: "safe".into(),
                    edits,
                }]
            })
            .unwrap_or_default();
        violations.push(StaticViolation {
            id: "SC8015".into(),
            rule: "prefer-show".into(),
            message: "Use Solid's `<Show />` component for conditionally showing content with a fallback.".into(),
            hint: "Solid's compiler already covers this case; `<Show>` is a stylistic preference.".into(),
            location: location(file.path.shared(), conditional.span),
            analysis_context: String::new(),
            fixes,
            uncertain: false,
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
