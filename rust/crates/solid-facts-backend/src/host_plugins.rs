//! Installed allowlist identities include transitive runtime dependencies.
//! Names/versions alone never authorize plugin behavior.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Pin {
    name: String,
    version: String,
    digest: String,
}

// One configuration pass shares exact byte observations across overlapping
// plugin dependency closures. A new pass always starts with an empty map.
pub(super) type PackageAudits = BTreeMap<PathBuf, (serde_json::Value, String)>;

pub(super) fn installed(
    directory: &Path,
    name: &str,
    inputs: &mut Vec<PathBuf>,
) -> Option<PathBuf> {
    for ancestor in directory.ancestors() {
        let candidate = ancestor.join("node_modules").join(name);
        inputs.push(ancestor.join("node_modules"));
        inputs.push(candidate.clone());
        inputs.push(candidate.join("package.json"));
        if candidate.join("package.json").is_file() {
            return fs::canonicalize(candidate).ok();
        }
    }
    None
}

fn tree(
    root: &Path,
    directory: &Path,
    rows: &mut BTreeMap<String, String>,
    inputs: &mut Vec<PathBuf>,
) -> Option<()> {
    inputs.push(directory.to_owned());
    for entry in fs::read_dir(directory).ok()? {
        let entry = entry.ok()?;
        if entry.file_name() == "node_modules" {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            return None;
        }
        if kind.is_dir() {
            tree(root, &path, rows, inputs)?;
        } else if kind.is_file() {
            inputs.push(path.clone());
            rows.insert(
                path.strip_prefix(root).ok()?.to_str()?.replace('\\', "/"),
                format!("{:x}", Sha256::digest(fs::read(path).ok()?)),
            );
        } else {
            return None;
        }
    }
    Some(())
}

fn package(root: &Path, inputs: &mut Vec<PathBuf>) -> Option<(serde_json::Value, String)> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("package.json")).ok()?).ok()?;
    let mut rows = BTreeMap::new();
    tree(root, root, &mut rows, inputs)?;
    let rows = rows.into_iter().collect::<Vec<_>>();
    let key = format!(
        "{}@{}:{:x}",
        manifest.get("name")?.as_str()?,
        manifest.get("version")?.as_str()?,
        Sha256::digest(serde_json::to_vec(&rows).ok()?)
    );
    Some((manifest, key))
}

pub(super) fn audit(
    directory: &Path,
    specifier: &str,
    inputs: &mut Vec<PathBuf>,
    packages: &mut PackageAudits,
) -> Result<(), String> {
    let name = match specifier {
        "filesystem-routing/vite" => "filesystem-routing",
        "unocss/vite" => "unocss",
        "unplugin-icons/vite" => "unplugin-icons",
        "@tanstack/router-plugin/vite" => "@tanstack/router-plugin",
        "vitest/config" => "vitest",
        name => name,
    };
    let root = installed(directory, name, inputs).ok_or_else(|| {
        format!(
            "plugin {name} is not installed above {}",
            directory.display()
        )
    })?;
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("package.json"))
            .map_err(|error| format!("{}: {error}", root.join("package.json").display()))?,
    )
    .map_err(|error| format!("{}: {error}", root.join("package.json").display()))?;
    if manifest
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{}: missing package name", root.display()))?
        != name
    {
        return Err(format!(
            "{}: installed plugin name differs from {name}",
            root.display()
        ));
    }
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{}: missing plugin version", root.display()))?;
    // Earlier implementations are independent Babel integrations, not the
    // audited Solid 2 forwarding API. Their option grammar is not reused.
    if name == "vite-plugin-solid" && version != "3.0.0-next.27" {
        return Err(format!(
            "{}: unaudited forwarding API {name}@{version}",
            root.display()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut rows = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(root) = pending.pop() {
        if !seen.insert(root.clone()) {
            continue;
        }
        if seen.len() > 512 {
            return Err(format!(
                "{}: plugin dependency closure exceeds 512 packages",
                root.display()
            ));
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = packages.entry(root.clone()) {
            entry.insert(package(&root, inputs).ok_or_else(|| {
                format!(
                    "{}: unreadable or symlink-bearing plugin package tree",
                    root.display()
                )
            })?);
        }
        let (manifest, key) = packages.get(&root).expect("inserted package").clone();
        rows.insert(key.clone());
        let mut dependencies = BTreeSet::new();
        for field in ["dependencies", "optionalDependencies", "peerDependencies"] {
            if let Some(deps) = manifest.get(field).and_then(serde_json::Value::as_object) {
                dependencies.extend(deps.keys().cloned());
            }
        }
        for name in dependencies {
            if let Some(target) = installed(&root, &name, inputs) {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    packages.entry(target.clone())
                {
                    entry.insert(package(&target, inputs).ok_or_else(|| {
                        format!(
                            "{}: unreadable or symlink-bearing plugin dependency tree",
                            target.display()
                        )
                    })?);
                }
                rows.insert(format!(
                    "{key}:{name}={}",
                    packages.get(&target).expect("inserted dependency").1
                ));
                pending.push(target);
            } else {
                // Absence is part of the exact audited installation, not
                // positive evidence of a dependency's runtime behavior.
                rows.insert(format!("{key}:{name}=absent"));
            }
        }
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&rows.into_iter().collect::<Vec<_>>())
                .expect("audit rows serialize")
        )
    );
    let pins: Vec<Pin> = serde_json::from_str(include_str!("host-plugin-pins.json"))
        .map_err(|error| format!("invalid compiled host plugin pins: {error}"))?;
    if pins
        .iter()
        .any(|pin| pin.name == name && pin.version == version && pin.digest == digest)
    {
        Ok(())
    } else {
        Err(format!(
            "{}: unaudited installed closure {name}@{version} sha256:{digest}",
            root.display()
        ))
    }
}
