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
mod upstream_data;

use std::collections::{HashMap, HashSet};

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

/// The top-level properties of the object literal `expression` is, or `None`
/// when the expression is not itself an object literal.
///
/// [`Span::contains`] alone cannot answer this: containment is transitive —
/// a node nested several levels down still satisfies it — so rules that must
/// see only an object literal's *own* entries go through this filter.
///
/// Upstream reads `ObjectExpression.properties` — the node's own entries.
/// The fact tables record every object property in the file flat, so "own"
/// is recovered in two steps: the expression's text (modulo wrapping
/// parentheses) must be the `{ ... }` literal itself, and a property nested
/// inside another contained property's key or value is someone else's.
pub(super) fn direct_object_literal_properties(
    file: &FileFacts,
    expression: Span,
) -> Option<Vec<&solid_facts::ast::ObjectPropertyFact>> {
    let mut source = text(file, expression).trim();
    while source.starts_with('(') && entire_delimited(source, '(', ')') {
        source = source[1..source.len() - 1].trim();
    }
    if !source.starts_with('{') || !entire_delimited(source, '{', '}') {
        return None;
    }
    let inside = file
        .ast
        .object_properties
        .iter()
        .filter(|property| expression.contains(property.span))
        .collect::<Vec<_>>();
    Some(
        inside
            .iter()
            .filter(|property| {
                !inside.iter().any(|other| {
                    other.span != property.span
                        && (other.value.contains(property.span)
                            || other.key.contains(property.span))
                })
            })
            .copied()
            .collect(),
    )
}

/// Whether the first delimiter closes at the end of `source` rather than at
/// some earlier point. Quotes and templates are skipped; unsupported lexical
/// shapes conservatively answer false through unbalanced depth, withholding a
/// fix rather than manufacturing an object-literal proof.
fn entire_delimited(source: &str, open: char, close: char) -> bool {
    if !source.starts_with(open) || !source.ends_with(close) {
        return false;
    }
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"' | '`') {
            quote = Some(character);
            continue;
        }
        if character == open {
            depth += 1;
        } else if character == close {
            let Some(next) = depth.checked_sub(1) else {
                return false;
            };
            depth = next;
            if depth == 0 {
                return index + character.len_utf8() == source.len();
            }
        }
    }
    false
}

/// The declaration text of the binding an exact identifier reference resolves
/// to, using TypeScript's canonical symbol rather than its spelling.
///
/// The reverse binding index includes local declarations, cross-file aliases,
/// and every compiler-proven reference. Parameters and shadowed names resolve
/// to their own symbols and therefore cannot fall through to an unrelated
/// same-spelled initializer.
pub(super) fn binding_initializer<'a>(
    context: &UpstreamCompatContext<'a>,
    file: &FileFacts,
    reference: Span,
) -> Option<(&'a FileFacts, Span, &'a str, SymbolId)> {
    let (binding_file, binding, symbol) = context
        .lookup
        .binding_at_reference(file.path.as_str(), reference)?;
    let initializer = binding.initializer?;
    Some((
        binding_file,
        initializer,
        text(binding_file, initializer),
        symbol,
    ))
}

/// Whether an exact identifier reference resolves to one of this file's
/// imports. Comparing canonical symbols keeps a parameter that shadows an
/// import from inheriting the import's treatment merely by sharing its name.
pub(super) fn is_import_reference(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    reference: Span,
) -> bool {
    let Some(symbol) = context.entities.at(file.path.as_str(), reference) else {
        return false;
    };
    file.ast.imports.iter().any(|import| {
        import.bindings.iter().any(|binding| {
            context.entities.at(file.path.as_str(), binding.local.span) == Some(symbol)
        })
    })
}

/// The static string an attribute value or expression resolves to, following
/// compiler-resolved local variable indirection and one level of `+`
/// concatenation. Not a general constant-folder: it is exactly the shape
/// upstream's own scope-based `getStaticValue` recovers for the common
/// patterns (a literal, a `const url = "..."`, or `"javascript:" +
/// something`), no more. [`literal_string_type`] complements it with the
/// values TypeScript proves.
pub(super) fn static_string_expression(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<String> {
    static_string_expression_inner(context, file, span, &mut HashSet::new())
}

fn static_string_expression_inner(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
    visiting: &mut HashSet<SymbolId>,
) -> Option<String> {
    let source = text(file, span).trim();
    if let Some(value) = decode_string_literal(source) {
        return Some(value);
    }
    if source
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
    {
        let (binding_file, initializer, _, symbol) = binding_initializer(context, file, span)?;
        if !visiting.insert(symbol.clone()) {
            return None;
        }
        let value = static_string_expression_inner(context, binding_file, initializer, visiting);
        visiting.remove(&symbol);
        return value;
    }
    concat_plus_joined_literal(source)
}

/// Joins a `"a" + "b" + ...` chain of quoted literals into their concatenated
/// value, or `None` unless the whole expression is quoted literals joined by
/// `+` (including the degenerate case of a single literal with no `+` at
/// all). The chain is read literal-by-literal rather than split on `+`, so a
/// `+` *inside* a literal (`'javascript:' + 'a+b'`) folds correctly instead
/// of splitting the literal apart.
pub(super) fn concat_plus_joined_literal(source: &str) -> Option<String> {
    let mut rest = source.trim_start();
    let mut value = String::new();
    let mut parts = 0_usize;
    loop {
        // The next token must be one whole quoted literal —
        // [`decode_string_literal`] carries the syntax validation and escape
        // decoding. Backtick parts are refused: `a` + `b` folds fine at
        // runtime, but upstream's folder never joins templates.
        if !rest.starts_with(['\'', '"']) {
            return None;
        }
        let end = quoted_literal_end(rest)?;
        value.push_str(&decode_string_literal(&rest[..end])?);
        parts += 1;
        rest = rest[end..].trim_start();
        if rest.is_empty() {
            break;
        }
        rest = rest.strip_prefix('+')?.trim_start();
    }
    (parts > 1).then_some(value)
}

/// The byte length of the quoted string literal `source` starts with — the
/// index one past its closing quote — honouring backslash escapes. Bytes are
/// compared directly: the quote and backslash are ASCII, so a multi-byte
/// character inside the literal can never be mistaken for either.
fn quoted_literal_end(source: &str) -> Option<usize> {
    let mut bytes = source.bytes().enumerate();
    let (_, quote) = bytes.next()?;
    let mut escaped = false;
    for (index, byte) in bytes {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(index + 1);
        }
    }
    None
}

/// The literal text of a quoted string span, unwrapping one optional
/// surrounding `{ ... }` JSX expression-container layer first (an
/// attribute's `value` span includes the braces when it came from
/// `attr={"literal"}` rather than `attr="literal"`).
pub(super) fn static_string(file: &FileFacts, span: Span) -> Option<String> {
    strip_string_literal(text(file, span))
}

pub(super) fn strip_string_literal(source: &str) -> Option<String> {
    string_literal_body(source).map(|(_, inside)| inside.to_owned())
}

fn string_literal_body(source: &str) -> Option<(u8, &str)> {
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
    let inside = &trimmed[1..trimmed.len() - 1];
    // Matching first and last quotes prove nothing if the text between them
    // leaves the literal: `'a' === x ? f : 'b'` starts and ends with a quote
    // but is a conditional, and `` `x${y}` `` is a template whose value is
    // not its text. An unescaped inner quote, or an interpolation in a
    // template, disqualifies the span — the same answer upstream's constant
    // folder gives for both.
    let mut escaped = false;
    for byte in inside.bytes() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return None;
        }
    }
    if quote == b'`' && inside.contains("${") {
        return None;
    }
    Some((quote, inside))
}

fn decode_string_literal(source: &str) -> Option<String> {
    let (_, inside) = string_literal_body(source)?;
    let mut decoded = String::with_capacity(inside.len());
    let mut characters = inside.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let escaped = characters.next()?;
        match escaped {
            '\'' => decoded.push('\''),
            '"' => decoded.push('"'),
            '`' => decoded.push('`'),
            '\\' => decoded.push('\\'),
            'b' => decoded.push('\u{0008}'),
            'f' => decoded.push('\u{000c}'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'v' => decoded.push('\u{000b}'),
            '0' if characters.peek().is_none_or(|next| !next.is_ascii_digit()) => {
                decoded.push('\0');
            }
            'x' => decoded.push(char::from_u32(read_hex_escape(&mut characters, 2)?)?),
            'u' if characters.peek() == Some(&'{') => {
                characters.next();
                let mut digits = String::new();
                let mut closed = false;
                for digit in characters.by_ref() {
                    if digit == '}' {
                        closed = true;
                        break;
                    }
                    if !digit.is_ascii_hexdigit() || digits.len() == 6 {
                        return None;
                    }
                    digits.push(digit);
                }
                if digits.is_empty() || !closed {
                    return None;
                }
                decoded.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
            }
            'u' => decoded.push(char::from_u32(read_hex_escape(&mut characters, 4)?)?),
            '\n' => {}
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
            }
            '\u{2028}' | '\u{2029}' => {}
            // ECMAScript identity escapes evaluate to the escaped character.
            other if !other.is_ascii_digit() => decoded.push(other),
            _ => return None,
        }
    }
    Some(decoded)
}

fn read_hex_escape(
    characters: &mut std::iter::Peekable<std::str::Chars<'_>>,
    length: usize,
) -> Option<u32> {
    let mut value = 0_u32;
    for _ in 0..length {
        value = value.checked_mul(16)? + characters.next()?.to_digit(16)?;
    }
    Some(value)
}

/// Widens a deletion span leftward over the whitespace that separated the
/// deleted text from what precedes it, so removing a JSX attribute leaves
/// `<div id="a"/>` rather than `<div  id="a"/>`. Purely byte-wise and ASCII:
/// a multi-byte character can never end in an ASCII whitespace byte, so the
/// walk cannot stop inside one.
pub(super) fn deletion_with_leading_whitespace(source: &str, span: Span) -> Span {
    let bytes = source.as_bytes();
    let mut start = span.start as usize;
    if start > bytes.len() {
        return span;
    }
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    Span::new(u32::try_from(start).unwrap_or(span.start), span.end)
}

/// Widens a deletion span leftward over the `,` separator (and the
/// whitespace on either side of it) that joined the deleted text to the
/// previous list item, so removing a call's last argument leaves `f(a)`
/// rather than `f(a, )`. When what precedes is not a comma — a comment, an
/// opening parenthesis — the span is returned unchanged rather than guess at
/// a byte that is not the separator.
pub(super) fn deletion_with_leading_comma(source: &str, span: Span) -> Span {
    let bytes = source.as_bytes();
    let mut start = span.start as usize;
    if start > bytes.len() {
        return span;
    }
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    if start == 0 || bytes[start - 1] != b',' {
        return span;
    }
    start -= 1;
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    Span::new(u32::try_from(start).unwrap_or(span.start), span.end)
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

/// The whole-project compat pass. Both dialects run it: the decomposed
/// `reactivity` rules apply to both language versions, while the 1.x-only
/// ESLint-era groups are gated inside [`check_file`], next to the catalogs'
/// version table it mirrors.
///
/// The location-keyed reference map is a pure function of the TypeScript
/// table and the proven accessor set, and `reusable` is exactly the
/// condition under which both are unchanged (the same gate the
/// interprocedural results are reused behind). The retained map moves
/// through the compat context and back into its slot rather than being
/// cloned: the context owns the field, and the rules only ever read it.
pub(crate) fn check_project(
    ctx: &crate::AnalysisContext<'_>,
    mut reference_slot: Option<&mut Option<crate::SourceReferenceLocations>>,
    reusable: bool,
    draft: &mut crate::ProgramDraft,
) {
    let retained = reference_slot.as_deref_mut().and_then(|slot| {
        if reusable {
            slot.take()
        } else {
            *slot = None;
            None
        }
    });
    let context = UpstreamCompatContext {
        dialect: ctx.dialect,
        lookup: ctx.semantic_lookup,
        entities: ctx.entities,
        accessors: ctx.accessors,
        source_kinds: ctx.source_kinds,
        prop_sources: ctx.prop_sources,
        source_reference_index: retained.unwrap_or_else(|| {
            crate::symbols::source_reference_locations(
                &ctx.facts.typescript,
                ctx.symbols_by_root,
                ctx.accessors.keys(),
            )
        }),
        contracted: ctx.contracted,
        options: ctx.rule_options,
    };
    draft.static_violations.extend(
        crate::parallel_file_results(&ctx.facts.files, |file| check_file(file, &context))
            .into_iter()
            .flatten(),
    );
    if let Some(slot) = reference_slot {
        *slot = Some(context.source_reference_index);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        concat_plus_joined_literal, decode_string_literal, deletion_with_leading_comma,
        deletion_with_leading_whitespace, entire_delimited, strip_string_literal,
    };
    use solid_facts::core::Span;

    /// What a deletion fix's output would be: the span's text removed.
    fn delete(source: &str, span: Span) -> String {
        format!(
            "{}{}",
            &source[..span.start as usize],
            &source[span.end as usize..]
        )
    }

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
    fn joins_literals_that_themselves_contain_a_plus() {
        // Split-on-`+` would shear `'a+b'` in half; the literal-by-literal
        // read folds it whole.
        assert_eq!(
            concat_plus_joined_literal("'javascript:' + 'a+b'"),
            Some("javascript:a+b".to_string())
        );
        assert_eq!(
            concat_plus_joined_literal("'+' + '+'"),
            Some("++".to_string())
        );
    }

    #[test]
    fn refuses_chains_with_a_non_plus_joiner_or_trailing_operand() {
        assert_eq!(concat_plus_joined_literal("'a' 'b'"), None);
        assert_eq!(concat_plus_joined_literal("'a' + 'b' +"), None);
        assert_eq!(concat_plus_joined_literal("'a' === x ? f : 'b'"), None);
    }

    #[test]
    fn argument_deletion_swallows_the_separating_comma() {
        let source = "createEffect(fn, [a])";
        let span = Span::new(17, 20); // `[a]`
        assert_eq!(
            delete(source, deletion_with_leading_comma(source, span)),
            "createEffect(fn)"
        );
        let spaced = "createEffect(fn ,  [a])";
        let span = Span::new(19, 22); // `[a]`
        assert_eq!(
            delete(spaced, deletion_with_leading_comma(spaced, span)),
            "createEffect(fn)"
        );
    }

    #[test]
    fn comma_swallowing_declines_when_no_comma_precedes() {
        // A comment (or anything else) between the comma and the deleted
        // text: the span comes back unchanged rather than eat a byte that
        // is not the separator.
        let source = "createEffect(fn, /* deps */ [a])";
        let span = Span::new(28, 31); // `[a]`
        assert_eq!(deletion_with_leading_comma(source, span), span);
    }

    #[test]
    fn attribute_deletion_swallows_the_separating_whitespace() {
        let source = "<div key={x} />";
        let span = Span::new(5, 12); // `key={x}`
        assert_eq!(
            delete(source, deletion_with_leading_whitespace(source, span)),
            "<div />"
        );
        let multiline = "<div\n  key={x}\n/>";
        let span = Span::new(7, 14); // `key={x}`
        assert_eq!(
            delete(multiline, deletion_with_leading_whitespace(multiline, span)),
            "<div\n/>"
        );
    }

    #[test]
    fn decodes_ecmascript_hex_and_unicode_escapes() {
        assert_eq!(
            decode_string_literal(r#"'\x6a\u0061\u{76}ascript:'"#),
            Some("javascript:".to_string())
        );
        assert_eq!(
            concat_plus_joined_literal(r#"'\u006aava' + 'script:'"#),
            Some("javascript:".to_string())
        );
    }

    #[test]
    fn delimiter_proof_rejects_separate_parenthesized_expressions() {
        assert!(entire_delimited("({ __html: html })", '(', ')'));
        assert!(!entire_delimited("({ __html: html }) + ({})", '(', ')'));
        assert!(!entire_delimited("{ __html: html } + {}", '{', '}'));
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
