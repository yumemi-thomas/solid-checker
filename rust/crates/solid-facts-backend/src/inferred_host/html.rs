//! The restricted HTML tokenizer that selects client roots.

use super::*;

/// Restricted HTML tokenizer: no unproved interfering script, base, template/noscript,
/// active foreign content or entity-bearing URLs. Balanced passive SVG shapes
/// are inert; script/integration elements, handlers and external refs refuse.
/// Comments/raw text are consumed as
/// such, so a script-shaped string in either never becomes a root.
#[cfg(test)]
pub(super) fn html_entries(html: &str) -> Option<Vec<String>> {
    html_entries_at(Path::new("index.html"), html).ok()
}

pub(super) fn html_entries_at(path: &Path, source: &str) -> Result<Vec<String>, DiscoveryRefusal> {
    let mut html = source;
    let mut foreign = Vec::<String>::new();
    let mut entries = Vec::new();
    while let Some(start) = html.find('<') {
        html = &html[start..];
        let offset = source.len() - html.len();
        let refusal = |reason: &str| DiscoveryRefusal::at(path, source, offset, reason);
        if html.starts_with("<!--") {
            html = &html[html
                .find("-->")
                .ok_or_else(|| refusal("unterminated HTML comment"))?
                + 3..];
            continue;
        }
        if html
            .get(..15)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype html>"))
        {
            html = &html[15..];
            continue;
        }
        let end = html
            .find('>')
            .ok_or_else(|| refusal("unterminated HTML tag"))?;
        let tag = html
            .get(1..end)
            .ok_or_else(|| refusal("invalid HTML tag boundary"))?;
        if tag.contains('<') {
            return Err(refusal("nested markup in HTML tag"));
        }
        html = &html[end + 1..];
        let tag = tag.trim();
        let closing = tag.starts_with('/');
        let tag = tag.trim_start_matches('/');
        let name_end = tag
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        if ![
            "html", "head", "body", "meta", "link", "title", "style", "div", "main", "script",
            "svg", "defs", "symbol", "path", "rect", "line", "polygon", "use",
        ]
        .contains(&name.as_str())
        {
            return Err(refusal(&format!(
                "unsupported HTML/foreign element <{name}>"
            )));
        }
        if !foreign.is_empty()
            && ![
                "svg", "defs", "symbol", "path", "rect", "line", "polygon", "use",
            ]
            .contains(&name.as_str())
        {
            return Err(refusal("active or integration element inside SVG"));
        }
        if closing {
            if !tag[name_end..].trim().is_empty() {
                return Err(refusal("attributes on a closing HTML tag"));
            }
            if !foreign.is_empty() && foreign.pop().as_deref() != Some(name.as_str()) {
                return Err(refusal("unbalanced passive SVG subtree"));
            }
            continue;
        }
        let mut attributes = BTreeMap::new();
        let mut rest = tag[name_end..].trim();
        while !rest.is_empty() && rest != "/" {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '=')
                .ok_or_else(|| refusal("unquoted or boolean HTML attribute"))?;
            let key = rest[..end].to_ascii_lowercase();
            rest = rest[end..].trim_start();
            rest = rest
                .strip_prefix('=')
                .ok_or_else(|| refusal("boolean HTML attribute is outside the entry grammar"))?
                .trim_start();
            let quote = rest
                .chars()
                .next()
                .ok_or_else(|| refusal("missing HTML attribute value"))?;
            if quote != '\'' && quote != '"' {
                return Err(refusal("unquoted HTML attribute"));
            }
            rest = &rest[1..];
            let end = rest
                .find(quote)
                .ok_or_else(|| refusal("unterminated HTML attribute value"))?;
            let value = &rest[..end];
            if value.contains('&') || attributes.insert(key, value.to_owned()).is_some() {
                return Err(refusal("entity-bearing or duplicate HTML attribute"));
            }
            rest = rest[end + 1..].trim_start();
        }
        if attributes.keys().any(|key| key.starts_with("on")) {
            return Err(refusal("executable inline event attribute"));
        }
        if name == "svg" || !foreign.is_empty() {
            if attributes.iter().any(|(key, value)| {
                ["href", "xlink:href"].contains(&key.as_str()) && !value.starts_with('#')
            }) {
                return Err(refusal("external SVG reference"));
            }
            if !tag.ends_with('/') {
                foreign.push(name);
            }
            continue;
        }
        if ["defs", "symbol", "path", "rect", "line", "polygon", "use"].contains(&name.as_str()) {
            return Err(refusal("foreign element outside a passive SVG subtree"));
        }
        // A source-authored CSP can forbid every module script. The presence
        // of a script element then supplies no feasible window entry. Other
        // http-equiv policies are also outside this closed HTML grammar.
        if name == "meta" && attributes.contains_key("http-equiv") {
            return Err(refusal(
                "authored http-equiv policy may forbid module execution",
            ));
        }
        if name == "script" {
            if attributes
                .keys()
                .any(|key| !["type", "src"].contains(&key.as_str()))
            {
                return Err(refusal("unsupported module-script attribute"));
            }
            let end = html
                .find("</script>")
                .ok_or_else(|| refusal("missing literal script closing tag"))?;
            let body = &html[..end];
            let module = attributes
                .get("type")
                .is_some_and(|value| value == "module");
            if !module {
                if attributes.contains_key("src")
                    || attributes
                        .get("type")
                        .is_some_and(|value| !value.eq_ignore_ascii_case("text/javascript"))
                    || !crate::host_inline::independent(body)
                {
                    return Err(refusal(
                        "classic script may interfere with HTML module entries",
                    ));
                }
                html = &html[end + 9..];
                continue;
            }
            let Some(source) = attributes.get("src") else {
                // This element supplies no analyzed root. Independent siblings
                // can still execute, even after this script throws. Preserve
                // the interference guard; arbitrary opaque module code is not
                // permission to navigate away before another entry executes.
                if !crate::host_inline::independent(body) {
                    return Err(refusal(
                        "inline module has unproved interference with external entries",
                    ));
                }
                html = &html[end + 9..];
                continue;
            };
            if source.contains(['?', '#', ':', '%', '\\']) || source.starts_with("//") {
                return Err(refusal("module URL is external or carries a query/escape"));
            }
            entries.push(source.trim_start_matches('/').to_owned());
            if !body.trim().is_empty() {
                return Err(refusal("inline module-script body"));
            }
            html = &html[end + 9..];
        } else if name == "style" {
            // Vite can turn inline styles into CSS proxy modules. Their
            // executable preprocessor side inputs have no transform proof.
            return Err(refusal("inline stylesheet preprocessing is unproved"));
        } else if name == "title" {
            let close = format!("</{name}>");
            html = &html[html
                .find(&close)
                .ok_or_else(|| refusal("unterminated HTML title"))?
                + close.len()..];
        }
    }
    if !foreign.is_empty() {
        return Err(DiscoveryRefusal::new(
            path,
            "unterminated passive SVG subtree",
        ));
    }
    if entries.is_empty() {
        return Err(DiscoveryRefusal::new(
            path,
            "no external module-script roots",
        ));
    }
    Ok(entries)
}
