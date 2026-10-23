//! Small helpers that reproduce Go's text and path conventions, so error
//! messages the client may surface stay identical to the Go producer's.

use std::path::Path;

/// Go's `%q` for a string (`strconv.Quote`). Printable-ness approximates
/// `unicode.IsPrint`: control characters and non-ASCII whitespace are escaped.
#[must_use]
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            ' ' => out.push(' '),
            printable if !printable.is_control() && !printable.is_whitespace() => {
                out.push(printable);
            }
            other => {
                let code = u32::from(other);
                if code < 0x80 {
                    out.push_str(&format!("\\x{code:02x}"));
                } else if code < 0x1_0000 {
                    out.push_str(&format!("\\u{code:04x}"));
                } else {
                    out.push_str(&format!("\\U{code:08x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// Renders an I/O error the way Go's `*PathError` does after the path:
/// the OS message, lowercased first letter, without Rust's "(os error N)".
#[must_use]
pub fn io_message(error: &std::io::Error) -> String {
    let text = error.to_string();
    let text = match text.rfind(" (os error ") {
        Some(index) => &text[..index],
        None => &text,
    };
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// `*PathError.Error()`: `<op> <path>: <message>`.
#[must_use]
pub fn path_error(operation: &str, path: &str, error: &std::io::Error) -> String {
    format!("{operation} {path}: {}", io_message(error))
}

/// `filepath.Clean` for Unix-style paths.
///
/// Nearly every path the producer cleans already is clean (program file names,
/// demand locations), so a single scan answers those with one copy instead of
/// the split/collect/join of the general case.
#[must_use]
pub fn clean(path: &str) -> String {
    if is_clean(path) {
        return path.to_owned();
    }
    clean_general(path)
}

/// [`clean`] without a copy when `path` already is clean.
#[must_use]
pub fn clean_cow(path: &str) -> std::borrow::Cow<'_, str> {
    if is_clean(path) {
        std::borrow::Cow::Borrowed(path)
    } else {
        std::borrow::Cow::Owned(clean_general(path))
    }
}

/// Whether `filepath.Clean(path) == path`: `.` and `/`, or no empty element
/// other than the leading root, no `.` element, no trailing separator, and
/// `..` only as leading elements of a relative path.
fn is_clean(path: &str) -> bool {
    if path == "/" || path == "." {
        return true;
    }
    let rooted = path.starts_with('/');
    let body = path.strip_prefix('/').unwrap_or(path);
    if body.is_empty() {
        return false;
    }
    let mut leading = !rooted;
    for element in body.split('/') {
        match element {
            "" | "." => return false,
            ".." if leading => {}
            ".." => return false,
            _ => leading = false,
        }
    }
    true
}

fn clean_general(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for element in path.split('/') {
        match element {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_owned(),
        (false, false) => joined,
    }
}

/// `path.Dir` / `filepath.Dir` for Unix-style paths: everything up to the last
/// separator, cleaned; `.` when there is none.
#[must_use]
pub fn dir(path: &str) -> String {
    match path.rfind('/') {
        None => ".".to_owned(),
        Some(index) => clean(&path[..=index]),
    }
}

/// `filepath.Abs` (which also cleans).
pub fn absolute(path: &str) -> Result<String, String> {
    if Path::new(path).has_root() {
        return Ok(clean(path));
    }
    let working = std::env::current_dir().map_err(|error| io_message(&error))?;
    let working = working.to_string_lossy();
    let base = if working.is_empty() {
        "."
    } else {
        working.as_ref()
    };
    Ok(clean(&format!("{base}/{path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_matches_go() {
        for (input, want) in [
            ("", "."),
            ("/", "/"),
            ("a//b/./c/..", "a/b"),
            ("/../a", "/a"),
            ("../../a/b/..", "../../a"),
            ("a/..", "."),
            ("/a/b/", "/a/b"),
        ] {
            assert_eq!(clean(input), want, "{input:?}");
        }
    }

    #[test]
    fn clean_fast_path_agrees_with_the_general_algorithm() {
        let inputs = [
            "",
            "/",
            "//",
            ".",
            "..",
            "/.",
            "/..",
            "a",
            "/a",
            "a/",
            "/a/",
            "a//b",
            "a/./b",
            "a/../b",
            "/a/b/c.ts",
            "./a",
            "../a",
            "a/b/..",
            "a/.b",
            "a/..b",
            "a/b..",
            ".hidden",
            "/.hidden/x",
            "bundled:///libs/lib.d.ts",
            "/a//",
            "///a",
            "a b/c d",
            "/a/b/./",
            "/a/b/../c",
            "c:/x/y",
            "/ü/é",
            "../..",
            "../../a",
            "../a/..",
            "a/../..",
            "/../a",
            "..a/..",
            "../.b",
        ];
        for input in inputs {
            assert_eq!(clean(input), clean_general(input), "{input:?}");
            assert_eq!(is_clean(input), clean_general(input) == input, "{input:?}");
        }
    }

    #[test]
    fn clean_fast_path_agrees_on_every_short_element_sequence() {
        const ELEMENTS: [&str; 5] = ["", ".", "..", "a", "..b"];
        let mut paths = vec![String::new()];
        let mut frontier = vec![Vec::<&str>::new()];
        for _ in 0..5 {
            let mut next = Vec::new();
            for sequence in &frontier {
                for element in ELEMENTS {
                    let mut longer = sequence.clone();
                    longer.push(element);
                    paths.push(longer.join("/"));
                    next.push(longer);
                }
            }
            frontier = next;
        }
        assert!(paths.len() > 3_000);
        for path in paths {
            assert_eq!(clean(&path), clean_general(&path), "{path:?}");
            assert_eq!(&*clean_cow(&path), clean_general(&path), "{path:?}");
            assert_eq!(is_clean(&path), clean_general(&path) == path, "{path:?}");
        }
    }

    #[test]
    fn dir_matches_go() {
        for (input, want) in [
            ("", "."),
            ("/", "/"),
            ("c.ts", "."),
            ("/a/b/c.ts", "/a/b"),
            ("/c.ts", "/"),
            ("a/b/", "a/b"),
            ("bundled:///libs/lib.d.ts", "bundled:/libs"),
        ] {
            assert_eq!(dir(input), want, "{input:?}");
        }
    }

    #[test]
    fn quote_matches_go() {
        assert_eq!(quote("a\"b\\c\n\u{1}é"), "\"a\\\"b\\\\c\\n\\x01é\"");
        assert_eq!(quote("\u{a0}"), "\"\\u00a0\"");
    }

    #[test]
    fn io_message_drops_os_error_suffix() {
        let error = std::fs::read("/definitely/not/here").unwrap_err();
        assert_eq!(io_message(&error), "no such file or directory");
    }
}
