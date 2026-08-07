//! The eslint-plugin-solid 0.14.5 rule surface, reproduced over the checker's
//! facts.
//!
//! These are the file-local rules the upstream plugin ships and the Solid 1.x
//! dialect exposes as `v1/<rule>`: JSX prop hygiene, handler conventions,
//! structural preferences, and the fine-grained decomposition of upstream's
//! monolithic `reactivity` rule. Everything here reads the same fact tables
//! the reactive engine reads — Oxc AST facts, resolved primitives, TypeScript
//! entities — never source text through a regex.
//!
//! # Which dialect runs which group
//!
//! [`check_file`] gates each submodule on the dialect version, mirroring
//! what the two catalogs declare: the ESLint-era surface (`syntax`,
//! `attributes`, `structure`, `imports`, `undef`) is 1.x vocabulary and runs
//! only there, while the decomposed `reactivity` rules describe defects in
//! both language versions and run for both (minus the one 1.x-only rule its
//! own gate documents). The version match here and the catalogs above cannot
//! drift silently: each dialect's solver panics on an emitted identity its
//! catalog does not resolve, and both rule crates' fixture suites execute
//! this pass.

mod attributes;
mod imports;
pub mod options;
mod reactivity;
mod structure;
mod syntax;
mod undef;

use std::collections::HashMap;

use crate::indexes::SemanticLookup;
use crate::{
    EntitySymbols, Fix, ReactiveSourceKind, StaticViolation, SymbolId, TextEdit, location,
};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::{Callability, Location};

/// The source text a span covers, or `""` when the span is not readable.
///
/// Every rule here locates its report by span and phrases it with the
/// author's own spelling, so this is the one shared way to get from one to
/// the other.
pub(super) fn text(file: &FileFacts, span: Span) -> &str {
    file.source_text(span).unwrap_or_default()
}

/// Whether a JSX tag names a DOM element rather than a component.
///
/// JSX's own rule: a lowercase-led tag is an intrinsic element, anything else
/// is a value reference. Rules about DOM attributes and listeners apply only
/// to the former.
pub(super) fn is_lowercase_led(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_lowercase())
}

/// Whether a type is provably an array or tuple, from its rendering plus the
/// checker's callability verdict for the same span. A screen, not a parser —
/// the rules consulting it treat a miss as "not proven", never as proof.
///
/// The rendering alone cannot settle a trailing `[]`: a *function* type
/// returning an array (`() => string[]`) ends the same way an array of
/// functions (`((n) => void)[]`) does. Callability is the discriminator —
/// TypeScript derives it from real call signatures, so the function is
/// `Callable` and the array is `NonCallable` — and only the `NonCallable`
/// verdict lets the suffix count.
pub(super) fn array_like_type(descriptor: &str, callability: Option<Callability>) -> bool {
    descriptor.starts_with('[')
        || descriptor.starts_with("readonly ")
        || descriptor.starts_with("Array<")
        || descriptor.starts_with("ReadonlyArray<")
        || (descriptor.ends_with("[]") && callability == Some(Callability::NonCallable))
}

/// The type TypeScript resolved for exactly this span, when the demand plan
/// asked for it there.
///
/// Exact-span deliberately, not smallest-contained: the smallest demanded
/// entity inside a call expression is its *callee*, and inside an arrow it
/// is some inner reference — either would answer for the wrong object. The
/// rules judging a whole expression's value must see that expression's own
/// type or nothing.
pub(super) fn expression_descriptor<'a>(
    context: &'a UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<&'a typefacts::TypeDescriptor> {
    context
        .lookup
        .entity_at(file.path.as_str(), span)
        .and_then(|entity| entity.type_descriptor.as_deref())
}

/// [`expression_descriptor`]'s callability twin: the checker's verdict for
/// exactly this span, derived from real call signatures, never from text.
pub(super) fn expression_callability(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<Callability> {
    context
        .lookup
        .entity_at(file.path.as_str(), span)
        .and_then(|entity| entity.callability)
}

// --- helpers shared by the rule modules ------------------------------------
//
// One copy each, here, so the documented guarantees cannot drift between
// modules: these used to be duplicated per file with the doc comments on only
// one of the copies.

/// Whether `outer` fully contains `inner`, byte-range-wise. Used to find the
/// object properties belonging to a `{...spread}` argument, and nothing
/// deeper: a nested object literal several levels down is not "contained" any
/// less by this test, which is exactly the same imprecision upstream's own
/// scope-walking has for a spread of a computed expression.
pub(super) fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// The declaration text of the nearest binding named `name` in this file, for
/// resolving a bare identifier back to its initializer.
///
/// A narrow, single-hop trace, first match in file order: it does not resolve
/// reassignment, shadowing, or which of several same-named bindings is in
/// scope at the use site. Acceptable for the rules that call it because the
/// result only ever loosens a stylistic judgement about what the value *looks
/// like* — it is never used to decide whether a name is defined at all, which
/// is exactly the judgement `undef.rs` refuses to make by hand, asking
/// TypeScript facts instead.
pub(super) fn binding_initializer<'a>(file: &'a FileFacts, name: &str) -> Option<(Span, &'a str)> {
    file.ast.bindings.iter().find_map(|binding| {
        binding
            .names
            .iter()
            .any(|candidate| text(file, candidate.span) == name)
            .then(|| binding.initializer.map(|span| (span, text(file, span))))
            .flatten()
    })
}

/// The static string an attribute value or expression resolves to, following
/// one level of local variable indirection and one level of `+`
/// concatenation. Not a general constant-folder: it is exactly the shape
/// upstream's own scope-based `getStaticValue` recovers for the common
/// patterns (a literal, a `const url = "..."`, or `"javascript:" +
/// something`), no more. [`literal_string_type`] complements it with the
/// values TypeScript proves.
pub(super) fn static_string_expression(file: &FileFacts, span: Span) -> Option<String> {
    let source = text(file, span).trim();
    if !source.contains('+')
        && let Some(value) = static_string(file, span)
    {
        return Some(unescape_common_sequences(&value));
    }
    if source
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
    {
        return binding_initializer(file, source)
            .and_then(|(initializer, _)| static_string_expression(file, initializer));
    }
    concat_plus_joined_literal(source)
}

fn unescape_common_sequences(value: &str) -> String {
    value
        .replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
}

/// Joins a `"a" + "b" + ...` chain of quoted literals into their concatenated
/// value, or `None` if any `+`-separated part is not itself a quoted literal
/// (including the degenerate case of no `+` at all).
pub(super) fn concat_plus_joined_literal(source: &str) -> Option<String> {
    let parts = source.split('+').map(str::trim).collect::<Vec<_>>();
    if parts.len() <= 1 {
        return None;
    }
    parts
        .into_iter()
        .map(|part| {
            let quote = part.as_bytes().first().copied()?;
            // A lone quote character passes the first==last comparison against
            // itself; the length gate keeps `"+"`-shaped attribute text (split
            // into single-quote parts) from slicing out of bounds.
            (part.len() >= 2
                && matches!(quote, b'\'' | b'"')
                && part.as_bytes().last().copied() == Some(quote))
            .then(|| part[1..part.len() - 1].to_owned())
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.concat())
}

/// The literal text of a quoted string span, unwrapping one optional
/// surrounding `{ ... }` JSX expression-container layer first (an
/// attribute's `value` span includes the braces when it came from
/// `attr={"literal"}` rather than `attr="literal"`).
pub(super) fn static_string(file: &FileFacts, span: Span) -> Option<String> {
    strip_string_literal(text(file, span))
}

pub(super) fn strip_string_literal(source: &str) -> Option<String> {
    let trimmed = source.trim();
    let trimmed = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map(str::trim)
        .unwrap_or(trimmed);
    let quote = trimmed.as_bytes().first().copied()?;
    if trimmed.len() < 2
        || !matches!(quote, b'\'' | b'"' | b'`')
        || trimmed.as_bytes().last().copied() != Some(quote)
    {
        return None;
    }
    Some(trimmed[1..trimmed.len() - 1].to_owned())
}

pub(super) fn fix_replace(
    file: &FileFacts,
    span: Span,
    message: impl Into<String>,
    new_text: impl Into<String>,
) -> Fix {
    Fix {
        message: message.into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), span),
            new_text: new_text.into(),
        }],
    }
}

/// Builds a static violation, filling in the boilerplate every rule shares
/// (identity, location, and an empty `analysis_context` — the rules that
/// need one, like `no-unknown-namespaces`, set it after the call).
pub(super) fn violation(
    file: &FileFacts,
    id: &str,
    rule: &str,
    message: impl Into<String>,
    hint: impl Into<String>,
    span: Span,
    fixes: Vec<Fix>,
) -> StaticViolation {
    StaticViolation {
        id: id.into(),
        rule: rule.into(),
        message: message.into(),
        hint: hint.into(),
        location: location(file.path.shared(), span),
        analysis_context: String::new(),
        fixes,
    }
}

/// The string value a type proves an expression to hold: a literal string
/// type (`"javascript:alert(1)"`) renders with its exact contents, so the
/// value is known wherever the binding was written — another file, an
/// inferred `const` — where a same-file text trace cannot follow. Exact-span
/// only: a literal-typed *operand* of a larger expression proves nothing
/// about the whole.
pub(super) fn literal_string_type(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<String> {
    let descriptor = expression_descriptor(context, file, span)?;
    let rendered = descriptor.text.as_ref();
    let inner = rendered.strip_prefix('"')?.strip_suffix('"')?;
    // TypeScript renders the literal with JSON-style escapes; decode the
    // common ones and refuse anything else rather than mis-decode it.
    let mut value = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        match characters.next() {
            Some('"') => value.push('"'),
            Some('\\') => value.push('\\'),
            Some('n') => value.push('\n'),
            Some('r') => value.push('\r'),
            Some('t') => value.push('\t'),
            _ => return None,
        }
    }
    Some(value)
}

/// Everything one file's upstream-compat checks may consult.
///
/// The reactive maps are the reason the decomposed `reactivity` rules can be
/// semantic rather than syntactic: upstream decides what is reactive from
/// naming conventions and call shapes, while these are the sources the engine
/// *proved*, through TypeScript symbol resolution, package contracts, and
/// cross-file propagation.
pub(super) struct UpstreamCompatContext<'a> {
    /// The vocabulary these rules are checking against. Consulted rather than
    /// assumed: which argument slot of a primitive is a tracked scope is a
    /// per-dialect fact, and the one place the two versions differ most.
    pub(super) dialect: &'a dyn solid_dialect::Dialect,
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    /// Proven reactive accessors, by symbol: display name and declaration.
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// Whether each proven source is an accessor or a store path.
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    /// Component props roots proven by component shape and propagated type
    /// facts. Member names may be unresolved (for example an inferred `any`),
    /// but the props object itself remains a reactive proxy.
    pub(super) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// The proven-source symbol at each exact TypeScript reference location,
    /// indexed path → byte range. Entity facts intentionally cover only
    /// semantically interesting expression shapes; ordinary operator operands
    /// can therefore have a symbol reference but no entity row. Rules that
    /// need exact identifier identity use this as the type-fact fallback
    /// rather than matching source text — indexed once here because scanning
    /// every source's reference list per identifier is quadratic in project
    /// size. Owned: the reference lists it is built from live shorter than
    /// this struct's borrows.
    pub(super) source_reference_index: HashMap<String, HashMap<(u64, u64), SymbolId>>,
    /// The imported symbols a package contract describes.
    ///
    /// Read as the negative: a callee absent from here, absent from the
    /// dialect's primitives, and with no body in the project is one whose
    /// reactive behaviour nothing in the analysis knows.
    pub(super) contracted: &'a HashMap<SymbolId, crate::contracts::ResolvedContractBinding>,
    /// Per-rule options, defaulted to upstream's defaults when the project
    /// carries no `.solid-checker/rule-options.json`. See [`options`].
    pub(super) options: &'a options::RuleOptions,
}

/// Runs every upstream-compat rule the dialect's catalog declares over one
/// file. See the module doc for the version/group table.
pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
) -> Vec<StaticViolation> {
    let mut violations = Vec::new();
    if context.dialect.version() == solid_dialect::Version::V1 {
        syntax::check_file(file, context, &mut violations);
        attributes::check_file(file, context, &mut violations);
        structure::check_file(file, context, &mut violations);
        imports::check_file(file, context, &mut violations);
        undef::check_file(file, context, &mut violations);
    }
    reactivity::check_file(file, context, &mut violations);
    violations
}

#[cfg(test)]
mod tests {
    use super::{concat_plus_joined_literal, strip_string_literal};

    #[test]
    fn strips_quotes_from_string_literals() {
        assert_eq!(strip_string_literal("\"hello\""), Some("hello".to_string()));
        assert_eq!(strip_string_literal("'hello'"), Some("hello".to_string()));
        assert_eq!(strip_string_literal("`hello`"), Some("hello".to_string()));
    }

    #[test]
    fn strips_an_expression_containers_braces_before_the_quotes() {
        assert_eq!(
            strip_string_literal("{ \"hello\" }"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_quoted_literal() {
        assert_eq!(strip_string_literal("foo"), None);
        assert_eq!(strip_string_literal("'unterminated"), None);
        assert_eq!(strip_string_literal(""), None);
    }

    #[test]
    fn joins_quoted_literal_concatenation() {
        assert_eq!(
            concat_plus_joined_literal("'java' + 'script:x'"),
            Some("javascript:x".to_string())
        );
    }

    #[test]
    fn refuses_to_join_when_any_part_is_not_a_literal() {
        assert_eq!(concat_plus_joined_literal("'java' + x"), None);
        assert_eq!(concat_plus_joined_literal("'just one'"), None);
    }

    #[test]
    fn survives_a_plus_adjacent_to_a_quote() {
        // Attribute text like `"+ Add"` splits into parts whose first is a
        // lone quote character; these must answer None, not slice `[1..0]`.
        assert_eq!(concat_plus_joined_literal("\"+ Add\""), None);
        assert_eq!(concat_plus_joined_literal("'+'"), None);
        assert_eq!(concat_plus_joined_literal("\"x+\""), None);
        assert_eq!(strip_string_literal("'"), None);
        assert_eq!(strip_string_literal("{ \" }"), None);
    }
}
