//! What an installed tree records about packages whose files are not the
//! published archive their lockfile integrity names (ADR 0131).
//!
//! A lockfile's integrity is the record of what was *fetched*. Every package
//! manager that patches a dependency keeps that record as it was and changes
//! the files on disk: pnpm applies `patchedDependencies` and keeps
//! `resolution.integrity`, Bun applies its own `patchedDependencies` the same
//! way, and `patch-package` rewrites `node_modules` from a postinstall script
//! the lockfile never hears about. Admission compares name, version and
//! integrity, so a contract certified for the published bytes would be
//! admitted for bytes the consumer patched.
//!
//! The certified package itself is covered by its bytes: the receipt signs the
//! archive's snapshot root, and admission recomputes it from the installed
//! files. The packages of its dependency environment state only
//! `{name, version, integrity}`, so for them this module reads the evidence
//! each mechanism leaves where the reader can see it, from every ancestor of
//! the install directory, the same walk the lockfile integrity takes:
//!
//! - `pnpm-lock.yaml`: `patchedDependencies` keys, and any `(patch_hash=...)`
//!   version suffix;
//! - `pnpm-workspace.yaml` and `package.json` `pnpm.patchedDependencies`,
//!   pnpm's two places to declare one;
//! - `bun.lock` and `package.json` `patchedDependencies`;
//! - `yarn.lock` entries resolved through the `patch:` protocol;
//! - `patch-package` patch files, in `patches/` beside any `package.json` and
//!   in any `--patch-dir` a `package.json` script names;
//! - a pnpm store directory whose name carries `patch_hash=`.
//!
//! A key that names a range or no version at all patches every version, which
//! is how pnpm and `patch-package` read it too. A file that exists and cannot be
//! read or parsed means whether anything is patched cannot be stated, and every
//! package then counts as patched: refusing is the only answer that is not a
//! guess.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The patches recorded for every package installed under one directory.
#[derive(Clone, Debug, Default)]
pub(crate) struct InstalledPatches {
    selectors: Vec<PatchSelector>,
    /// Set when a file that could record a patch exists and cannot be read:
    /// every package then answers with it.
    unreadable: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatchSelector {
    name: String,
    /// `None` patches every version: a bare name, or a range.
    version: Option<String>,
    evidence: String,
}

impl InstalledPatches {
    /// Every patch the tree above `install_directory` records.
    pub(crate) fn read(install_directory: &Path) -> Self {
        let mut patches = Self::default();
        for ancestor in install_directory.ancestors() {
            patches.read_directory(ancestor);
        }
        patches
    }

    /// Why the package `name@version` installed at `package_directory` is not
    /// the published archive, or `None` when nothing here says it is patched.
    pub(crate) fn of(&self, package_directory: &Path, name: &str, version: &str) -> Option<String> {
        if let Some(unreadable) = &self.unreadable {
            return Some(unreadable.clone());
        }
        if package_directory.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|component| component.contains("patch_hash="))
        }) {
            return Some("its pnpm store directory is a patched copy (patch_hash)".into());
        }
        self.selectors
            .iter()
            .find(|selector| {
                selector.name == name
                    && selector
                        .version
                        .as_deref()
                        .is_none_or(|patched| patched == version)
            })
            .map(|selector| selector.evidence.clone())
    }

    fn unreadable(&mut self, path: &Path, detail: impl std::fmt::Display) {
        if self.unreadable.is_none() {
            self.unreadable = Some(format!(
                "{} cannot be read ({detail}), so whether it patches this package cannot be \
                 stated",
                path.display()
            ));
        }
    }

    fn select(&mut self, key: &str, evidence: &str) {
        let (name, version) = split_patch_key(key);
        self.selectors.push(PatchSelector {
            name: name.to_owned(),
            version: version
                .filter(|version| is_exact_version(version))
                .map(str::to_owned),
            evidence: evidence.to_owned(),
        });
    }

    fn read_directory(&mut self, directory: &Path) {
        if let Some(text) = self.read_text(&directory.join("pnpm-lock.yaml")) {
            match yaml_block_keys(&text, "patchedDependencies") {
                Ok(keys) => {
                    for key in keys {
                        self.select(&key, "pnpm-lock.yaml patchedDependencies");
                    }
                }
                Err(detail) => self.unreadable(&directory.join("pnpm-lock.yaml"), detail),
            }
            for key in pnpm_patch_hash_keys(&text) {
                self.select(&key, "pnpm-lock.yaml patch_hash");
            }
        }
        if let Some(text) = self.read_text(&directory.join("pnpm-workspace.yaml")) {
            match yaml_block_keys(&text, "patchedDependencies") {
                Ok(keys) => {
                    for key in keys {
                        self.select(&key, "pnpm-workspace.yaml patchedDependencies");
                    }
                }
                Err(detail) => self.unreadable(&directory.join("pnpm-workspace.yaml"), detail),
            }
        }
        let bun = directory.join("bun.lock");
        if let Some(text) = self.read_text(&bun) {
            match crate::diagnostics::parse_json_with_trailing_commas::<serde_json::Value>(
                text.as_bytes(),
            ) {
                Some(lock) => {
                    for key in object_keys(&lock, &["patchedDependencies"]) {
                        self.select(&key, "bun.lock patchedDependencies");
                    }
                }
                None => self.unreadable(&bun, "not JSON"),
            }
        }
        if let Some(text) = self.read_text(&directory.join("yarn.lock")) {
            for name in yarn_patch_protocol_names(&text) {
                self.select(&name, "yarn.lock patch: protocol");
            }
        }
        let manifest = directory.join("package.json");
        let Some(text) = self.read_text(&manifest) else {
            return;
        };
        let Ok(manifest_value) = serde_json::from_str::<serde_json::Value>(&text) else {
            self.unreadable(&manifest, "not JSON");
            return;
        };
        for key in object_keys(&manifest_value, &["patchedDependencies"]) {
            self.select(&key, "package.json patchedDependencies");
        }
        for key in object_keys(&manifest_value, &["pnpm", "patchedDependencies"]) {
            self.select(&key, "package.json pnpm.patchedDependencies");
        }
        for patch_directory in patch_package_directories(directory, &manifest_value) {
            match patch_files(&patch_directory) {
                Ok(files) => {
                    for file in files {
                        let Some(name) = patch_package_target(&file) else {
                            continue;
                        };
                        let relative = file.strip_prefix(directory).unwrap_or(&file);
                        self.select(
                            &name,
                            &format!("patch-package patch {}", relative.display()),
                        );
                    }
                }
                Err(error) => self.unreadable(&patch_directory, error),
            }
        }
    }

    /// The file's text, `None` when it does not exist; any other failure
    /// records the tree as unreadable.
    fn read_text(&mut self, path: &Path) -> Option<String> {
        match fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Some(text),
                Err(_) => {
                    self.unreadable(path, "not UTF-8");
                    None
                }
            },
            Err(error) if absent(&error) => None,
            Err(error) => {
                self.unreadable(path, error);
                None
            }
        }
    }
}

fn absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// `name@spec` split at the version separator, the last `@` that is not a
/// scope's leading one: `@scope/pkg@1.0.0` is `(@scope/pkg, 1.0.0)`, and a
/// bare `pkg` has no version.
fn split_patch_key(key: &str) -> (&str, Option<&str>) {
    match key.rfind('@') {
        Some(at) if at > 0 => (&key[..at], Some(&key[at + 1..])),
        _ => (key, None),
    }
}

/// `MAJOR.MINOR.PATCH` with an optional prerelease or build suffix. Anything
/// else in a patch key is a range, which patches every version it matches,
/// and this reader does not evaluate ranges.
fn is_exact_version(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The keys of the top-level YAML mapping `header`, in the block style pnpm
/// writes (`header:` then two-space-indented keys). An inline `{}` is empty;
/// any other inline value is refused rather than guessed at.
///
/// Every top-level occurrence is read, not the first: pnpm 11+ leads
/// `pnpm-lock.yaml` with an env document, so one file can hold the mapping in
/// each of its documents, and a patch recorded in either one is a patch. The
/// union is the direction that cannot miss one.
fn yaml_block_keys(text: &str, header: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(rest) = line
            .strip_prefix(header)
            .filter(|rest| rest.trim_start().starts_with(':'))
        {
            let inline = rest.trim_start()[1..].trim();
            inside = inline.is_empty() || inline.starts_with('#');
            if !inside && inline != "{}" {
                return Err(format!("an inline {header} mapping"));
            }
            continue;
        }
        if !inside || line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            inside = false;
            continue;
        }
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        keys.push(yaml_mapping_key(rest).ok_or_else(|| format!("an unreadable {header} key"))?);
    }
    Ok(keys)
}

/// The key of one YAML mapping line (`key: value` or `key:`), plain, single-
/// or double-quoted.
fn yaml_mapping_key(line: &str) -> Option<String> {
    let line = line.trim_end();
    if let Some(quote) = line.chars().next().filter(|it| *it == '\'' || *it == '"') {
        let close = line[1..].find(quote)? + 1;
        line[close + 1..]
            .trim_start()
            .starts_with(':')
            .then_some(())?;
        return Some(line[1..close].to_owned());
    }
    let key = match line.find(": ") {
        Some(at) => &line[..at],
        None => line.strip_suffix(':')?,
    };
    (!key.is_empty()).then(|| key.to_owned())
}

/// Every `name@version` a pnpm lockfile writes with a `(patch_hash=...)`
/// suffix: a snapshot key (`'name@1.0.0(patch_hash=...)':`) or a dependency
/// version (`name: 1.0.0(patch_hash=...)`).
fn pnpm_patch_hash_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for line in text.lines().filter(|line| line.contains("patch_hash=")) {
        let body = line.trim();
        let body = body.strip_prefix("- ").unwrap_or(body);
        // The key, quoted or plain, and whatever follows its `:`.
        let (key, rest) = match body.chars().next() {
            Some(quote @ ('\'' | '"')) => {
                let Some(close) = body[1..].find(quote).map(|at| at + 1) else {
                    continue;
                };
                (&body[1..close], &body[close + 1..])
            }
            _ => body.split_once(':').unwrap_or((body, "")),
        };
        if key.contains("(patch_hash=") {
            keys.push(key.split('(').next().unwrap_or_default().to_owned());
            continue;
        }
        let value = rest
            .trim_start()
            .trim_start_matches(':')
            .trim()
            .trim_matches(|it| it == '\'' || it == '"');
        if !value.contains("(patch_hash=") {
            continue;
        }
        let version = value.split('(').next().unwrap_or_default();
        // `version: 1.0.0(patch_hash=...)` under an importer's dependency names
        // its package one line up; the same patch is in the top-level
        // `patchedDependencies` and in the snapshot key, which are read.
        if version.is_empty() || matches!(key, "version" | "specifier") {
            continue;
        }
        if version.contains('@') {
            // An npm alias's value names the real package.
            keys.push(version.to_owned());
        } else {
            keys.push(format!("{key}@{version}"));
        }
    }
    keys
}

/// The keys of the JSON object at `path` inside `value`, if it is one.
fn object_keys(value: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut at = value;
    for segment in path {
        match at.get(segment) {
            Some(next) => at = next,
            None => return Vec::new(),
        }
    }
    at.as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

/// The package names of every Yarn lockfile entry whose descriptor resolves
/// through the `patch:` protocol (`"name@patch:name@npm%3A1.0.0#..."`). The
/// classic format has no such protocol and Berry's is refused for integrity
/// anyway; this keeps a Berry lock beside another lockfile from admitting a
/// patched package.
fn yarn_patch_protocol_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines().filter(|line| !line.starts_with(' ')) {
        for descriptor in line.trim_end_matches(':').split(", ") {
            let descriptor = descriptor.trim_matches('"');
            if let Some(at) = descriptor.find("@patch:").filter(|at| *at > 0) {
                names.push(descriptor[..at].to_owned());
            }
        }
    }
    names
}

/// Where `patch-package` reads patches for the project whose manifest is
/// `manifest`: `patches/`, and every `--patch-dir` a script passes it.
fn patch_package_directories(directory: &Path, manifest: &serde_json::Value) -> Vec<PathBuf> {
    let mut directories = vec![directory.join("patches")];
    let scripts = manifest
        .get("scripts")
        .and_then(serde_json::Value::as_object);
    for script in scripts.into_iter().flat_map(|scripts| scripts.values()) {
        let Some(script) = script.as_str() else {
            continue;
        };
        let mut words = script.split_whitespace();
        while let Some(word) = words.next() {
            let named = match word.strip_prefix("--patch-dir") {
                Some("") => words.next(),
                Some(rest) => rest.strip_prefix('='),
                None => None,
            };
            if let Some(named) = named {
                let named = named.trim_matches(|it| it == '\'' || it == '"');
                if !named.is_empty() {
                    directories.push(directory.join(named));
                }
            }
        }
    }
    directories.sort();
    directories.dedup();
    directories
}

/// Every `.patch` file under `directory`, recursively, as `patch-package`
/// reads them; empty when the directory does not exist.
fn patch_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(at) = pending.pop() {
        let entries = match fs::read_dir(&at) {
            Ok(entries) => entries,
            Err(error) if absent(&error) => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "patch")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// The package a `patch-package` file patches: the last `++`-separated
/// segment of `<parent>++<scope>+<name>+<version>[...].patch`, with its
/// version and any suffix dropped. `None` for a name in no such shape --
/// pnpm's and Bun's own patch files (`name@version.patch`) live in the same
/// directory and are read from their lockfiles instead.
fn patch_package_target(file: &Path) -> Option<String> {
    let stem = file.file_name()?.to_str()?.strip_suffix(".patch")?;
    let last = stem.rsplit("++").next()?;
    let mut parts = last.split('+');
    let first = parts.next()?;
    let name = if first.starts_with('@') {
        format!("{first}/{}", parts.next()?)
    } else {
        first.to_owned()
    };
    // The version follows; a file with none is not patch-package's.
    let version = parts.next()?;
    (!version.is_empty()
        && version
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit()))
    .then_some(name)
}

/// Every patch file [`InstalledPatches::read`] would consult for the tree
/// above `install_directory`, for a host that caches an admission verdict.
/// The declaring files themselves (`package.json`, `pnpm-workspace.yaml`, the
/// lockfiles) are listed by the caller; these are the files a directory
/// listing adds.
pub(crate) fn patch_input_paths(install_directory: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for ancestor in install_directory.ancestors() {
        let Ok(bytes) = fs::read(ancestor.join("package.json")) else {
            continue;
        };
        let manifest = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default();
        for directory in patch_package_directories(ancestor, &manifest) {
            paths.extend(patch_files(&directory).unwrap_or_default());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "solid-checker-installed-patches-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn patched(root: &Path, name: &str, version: &str) -> Option<String> {
        InstalledPatches::read(root).of(&root.join("node_modules").join(name), name, version)
    }

    #[test]
    fn a_pnpm_lock_names_its_patches_at_the_top_level_and_in_snapshot_keys() {
        let root = scratch("pnpm");
        write(
            &root,
            "pnpm-lock.yaml",
            "lockfileVersion: '9.0'\n\npatchedDependencies:\n  '@tanstack/solid-start@2.0.0-rc.8':\n    hash: abc\n    path: patches/@tanstack__solid-start@2.0.0-rc.8.patch\n  bare-name: def\n  ranged@^1.0.0: ghi\n\nimporters:\n  .:\n    dependencies:\n      '@tanstack/solid-start':\n        specifier: 2.0.0-rc.8\n        version: 2.0.0-rc.8(patch_hash=abc)\n\nsnapshots:\n  hashed-only@3.1.0(patch_hash=zzz):\n    dependencies:\n      nested-dep: 1.2.3(patch_hash=yyy)\n",
        );
        assert_eq!(
            patched(&root, "@tanstack/solid-start", "2.0.0-rc.8").as_deref(),
            Some("pnpm-lock.yaml patchedDependencies")
        );
        assert_eq!(patched(&root, "@tanstack/solid-start", "2.0.0-rc.9"), None);
        assert!(
            patched(&root, "bare-name", "9.9.9").is_some(),
            "a bare name patches every version"
        );
        assert!(
            patched(&root, "ranged", "1.4.0").is_some(),
            "a range is not evaluated"
        );
        assert_eq!(
            patched(&root, "hashed-only", "3.1.0").as_deref(),
            Some("pnpm-lock.yaml patch_hash")
        );
        assert_eq!(
            patched(&root, "nested-dep", "1.2.3").as_deref(),
            Some("pnpm-lock.yaml patch_hash")
        );
        assert_eq!(patched(&root, "@tanstack/solid-router", "2.0.0-rc.8"), None);
        let _ = fs::remove_dir_all(&root);
    }

    fn real_lockfile(name: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lockfiles")
                .join(name),
        )
        .unwrap()
    }

    /// pnpm 11+ leads the lockfile with an env document. The patch reader and
    /// the integrity reader parse the same file, so a patch the project
    /// document records must still be seen behind it (finds.team's real
    /// lockfile), and a patch recorded in *either* document is a patch.
    #[test]
    fn a_pnpm_11_lockfile_with_an_env_document_still_names_its_patches() {
        let root = scratch("pnpm-env-document");
        let lock = real_lockfile("finds-team.pnpm-lock.yaml");
        write(&root, "pnpm-lock.yaml", &lock);
        assert_eq!(
            patched(&root, "@tanstack/solid-start", "2.0.0-rc.8").as_deref(),
            Some("pnpm-lock.yaml patchedDependencies")
        );
        assert_eq!(patched(&root, "@tanstack/solid-router", "2.0.0-rc.8"), None);
        assert_eq!(patched(&root, "pnpm", "12.5.1"), None);

        // Both documents record one: neither may hide the other.
        let both = lock.replacen(
            "\nimporters:\n",
            "\npatchedDependencies:\n  pnpm@12.5.1: 0000\n\nimporters:\n",
            1,
        );
        assert!(both.find("patchedDependencies:") < both.find("\n---\n"));
        write(&root, "pnpm-lock.yaml", &both);
        assert!(patched(&root, "pnpm", "12.5.1").is_some());
        assert!(patched(&root, "@tanstack/solid-start", "2.0.0-rc.8").is_some());
        let _ = fs::remove_dir_all(&root);
    }

    /// `bun.lock` `lockfileVersion: 1` keeps `patchedDependencies` where
    /// version 2 does (Civil's real lockfile).
    #[test]
    fn a_bun_lock_version_1_names_its_patches() {
        let root = scratch("bun-v1");
        write(&root, "bun.lock", &real_lockfile("civil.bun.lock"));
        assert_eq!(
            patched(&root, "@stacksjs/ts-cache", "0.1.5").as_deref(),
            Some("bun.lock patchedDependencies")
        );
        assert_eq!(patched(&root, "@solidjs/meta", "1.0.0-next.2"), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pnpm_and_bun_declarations_outside_the_lock_are_read_too() {
        let root = scratch("declared");
        write(
            &root,
            "pnpm-workspace.yaml",
            "packages:\n  - apps/*\npatchedDependencies:\n  from-workspace@1.0.0: patches/a.patch\n",
        );
        write(
            &root,
            "package.json",
            r#"{"pnpm":{"patchedDependencies":{"from-pnpm-field@2.0.0":"patches/b.patch"}},
                "patchedDependencies":{"from-bun-field@3.0.0":"patches/c.patch"}}"#,
        );
        write(
            &root,
            "bun.lock",
            "{\n  \"lockfileVersion\": 1,\n  \"patchedDependencies\": {\n    \"from-bun-lock@4.0.0\": \"patches/d.patch\",\n  },\n}\n",
        );
        for (name, version, evidence) in [
            (
                "from-workspace",
                "1.0.0",
                "pnpm-workspace.yaml patchedDependencies",
            ),
            (
                "from-pnpm-field",
                "2.0.0",
                "package.json pnpm.patchedDependencies",
            ),
            (
                "from-bun-field",
                "3.0.0",
                "package.json patchedDependencies",
            ),
            ("from-bun-lock", "4.0.0", "bun.lock patchedDependencies"),
        ] {
            assert_eq!(patched(&root, name, version).as_deref(), Some(evidence));
            assert_eq!(
                patched(&root, name, "0.0.1"),
                None,
                "{name} at another version"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn patch_package_files_are_read_from_patches_and_a_named_patch_dir() {
        let root = scratch("patch-package");
        write(
            &root,
            "package.json",
            r#"{"scripts":{"postinstall":"patch-package --patch-dir custom/dir"}}"#,
        );
        write(&root, "patches/left-pad+1.3.0.patch", "");
        write(&root, "patches/nested/@scope+pkg+2.0.0.patch", "");
        write(&root, "patches/parent++child+1.0.0+001+initial.patch", "");
        write(&root, "custom/dir/elsewhere+5.0.0.dev.patch", "");
        // pnpm's and Bun's naming is theirs; the lockfile is what says whether
        // one was applied.
        write(&root, "patches/pnpm-style@1.0.0.patch", "");
        for (name, patch) in [
            ("left-pad", "patches/left-pad+1.3.0.patch"),
            ("@scope/pkg", "patches/nested/@scope+pkg+2.0.0.patch"),
            ("child", "patches/parent++child+1.0.0+001+initial.patch"),
            ("elsewhere", "custom/dir/elsewhere+5.0.0.dev.patch"),
        ] {
            assert_eq!(
                patched(&root, name, "9.9.9"),
                Some(format!("patch-package patch {patch}")),
                "patch-package applies a patch whatever the installed version"
            );
        }
        assert_eq!(patched(&root, "pnpm-style", "1.0.0"), None);
        assert_eq!(
            patched(&root, "left", "1.3.0"),
            None,
            "a name prefix is not a match"
        );
        assert_eq!(patched(&root, "@scope/pk", "2.0.0"), None);
        let mut inputs = patch_input_paths(&root.join("node_modules"));
        inputs.sort();
        assert_eq!(inputs.len(), 5, "{inputs:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_yarn_patch_resolution_and_a_pnpm_patched_store_directory_are_patches() {
        let root = scratch("yarn");
        write(
            &root,
            "yarn.lock",
            "__metadata:\n  version: 8\n\n\"patched-pkg@npm:1.0.0, patched-pkg@patch:patched-pkg@npm%3A1.0.0#~/.yarn/patches/x.patch\":\n  version: 1.0.0\n",
        );
        assert_eq!(
            patched(&root, "patched-pkg", "1.0.0").as_deref(),
            Some("yarn.lock patch: protocol")
        );
        let store = root.join("node_modules/.pnpm/x@1.0.0_patch_hash=abc/node_modules/x");
        assert!(
            InstalledPatches::read(&root)
                .of(&store, "x", "1.0.0")
                .is_some()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_declaring_file_that_cannot_be_read_refuses_every_package() {
        let root = scratch("unreadable");
        write(&root, "package.json", "{ not json");
        let refusal = patched(&root, "anything", "1.0.0").unwrap();
        assert!(refusal.contains("package.json cannot be read"), "{refusal}");
        write(&root, "package.json", "{}");
        write(
            &root,
            "pnpm-lock.yaml",
            "patchedDependencies: { a@1.0.0: x }\n",
        );
        assert!(patched(&root, "anything", "1.0.0").is_some());
        write(&root, "pnpm-lock.yaml", "patchedDependencies: {}\n");
        assert_eq!(patched(&root, "anything", "1.0.0"), None);
        let _ = fs::remove_dir_all(&root);
    }
}
