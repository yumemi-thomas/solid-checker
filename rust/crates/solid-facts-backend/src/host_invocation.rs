//! Visible enforcement of ADR 0270's conventional Vite invocation premise.
//! This is a closed literal command grammar, never shell execution.

use std::path::{Path, PathBuf};

fn commands(source: &str) -> Option<Vec<Vec<String>>> {
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = source.chars();
    while let Some(ch) = chars.next() {
        // Expansion, substitution, globbing and redirection are not literal
        // evidence, even in a command that does not visibly spell Vite.
        if matches!(ch, '$' | '`')
            || (quote.is_none()
                && matches!(
                    ch,
                    '*' | '?' | '[' | ']' | '{' | '}' | '(' | ')' | '<' | '>' | '#'
                ))
        {
            return None;
        }
        if ch == '\\' && quote != Some('\'') {
            let escaped = chars.next()?;
            if escaped == '\n' || escaped == '\r' {
                return None;
            }
            word.push(escaped);
            started = true;
        } else if quote == Some(ch) {
            quote = None;
        } else if quote.is_some() {
            word.push(ch);
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
        } else if ch.is_whitespace() || matches!(ch, ';' | '&' | '|') {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
            if matches!(ch, ';' | '&' | '|' | '\n' | '\r') && !words.is_empty() {
                commands.push(std::mem::take(&mut words));
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    Some(commands)
}

fn vite_word(word: &str) -> bool {
    word == "vite"
        || word == "vite@latest"
        || word.starts_with("vite@")
        || word.ends_with("/node_modules/.bin/vite")
        || word == "node_modules/.bin/vite"
        || word.ends_with("/vite/bin/vite.js")
        || word.ends_with("/vite")
        || word.ends_with("/vite.cmd")
}

fn selected_environment(word: &str) -> bool {
    let option = word.split_once('=').map_or(word, |(key, _)| key);
    matches!(
        option,
        "--config" | "-c" | "--configLoader" | "--mode" | "-m"
    ) || word.starts_with("-c")
        || word.starts_with("-m")
}

/// All scripts, including lifecycle hooks, are checked. Enclosing scripts are
/// conservatively checked whenever they mention Vite; exact unrelated filters
/// may be excluded, but wildcard/opaque filters never establish exclusion.
pub(super) fn refusal(
    scripts: &serde_json::Value,
    working_directory: &Path,
    app: &Path,
    app_name: Option<&str>,
    inputs: &mut Vec<PathBuf>,
) -> Option<String> {
    let scripts = match scripts.as_object() {
        Some(scripts) => scripts,
        None => return Some("scripts is not a literal object".into()),
    };
    for (name, value) in scripts {
        let Some(source) = value.as_str() else {
            return Some(format!("scripts.{name}: non-literal script"));
        };
        let Some(commands) = commands(source) else {
            return Some(format!(
                "scripts.{name}: non-literal or unsupported shell script"
            ));
        };
        let mut cwd = working_directory.to_path_buf();
        for words in commands {
            if words.iter().any(|word| {
                matches!(
                    word.as_str(),
                    "sh" | "bash" | "zsh" | "dash" | "fish" | "cmd" | "powershell" | "pwsh"
                )
            }) {
                return Some(format!("scripts.{name}: opaque shell invocation"));
            }
            if words.first().is_some_and(|word| word == "cd") {
                let [_, target] = words.as_slice() else {
                    return Some(format!("scripts.{name}: undecidable working directory"));
                };
                inputs.push(cwd.join(target));
                let Ok(target) = std::fs::canonicalize(cwd.join(target)) else {
                    return Some(format!("scripts.{name}: undecidable working directory"));
                };
                cwd = target;
                continue;
            }
            let Some(vite) = words.iter().position(|word| vite_word(word)) else {
                if words.iter().any(|word| {
                    word.contains("vite ")
                        || word.contains("vite\t")
                        || word.contains("'vite'")
                        || word.contains("\"vite\"")
                }) {
                    return Some(format!("scripts.{name}: opaque Vite invocation"));
                }
                continue;
            };
            let mut selected_cwd = cwd.clone();
            let mut filtered = false;
            let mut unrelated = false;
            let mut targets_app = false;
            let mut index = 0;
            while index < vite {
                let (mut option, mut inline) = words[index]
                    .split_once('=')
                    .map_or((words[index].as_str(), None), |(key, value)| {
                        (key, Some(value))
                    });
                if let Some(value) = words[index]
                    .strip_prefix("-C")
                    .filter(|value| !value.is_empty())
                {
                    option = "-C";
                    inline = Some(value);
                }
                if option == "--workspace-root" {
                    return Some(format!("scripts.{name}: undecidable workspace root"));
                }
                if matches!(
                    option,
                    "-C" | "--dir"
                        | "--cwd"
                        | "--prefix"
                        | "--filter"
                        | "-F"
                        | "-w"
                        | "--workspace"
                ) {
                    let value = if let Some(value) = inline {
                        value
                    } else {
                        index += 1;
                        let Some(value) = words.get(index) else {
                            return Some(format!("scripts.{name}: missing workspace option value"));
                        };
                        value.as_str()
                    };
                    if matches!(option, "--filter" | "-F" | "-w" | "--workspace") {
                        filtered = true;
                        if Some(value) != app_name
                            && value != app.to_str().unwrap_or("")
                            && value
                                != format!(
                                    "./{}",
                                    app.strip_prefix(working_directory).unwrap_or(app).display()
                                )
                        {
                            // Only exact package-name filters prove exclusion.
                            if value.contains(['.', '/', '!', '^', '*', '?', '[', ']', '{', '}'])
                                || app_name.is_none()
                            {
                                return Some(format!(
                                    "scripts.{name}: undecidable workspace filter {value}"
                                ));
                            }
                            unrelated = true;
                        } else {
                            targets_app = true;
                        }
                    } else {
                        inputs.push(cwd.join(value));
                        let Ok(target) = std::fs::canonicalize(cwd.join(value)) else {
                            return Some(format!(
                                "scripts.{name}: undecidable workspace directory {value}"
                            ));
                        };
                        selected_cwd = target;
                    }
                }
                index += 1;
            }
            if unrelated && !targets_app && selected_cwd != app {
                continue;
            }
            if filtered {
                if !app.starts_with(&selected_cwd) {
                    return Some(format!(
                        "scripts.{name}: workspace directory is not an enclosing app directory"
                    ));
                }
                selected_cwd = app.to_path_buf();
            }
            if selected_cwd != app {
                return Some(format!(
                    "scripts.{name}: Vite working directory differs from the app"
                ));
            }
            if let Some(word) = words[vite + 1..]
                .iter()
                .find(|word| selected_environment(word))
            {
                return Some(format!(
                    "scripts.{name}: Vite {word} selects a non-conventional config environment"
                ));
            }
            let mut positional = false;
            let mut index = vite + 1;
            if words
                .get(index)
                .is_some_and(|word| matches!(word.as_str(), "build" | "dev" | "serve" | "preview"))
            {
                index += 1;
            }
            while index < words.len() {
                let word = &words[index];
                let (option, inline) = word
                    .split_once('=')
                    .map_or((word.as_str(), None), |(key, value)| (key, Some(value)));
                if option == "--root" || !word.starts_with('-') {
                    let root = if option == "--root" {
                        if let Some(value) = inline {
                            value
                        } else {
                            index += 1;
                            let Some(value) = words.get(index) else {
                                return Some(format!("scripts.{name}: missing Vite root"));
                            };
                            value.as_str()
                        }
                    } else {
                        word.as_str()
                    };
                    inputs.push(selected_cwd.join(root));
                    if positional
                        || std::fs::canonicalize(selected_cwd.join(root))
                            .ok()
                            .as_deref()
                            != Some(app)
                    {
                        return Some(format!(
                            "scripts.{name}: Vite root {root} differs from the app or is undecidable"
                        ));
                    }
                    positional = true;
                } else if matches!(option, "--host" | "--open") {
                    // Optional values cannot silently become a positional root.
                    if inline.is_none()
                        && words
                            .get(index + 1)
                            .is_some_and(|word| !word.starts_with('-'))
                    {
                        index += 1;
                    }
                } else if matches!(
                    option,
                    "--port"
                        | "--base"
                        | "--logLevel"
                        | "-l"
                        | "--outDir"
                        | "--assetsDir"
                        | "--target"
                ) {
                    if inline.is_none() {
                        index += 1;
                        if words.get(index).is_none() {
                            return Some(format!("scripts.{name}: missing Vite option value"));
                        }
                    }
                } else if !matches!(
                    option,
                    "--" | "--force"
                        | "--strictPort"
                        | "--clearScreen"
                        | "--emptyOutDir"
                        | "--sourcemap"
                        | "--minify"
                        | "--debug"
                        | "-d"
                ) {
                    return Some(format!("scripts.{name}: unsupported Vite option {word}"));
                }
                index += 1;
            }
        }
    }
    None
}
