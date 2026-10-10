//! Visible enforcement of ADR 0270's conventional Vite invocation premise.
//! Admit exact commands only; shell syntax and launchers are never simulated.

fn plain_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || b"._:/=@-".contains(&ch))
}

fn admitted(source: &str) -> bool {
    let words: Vec<_> = source.split_ascii_whitespace().collect();
    if words.first() != Some(&"vite") || !words.iter().all(|word| plain_word(word)) {
        return false;
    }
    let mut index = 1;
    if words
        .get(index)
        .is_some_and(|word| matches!(*word, "build" | "dev" | "serve" | "preview"))
    {
        index += 1;
    }
    while let Some(word) = words.get(index) {
        let (option, inline) = word
            .split_once('=')
            .map_or((*word, None), |(key, value)| (key, Some(value)));
        index += 1;
        let value = match option {
            "--open" | "--strictPort" | "--emptyOutDir" | "--sourcemap" | "--force" | "--cors"
            | "--watch" => {
                if inline.is_some() {
                    return false;
                }
                continue;
            }
            "--host" | "--minify" => inline.or_else(|| {
                words
                    .get(index)
                    .filter(|next| !next.starts_with('-'))
                    .map(|next| {
                        index += 1;
                        *next
                    })
            }),
            "--port" | "--base" | "--outDir" | "--logLevel" | "--clearScreen" => {
                let value = inline.or_else(|| {
                    let next = words.get(index)?;
                    index += 1;
                    Some(*next)
                });
                if value.is_none() {
                    return false;
                }
                value
            }
            _ => return false,
        };
        if let Some(value) = value {
            if !plain_word(value) || value.starts_with('-') {
                return false;
            }
            let valid = match option {
                "--port" => {
                    value.bytes().all(|ch| ch.is_ascii_digit()) && value.parse::<u16>().is_ok()
                }
                "--clearScreen" => matches!(value, "true" | "false"),
                "--logLevel" => matches!(value, "info" | "warn" | "error" | "silent"),
                "--minify" => matches!(value, "esbuild" | "terser" | "false"),
                _ => true,
            };
            if !valid {
                return false;
            }
        }
    }
    true
}

fn shell_syntax(source: &str) -> bool {
    source.chars().any(|ch| {
        matches!(
            ch,
            '&' | '|' | ';' | '<' | '>' | '$' | '\u{60}' | '\'' | '"' | '(' | ')' | '\n' | '\r'
        )
    })
}

fn ignored(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    !lower.contains("vite")
        && !shell_syntax(source)
        // Escapes/globs must not disguise a forbidden launcher word. No shell
        // interpretation is used even to establish an unrelated command.
        && source.split_ascii_whitespace().all(plain_word)
        && !lower
            .split(|ch: char| !ch.is_ascii_alphanumeric() && !matches!(ch, '_' | '-'))
            .any(|word| {
                matches!(
                    word,
                    "cd" | "sh"
                        | "bash"
                        | "zsh"
                        | "node"
                        | "npx"
                        | "bunx"
                        | "pnpm"
                        | "npm"
                        | "yarn"
                        | "bun"
                        | "deno"
                        | "env"
                        | "cross-env"
                        | "exec"
                        | "run"
                        | "concurrently"
                        | "npm-run-all"
                        | "turbo"
                        | "nx"
                        | "lerna"
                )
            })
}

/// Check every app/enclosing manifest script, including lifecycle hooks.
/// The caller enrolls the manifest before inspection and names it in the note.
pub(super) fn refusal(scripts: &serde_json::Value) -> Option<String> {
    let Some(scripts) = scripts.as_object() else {
        return Some("scripts is not a literal object".into());
    };
    for (name, value) in scripts {
        let Some(source) = value.as_str() else {
            return Some(format!("scripts.{name}: non-literal script"));
        };
        // Inspect before trimming: even a trailing newline is shell syntax.
        if (!shell_syntax(source) && admitted(source.trim())) || ignored(source) {
            continue;
        }
        return Some(format!(
            "scripts.{name}: script is outside the exact conventional Vite allowlist"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_scripts_only() {
        for source in [
            " vite build --outDir dist ",
            "vite",
            "vite dev",
            "vite serve",
            "vite preview",
            "vite build --port 3000 --host localhost --open --strictPort --base /app/ --outDir=dist --emptyOutDir --sourcemap --minify terser --logLevel warn --clearScreen false --force --cors --watch",
            "vite --host --minify",
            "tsc -b",
            "eslint src",
            "prettier --check .",
        ] {
            assert!(
                refusal(&serde_json::json!({"build":source})).is_none(),
                "{source}"
            );
        }
        for source in [
            "vite build --config other.ts",
            "vite --mode test",
            "vite --root .",
            "vite .",
            "vite --configLoader native",
            "vite --unknown",
            "vite --port",
            "vite --port nope",
            "vite --clearScreen maybe",
            "vite --logLevel debug",
            "vite --minify unknown",
            "vite --outDir --config",
            "vite --outDir=",
            "vite --open=foo",
            "vite --",
            "VITE build",
            "echo VITE",
            "vite build\n",
            "vite build *.ts",
            "vite build dist\\ dir",
            "vite build --base https://example.test/?x",
            "echo $COMMAND",
            "echo 'safe'",
            "echo safe && tsc",
            "(tsc)",
            "tsc > out",
            "tsc; eslint",
            "cd app",
            "/bin/sh script",
            "bash script",
            "zsh script",
            "node script",
            "n\\ode scripts/build.mjs",
            "n?de scripts/build.mjs",
            "npx tool",
            "bunx tool",
            "pnpm build",
            "npm test",
            "yarn test",
            "bun test",
            "deno run script",
            "env X=1 tsc",
            "cross-env X=1 tsc",
            "exec tsc",
            "run build",
            "concurrently tsc",
            "npm-run-all build",
            "turbo run build",
            "nx build",
            "lerna build",
        ] {
            assert!(
                refusal(&serde_json::json!({"postbuild":source}))
                    .unwrap()
                    .contains("scripts.postbuild"),
                "{source}"
            );
        }
    }
}
