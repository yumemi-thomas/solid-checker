//! `v1/prefer-classlist` —
//! eslint-plugin-solid's attribute-value rules, ported from the 1.x reactive
//! solver's `solid_1_rules.rs` onto this checker's fact tables.
//!
//! # Options
//!
//! `prefer-classlist { classnames }` is read from the project's
//! `.solid-checker/rule-options.json` (see [`super::solid1x_options`]),
//! defaulting to upstream's defaults.

use solid_facts::FileFacts;
use solid_facts::ast::JsxElementFact;

use super::{UpstreamCompatContext, fix_replace, text, violation};
use crate::StaticViolation;

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        prefer_classlist(file, context, element, violations);
    }
}

/// `v1/prefer-classlist` (SC8013) — a `class`/`className` prop set from a
/// `classnames`-shaped helper call (`cn`, `clsx`, `classnames`) applied to an
/// object literal. Solid's `classlist` prop accepts that exact `{ [name]:
/// boolean }` shape natively and updates only the classes whose value
/// changed, where the helper call reconstructs the whole class string on
/// every render.
fn prefer_classlist(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    element: &JsxElementFact,
    violations: &mut Vec<StaticViolation>,
) {
    // Which helper names count is upstream's `classnames` option; the
    // default is upstream's default list.
    let helpers = &context.solid1x_options.prefer_classlist.classnames;
    // Upstream guards on its own lowercase `classlist` spelling only; the
    // prop Solid's compiler special-cases is `classList`, so an element
    // already written that way must count as "already using it" too.
    if element
        .attributes
        .iter()
        .any(|attribute| matches!(text(file, attribute.name), "classlist" | "classList"))
    {
        return;
    }
    for attribute in element
        .attributes
        .iter()
        .filter(|attribute| matches!(text(file, attribute.name), "class" | "className"))
    {
        let Some(expression) = attribute.expression else {
            continue;
        };
        let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == expression && call.arguments.len() == 1)
        else {
            continue;
        };
        let Some(callee) = call.static_callee(&file.source) else {
            continue;
        };
        if !helpers.iter().any(|helper| helper == callee)
            || !text(file, call.arguments[0].span)
                .trim_start()
                .starts_with('{')
        {
            continue;
        }
        let mut result = violation(
            file,
            "SC8013",
            "prefer-classlist",
            format!(
                "The classlist prop should be used instead of {callee} to efficiently set classes based on an object."
            ),
            "classList takes the same { [name]: boolean } object and updates only the classes whose value actually changed, instead of recomputing and reassigning the whole class string.",
            attribute.span,
            vec![],
        );
        // The report matches on the helper's conventional names, as upstream
        // does — but a rewrite is only behaviour-preserving when the callee
        // really is the classnames-style helper, not a local function that
        // happens to share a name. An import from one of the packages is that
        // proof; without it the report stands and the fix is withheld.
        let imported_helper = file.ast.imports.iter().any(|import| {
            matches!(import.module.as_str(), "clsx" | "classnames")
                && import
                    .bindings
                    .iter()
                    .any(|binding| text(file, binding.local.span) == callee)
        });
        if imported_helper {
            // Upstream's fixer writes `classlist=`, a spelling the compiler
            // does not special-case; `classList` is the prop Solid actually
            // handles, so the working rewrite is the one worth offering.
            result.fixes.push(fix_replace(
                file,
                attribute.span,
                "rewrite as classList",
                format!("classList={{{}}}", text(file, call.arguments[0].span)),
            ));
        }
        violations.push(result);
    }
}
