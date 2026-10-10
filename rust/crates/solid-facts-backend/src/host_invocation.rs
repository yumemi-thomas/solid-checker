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
    let mut words = source.split_ascii_whitespace();
    // This is a positive tool allowlist, not absence of known launchers.
    // Audit notes: docs/adr/0270-inferred-host-script-tools.md. Tools with
    // configured command hooks, app servers, or unknown dispatch stay closed.
    matches!(
        words.next(),
        Some(
            "tsc"
                | "eslint"
                | "prettier"
                | "oxlint"
                | "oxfmt"
                | "biome"
                | "stylelint"
                | "rimraf"
                | "rm"
                | "mkdir"
                | "cp"
                | "echo"
                | "true"
                | "openapi-typescript"
                | "depcheck"
                | "syncpack"
        )
    ) && !source.to_ascii_lowercase().contains("vite")
        && !shell_syntax(source)
        // Check every word, including the executable, before shell expansion.
        && source.split_ascii_whitespace().all(plain_word)
}

/// Check every app/enclosing manifest script, including lifecycle hooks.
/// The caller enrolls the manifest before inspection and names it in the note.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ManifestRole {
    Application,
    Enclosing,
}

pub(super) fn refusal(scripts: &serde_json::Value, role: ManifestRole) -> Option<String> {
    let Some(scripts) = scripts.as_object() else {
        return Some("scripts is not a literal object".into());
    };
    for (name, value) in scripts {
        let Some(source) = value.as_str() else {
            return Some(format!("scripts.{name}: non-literal script"));
        };
        // Inspect before trimming: even a trailing newline is shell syntax.
        if ignored(source) {
            continue;
        }
        if !shell_syntax(source) && admitted(source.trim()) {
            if role == ManifestRole::Application {
                continue;
            }
            // Package scripts run in their manifest's directory. Even with no
            // parent config, this invocation cannot select the app's config.
            return Some(format!(
                "scripts.{name}: enclosing manifest launches Vite from a different directory"
            ));
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
                refusal(
                    &serde_json::json!({"build":source}),
                    ManifestRole::Application
                )
                .is_none(),
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
                refusal(
                    &serde_json::json!({"postbuild":source}),
                    ManifestRole::Application
                )
                .unwrap()
                .contains("scripts.postbuild"),
                "{source}"
            );
        }
    }

    #[test]
    fn enclosing_manifests_admit_only_ignored_scripts() {
        for source in [
            "vite",
            "vite build",
            "vite dev",
            "vite serve",
            "vite preview",
            "vite build --outDir dist",
        ] {
            let scripts = serde_json::json!({"postbuild": source});
            assert!(refusal(&scripts, ManifestRole::Application).is_none());
            assert_eq!(
                refusal(&scripts, ManifestRole::Enclosing).unwrap(),
                "scripts.postbuild: enclosing manifest launches Vite from a different directory"
            );
        }
        for source in ["tsc -b", "eslint src", "prettier --check ."] {
            assert!(
                refusal(
                    &serde_json::json!({"build": source}),
                    ManifestRole::Enclosing
                )
                .is_none()
            );
        }
        for source in ["vitest", "node build.mjs", "echo safe && tsc", "VITE build"] {
            assert!(
                refusal(
                    &serde_json::json!({"build": source}),
                    ManifestRole::Enclosing
                )
                .unwrap()
                .contains("scripts.build")
            );
        }
    }

    #[test]
    fn ignored_tools_require_an_exact_audited_first_word() {
        for source in [
            "tsc --noEmit",
            "eslint .",
            "prettier --check .",
            "oxlint src",
            "oxfmt --check .",
            "biome check .",
            "stylelint src/app.css",
            "rimraf dist",
            "rm -rf dist",
            "mkdir -p dist",
            "cp src/app.css dist/app.css",
            "echo ready",
            "true",
            "openapi-typescript http://localhost:3000/openapi.json --output ./src/api/schema.gen.ts",
            "depcheck --json",
            "syncpack list",
            // Arguments are data for these tools, not executable dispatch.
            "echo node",
        ] {
            assert!(ignored(source), "{source}");
            for role in [ManifestRole::Application, ManifestRole::Enclosing] {
                assert!(refusal(&serde_json::json!({"check":source}), role).is_none());
            }
        }
        for source in [
            "vp build",
            "vp",
            "vp build --config ../other.ts",
            "vp -C .. build",
            "vp build --mode production",
            "playwright test",
            "cypress run",
            "vitest",
            "storybook dev",
            "astro dev",
            "lint-staged",
            "tsx build.ts",
            "ts-node build.ts",
            "husky",
            "graphql-codegen",
            "knip",
            "changeset publish",
            "unknown-tool build",
            "./tsc --noEmit",
            "TSC --noEmit",
            "",
            "  ",
            "tsc --noEmit\n",
            "eslint *.ts",
            "echo `command`",
            "echo safe\\ word",
            "echo VITE",
        ] {
            assert!(!ignored(source), "{source}");
            for role in [ManifestRole::Application, ManifestRole::Enclosing] {
                assert!(
                    refusal(&serde_json::json!({"check":source}), role)
                        .unwrap()
                        .contains("scripts.check"),
                    "{source}"
                );
            }
        }
    }
}
