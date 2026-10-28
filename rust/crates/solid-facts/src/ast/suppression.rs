//! Suppression comments: the syntax of `eslint-disable` and its
//! `solid-checker-disable` twin.
//!
//! The grammar is ESLint's (`@eslint/plugin-kit`'s `parseDirective` and
//! `parseListConfig`), so a comment means the same thing to the checker as to
//! ESLint: a label, a comma-separated list of names, and an optional
//! justification after ` -- `. This module decides what is a suppression
//! comment and what it names; which rule a name denotes is not its business.

use oxc_ast::ast::Comment;

use super::{SuppressionAction, SuppressionCommentFact, SuppressionOrigin};
use crate::core::Span;
use compact_str::CompactString;

pub(super) fn suppression_comments(
    source: &str,
    comments: &[Comment],
) -> Vec<SuppressionCommentFact> {
    let mut facts: Vec<_> = comments
        .iter()
        .filter_map(|comment| {
            let content = comment.content_span();
            let text = source.get(content.start as usize..content.end as usize)?;
            let (origin, action, names) = parse_suppression(text, comment.is_line())?;
            let span = comment.span;
            Some(SuppressionCommentFact {
                span: Span::new(span.start, span.end),
                origin,
                action,
                block: comment.is_block(),
                names,
            })
        })
        .collect();
    facts.sort_by_key(|fact| fact.span);
    facts
}

/// Whether any comment reads as a TypeScript checking directive
/// (`AstFacts::typescript_directive_comment`).
pub(super) fn typescript_directive_comment(source: &str, comments: &[Comment]) -> bool {
    comments.iter().any(|comment| {
        let content = comment.content_span();
        source
            .get(content.start as usize..content.end as usize)
            .is_some_and(|text| {
                let text = text.trim_start_matches(|c: char| c.is_whitespace() || c == '*');
                ["@ts-nocheck", "@ts-ignore", "@ts-expect-error"]
                    .iter()
                    .any(|directive| text.starts_with(directive))
            })
    })
}

/// The suppression a comment's text (delimiters removed) spells, if it spells
/// one.
fn parse_suppression(
    text: &str,
    line_comment: bool,
) -> Option<(SuppressionOrigin, SuppressionAction, Vec<CompactString>)> {
    let (directive, _justification) = split_justification(text);
    let directive = trim(directive);
    let label = label_of(directive)?;
    let (origin, action) = classify(label)?;
    // ESLint reads a `//` comment only as a line-scoped directive.
    if line_comment
        && !matches!(
            action,
            SuppressionAction::DisableLine | SuppressionAction::DisableNextLine
        )
    {
        return None;
    }
    let names = directive[label.len()..]
        .split(',')
        .map(|name| unquote(trim(name)))
        .filter(|name| !name.is_empty())
        .map(CompactString::new)
        .collect();
    Some((origin, action, names))
}

fn classify(label: &str) -> Option<(SuppressionOrigin, SuppressionAction)> {
    let (origin, action) = if let Some(action) = label.strip_prefix("eslint-") {
        (SuppressionOrigin::Eslint, action)
    } else {
        (
            SuppressionOrigin::SolidChecker,
            label.strip_prefix("solid-checker-")?,
        )
    };
    let action = match action {
        "disable" => SuppressionAction::Disable,
        "enable" => SuppressionAction::Enable,
        "disable-line" => SuppressionAction::DisableLine,
        "disable-next-line" => SuppressionAction::DisableNextLine,
        _ => return None,
    };
    Some((origin, action))
}

/// JavaScript's `String.prototype.trim` whitespace: Unicode white space and
/// the byte order mark.
fn is_js_space(character: char) -> bool {
    character.is_whitespace() || character == '\u{feff}'
}

fn trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// Splits at the first match of `/\s-{2,}\s/`: the directive before it, the
/// justification after.
fn split_justification(text: &str) -> (&str, &str) {
    for (start, character) in text.char_indices() {
        if !is_js_space(character) {
            continue;
        }
        let rest = &text[start + character.len_utf8()..];
        let hyphens = rest.bytes().take_while(|byte| *byte == b'-').count();
        if hyphens >= 2
            && let Some(after) = rest[hyphens..].chars().next()
            && is_js_space(after)
        {
            let end = start + character.len_utf8() + hyphens + after.len_utf8();
            return (&text[..start], &text[end..]);
        }
    }
    (text, "")
}

/// `/^([a-z]+(?:-[a-z]+)*)(?:\s|$)/`: the leading run of lowercase words
/// joined by single hyphens, when whitespace or the end follows it.
fn label_of(directive: &str) -> Option<&str> {
    let bytes = directive.as_bytes();
    let mut end = 0;
    loop {
        let word = bytes[end..]
            .iter()
            .take_while(|byte| byte.is_ascii_lowercase())
            .count();
        if word == 0 {
            return None;
        }
        end += word;
        if bytes.get(end) == Some(&b'-') && bytes.get(end + 1).is_some_and(u8::is_ascii_lowercase) {
            end += 1;
        } else {
            break;
        }
    }
    let followed = directive[end..].chars().next();
    followed.is_none_or(is_js_space).then(|| &directive[..end])
}

/// Removes one pair of matching surrounding quotes, as ESLint's
/// `parseListConfig` does.
fn unquote(name: &str) -> &str {
    let bytes = name.as_bytes();
    match (bytes.first(), bytes.last()) {
        (Some(first @ (b'"' | b'\'')), Some(last)) if first == last && bytes.len() >= 2 => {
            &name[1..name.len() - 1]
        }
        _ => name,
    }
}

/// Line starts of one source text, with ESLint's line terminators: `\n`,
/// `\r\n`, a lone `\r`, U+2028 and U+2029. A suppression comment's line is
/// ESLint's line for it, so a finding's line has to be counted the same way
/// to be compared with it.
#[derive(Clone, Debug)]
pub struct LineTable {
    starts: Vec<u32>,
}

impl LineTable {
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0];
        let mut chars = source.char_indices().peekable();
        while let Some((index, character)) = chars.next() {
            let next = match character {
                '\r' if chars.peek().is_some_and(|(_, next)| *next == '\n') => {
                    chars.next();
                    index + 2
                }
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => index + character.len_utf8(),
                _ => continue,
            };
            starts.push(u32::try_from(next).unwrap_or(u32::MAX));
        }
        Self { starts }
    }

    /// The one-based line a byte offset falls on.
    #[must_use]
    pub fn line(&self, offset: u32) -> u32 {
        let line = self.starts.partition_point(|start| *start <= offset);
        u32::try_from(line).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(
        text: &str,
        line: bool,
    ) -> Option<(SuppressionOrigin, SuppressionAction, Vec<String>)> {
        parse_suppression(text, line).map(|(origin, action, names)| {
            (
                origin,
                action,
                names.iter().map(|n| n.to_string()).collect(),
            )
        })
    }

    #[test]
    fn labels_follow_eslints_directive_grammar() {
        use SuppressionAction::*;
        use SuppressionOrigin::*;
        assert_eq!(
            parse(" eslint-disable-next-line solid-checker/a, solid/b ", true),
            Some((
                Eslint,
                DisableNextLine,
                vec!["solid-checker/a".into(), "solid/b".into()]
            ))
        );
        assert_eq!(
            parse("solid-checker-disable", false),
            Some((SolidChecker, Disable, vec![]))
        );
        assert_eq!(
            parse("eslint-enable 'a' , \"b\" ,, ", false),
            Some((Eslint, Enable, vec!["a".into(), "b".into()]))
        );
        // A justification is not a name.
        assert_eq!(
            parse("eslint-disable-line a -- because", true),
            Some((Eslint, DisableLine, vec!["a".into()]))
        );
        assert_eq!(
            parse("eslint-disable-line a --because", true),
            Some((Eslint, DisableLine, vec!["a --because".into()]))
        );
        // Names are comma-separated: a space does not split.
        assert_eq!(
            parse("eslint-disable a b", false),
            Some((Eslint, Disable, vec!["a b".into()]))
        );
        // A line comment carries only the line-scoped forms.
        assert_eq!(parse("eslint-disable a", true), None);
        assert_eq!(parse("solid-checker-enable a", true), None);
        assert!(parse("solid-checker-disable-line a", true).is_some());
        // Not a label: no whitespace after, unknown action, another word.
        assert_eq!(parse("eslint-disable-nextline a", false), None);
        assert_eq!(parse("eslint-disable- a", false), None);
        assert_eq!(parse("eslint-disabled a", false), None);
        assert_eq!(parse("eslint-env browser", false), None);
        assert_eq!(parse("solid-checker-disable-next-line-x", true), None);
        assert_eq!(parse("a eslint-disable", false), None);
        assert_eq!(parse("", false), None);
        assert_eq!(parse("eslint-disable\ta", false).unwrap().2, ["a"]);
    }

    #[test]
    fn lines_break_where_eslint_breaks_them() {
        let table = LineTable::new("a\nb\r\nc\rd\u{2028}e\u{2029}f");
        let lines: Vec<u32> = [0, 1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 15]
            .iter()
            .map(|offset| table.line(*offset))
            .collect();
        assert_eq!(lines, [1, 1, 2, 2, 2, 3, 3, 4, 4, 5, 5, 6]);
    }
}
