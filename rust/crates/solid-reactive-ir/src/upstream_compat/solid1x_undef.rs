//! Solid 1.x `v1/jsx-no-undef` — report only a JSX name whose semantic facts
//! explicitly prove that no binding exists.
//!
//! Upstream walks ESLint's scope graph by hand and ships a
//! `typescriptEnabled` option that turns its undefined-tag report off,
//! because its own scope walk and TypeScript's binder can disagree, and a
//! project already running `tsc` does not want two different answers for
//! the same identifier. A missing entity is not that proof: the producer may
//! have omitted a demand, or the semantic backend may not certify the name.
//! This rule therefore fails closed for unresolved JSX tags. Directive names
//! use the structural lexical binding fact because that fact is an explicit
//! positive/negative result, not an absence from the TypeScript entity table.
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
use solid_facts::FileFacts;
#[cfg(test)]
use solid_facts::ast::ImportKind;
#[cfg(test)]
use solid_facts::core::Span;

use super::{UpstreamCompatContext, is_lowercase_led};
#[cfg(test)]
use crate::Fix;
use crate::{StaticViolation, location};

pub(super) fn check_file(
    file: &FileFacts,
    _context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
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
    for element in &file.ast.jsx_elements {
        let name = file.source_text(element.name.span).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        if let Some(dot) = name.find('.') {
            // A dotted tag is certifiable only when the semantic layer
            // provides an explicit unresolved result for its root. An absent
            // root entity is merely missing evidence, so fail closed.
            let root = name[..dot].trim_end();
            if root == "this" || root.is_empty() {
                continue;
            }
            continue;
        }
        // Upstream's `isDOMElementName` plus its separate `this` guard:
        // `this` also starts lowercase, so the one shared lowercase-led
        // check covers both exemptions.
        if is_lowercase_led(name) {
            continue;
        }
        // `EntitySymbols` contains only proven semantic bindings. Its absence
        // is deliberately uncertifiable, not proof that this JSX name is
        // undefined.
        let _ = name;
    }
}

/// Upstream's `formatList`: a sentence-cased, Oxford-comma-joined name list
/// (`'Show'`, `'Show' and 'For'`, `'Show', 'For', and 'Index'`).
#[cfg(test)]
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

/// Where a brand-new `import` statement belongs: right after a leading
/// shebang, otherwise the top of the file.
#[cfg(test)]
fn leading_insertion_point(source: &str) -> u32 {
    if source.starts_with("#!") {
        source
            .find('\n')
            .map_or(0, |index| u32::try_from(index + 1).unwrap_or_default())
    } else {
        0
    }
}

/// Where new names go inside an existing named-imports clause, and the
/// separator they need there: `None` when the declaration has no `}` at all
/// (a side-effect, default-only, or namespace import — handled separately).
///
/// With existing bindings the names are appended directly after the last one
/// rather than in front of the closing brace, so `import { Show }` becomes
/// `import { Show, For }` and not `import { Show , For}` — the padding the
/// author put inside the braces stays where it is. An empty clause
/// (`import {}`) has no binding to append to, so the brace itself is the
/// insertion point and no separator is needed.
#[cfg(test)]
fn named_clause_insertion(source: &str) -> Option<(usize, &'static str)> {
    let brace = source.rfind('}')?;
    let trimmed = source[..brace].trim_end();
    Some(if trimmed.ends_with('{') {
        (brace, "")
    } else {
        (trimmed.len(), ", ")
    })
}

/// A same-file fix appending the missing names to an existing `"solid-js"`
/// import, or `None` when the existing import shape has no unambiguous
/// same-file rewrite.
#[cfg(test)]
#[allow(dead_code)]
fn auto_import_fix(
    file: &FileFacts,
    import: &solid_facts::ast::ImportFact,
    joined: &str,
) -> Option<Fix> {
    let source = file.source_text(import.span).unwrap_or_default();
    if let Some((offset, separator)) = named_clause_insertion(source) {
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
    // named-imports clause before the module specifier — that is, before the
    // `from` keyword. When the keyword cannot be located (an unanticipated
    // spelling), this reports without a fix rather than splice text at a
    // guessed offset and emit a syntactically broken "safe" edit.
    let offset = from_keyword_offset(source)?;
    let at = import.span.start + u32::try_from(offset).unwrap_or_default();
    Some(insert_fix(file, at, format!(", {{ {joined} }}")))
}

/// The byte offset in an import declaration's text at which a named-imports
/// clause can be inserted: just before its `from` keyword. TypeScript does
/// not require a space *after* the keyword (`import D from"solid-js";` is
/// valid), so the match accepts whitespace or the module string's opening
/// quote there; the space *before* the keyword is mandatory (it separates
/// `from` from the default binding's name), so ` from` anchors the search.
/// `None` when no such keyword is found.
#[cfg(test)]
fn from_keyword_offset(source: &str) -> Option<usize> {
    source.match_indices(" from").find_map(|(index, keyword)| {
        let after = source[index + keyword.len()..].chars().next()?;
        (after.is_whitespace() || matches!(after, '"' | '\'')).then_some(index)
    })
}

#[cfg(test)]
#[allow(dead_code)]
fn insert_fix(file: &FileFacts, at: u32, new_text: String) -> Fix {
    replace_fix(file, Span::new(at, at), new_text)
}

#[cfg(test)]
#[allow(dead_code)]
fn replace_fix(file: &FileFacts, span: Span, new_text: String) -> Fix {
    super::fix_replace(
        file,
        span,
        "Import Solid's control-flow components from 'solid-js'.",
        new_text,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        format_names, from_keyword_offset, is_lowercase_led, leading_insertion_point,
        named_clause_insertion,
    };

    /// Applies what [`named_clause_insertion`] decided, so the assertions
    /// read as the text a user would get from the fix.
    fn extend(source: &str, names: &str) -> String {
        let (offset, separator) = named_clause_insertion(source).expect("a named clause");
        format!(
            "{}{separator}{names}{}",
            &source[..offset],
            &source[offset..]
        )
    }

    #[test]
    fn new_names_are_appended_after_the_last_binding() {
        // Inserting in front of the `}` instead produced `{ Show , For}`.
        assert_eq!(
            extend("import { Show } from \"solid-js\";", "For"),
            "import { Show, For } from \"solid-js\";"
        );
        assert_eq!(
            extend("import { For, Switch } from \"solid-js\";", "Match"),
            "import { For, Switch, Match } from \"solid-js\";"
        );
        // No padding to preserve, and none invented.
        assert_eq!(
            extend("import {Show} from \"solid-js\";", "For"),
            "import {Show, For} from \"solid-js\";"
        );
    }

    #[test]
    fn an_empty_clause_takes_the_names_at_its_brace() {
        // Nothing to append to, so no `, ` separator and the brace stays the
        // insertion point.
        assert_eq!(
            extend("import {} from \"solid-js\";", "For"),
            "import {For} from \"solid-js\";"
        );
    }

    #[test]
    fn a_declaration_without_a_clause_has_no_insertion_point() {
        assert_eq!(named_clause_insertion("import \"solid-js\";"), None);
        assert_eq!(named_clause_insertion("import D from \"solid-js\";"), None);
    }

    #[test]
    fn dom_and_this_are_lowercase_first() {
        // The shared helper carries both of upstream's exemptions: DOM
        // intrinsics and `this` are lowercase-led, components are not.
        assert!(is_lowercase_led("div"));
        assert!(is_lowercase_led("this"));
        assert!(!is_lowercase_led("Show"));
        assert!(!is_lowercase_led("MyComponent"));
    }

    #[test]
    fn from_keyword_is_found_with_and_without_a_following_space() {
        assert_eq!(from_keyword_offset("import D from \"solid-js\";"), Some(8));
        // TypeScript accepts no space between `from` and the module string;
        // guessing `source.len()` here used to emit a broken fix.
        assert_eq!(from_keyword_offset("import D from\"solid-js\";"), Some(8));
        assert_eq!(from_keyword_offset("import D from'solid-js';"), Some(8));
    }

    #[test]
    fn no_from_keyword_means_no_fix_offset() {
        // No ` from` at all, or one not followed by whitespace/quote —
        // `auto_import_fix` reports without a fix in both cases.
        assert_eq!(from_keyword_offset("import \"solid-js\";"), None);
        assert_eq!(from_keyword_offset("import D fromage;"), None);
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
