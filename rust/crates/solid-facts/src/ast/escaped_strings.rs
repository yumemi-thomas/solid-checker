//! The strings and templates a file writes with an escape that a check may read as a name (ADR 1152, facts schema 108).
//!
//! A string is a value, and its source text is the value's spelling: `"\n"` is a newline however a check reads it. A check that takes a string for a *name* (a property key, an
//! event name, an option's key, an import's name) compares the name against a literal, so a name written with an escape (`"cl@u0069ck"`, whose value is `click`) is a name the
//! check does not see. A name a check compares is a word: letters and digits with `_ $ - : .` among them (`click`, `__proto__`, `data-id`, `app:ready`). So this lists the
//! strings and the templates written with an escape whose whole value is such a word, with the value, wherever they stand (a key, an argument, an import's name): the position
//! says how a check reads one, and no position makes a text that is no word a name any check holds.
//!
//! A string whose value is no word (`"\n"`, `"a\tb"`, a sentence, one punctuation mark, text in another script) is a plain value: it is not listed. A template is listed only
//! when it has no substitution (a name written with one is dynamic and no check reads it as a name), and a tagged one is not (the tag reads its raw text). A JSX attribute's
//! string is not listed: JSX strings have no escapes, a backslash in one is a character.

use compact_str::CompactString;
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

use super::EscapedStringFact;
use crate::core::Span;

/// The escaped strings of a file, sorted by span.
pub(super) fn escaped_strings(semantic: &Semantic<'_>, source: &str) -> Vec<EscapedStringFact> {
    let nodes = semantic.nodes();
    let mut facts = Vec::new();
    for node in nodes.iter() {
        let (span, text, context) = match node.kind() {
            AstKind::StringLiteral(literal) => (
                literal.span,
                literal.value.as_str(),
                nodes.kind(nodes.parent_id(node.id())),
            ),
            AstKind::TemplateElement(piece) => {
                let template_id = nodes.parent_id(node.id());
                let AstKind::TemplateLiteral(template) = nodes.kind(template_id) else {
                    continue;
                };
                // A name written with a substitution is dynamic; the piece is one fragment of it.
                if !template.expressions.is_empty() {
                    continue;
                }
                let Some(cooked) = &piece.value.cooked else {
                    continue;
                };
                // The literal as written, backticks included: the span a check reads.
                (
                    template.span,
                    cooked.as_str(),
                    nodes.kind(nodes.parent_id(template_id)),
                )
            }
            _ => continue,
        };
        if matches!(
            context,
            AstKind::JSXAttribute(_) | AstKind::TaggedTemplateExpression(_)
        ) {
            continue;
        }
        if is_word(text)
            && source
                .get(span.start as usize..span.end as usize)
                .is_some_and(|raw| raw.contains('\\'))
        {
            facts.push(EscapedStringFact {
                span: Span::new(span.start, span.end),
                text: CompactString::from(text),
            });
        }
    }
    facts.sort_by_key(|fact| (fact.span.start, fact.span.end));
    facts.dedup_by_key(|fact| (fact.span.start, fact.span.end));
    facts
}

/// Whether `text` is one word: ASCII letters and digits with `_`, `$`, `-`, `:` and `.` among them, at least one of them a letter, a digit, `_` or `$`.
fn is_word(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-' | ':' | '.'))
        && text
            .chars()
            .any(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
}

#[cfg(test)]
mod tests {
    use crate::ast::extract;

    /// The listed strings, each as its source text and its value. In `source`, `@` stands for a backslash.
    fn strings(source: &str) -> Vec<(String, String)> {
        let source = source.replace('@', "\\");
        extract("app.tsx", &source)
            .expect("the source parses")
            .escaped_strings
            .iter()
            .map(|fact| {
                (
                    source[fact.span.start as usize..fact.span.end as usize].to_owned(),
                    fact.text.to_string(),
                )
            })
            .collect()
    }

    fn pair(raw: &str, text: &str) -> (String, String) {
        (raw.replace('@', "\\"), text.to_owned())
    }

    #[test]
    fn a_file_that_writes_no_escape_lists_nothing() {
        assert!(
            strings("el.addEventListener('click', f); const o = { 'k': 1 }; o['k']; f('a-b');")
                .is_empty()
        );
    }

    #[test]
    fn a_plain_value_is_not_listed() {
        for source in [
            "f('@n'); f('a@tb'); f(`@r@n`); f('say @'hi@'');",
            "f('caf@u00e9'); f('@u4e2d@u6587'); f('@u00a0'); f('@@');",
            "f('call set@u0041ttribute here');",
            "f('@u002d'); f('@x2e'); f('@u0000'); f('');",
            "const a = <a href=\"C:@path\" />;",
            "const t = tag`c@u006cick`; const u = `a${b}@u0063`;",
            // A key whose value is no word is a key no check holds a name for.
            "const o = { '@n': 1, '@u00e9': 2 }; o['@\\']; class K { '@t'() {} }",
        ] {
            assert_eq!(strings(source), Vec::new(), "{source}");
        }
    }

    #[test]
    fn a_string_that_is_one_word_is_listed_wherever_it_stands() {
        assert_eq!(
            strings(
                "el.addEventListener('c@u006cick', f); f(\"__pr@x6fto__\"); g(`data@x2did`); h('app@u003aready'); i('@evaluate');"
            ),
            [
                pair("'c@u006cick'", "click"),
                pair("\"__pr@x6fto__\"", "__proto__"),
                pair("`data@x2did`", "data-id"),
                pair("'app@u003aready'", "app:ready"),
                pair("'@evaluate'", "evaluate"),
            ]
        );
    }

    #[test]
    fn a_name_in_a_name_position_is_listed() {
        assert_eq!(
            strings(
                "const o = { 'a@u0062': 1, [`c@u0064`]: 2 }; o['x@u0079']; o[`z@u007a`]; \
                 class K { 'm@u0031'() {} 'f@u0032' = 1; } const { 'p@u0033': q } = o; \
                 type T = { 'n@u0034': number }; \
                 import { 'i@u0035' as j } from 'm@u006fdule'; export { j as 'e@u0036' };"
            )
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>(),
            [
                "ab", "cd", "xy", "zz", "m1", "f2", "p3", "n4", "i5", "module", "e6"
            ]
        );
    }

    #[test]
    fn a_value_that_is_a_word_is_listed_whether_or_not_it_is_a_key() {
        // A key written plain beside a value that is a sentence: nothing.
        assert!(strings("const o = { k: 'a@tb', 'm': 'x y@u0021' };").is_empty());
        // A value that is a word is listed: it may be a name.
        assert_eq!(
            strings("const o = { k: 'v@u0032' };"),
            [pair("'v@u0032'", "v2")]
        );
    }
}
