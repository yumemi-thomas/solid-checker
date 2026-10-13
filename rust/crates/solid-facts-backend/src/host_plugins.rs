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

fn tree(directory: &Path, files: &mut Vec<PathBuf>, inputs: &mut Vec<PathBuf>) -> Option<()> {
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
            tree(&path, files, inputs)?;
        } else if kind.is_file() {
            inputs.push(path.clone());
            files.push(path);
        } else {
            return None;
        }
    }
    Some(())
}

fn package_tree(
    root: &Path,
    inputs: &mut Vec<PathBuf>,
    generation: Option<&BTreeMap<String, String>>,
) -> Option<(serde_json::Value, Vec<PathBuf>)> {
    let path = root.join("package.json");
    let bytes = fs::read(&path).ok()?;
    if let Some(expected) = generation.and_then(|inputs| inputs.get(path.to_str()?))
        && expected != &format!("sha256:{:x}", Sha256::digest(&bytes))
    {
        return None;
    }
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let mut files = Vec::new();
    tree(root, &mut files, inputs)?;
    Some((manifest, files))
}

pub(super) fn audit(
    directory: &Path,
    specifier: &str,
    inputs: &mut Vec<PathBuf>,
    packages: &mut PackageAudits,
    generation: Option<&BTreeMap<String, String>>,
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
    let mut closure = BTreeMap::new();
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
        let manifest = packages
            .get(&root)
            .map(|(manifest, _)| manifest.clone())
            .or_else(|| serde_json::from_slice(&fs::read(root.join("package.json")).ok()?).ok())
            .ok_or_else(|| format!("{}: unreadable plugin dependency manifest", root.display()))?;
        let mut dependencies = BTreeSet::new();
        for field in ["dependencies", "optionalDependencies", "peerDependencies"] {
            if let Some(deps) = manifest.get(field).and_then(serde_json::Value::as_object) {
                dependencies.extend(deps.keys().cloned());
            }
        }
        let edges = dependencies
            .into_iter()
            .map(|name| {
                let target = installed(&root, &name, inputs);
                if let Some(target) = &target {
                    pending.push(target.clone());
                }
                (name, target)
            })
            .collect::<Vec<_>>();
        closure.insert(root, (manifest, edges));
    }
    let missing = closure
        .keys()
        .filter(|root| !packages.contains_key(*root))
        .collect::<Vec<_>>();
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8);
    let observations = std::thread::scope(|scope| {
        let jobs = (0..workers.min(missing.len()))
            .map(|worker| {
                let missing = &missing;
                scope.spawn(move || {
                    (worker..missing.len())
                        .step_by(workers)
                        .map(|index| {
                            let root = missing[index];
                            let mut paths = Vec::new();
                            let observed = package_tree(root, &mut paths, generation);
                            (root.clone(), observed, paths)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .flat_map(|job| job.join().expect("plugin fingerprint worker panicked"))
            .collect::<Vec<_>>()
    });
    let mut trees = BTreeMap::new();
    let mut files = Vec::new();
    for (root, observed, paths) in observations {
        inputs.extend(paths);
        let (manifest, observed_files) = observed.ok_or_else(|| {
            format!(
                "{}: unreadable or symlink-bearing plugin package tree",
                root.display()
            )
        })?;
        files.extend(observed_files.into_iter().map(|path| (root.clone(), path)));
        trees.insert(root, (manifest, BTreeMap::new()));
    }
    // One pool across all files prevents a large package from holding up an
    // otherwise finished closure. The same sorted per-package rows define pins.
    // Replay pins against the independently captured manifest generation. Its
    // file digests are already byte observations, never metadata trust. Native
    // inference validates that entire generation before returning and before/
    // after analysis. Manifests above are still checked before choosing edges.
    let missing = files
        .iter()
        .filter(|(_, path)| {
            generation
                .and_then(|inputs| path.to_str().and_then(|path| inputs.get(path)))
                .is_none()
        })
        .map(|(_, path)| path.as_path())
        .collect::<Vec<_>>();
    let mut fresh = crate::inferred_host::identities(&missing).into_iter();
    let identities = files
        .iter()
        .map(|(_, path)| {
            generation
                .and_then(|inputs| path.to_str().and_then(|path| inputs.get(path)))
                .cloned()
                .or_else(|| fresh.next().flatten())
        })
        .collect::<Vec<_>>();
    for ((root, path), identity) in files.into_iter().zip(identities) {
        let row = || {
            Some((
                path.strip_prefix(&root).ok()?.to_str()?.replace('\\', "/"),
                identity.as_deref()?.strip_prefix("sha256:")?.to_owned(),
            ))
        };
        let (path, digest) =
            row().ok_or_else(|| format!("{}: cannot fingerprint plugin file", path.display()))?;
        trees
            .get_mut(&root)
            .expect("observed tree")
            .1
            .insert(path, digest);
    }
    for (root, (manifest, rows)) in trees {
        let key = || {
            Some(format!(
                "{}@{}:{:x}",
                manifest.get("name")?.as_str()?,
                manifest.get("version")?.as_str()?,
                Sha256::digest(serde_json::to_vec(&rows.into_iter().collect::<Vec<_>>()).ok()?)
            ))
        };
        let key = key().ok_or_else(|| format!("{}: invalid plugin identity", root.display()))?;
        packages.insert(root, (manifest, key));
    }
    for (root, (observed_manifest, edges)) in closure {
        let (manifest, key) = &packages[&root];
        if manifest != &observed_manifest {
            return Err(format!(
                "{}: plugin manifest changed; retry analysis",
                root.display()
            ));
        }
        rows.insert(key.clone());
        for (name, target) in edges {
            // Absence remains an audited input, never runtime authority.
            rows.insert(format!(
                "{key}:{name}={}",
                target
                    .as_ref()
                    .map_or("absent", |target| packages[target].1.as_str())
            ));
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
