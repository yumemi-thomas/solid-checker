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

/// A word that may change the shell's directory or which program `vite`
/// names, in sh, zsh or cmd: `cd`, `CD`, `cd..`, `cd/d`, `chdir`, `pushd`,
/// `popd`, a drive selector (`D:`), cmd's `path`/`set` (also `path..`,
/// `set/a`), and any assignment
/// (`PATH=./bin`, `alias vite=...`, `set /a PATH-=1`). cmd's `@` echo prefix
/// is ignored. `--outDir=dist` starts with `-` and is a flag value.
fn shell_state_word(word: &str) -> bool {
    if word.contains('=') && !word.starts_with('-') {
        return true;
    }
    let lower = word.trim_start_matches('@').to_ascii_lowercase();
    let drive =
        lower.len() == 2 && lower.as_bytes()[0].is_ascii_alphabetic() && lower.ends_with(':');
    drive
        || ["cd", "chdir", "pushd", "popd", "path", "set"]
            .iter()
            .any(|builtin| {
                lower
                    .strip_prefix(builtin)
                    .is_some_and(|rest| !rest.starts_with(|ch: char| ch.is_ascii_alphanumeric()))
            })
}

/// A case-insensitive `vite` not followed by a letter or digit: `vite`,
/// `vite.js`, `VITE_X`, `-Svite`, `/cvite`, `bin\vite.cmd`, but not
/// `vitest` or `@vitejs`. Nothing is required before it, so attached option
/// payloads and path segments count (and so does `invite`, conservatively).
/// Scripts that never name Vite are covered by the premise, whatever tool
/// they run.
fn names_vite(source: &str) -> bool {
    // Quotes, escapes and line continuations vanish before execution:
    // `vit"e"`, cmd's `v^ite` and `v\<newline>ite` all run `vite`. A bare
    // newline still separates commands, so it becomes a space.
    let lower: String = source
        .replace("\\\r\n", "")
        .replace("\\\n", "")
        .replace("^\r\n", "")
        .replace("^\n", "")
        .chars()
        .filter(|ch| !matches!(ch, '\'' | '"' | '\\' | '^'))
        .map(|ch| if matches!(ch, '\n' | '\r') { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    lower.match_indices("vite").any(|(index, _)| {
        !lower
            .as_bytes()
            .get(index + 4)
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
    })
}

/// Check every app/enclosing manifest script, including lifecycle hooks.
/// The caller enrolls the manifest before inspection and names it in the note.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ManifestRole {
    Application,
    Enclosing,
}

/// ADR 0270 premises the conventional Vite config; this vetoes only where a
/// script visibly selects another one. A script naming Vite must be plain
/// words joined by `&&`, each command either Vite-free or an exact
/// conventional `vite` command, with no directory change.
pub(super) fn refusal(scripts: &serde_json::Value, role: ManifestRole) -> Option<String> {
    let Some(scripts) = scripts.as_object() else {
        return Some("scripts is not a literal object".into());
    };
    for (name, value) in scripts {
        let Some(source) = value.as_str() else {
            return Some(format!("scripts.{name}: non-literal script"));
        };
        if !names_vite(source) {
            continue;
        }
        if role == ManifestRole::Enclosing {
            // Package scripts run in their manifest's directory, so this
            // invocation cannot select the app's config.
            return Some(format!(
                "scripts.{name}: enclosing manifest launches Vite from a different directory"
            ));
        }
        // Only plain words joined by `&&`: no other separator, newline,
        // quoting, expansion, grouping or escape can reach the shell.
        let conventional = source.split("&&").all(|command| {
            let words: Vec<_> = command.split_ascii_whitespace().collect();
            !command.contains(['\n', '\r'])
                && !words.is_empty()
                && words
                    .iter()
                    .all(|word| plain_word(word) && !shell_state_word(word))
                && (!names_vite(command) || admitted(command))
        });
        if !conventional {
            return Some(format!(
                "scripts.{name}: script may select a non-conventional Vite config"
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(source: &str) -> Option<String> {
        refusal(
            &serde_json::json!({"build": source}),
            ManifestRole::Application,
        )
    }

    #[test]
    fn vite_free_scripts_are_premised() {
        for source in [
            "tsc -b",
            "eslint .",
            "playwright test",
            "vitest",
            "vp build",
            "node scripts/build.mjs",
            "npm run i18n && tsr generate",
            "rm -rf dist && echo 'ok'",
            "pnpm add @vitejs/plugin-react",
        ] {
            assert!(app(source).is_none(), "{source}");
            assert!(
                refusal(
                    &serde_json::json!({"build": source}),
                    ManifestRole::Enclosing
                )
                .is_none(),
                "{source}"
            );
        }
    }

    #[test]
    fn exact_vite_commands_only() {
        for source in [
            " vite build --outDir dist ",
            "vite",
            "vite dev",
            "vite serve",
            "vite preview",
            "vite build --port 3000 --host localhost --open --strictPort --base /app/ --outDir=dist --emptyOutDir --sourcemap --minify terser --logLevel warn --clearScreen false --force --cors --watch",
            "vite --host --minify",
            "tsc -b && vite build",
            "npm run i18n && tsr generate && tsc -b && vite build",
            "vite build&&tsc -p tsconfig.build.json",
            "cdk synth && vite build",
        ] {
            assert!(app(source).is_none(), "{source}");
        }
        for source in [
            "vite build --config other.ts",
            "vite --mode test",
            "vite --root .",
            "vite .",
            "vite --configLoader native",
            "vite --unknown",
            "vite --port nope",
            "vite --outDir --config",
            "vite --open=foo",
            "VITE build",
            "VITE_MODE=x vite",
            "vite build\n",
            "vite build *.ts",
            "vite build $VITE_FLAGS",
            "vite build $(cat flags)",
            "\"vite\" build",
            "v\\ite build",
            "tsc && vite build --mode staging",
            "cd .. && vite build",
            "cd .. || cd app && vite build",
            "X=1 cd .. && vite build",
            "command cd ..; vite build",
            "builtin pushd ..; vite build",
            "true;cd ..;vite build",
            "cd;vite build",
            "vite build; tsc -p tsconfig.build.json",
            "tsc | vite build",
            "{cd,..};vite build",
            "{cd,..} && vite build",
            "c[d] .. && vite build",
            "c* .. && vite build",
            "{cd ..;};vite build",
            "chdir .. && vite build",
            "builtin chdir .. && vite build",
            "CD .. && vite build",
            "cd.. && vite build",
            "cd/d .. && vite build",
            "PATH=./bin && vite build",
            "export PATH=./bin && vite build",
            "alias vite=other && vite build",
            "@cd .. && vite build",
            "@chdir .. && vite build",
            "@pushd .. && vite build",
            "D: && vite build",
            "path C:/alternate/bin && vite build",
            "set /a PATH-=1 && vite build",
            "v^ite build --config ../other.ts",
            "vi^te build --mode staging",
            "env -Svite build --config ../other.ts",
            "cd .. && env -Svite build",
            "cmd /cvite build --config ../other.ts",
            "node_modules\\.bin\\vite.cmd build --config ../other.ts",
            "node node_modules\\vite\\bin\\vite.js build --config ../other.ts",
            "path.. && vite build",
            "path/alternate/bin && vite build",
            "v^\nite build --config ../other.ts",
            "c^d ..&vite build",
            "! true && vite build",
            "# x && vite build",
            "true \u{b}cd .. && vite build",
            "&& vite build",
            "vite build &&",
            "true\nvite build --config ../other.ts",
            "v\\\nite build --config ../other.ts",
            "npx vite build",
            "pnpm --dir ../.. exec vite build",
            "pnpm -C.. exec vite dev",
            "node node_modules/vite/bin/vite.js build",
            "sh -c 'vite build'",
            "sh -c 'vit\"e\" build --config ../other.ts'",
            "electron-vite build",
            "env X=1 vite",
        ] {
            assert!(app(source).unwrap().contains("scripts.build"), "{source}");
        }
        assert!(
            refusal(
                &serde_json::json!({"build": ["vite"]}),
                ManifestRole::Application
            )
            .unwrap()
            .contains("non-literal")
        );
    }

    #[test]
    fn enclosing_manifests_refuse_vite() {
        for source in [
            "vite",
            "vite build",
            "tsc && vite build",
            "pnpm --filter app exec vite",
            "v\\\nite build --config ../other.ts",
            "true\nvite build --config ../other.ts",
            "v^ite build --config ../other.ts",
            "env -Svite build --config ../other.ts",
            "node_modules\\.bin\\vite.cmd build --config ../other.ts",
            "v^\nite build --config ../other.ts",
        ] {
            assert_eq!(
                refusal(
                    &serde_json::json!({"postbuild": source}),
                    ManifestRole::Enclosing
                )
                .unwrap(),
                "scripts.postbuild: enclosing manifest launches Vite from a different directory"
            );
        }
    }
}
