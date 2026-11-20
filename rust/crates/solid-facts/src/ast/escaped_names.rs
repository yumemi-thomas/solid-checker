//! The names a file writes with an escape, by span (ADR 1152, facts schema 108).
//!
//! An identifier written `ev\u0061l` is the name `eval`: ECMAScript reads the identifier's StringValue, and every check of the checker that compares a name, takes its first
//! letter or looks it up in a list asks for the name, where the source text of its span is the spelling. So the facts hold, for each span that is one identifier written with an
//! escape, and for each chain of names that holds one (`window.add\u0045ventListener`, `a?.b`), the text the program means. A string or a template is a literal, read as
//! one: it is no span here (`escaped_strings` lists those a check may read as a name).
//!
//! The chain is the source text with each escaped identifier replaced by its name, so it equals what the same chain written without escapes is, to the character; a chain that holds
//! anything but names (a call, a string, an index) is no chain here.

use compact_str::CompactString;
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

use super::EscapedNameFact;
use crate::core::Span;

/// The escaped names of a file, sorted by span.
pub(super) fn escaped_names(semantic: &Semantic<'_>, source: &str) -> Vec<EscapedNameFact> {
    let text = |span: oxc_span::Span| source.get(span.start as usize..span.end as usize);
    let mut names: Vec<EscapedNameFact> = Vec::new();
    for node in semantic.nodes().iter() {
        let (span, name) = match node.kind() {
            AstKind::IdentifierName(identifier) => (identifier.span, identifier.name.as_str()),
            AstKind::IdentifierReference(identifier) => (identifier.span, identifier.name.as_str()),
            AstKind::BindingIdentifier(identifier) => (identifier.span, identifier.name.as_str()),
            AstKind::LabelIdentifier(identifier) => (identifier.span, identifier.name.as_str()),
            // A private name's span holds its `#`; the name the program means holds it too.
            AstKind::PrivateIdentifier(identifier) => {
                if text(identifier.span).is_some_and(|raw| raw.contains('\\')) {
                    names.push(EscapedNameFact {
                        span: Span::new(identifier.span.start, identifier.span.end),
                        name: CompactString::from(format!("#{}", identifier.name)),
                    });
                }
                continue;
            }
            _ => continue,
        };
        if text(span).is_some_and(|raw| raw.contains('\\')) {
            names.push(EscapedNameFact {
                span: Span::new(span.start, span.end),
                name: CompactString::from(name),
            });
        }
    }
    sort(&mut names);
    let identifiers = names.clone();
    for node in semantic.nodes().iter() {
        let AstKind::StaticMemberExpression(member) = node.kind() else {
            continue;
        };
        let Some(raw) = text(member.span).filter(|raw| raw.contains('\\')) else {
            continue;
        };
        if let Some(chain) = chain(raw, member.span.start, &identifiers) {
            names.push(EscapedNameFact {
                span: Span::new(member.span.start, member.span.end),
                name: chain,
            });
        }
    }
    sort(&mut names);
    names
}

fn sort(names: &mut Vec<EscapedNameFact>) {
    names.sort_by_key(|fact| (fact.span.start, fact.span.end));
    names.dedup_by_key(|fact| (fact.span.start, fact.span.end));
}

/// `raw`, the source of a member chain starting at `base`, with each escaped identifier inside it replaced by its name; `None` when what remains is more than names and the
/// dots and `?` between them (a call, a string, an index, a private name, a type assertion: the text is no chain).
fn chain(raw: &str, base: u32, identifiers: &[EscapedNameFact]) -> Option<CompactString> {
    let end = base + u32::try_from(raw.len()).ok()?;
    let first = identifiers.partition_point(|fact| fact.span.start < base);
    let mut out = String::with_capacity(raw.len());
    let mut copied = 0_usize;
    for fact in identifiers[first..]
        .iter()
        .take_while(|fact| fact.span.end <= end)
    {
        let from = (fact.span.start - base) as usize;
        let to = (fact.span.end - base) as usize;
        out.push_str(raw.get(copied..from)?);
        out.push_str(&fact.name);
        copied = to;
    }
    out.push_str(raw.get(copied..)?);
    out.chars()
        .all(|c| {
            c.is_ascii_alphanumeric()
                || c == '_'
                || c == '$'
                || c == '.'
                || c == '?'
                || c.is_whitespace()
        })
        .then(|| CompactString::from(out))
}

#[cfg(test)]
mod tests {
    use crate::ast::extract;

    fn names(source: &str) -> Vec<(String, String)> {
        let facts = extract("app.tsx", source).expect("the source parses");
        facts
            .escaped_names
            .iter()
            .map(|fact| {
                (
                    source[fact.span.start as usize..fact.span.end as usize].to_owned(),
                    fact.name.to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn a_file_that_writes_no_escape_lists_nothing() {
        assert!(names("window.addEventListener('click', f); const x = a?.b.c;").is_empty());
    }

    #[test]
    fn an_identifier_written_with_an_escape_is_its_name() {
        assert_eq!(
            names("ev\\u0061l(x);"),
            [("ev\\u0061l".to_owned(), "eval".to_owned())]
        );
        assert_eq!(
            names("const \\u{4D}isuse = 1; use(\\u{4D}isuse);"),
            [
                ("\\u{4D}isuse".to_owned(), "Misuse".to_owned()),
                ("\\u{4D}isuse".to_owned(), "Misuse".to_owned())
            ]
        );
        // A key, a pattern's key and a label are identifiers too.
        for source in [
            "({ k\\u0065y: 1 });",
            "const { k\\u0065y } = o;",
            "k\\u0065y: for (;;) { break k\\u0065y; }",
        ] {
            assert!(
                names(source).iter().all(|(_, name)| name == "key"),
                "{source}"
            );
            assert!(!names(source).is_empty(), "{source}");
        }
        // A member's name is one, and the chain that holds it is another span.
        assert_eq!(
            names("o.k\\u0065y;"),
            [
                ("o.k\\u0065y".to_owned(), "o.key".to_owned()),
                ("k\\u0065y".to_owned(), "key".to_owned())
            ]
        );
    }

    #[test]
    fn a_private_name_written_with_an_escape_is_its_name_with_its_hash() {
        let listed = names("class A { #p\\u0061 = 1; m() { return this.#p\\u0061; } }");
        assert_eq!(listed.len(), 2, "{listed:?}");
        assert!(
            listed
                .iter()
                .all(|(raw, name)| raw == "#p\\u0061" && name == "#pa")
        );
        // A private name written plain lists nothing.
        assert!(names("class A { #pa = 1; m() { return this.#pa; } }").is_empty());
    }

    #[test]
    fn a_chain_that_holds_an_escape_is_its_text_without_it() {
        let chain = names("window.add\\u0045ventListener('click', f);");
        assert!(chain.contains(&(
            "window.add\\u0045ventListener".to_owned(),
            "window.addEventListener".to_owned()
        )));
        assert!(chain.contains(&(
            "add\\u0045ventListener".to_owned(),
            "addEventListener".to_owned()
        )));
        let optional = names("a?.b\\u0063.d;");
        assert!(optional.contains(&("a?.b\\u0063.d".to_owned(), "a?.bc.d".to_owned())));
        assert!(optional.contains(&("a?.b\\u0063".to_owned(), "a?.bc".to_owned())));
        // The spacing of the source stays (it equals what the unescaped chain's text is).
        assert!(names("O\\u0062ject . assign(a);").contains(&(
            "O\\u0062ject . assign".to_owned(),
            "Object . assign".to_owned()
        )));
    }

    #[test]
    fn a_text_that_is_more_than_names_is_no_chain() {
        for source in [
            "f(ev\\u0061l).g;",
            "o['a\\u0062'].c;",
            "(a as any).b\\u0063;",
            "o.#p\\u0061;",
        ] {
            let chains = names(source);
            assert!(
                chains.iter().all(|(raw, _)| !raw.contains('(')
                    && !raw.contains('[')
                    && !raw.contains(" as ")),
                "{source}: {chains:?}"
            );
        }
        // A string is a literal: no span is listed for it.
        assert!(names("f('ev\\u0061l', `x\\u0079`);").is_empty());
    }
}
