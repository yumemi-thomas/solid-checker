//! `v1/jsx-no-undef` — a JSX tag name that resolves to no binding anywhere:
//! not a local variable, not an import, not a global, not a JSX intrinsic.
//!
//! Upstream walks ESLint's scope graph by hand and ships a
//! `typescriptEnabled` option that turns its undefined-tag report off,
//! because its own scope walk and TypeScript's binder can disagree, and a
//! project already running `tsc` does not want two different answers for
//! the same identifier. This port never had that disagreement to begin
//! with: it asks `context.entities`, the same demand-driven TypeScript
//! resolution the rest of the checker already asks, keyed by the exact byte
//! spans the fact-extraction demand plan requested — there is no separate
//! scope walk here to turn off.
//!
//! A dotted JSX tag name (`<Foo.Bar />`) is handled the way upstream handles
//! it: only the root identifier's own resolution is judged, from a demand the
//! plan places on exactly that root span. Whether a resolved `Foo` actually
//! has a `Bar` member is a different question with a different fix, and this
//! rule does not speculate about it — upstream does not either.
//!
//! TypeScript deliberately does not bind the local-name node in a namespaced
//! JSX attribute, so `GetSymbolAtLocation` cannot answer for
//! `use:directive`. The structural fact extractor therefore runs Oxc's
//! semantic binder over the same AST and records the exact lexical
//! declaration selected for each directive name. That handles imports,
//! hoisting, nested scopes, and shadowing without a text-based declaration
//! scan.
//!
//! What remains — a plain (non-dotted) JSX tag name — is exactly what the
//! demand plan always asks about, for every JSX element, dotted or not, so
//! an unresolved entity there reliably means the compiler looked and found
//! nothing.

use std::collections::BTreeSet;

use solid_facts::FileFacts;
use solid_facts::ast::ImportKind;
use solid_facts::core::Span;

use super::UpstreamCompatContext;
use crate::{Fix, StaticViolation, TextEdit, location};

/// Control-flow components upstream auto-imports from `"solid-js"` instead
/// of reporting undefined, on the theory that a bare `<For>`/`<Show>` used
/// without an import is a forgotten import rather than a typo.
const AUTO_IMPORT_COMPONENTS: [&str; 5] = ["Show", "For", "Index", "Switch", "Match"];

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let path = file.path.as_str();
    for element in &file.ast.jsx_elements {
        for attribute in &element.attributes {
            if attribute
                .namespace
                .and_then(|namespace| file.source_text(namespace))
                != Some("use")
            {
                continue;
            }
            if attribute.directive_binding.is_some() {
                continue;
            }
            let name = file.source_text(attribute.local_name).unwrap_or_default();
            violations.push(StaticViolation {
                id: "SC8005".into(),
                rule: "jsx-no-undef".into(),
                message: format!("'{name}' is not defined."),
                hint: "Import or declare this custom directive — lexical scope contains no binding for the use: name.".into(),
                location: location(file.path.shared(), attribute.local_name),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
    let mut missing_auto_imports = BTreeSet::new();
    for element in &file.ast.jsx_elements {
        let name = file.source_text(element.name.span).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        if let Some(dot) = name.find('.') {
            // A dotted tag name: upstream walks to the root identifier and
            // checks that alone — `<Foo.Bar/>` reports 'Foo' when `Foo` is
            // unbound, and never speculates about the `Bar` member. The
            // demand plan asks TypeScript about exactly this root span, so
            // an unresolved entity here means the compiler looked and found
            // nothing. No DOM exemption (upstream applies it to plain tags
            // only) and no auto-import; `this` is never a binding.
            let root = name[..dot].trim_end();
            let root_span = Span::new(
                element.name.span.start,
                element.name.span.start + u32::try_from(root.len()).unwrap_or_default(),
            );
            if root == "this" || root.is_empty() {
                continue;
            }
            if context.entities.at(path, root_span).is_some() {
                continue;
            }
            violations.push(StaticViolation {
                id: "SC8005".into(),
                rule: "jsx-no-undef".into(),
                message: format!("'{root}' is not defined."),
                hint: "Import it, declare it, or check the spelling — TypeScript could not resolve this tag's object to any binding.".into(),
                location: location(file.path.shared(), root_span),
                analysis_context: String::new(),
                fixes: vec![],
            });
            continue;
        }
        if is_dom_or_this(name) {
            continue;
        }
        if context.entities.at(path, element.name.span).is_some() {
            continue;
        }
        if AUTO_IMPORT_COMPONENTS.contains(&name) {
            missing_auto_imports.insert(name);
            continue;
        }
        violations.push(StaticViolation {
            id: "SC8005".into(),
            rule: "jsx-no-undef".into(),
            message: format!("'{name}' is not defined."),
            hint: "Import it, declare it, or check the spelling — TypeScript could not resolve this tag to any binding.".into(),
            location: location(file.path.shared(), element.name.span),
            analysis_context: String::new(),
            fixes: vec![],
        });
    }
    if !missing_auto_imports.is_empty() {
        report_missing_auto_imports(file, &missing_auto_imports, violations);
    }
}

/// Upstream's `isDOMElementName`: a lowercase-first tag name is a DOM
/// intrinsic (`div`, `span`), never a component. `this` also starts
/// lowercase, so the same check covers upstream's separate `this` guard.
fn is_dom_or_this(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

/// Upstream's `formatList`: a sentence-cased, Oxford-comma-joined name list
/// (`'Show'`, `'Show' and 'For'`, `'Show', 'For', and 'Index'`).
fn format_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => format!("'{only}'"),
        [first, second] => format!("'{first}' and '{second}'"),
        [rest @ .., last] => format!(
            "{}, and '{last}'",
            rest.iter()
                .map(|name| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn report_missing_auto_imports(
    file: &FileFacts,
    missing: &BTreeSet<&str>,
    violations: &mut Vec<StaticViolation>,
) {
    let names = missing.iter().copied().collect::<Vec<_>>();
    let joined = names.join(", ");
    let solid_import = file
        .ast
        .imports
        .iter()
        .find(|import| import.module.as_str() == "solid-js" && !import.type_only);
    let (report_span, fix) = match solid_import {
        Some(import) => (import.span, auto_import_fix(file, import, &joined)),
        None => {
            let insertion = leading_insertion_point(&file.source);
            (
                Span::new(insertion, insertion),
                Some(insert_fix(
                    file,
                    insertion,
                    format!("import {{ {joined} }} from \"solid-js\";\n"),
                )),
            )
        }
    };
    violations.push(StaticViolation {
        id: "SC8005".into(),
        rule: "jsx-no-undef".into(),
        message: format!(
            "{} should be imported from 'solid-js'.",
            format_names(&names)
        ),
        hint: "Solid's control-flow components are ordinary named exports of 'solid-js'.".into(),
        location: location(file.path.shared(), report_span),
        analysis_context: String::new(),
        fixes: fix.into_iter().collect(),
    });
}

/// Where a brand-new `import` statement belongs: right after a leading
/// shebang, otherwise the top of the file.
fn leading_insertion_point(source: &str) -> u32 {
    if source.starts_with("#!") {
        source
            .find('\n')
            .map_or(0, |index| u32::try_from(index + 1).unwrap_or_default())
    } else {
        0
    }
}

/// A same-file fix appending the missing names to an existing `"solid-js"`
/// import, or `None` when the existing import shape has no unambiguous
/// same-file rewrite.
fn auto_import_fix(
    file: &FileFacts,
    import: &solid_facts::ast::ImportFact,
    joined: &str,
) -> Option<Fix> {
    let source = file.source_text(import.span).unwrap_or_default();
    if let Some(offset) = source.rfind('}') {
        let prefix = &source[..offset];
        let separator = if prefix.trim_end().ends_with('{') {
            ""
        } else {
            ", "
        };
        let at = import.span.start + u32::try_from(offset).unwrap_or_default();
        return Some(insert_fix(file, at, format!("{separator}{joined}")));
    }
    if import.bindings.is_empty() {
        // A side-effect-only import (`import "solid-js";`): there is no
        // named-imports clause to extend, so replace the whole declaration
        // with one that has it.
        return Some(replace_fix(
            file,
            import.span,
            format!("import {{ {joined} }} from \"solid-js\";"),
        ));
    }
    if import
        .bindings
        .iter()
        .any(|binding| binding.kind == ImportKind::Namespace)
    {
        // `import * as X, { Y } from "m"` is a syntax error: a namespace
        // import cannot also carry a named-imports clause. Rewriting the
        // call sites to `X.Show` instead is not a same-file text edit, so
        // this stays reported without a fix rather than emit broken code.
        return None;
    }
    // A default-only import (`import Default from "solid-js";`): add a
    // named-imports clause before the module specifier.
    let offset = source.find(" from ").unwrap_or(source.len());
    let at = import.span.start + u32::try_from(offset).unwrap_or_default();
    Some(insert_fix(file, at, format!(", {{ {joined} }}")))
}

fn insert_fix(file: &FileFacts, at: u32, new_text: String) -> Fix {
    replace_fix(file, Span::new(at, at), new_text)
}

fn replace_fix(file: &FileFacts, span: Span, new_text: String) -> Fix {
    Fix {
        message: "Import Solid's control-flow components from 'solid-js'.".into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), span),
            new_text,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::{format_names, is_dom_or_this, leading_insertion_point};

    #[test]
    fn dom_and_this_are_lowercase_first() {
        assert!(is_dom_or_this("div"));
        assert!(is_dom_or_this("this"));
        assert!(!is_dom_or_this("Show"));
        assert!(!is_dom_or_this("MyComponent"));
    }

    #[test]
    fn name_lists_read_as_a_sentence() {
        assert_eq!(format_names(&["Show"]), "'Show'");
        assert_eq!(format_names(&["Show", "For"]), "'Show' and 'For'");
        assert_eq!(
            format_names(&["Show", "For", "Index"]),
            "'Show', 'For', and 'Index'"
        );
    }

    #[test]
    fn insertion_point_is_the_top_of_the_file_without_a_shebang() {
        assert_eq!(leading_insertion_point("import x from 'y';"), 0);
    }

    #[test]
    fn insertion_point_skips_a_leading_shebang() {
        let source = "#!/usr/bin/env node\nrest";
        let expected = u32::try_from(source.find('\n').unwrap()).unwrap() + 1;
        assert_eq!(leading_insertion_point(source), expected);
    }
}
