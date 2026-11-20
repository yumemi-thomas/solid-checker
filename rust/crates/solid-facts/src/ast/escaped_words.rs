//! The words a file spells with an escape (ADR 1148 round 2, facts schema 107).
//!
//! A scan of the source text for a name (the DOM's `setAttribute`, `attributes`, `evaluate`, the key `type`) cannot see the name written with an escape: the parser decodes
//! `document.ev\u0061luate`, `el["\u0061ttributes"]`, `` `ev\u0061luate` ``, `"\evaluate"` (an escape of a character that needs none) and `"eval\<newline>uate"` (a line
//! continuation) to the very name the scan looks for, and none of them holds it as text. Every consumer that matches a name by its spelling has to ask the decoded one too.
//!
//! This lists, for each identifier, string literal and template piece whose source text holds a backslash, the words of its decoded text: the runs of characters an
//! identifier is made of (the unit a word-for-word scan of the source matches), so a name inside a longer text (`"call set\u0041ttribute"`) is found as the scan finds it in the
//! source. A word the file writes without an escape is the source text's own and is not listed: a file that writes no escape lists nothing, and a file whose escapes decode
//! to no word (`"\n"`) lists nothing for them. The IR decides what a word means; no list of names is held here.

use std::collections::BTreeSet;

use compact_str::CompactString;
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

/// Whether `character` is part of a word: the characters an identifier is made of (a scan of source text for a name matches whole words by this).
fn word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

/// The words of the decoded text of every identifier, string literal and template piece in the file that is written with an escape, sorted.
pub(super) fn escaped_words(semantic: &Semantic<'_>, source: &str) -> Vec<CompactString> {
    let mut words = BTreeSet::new();
    let mut add = |decoded: &str| {
        for word in decoded
            .split(|character| !word_character(character))
            .filter(|word| !word.is_empty())
        {
            words.insert(CompactString::from(word));
        }
    };
    // An identifier's or a string's source text holds a backslash exactly where an escape is written; the node's own value is the decoded one.
    let escaped = |span: oxc_span::Span| {
        source
            .get(span.start as usize..span.end as usize)
            .is_some_and(|text| text.contains('\\'))
    };
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::IdentifierName(name) if escaped(name.span) => add(name.name.as_str()),
            AstKind::IdentifierReference(name) if escaped(name.span) => add(name.name.as_str()),
            AstKind::BindingIdentifier(name) if escaped(name.span) => add(name.name.as_str()),
            AstKind::LabelIdentifier(name) if escaped(name.span) => add(name.name.as_str()),
            AstKind::StringLiteral(literal) if escaped(literal.span) => {
                add(literal.value.as_str());
            }
            AstKind::TemplateElement(piece) if piece.value.raw.contains('\\') => {
                // A piece whose escape is invalid has no cooked text (a tagged template's): its raw text is what the tag reads.
                if let Some(cooked) = &piece.value.cooked {
                    add(cooked.as_str());
                }
            }
            _ => {}
        }
    }
    words.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use crate::ast::extract;

    fn words(source: &str) -> Vec<String> {
        extract("app.tsx", source)
            .expect("the source parses")
            .escaped_words
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_file_that_writes_no_escape_lists_nothing() {
        for source in [
            "document.evaluate(x).singleNodeValue; el.setAttribute('type', 'text'); el.type = `text`;",
            "const attributes = el['attributes'];",
            // A regular expression's backslash is no identifier's or string's.
            "const r = /\\u0061ttributes/; r.test(x);",
            // A comment's is no code.
            "// ev\\u0061luate\nconst a = 1;",
        ] {
            assert_eq!(words(source), Vec::<String>::new(), "{source}");
        }
    }

    #[test]
    fn an_identifier_written_with_an_escape_is_listed_decoded() {
        // A property name (a member's, an object key's) is an `IdentifierName`.
        assert_eq!(words("document.ev\\u0061luate(x);"), ["evaluate"]);
        assert_eq!(words("el.\\u{61}ttributes;"), ["attributes"]);
        assert_eq!(words("({ ev\\u0061luate: 1 });"), ["evaluate"]);
        assert_eq!(words("el.t\\u0079pe = 'text';"), ["type"]);
        // A name read is an `IdentifierReference`, a name bound a `BindingIdentifier`, a label a `LabelIdentifier`: each alone.
        assert_eq!(words("ev\\u0061luate(x);"), ["evaluate"]);
        assert_eq!(words("let ev\\u0061luate;"), ["evaluate"]);
        assert_eq!(words("ev\\u0061luate: for (;;) { break; }"), ["evaluate"]);
        // A destructured name is a key and a binding.
        assert_eq!(words("const { ev\\u0061luate } = document;"), ["evaluate"]);
    }

    #[test]
    fn a_string_or_template_written_with_an_escape_is_listed_decoded() {
        assert_eq!(words("el['\\u0061ttributes'];"), ["attributes"]);
        assert_eq!(words("el[\"\\x61ttributes\"];"), ["attributes"]);
        assert_eq!(words("el[`ev\\u0061luate`];"), ["evaluate"]);
        // An escape of a character that needs none, and a line continuation, decode to the name too.
        assert_eq!(words("el['\\evaluate'];"), ["evaluate"]);
        assert_eq!(words("el['eval\\\nuate'];"), ["evaluate"]);
        assert_eq!(words("el[`eval\\\nuate`];"), ["evaluate"]);
        // A name inside a longer text is a word of it.
        assert_eq!(
            words("f('call set\\u0041ttribute here');"),
            ["call", "here", "setAttribute"]
        );
        // A template's pieces around a substitution are read each.
        assert_eq!(words("el[`s${a}\\u0041ttr`];"), ["Attr"]);
    }

    #[test]
    fn an_escape_that_decodes_to_no_word_lists_nothing() {
        assert_eq!(words("f('a\\nb', '\\t', `\\u00a0`);"), ["a", "b"]);
        assert_eq!(words("f('\\n', `\\t\\r`, '\\\\');"), Vec::<String>::new());
    }
}
