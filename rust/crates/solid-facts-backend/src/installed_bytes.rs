//! What the backend reads of the installed `@solidjs/router` (ADR 0324).
//!
//! The router premise rests on the audited release's bytes, and a release
//! string is a claim that a `patch-package` patch, a fork installed under the
//! same version, an `overrides` tarball or a `link:` keeps. So the backend
//! hashes the bytes of every router install the project's imports resolve to
//! (the same digest `scripts/router-digest.mjs` computes, which the dialect
//! records for each audited release), and records whether the project's
//! manifests announce a replacement at all. The IR compares; it reads no disk.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use solid_facts::ProjectFacts;
use solid_reactive_ir::router_params::{ROUTER_PACKAGE, RouterInstallEvidence};

/// The manifest and lockfile names whose lines can record a patch of an
/// installed package, in the order [`replacement_recorded`] reads them. The
/// daemon treats each of them, on every ancestor, as an input of the answer.
pub const PATCH_RECORD_FILES: [&str; 5] = [
    "package.json",
    "bun.lock",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "yarn.lock",
];

/// Directories whose entries are patch files a package manager applies.
const PATCH_DIRECTORIES: [&str; 2] = ["patches", ".yarn/patches"];

/// The manifest fields the resolution consults, in digest order.
const MANIFEST_FIELDS: [&str; 6] = ["name", "version", "main", "module", "exports", "type"];

/// The directories of the router installs `facts` resolves to: the folder of
/// the `package.json` each router import's resolution consulted.
#[must_use]
pub fn router_install_roots(facts: &ProjectFacts) -> Vec<PathBuf> {
    let mut roots = facts
        .resolved_imports
        .iter()
        .flat_map(solid_facts::AttestedImportIndex::iter)
        .filter(|(_, row)| row.package_name.as_deref() == Some(ROUTER_PACKAGE))
        .filter_map(|(_, row)| row.package_manifest.as_deref())
        .filter_map(|manifest| Path::new(manifest).parent().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

/// The router evidence for `project`'s analysis of `facts`.
#[must_use]
pub fn router_install_evidence(project: &Path, facts: &ProjectFacts) -> RouterInstallEvidence {
    let mut digests = BTreeMap::new();
    for root in router_install_roots(facts) {
        if let Some(digest) = package_digest(&root) {
            digests.insert(
                root.join("package.json").to_string_lossy().into_owned(),
                digest,
            );
        }
    }
    RouterInstallEvidence {
        replacement_recorded: replacement_recorded(project),
        digests,
    }
}

/// The digest of an installed package's bytes, hex sha256 over:
///
/// 1. the line `solid-checker router bytes v1`;
/// 2. for each of `name`, `version`, `main`, `module`, `exports`, `type` of
///    `package.json`, the line `<key>=<value>`, the value being the field's
///    JSON with object keys sorted at every depth and no whitespace, or
///    `undefined` when the field is absent;
/// 3. for each regular file under `dist/`, recursively, sorted by its path
///    relative to `dist/` (UTF-8 bytes, `/` separators): `file\0<path>\0`,
///    the content length as a little-endian `u64`, then the content.
///
/// A symlink or an entry that is neither a file nor a directory under `dist/`,
/// or an unreadable one, gives no digest, which the premise reads as bytes it
/// cannot vouch for. `scripts/router-digest.mjs` computes the same value.
#[must_use]
pub fn package_digest(directory: &Path) -> Option<String> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("package.json")).ok()?).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(b"solid-checker router bytes v1\n");
    for key in MANIFEST_FIELDS {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        match manifest.get(key) {
            Some(value) => hasher.update(canonical_json(value).as_bytes()),
            None => hasher.update(b"undefined"),
        }
        hasher.update(b"\n");
    }
    let dist = directory.join("dist");
    let mut files = Vec::new();
    collect_files(&dist, "", &mut files)?;
    files.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    for (relative, path) in files {
        let content = fs::read(path).ok()?;
        hasher.update(b"file\0");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(&content);
    }
    Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn collect_files(directory: &Path, prefix: &str, found: &mut Vec<(String, PathBuf)>) -> Option<()> {
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().into_string().ok()?;
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            return None;
        }
        if kind.is_dir() {
            collect_files(&entry.path(), &format!("{prefix}{name}/"), found)?;
        } else if kind.is_file() {
            found.push((format!("{prefix}{name}"), entry.path()));
        } else {
            return None;
        }
    }
    Some(())
}

/// `value` as JSON with object keys sorted at every depth and no whitespace
/// (the form `JSON.stringify` gives after sorting keys).
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let members = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::Value::String(key.clone()),
                        canonical_json(&map[key])
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", members.join(","))
        }
        serde_json::Value::Array(items) => {
            let items = items.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// Whether a manifest, lockfile or patch directory above `project` announces
/// that the installed router is not the registry's bytes of its version
/// (ADR 0324): a patch line naming it (`patchedDependencies`, a yarn `patch:`
/// descriptor, a pnpm `patch_hash`), a `patches/` entry naming it
/// (`patch-package` writes no manifest entry), an `overrides`, `resolutions` or
/// `pnpm.overrides` entry for it, or a dependency on it, that is not a plain
/// registry version (`github:`, a URL, `file:`, `link:`, `npm:`, `workspace:`).
///
/// This is the cheap layer; the byte digest is the proof. A record that cannot
/// be read is no record.
#[must_use]
pub fn replacement_recorded(project: &Path) -> bool {
    let start = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or(project)
    };
    start.ancestors().any(|directory| {
        PATCH_RECORD_FILES
            .iter()
            .any(|name| record_names_router(&directory.join(name)))
            || PATCH_DIRECTORIES.iter().any(|patches| {
                fs::read_dir(directory.join(patches)).is_ok_and(|entries| {
                    entries.flatten().any(|entry| {
                        let name = entry.file_name().to_string_lossy().to_lowercase();
                        name.contains("solidjs") && name.contains("router")
                    })
                })
            })
    })
}

fn record_names_router(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let lines = text.lines().collect::<Vec<_>>();
    let patched = lines.iter().enumerate().any(|(at, line)| {
        line.contains(ROUTER_PACKAGE)
            && lines[at..lines.len().min(at + 4)].iter().any(|nearby| {
                nearby.contains("patch:")
                    || nearby.contains("patch_hash")
                    || nearby.contains("patches/")
                    || nearby.contains(".patch")
            })
    });
    patched
        || (path.file_name().is_some_and(|name| name == "package.json")
            && serde_json::from_str::<serde_json::Value>(&text)
                .is_ok_and(|manifest| manifest_replaces_router(&manifest)))
}

fn manifest_replaces_router(manifest: &serde_json::Value) -> bool {
    let not_plain =
        |value: &serde_json::Value| value.as_str().is_none_or(|spec| !plain_registry_spec(spec));
    let overrides = |table: Option<&serde_json::Value>| {
        table
            .and_then(serde_json::Value::as_object)
            .is_some_and(|table| {
                table
                    .iter()
                    .any(|(key, value)| key.contains(ROUTER_PACKAGE) && not_plain(value))
            })
    };
    let dependencies = |table: Option<&serde_json::Value>| {
        table
            .and_then(serde_json::Value::as_object)
            .and_then(|table| table.get(ROUTER_PACKAGE))
            .is_some_and(not_plain)
    };
    overrides(manifest.get("overrides"))
        || overrides(manifest.get("resolutions"))
        || overrides(manifest.pointer("/pnpm/overrides"))
        || [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ]
        .iter()
        .any(|table| dependencies(manifest.get(table)))
}

/// A registry version, range or tag: no protocol, no path, no fragment.
fn plain_registry_spec(spec: &str) -> bool {
    let spec = spec.trim();
    !spec.is_empty() && !spec.contains([':', '/', '#', '\\']) && !spec.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir")
            .join(name)
            .join("node_modules/@solidjs/router")
    }

    /// The fixtures' router is the published package, so it hashes to the
    /// digest the dialect records for the audited release; one changed byte
    /// does not. `scripts/router-digest.mjs` computes the same value.
    #[test]
    fn the_published_router_hashes_to_the_audited_digest_and_a_changed_byte_does_not() {
        let audited = crate::dialect::default_dialect()
            .vocabulary
            .audited_router_digest("2.0.0-next.26");
        let published = package_digest(&fixture("router-params-stock-history"));
        assert_eq!(published.as_deref(), audited);
        assert_eq!(
            package_digest(&fixture("router-params")),
            published,
            "the same bytes in another fixture"
        );
        assert_ne!(
            package_digest(&fixture("router-params-modified-bytes")),
            published
        );
        assert_eq!(
            package_digest(&fixture("router-params-no-such-fixture")),
            None
        );
    }

    #[test]
    fn a_plain_registry_spec_names_no_protocol_or_path() {
        for plain in ["2.0.0-next.26", "^2.0.0", "~2", ">=2.0.0 <3", "next", "*"] {
            assert!(plain_registry_spec(plain), "{plain}");
        }
        for other in [
            "github:someone/router#main",
            "someone/router",
            "https://example.com/router.tgz",
            "file:../router",
            "link:../router",
            "npm:other@1",
            "workspace:*",
            "git+ssh://x/y.git",
            "./router",
            "",
        ] {
            assert!(!plain_registry_spec(other), "{other}");
        }
    }
}
