//! Command-line parsing identical to Go's `flag.FlagSet` with
//! `ContinueOnError` and discarded usage output, over the flags defined in
//! the Go producer's main.go (archived, ADR 0365).

use crate::gocompat::quote;

/// The lifecycle schema version the only supported `-schema` value names.
pub const SCHEMA_VERSION_V1: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// `-project`: path to tsconfig.json.
    pub project: String,
    /// `-project-config`: file holding the bytes of `-project`.
    pub project_config: String,
    /// `-schema`: lifecycle schema version.
    pub schema: u64,
    /// `-cpuprofile`: accepted; this producer writes an empty file there.
    pub cpu_profile: String,
    /// `-transition-arena`: Rust-owned transition arena path.
    pub transition_arena: String,
    /// `-inline-sources`: answer sources inline, not through an arena file.
    pub inline_sources: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            project: String::new(),
            project_config: String::new(),
            schema: SCHEMA_VERSION_V1,
            cpu_profile: String::new(),
            transition_arena: String::new(),
            inline_sources: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Unsigned,
    Boolean,
}

fn kind_of(name: &str) -> Option<Kind> {
    match name {
        "project" | "project-config" | "cpuprofile" | "transition-arena" => Some(Kind::Text),
        "schema" => Some(Kind::Unsigned),
        "inline-sources" => Some(Kind::Boolean),
        _ => None,
    }
}

fn store(options: &mut Options, name: &str, value: &str, kind: Kind) -> Result<(), &'static str> {
    match kind {
        Kind::Text => {
            let target = match name {
                "project" => &mut options.project,
                "project-config" => &mut options.project_config,
                "cpuprofile" => &mut options.cpu_profile,
                _ => &mut options.transition_arena,
            };
            value.clone_into(target);
        }
        Kind::Unsigned => options.schema = parse_uint(value)?,
        Kind::Boolean => options.inline_sources = parse_bool(value)?,
    }
    Ok(())
}

/// `strconv.ParseBool`, with flag's "parse error" wording.
fn parse_bool(value: &str) -> Result<bool, &'static str> {
    match value {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
        _ => Err("parse error"),
    }
}

/// `strconv.ParseUint(s, 0, 64)`, with flag's error wording.
fn parse_uint(text: &str) -> Result<u64, &'static str> {
    const SYNTAX: &str = "parse error";
    const RANGE: &str = "value out of range";
    if text.is_empty() {
        return Err(SYNTAX);
    }
    let bytes = text.as_bytes();
    let (base, digits): (u64, &[u8]) = if bytes[0] == b'0' {
        match bytes {
            [_, prefix, rest @ ..]
                if bytes.len() >= 3
                    && matches!(prefix.to_ascii_lowercase(), b'b' | b'o' | b'x') =>
            {
                (
                    match prefix.to_ascii_lowercase() {
                        b'b' => 2,
                        b'o' => 8,
                        _ => 16,
                    },
                    rest,
                )
            }
            [_, rest @ ..] => (8, rest),
            [] => (10, bytes),
        }
    } else {
        (10, bytes)
    };
    let mut underscores = false;
    let mut value: u64 = 0;
    for &byte in digits {
        let digit = match byte {
            b'_' => {
                underscores = true;
                continue;
            }
            b'0'..=b'9' => u64::from(byte - b'0'),
            b'a'..=b'z' => u64::from(byte - b'a') + 10,
            b'A'..=b'Z' => u64::from(byte - b'A') + 10,
            _ => return Err(SYNTAX),
        };
        if digit >= base {
            return Err(SYNTAX);
        }
        value = value
            .checked_mul(base)
            .and_then(|scaled| scaled.checked_add(digit))
            .ok_or(RANGE)?;
    }
    if underscores && !underscore_ok(bytes) {
        return Err(SYNTAX);
    }
    Ok(value)
}

/// Go's `underscoreOK`: underscores only between digits or after a base prefix.
fn underscore_ok(text: &[u8]) -> bool {
    let mut saw = b'^';
    let mut index = 0;
    let mut text = text;
    if let [b'-' | b'+', rest @ ..] = text {
        text = rest;
    }
    let mut hex = false;
    if text.len() >= 2
        && text[0] == b'0'
        && matches!(text[1].to_ascii_lowercase(), b'b' | b'o' | b'x')
    {
        index = 2;
        saw = b'0';
        hex = text[1].eq_ignore_ascii_case(&b'x');
    }
    while index < text.len() {
        let byte = text[index];
        if byte.is_ascii_digit() || (hex && matches!(byte.to_ascii_lowercase(), b'a'..=b'f')) {
            saw = b'0';
        } else if byte == b'_' {
            if saw != b'0' {
                return false;
            }
            saw = b'_';
        } else {
            if saw == b'_' {
                return false;
            }
            saw = b'!';
        }
        index += 1;
    }
    saw != b'_'
}

/// Parses `args` (without the program name). Stops at the first non-flag
/// argument or `--`; remaining arguments are ignored, as the Go producer
/// ignores them. Errors carry Go's exact text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut rest = args;
    while let Some(first) = rest.first() {
        if first.len() < 2 || !first.starts_with('-') {
            break;
        }
        let mut minuses = 1;
        if first.as_bytes()[1] == b'-' {
            minuses = 2;
            if first.len() == 2 {
                break;
            }
        }
        let mut name = &first[minuses..];
        if name.is_empty() || name.starts_with('-') || name.starts_with('=') {
            return Err(format!("bad flag syntax: {first}"));
        }
        rest = &rest[1..];
        let mut value = None;
        // Go searches from index 1, so a leading '=' was rejected above.
        if let Some(index) = name[1..].find('=') {
            value = Some(&name[index + 2..]);
            name = &name[..=index];
        }
        let Some(kind) = kind_of(name) else {
            if name == "help" || name == "h" {
                return Err("flag: help requested".to_owned());
            }
            return Err(format!("flag provided but not defined: -{name}"));
        };
        if let Kind::Boolean = kind {
            let text = value.unwrap_or("true");
            store(&mut options, name, text, kind).map_err(|reason| {
                format!(
                    "invalid boolean value {} for -{name}: {reason}",
                    quote(text)
                )
            })?;
            continue;
        }
        let text = match value {
            Some(text) => text,
            None => match rest.split_first() {
                Some((next, remaining)) => {
                    rest = remaining;
                    next.as_str()
                }
                None => return Err(format!("flag needs an argument: -{name}")),
            },
        };
        store(&mut options, name, text, kind).map_err(|reason| {
            format!("invalid value {} for flag -{name}: {reason}", quote(text))
        })?;
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Options, String> {
        parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn accepts_every_go_spelling() {
        let options = run(&[
            "-project",
            "/a/tsconfig.json",
            "--schema=0x1",
            "-inline-sources",
            "-transition-arena=/t",
            "-cpuprofile",
            "/p",
            "-project-config=/c",
            "positional",
            "-project=ignored",
        ])
        .unwrap();
        assert_eq!(options.project, "/a/tsconfig.json");
        assert_eq!(options.schema, 1);
        assert!(options.inline_sources);
        assert_eq!(options.transition_arena, "/t");
        assert_eq!(options.cpu_profile, "/p");
        assert_eq!(options.project_config, "/c");
    }

    #[test]
    fn error_text_matches_go() {
        let cases: &[(&[&str], &str)] = &[
            (&["-bogus"], "flag provided but not defined: -bogus"),
            (&["-project"], "flag needs an argument: -project"),
            (&["---x"], "bad flag syntax: ---x"),
            (&["-=x"], "bad flag syntax: -=x"),
            (
                &["-schema", "x"],
                "invalid value \"x\" for flag -schema: parse error",
            ),
            (
                &["-schema=18446744073709551616"],
                "invalid value \"18446744073709551616\" for flag -schema: value out of range",
            ),
            (
                &["-inline-sources=maybe"],
                "invalid boolean value \"maybe\" for -inline-sources: parse error",
            ),
            (&["-h"], "flag: help requested"),
            (&["-help"], "flag: help requested"),
        ];
        for (args, want) in cases {
            assert_eq!(run(args).unwrap_err(), *want, "{args:?}");
        }
    }

    #[test]
    fn terminators_stop_parsing() {
        assert_eq!(run(&["--", "-bogus"]).unwrap(), Options::default());
        assert_eq!(run(&["-", "-bogus"]).unwrap(), Options::default());
        assert_eq!(run(&["x", "-bogus"]).unwrap(), Options::default());
    }

    #[test]
    fn uint_follows_parse_uint_base_zero() {
        assert_eq!(parse_uint("1_0"), Ok(10));
        assert_eq!(parse_uint("1__0"), Err("parse error"));
        assert_eq!(parse_uint("_1"), Err("parse error"));
        assert_eq!(parse_uint("0b1"), Ok(1));
        assert_eq!(parse_uint("0o17"), Ok(15));
        assert_eq!(parse_uint("017"), Ok(15));
        assert_eq!(parse_uint("0x_1"), Ok(1));
        assert_eq!(parse_uint("0x"), Err("parse error"));
        assert_eq!(parse_uint("0"), Ok(0));
        assert_eq!(parse_uint("-1"), Err("parse error"));
        assert_eq!(parse_uint("+1"), Err("parse error"));
        assert_eq!(parse_uint("08"), Err("parse error"));
    }
}
