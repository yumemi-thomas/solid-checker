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
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use sha2::{Digest, Sha256};
use solid_facts::ProjectFacts;
use solid_reactive_ir::route_components::{ROUTE_CORE_PACKAGE, ROUTE_PACKAGE, RouteInstall};
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
    package_install_roots(facts, ROUTER_PACKAGE)
}

/// ADR 0334: the directories of the `@tanstack/solid-router` installs
/// `facts` resolves to, and of the `@tanstack/router-core` each loads.
#[must_use]
pub fn route_install_roots(facts: &ProjectFacts) -> Vec<PathBuf> {
    let mut roots = package_install_roots(facts, ROUTE_PACKAGE);
    let cores = roots
        .iter()
        .filter_map(|root| route_core_root(root))
        .collect::<Vec<_>>();
    roots.extend(cores);
    roots.sort();
    roots.dedup();
    roots
}

/// The `@tanstack/router-core` Node loads from the router installed at
/// `router`: the nearest `node_modules/@tanstack/router-core` above the
/// router's *real* directory (pnpm places a package's dependencies beside
/// its link target, not beside the link).
fn route_core_root(router: &Path) -> Option<PathBuf> {
    // The first candidate that exists at all is the one Node loads; without
    // a readable manifest it is no install this premise can vouch for.
    let core = route_core_candidates(router)
        .into_iter()
        .find(|candidate| fs::symlink_metadata(candidate).is_ok())?;
    core.join("package.json").is_file().then_some(core)
}

/// Every `node_modules/@tanstack/router-core` directory Node's lookup
/// consults from the router's real directory, nearest first (an ancestor
/// itself named `node_modules` is skipped, as Node skips it). Each is an
/// input of a cached answer, present or absent: a nearer install appearing
/// changes which core Node loads (ADR 0334).
#[must_use]
pub fn route_core_candidates(router: &Path) -> Vec<PathBuf> {
    let Ok(real) = fs::canonicalize(router) else {
        return Vec::new();
    };
    real.ancestors()
        .filter(|directory| {
            directory
                .file_name()
                .is_none_or(|name| name != "node_modules")
        })
        .map(|directory| directory.join("node_modules").join(ROUTE_CORE_PACKAGE))
        .collect()
}

fn package_install_roots(facts: &ProjectFacts, package: &str) -> Vec<PathBuf> {
    let mut roots = facts
        .resolved_imports
        .iter()
        .flat_map(solid_facts::AttestedImportIndex::iter)
        .filter(|(_, row)| row.package_name.as_deref() == Some(package))
        .filter_map(|(_, row)| row.package_manifest.as_deref())
        .filter_map(|manifest| Path::new(manifest).parent().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

/// Which installed `solid-js` each analyzed file loads at runtime: the
/// nearest `node_modules/solid-js` (with a `package.json`) in the file's
/// directory or above, symlinks resolved. Two copies of one version are two
/// instances however TypeScript merged their declarations.
#[must_use]
pub fn package_instance_evidence(
    facts: &ProjectFacts,
) -> solid_reactive_ir::PackageInstanceEvidence {
    solid_reactive_ir::PackageInstanceEvidence {
        solid_js_by_file: solid_js_by_file(
            facts.files.iter().map(|file| file.path.as_str()),
            &mut Disk,
        ),
    }
}

/// The two file-system questions the instance lookup asks, so a test can
/// count them.
trait InstanceFs {
    fn canonicalize(&mut self, path: &Path) -> std::io::Result<PathBuf>;
    fn is_file(&mut self, path: &Path) -> bool;
}

struct Disk;

impl InstanceFs for Disk {
    fn canonicalize(&mut self, path: &Path) -> std::io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn is_file(&mut self, path: &Path) -> bool {
        path.is_file()
    }
}

/// The `solid-js` real directory of each of `paths` that has one, by path.
///
/// A project's files share a few hundred directories, so one request asks the
/// file system once per directory, not once per file: a raw directory is
/// canonicalized once (a failure too, which falls back to the raw directory),
/// and a directory's nearest install is looked up once, so the `node_modules`
/// of an ancestor that many directories share is probed once. Both maps die
/// with the call. Nothing is kept for the next request, which asks the file
/// system again.
fn solid_js_by_file<'a>(
    paths: impl Iterator<Item = &'a str>,
    disk: &mut impl InstanceFs,
) -> BTreeMap<String, String> {
    let mut by_raw: HashMap<&'a Path, Option<Rc<str>>> = HashMap::new();
    let mut nearest: HashMap<PathBuf, Option<Rc<str>>> = HashMap::new();
    let mut solid_js_by_file = BTreeMap::new();
    for path in paths {
        let Some(directory) = Path::new(path).parent() else {
            continue;
        };
        let resolved = match by_raw.get(directory) {
            Some(resolved) => resolved.clone(),
            None => {
                // Node resolves from the file's real directory: a file reached
                // through a symlinked workspace package sees the workspace's
                // own `node_modules`, not the linking project's.
                let real = disk
                    .canonicalize(directory)
                    .unwrap_or_else(|_| directory.to_path_buf());
                let resolved = nearest_solid_js(&real, &mut nearest, disk);
                by_raw.insert(directory, resolved.clone());
                resolved
            }
        };
        if let Some(resolved) = resolved {
            solid_js_by_file.insert(path.to_owned(), resolved.to_string());
        }
    }
    solid_js_by_file
}

/// The real directory of the nearest `node_modules/solid-js` that holds a
/// `package.json`, from `directory` up. `known` holds the answer for every
/// directory a lookup of this request already passed through, so a walk ends
/// at the first one it knows (its answer is the same, being the answer for
/// the directory above it). The directories are probed nearest first, each
/// at most once.
fn nearest_solid_js(
    directory: &Path,
    known: &mut HashMap<PathBuf, Option<Rc<str>>>,
    disk: &mut impl InstanceFs,
) -> Option<Rc<str>> {
    let mut passed = Vec::new();
    let mut answer = None;
    for ancestor in directory.ancestors() {
        if let Some(hit) = known.get(ancestor) {
            answer = hit.clone();
            break;
        }
        let package = ancestor.join("node_modules").join("solid-js");
        if disk.is_file(&package.join("package.json")) {
            let real = disk.canonicalize(&package).unwrap_or(package);
            answer = Some(Rc::from(real.to_string_lossy()));
            known.insert(ancestor.to_path_buf(), answer.clone());
            break;
        }
        passed.push(ancestor);
    }
    for ancestor in passed {
        known.insert(ancestor.to_path_buf(), answer.clone());
    }
    answer
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
    let mut route_installs = BTreeMap::new();
    for root in package_install_roots(facts, ROUTE_PACKAGE) {
        let core_root = route_core_root(&root);
        let core = core_root.as_ref().and_then(|core| {
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(core.join("package.json")).ok()?).ok()?;
            let version = manifest.get("version")?.as_str()?.to_owned();
            Some((version, package_digest(core)?))
        });
        route_installs.insert(
            root.join("package.json").to_string_lossy().into_owned(),
            RouteInstall {
                router_digest: package_digest(&root),
                core_directory: core_root.map(|core| core.to_string_lossy().into_owned()),
                core,
            },
        );
    }
    RouterInstallEvidence {
        replacement_recorded: replacement_recorded(project),
        digests,
        route_replacement_recorded: route_replacement_recorded(project),
        route_installs,
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
    replacement_recorded_for(project, &[ROUTER_PACKAGE], &["solidjs", "router"])
}

/// ADR 0334: [`replacement_recorded`] for `@tanstack/solid-router` and the
/// `@tanstack/router-core` the premise also reads.
#[must_use]
pub fn route_replacement_recorded(project: &Path) -> bool {
    replacement_recorded_for(
        project,
        &[ROUTE_PACKAGE, ROUTE_CORE_PACKAGE],
        &["tanstack", "router"],
    )
}

/// Whether a record above `project` replaces one of `packages`; a patch file
/// whose lowercased name holds every one of `patch_words` counts.
fn replacement_recorded_for(project: &Path, packages: &[&str], patch_words: &[&str]) -> bool {
    let start = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or(project)
    };
    start.ancestors().any(|directory| {
        PATCH_RECORD_FILES.iter().any(|name| {
            packages
                .iter()
                .any(|package| record_names_package(&directory.join(name), package))
        }) || PATCH_DIRECTORIES.iter().any(|patches| {
            fs::read_dir(directory.join(patches)).is_ok_and(|entries| {
                entries.flatten().any(|entry| {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    patch_words.iter().all(|word| name.contains(word))
                })
            })
        })
    })
}

fn record_names_package(path: &Path, package: &str) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let lines = text.lines().collect::<Vec<_>>();
    let patched = lines.iter().enumerate().any(|(at, line)| {
        line.contains(package)
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
                .is_ok_and(|manifest| manifest_replaces_package(&manifest, package)))
}

fn manifest_replaces_package(manifest: &serde_json::Value, package: &str) -> bool {
    let not_plain =
        |value: &serde_json::Value| value.as_str().is_none_or(|spec| !plain_registry_spec(spec));
    let overrides = |table: Option<&serde_json::Value>| {
        table
            .and_then(serde_json::Value::as_object)
            .is_some_and(|table| {
                table
                    .iter()
                    .any(|(key, value)| key.contains(package) && not_plain(value))
            })
    };
    let dependencies = |table: Option<&serde_json::Value>| {
        table
            .and_then(serde_json::Value::as_object)
            .and_then(|table| table.get(package))
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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
            package_digest(&fixture("router-params-replaced-wrapper")),
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

    /// ADR 0324, 0335: the `router-params` and `route-config` fixtures ship the
    /// published `2.0.0-next.41`, which hashes to its recorded digest.
    #[test]
    fn the_published_next41_router_hashes_to_its_audited_digest() {
        let vocabulary = crate::dialect::default_dialect().vocabulary;
        let audited = vocabulary.audited_router_digest("2.0.0-next.41");
        assert!(audited.is_some());
        assert_eq!(
            vocabulary.audited_route_config_digest("2.0.0-next.41"),
            audited
        );
        for name in ["router-params", "route-config", "router-params-links"] {
            assert_eq!(package_digest(&fixture(name)).as_deref(), audited, "{name}");
        }
    }

    /// ADR 0334: the fixture's TanStack router and the router-core Node loads
    /// from it hash to the digests the dialect records for the audited
    /// release; a changed router-core byte does not.
    #[test]
    fn the_published_tanstack_router_and_its_core_hash_to_the_audited_digests() {
        let audited = crate::dialect::default_dialect()
            .vocabulary
            .audited_route_components("2.0.0-rc.6")
            .expect("audited release");
        let tanstack = |name: &str| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/reactive-ir")
                .join(name)
                .join("node_modules/@tanstack/solid-router")
        };
        let router = tanstack("route-components");
        assert_eq!(
            package_digest(&router).as_deref(),
            Some(audited.router_digest)
        );
        let core = route_core_root(&router).expect("router-core beside the router");
        assert_eq!(package_digest(&core).as_deref(), Some(audited.core_digest));
        let modified =
            route_core_root(&tanstack("route-components/variants/core-modified")).unwrap();
        assert_ne!(
            package_digest(&modified).as_deref(),
            Some(audited.core_digest)
        );
    }

    /// ADR 0334: the rc.8 fixture's router and the router-core beside it hash
    /// to the digests recorded for `2.0.0-rc.8`.
    #[test]
    fn the_published_rc8_tanstack_router_hashes_to_its_audited_digests() {
        let audited = crate::dialect::default_dialect()
            .vocabulary
            .audited_route_components("2.0.0-rc.8")
            .expect("audited release");
        let router = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/route-components/variants/rc8")
            .join("node_modules/@tanstack/solid-router");
        assert_eq!(
            package_digest(&router).as_deref(),
            Some(audited.router_digest)
        );
        let core = route_core_root(&router).expect("router-core beside the router");
        assert_eq!(package_digest(&core).as_deref(), Some(audited.core_digest));
    }

    /// ADR 0334: the rc.9 fixture's router and the router-core beside it hash
    /// to the digests recorded for `2.0.0-rc.9`.
    #[test]
    fn the_published_rc9_tanstack_router_hashes_to_its_audited_digests() {
        let audited = crate::dialect::default_dialect()
            .vocabulary
            .audited_route_components("2.0.0-rc.9")
            .expect("audited release");
        let router = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/route-components/variants/rc9")
            .join("node_modules/@tanstack/solid-router");
        assert_eq!(
            package_digest(&router).as_deref(),
            Some(audited.router_digest)
        );
        let core = route_core_root(&router).expect("router-core beside the router");
        assert_eq!(package_digest(&core).as_deref(), Some(audited.core_digest));
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

    /// A temporary project tree. `install` writes the `solid-js` that Node
    /// would find, and gives its real directory.
    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "solid-checker-instances-{}-{}-{tag}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self, relative: &str) -> PathBuf {
            let mut path = self.root.clone();
            path.extend(relative.split('/').filter(|part| !part.is_empty()));
            path
        }

        fn dir(&self, relative: &str) -> PathBuf {
            let path = self.path(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn install(&self, directory: &str) -> String {
            let package = self.dir(&format!("{directory}/node_modules/solid-js"));
            fs::write(package.join("package.json"), "{}").unwrap();
            fs::canonicalize(package)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        }

        /// The path of a file in `relative`'s directory; the file itself is
        /// never written, only the directories it names matter.
        fn file(&self, relative: &str) -> String {
            self.path(relative).to_string_lossy().into_owned()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// The disk, writing down what it is asked. `refuse` names a directory
    /// whose canonicalization fails.
    #[derive(Default)]
    struct Counting {
        canonicalized: Vec<PathBuf>,
        probed: Vec<PathBuf>,
        refuse: Option<PathBuf>,
    }

    impl InstanceFs for Counting {
        fn canonicalize(&mut self, path: &Path) -> std::io::Result<PathBuf> {
            self.canonicalized.push(path.to_path_buf());
            if self.refuse.as_deref() == Some(path) {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            fs::canonicalize(path)
        }

        fn is_file(&mut self, path: &Path) -> bool {
            self.probed.push(path.to_path_buf());
            path.is_file()
        }
    }

    impl Counting {
        fn asked_once(&self) {
            for (what, asked) in [
                ("canonicalized", &self.canonicalized),
                ("probed", &self.probed),
            ] {
                let mut sorted = asked.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), asked.len(), "{what} twice: {asked:?}");
            }
        }
    }

    fn lookup(files: &[String], disk: &mut Counting) -> BTreeMap<String, String> {
        solid_js_by_file(files.iter().map(String::as_str), disk)
    }

    /// The lookup as it was before a request shared its answers: every file
    /// canonicalizes its directory, and a directory's install is found by
    /// walking up from it.
    fn per_file(files: &[String]) -> BTreeMap<String, String> {
        let mut by_directory: HashMap<PathBuf, Option<String>> = HashMap::new();
        let mut found = BTreeMap::new();
        for file in files {
            let Some(directory) = Path::new(file).parent() else {
                continue;
            };
            let directory = fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
            let resolved = by_directory
                .entry(directory.clone())
                .or_insert_with(|| {
                    directory.ancestors().find_map(|ancestor| {
                        let package = ancestor.join("node_modules").join("solid-js");
                        package.join("package.json").is_file().then(|| {
                            fs::canonicalize(&package)
                                .unwrap_or(package)
                                .to_string_lossy()
                                .into_owned()
                        })
                    })
                })
                .clone();
            if let Some(resolved) = resolved {
                found.insert(file.clone(), resolved);
            }
        }
        found
    }

    /// Files of one directory cost one canonicalization and one probe of each
    /// directory up to the install, however many there are.
    #[test]
    fn files_that_share_a_directory_ask_the_file_system_once() {
        let tree = Tree::new("shared-parent");
        let install = tree.install("");
        tree.dir("src");
        let files = ["a.ts", "b.ts", "c.ts", "d.ts"]
            .map(|name| tree.file(&format!("src/{name}")))
            .to_vec();
        let mut disk = Counting::default();
        let found = lookup(&files, &mut disk);
        assert_eq!(found.len(), 4);
        assert!(found.values().all(|resolved| *resolved == install));
        assert_eq!(found, per_file(&files));
        // `src` itself, then the install found at the root.
        assert_eq!(disk.canonicalized.len(), 2, "{:?}", disk.canonicalized);
        assert_eq!(disk.probed.len(), 2, "{:?}", disk.probed);
        disk.asked_once();
    }

    /// Sibling directories under one install probe the directories they share
    /// once: the second walk ends at the first directory the first passed.
    #[test]
    fn directories_below_one_install_probe_their_shared_ancestors_once() {
        let tree = Tree::new("shared-ancestors");
        let install = tree.install("");
        for directory in ["a/x", "a/y", "b"] {
            tree.dir(directory);
        }
        let files = ["a/x/f.ts", "a/y/f.ts", "a/x/g.ts", "b/f.ts"]
            .map(|file| tree.file(file))
            .to_vec();
        let mut disk = Counting::default();
        let found = lookup(&files, &mut disk);
        assert!(found.values().all(|resolved| *resolved == install));
        assert_eq!(found, per_file(&files));
        // Three directories and the install; a/x, a and the root, then a/y,
        // then b.
        assert_eq!(disk.canonicalized.len(), 4, "{:?}", disk.canonicalized);
        assert_eq!(disk.probed.len(), 5, "{:?}", disk.probed);
        disk.asked_once();
    }

    /// The nearest install wins, a file with none above it has no entry, and
    /// an install nearer than the root's is found only from below it.
    #[test]
    fn a_nested_install_serves_only_the_directories_below_it() {
        let tree = Tree::new("nested");
        let root = tree.install("");
        let nested = tree.install("packages/a");
        assert_ne!(root, nested);
        tree.dir("packages/a/src/deep");
        tree.dir("packages/b/src");
        tree.dir("src");
        let files = [
            "packages/a/src/deep/x.ts",
            "packages/a/src/y.ts",
            "packages/a/z.ts",
            "packages/b/src/w.ts",
            "src/v.ts",
            "u.ts",
        ]
        .map(|file| tree.file(file))
        .to_vec();
        let mut disk = Counting::default();
        let found = lookup(&files, &mut disk);
        for (file, expected) in [
            ("packages/a/src/deep/x.ts", &nested),
            ("packages/a/src/y.ts", &nested),
            ("packages/a/z.ts", &nested),
            ("packages/b/src/w.ts", &root),
            ("src/v.ts", &root),
            ("u.ts", &root),
        ] {
            assert_eq!(found.get(&tree.file(file)), Some(expected), "{file}");
        }
        assert_eq!(found, per_file(&files));
        disk.asked_once();

        let bare = Tree::new("bare");
        bare.dir("src");
        let files = vec![bare.file("src/a.ts")];
        assert_eq!(lookup(&files, &mut Counting::default()), BTreeMap::new());
        assert_eq!(per_file(&files), BTreeMap::new());
    }

    /// A file reached through a symlinked workspace package resolves from the
    /// workspace's real directory, whichever path names it, and the linking
    /// project's own install does not serve it.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_workspace_resolves_from_its_real_directory() {
        let tree = Tree::new("symlinked-workspace");
        let app = tree.install("app");
        let workspace = tree.install("ws");
        assert_ne!(app, workspace);
        tree.dir("app/src");
        tree.dir("ws/src");
        std::os::unix::fs::symlink("../../ws", tree.path("app/node_modules/ws")).unwrap();
        let files = [
            "app/src/main.ts",
            "app/node_modules/ws/src/a.ts",
            "ws/src/a.ts",
        ]
        .map(|file| tree.file(file))
        .to_vec();
        let mut disk = Counting::default();
        let found = lookup(&files, &mut disk);
        assert_eq!(found[&files[0]], app);
        assert_eq!(found[&files[1]], workspace);
        assert_eq!(found[&files[2]], workspace);
        assert_eq!(found, per_file(&files));
        // Three raw directories, the two installs; the link and the real path
        // name one directory, which is probed once.
        assert_eq!(disk.canonicalized.len(), 5, "{:?}", disk.canonicalized);
        assert_eq!(disk.probed.len(), 4, "{:?}", disk.probed);
        disk.asked_once();
    }

    /// A directory that cannot be canonicalized is looked up as written, as it
    /// was per file, and the failure is asked once however many files it holds.
    #[test]
    fn a_directory_that_cannot_be_canonicalized_is_looked_up_as_written_once() {
        let tree = Tree::new("failed-canonicalize");
        let install = tree.install("");
        let present = tree.dir("src");
        let files = [
            "src/a.ts",
            "src/b.ts",
            "ghost/src/a.ts",
            "ghost/src/b.ts",
            "ghost/c.ts",
        ]
        .map(|file| tree.file(file))
        .to_vec();
        // `ghost` is no directory: canonicalizing it fails by itself. `src`
        // exists and is made to fail.
        let mut disk = Counting {
            refuse: Some(present.clone()),
            ..Counting::default()
        };
        let found = lookup(&files, &mut disk);
        assert_eq!(found.len(), files.len());
        assert!(found.values().all(|resolved| *resolved == install));
        assert_eq!(found, per_file(&files));
        for directory in [present, tree.path("ghost/src"), tree.path("ghost")] {
            let asked = disk
                .canonicalized
                .iter()
                .filter(|asked| **asked == directory)
                .count();
            assert_eq!(asked, 1, "{directory:?}");
        }
        // The refused directory is walked from its written path: its own
        // `node_modules` is probed under the unresolved root.
        assert!(
            disk.probed
                .contains(&tree.path("src/node_modules/solid-js/package.json")),
            "{:?}",
            disk.probed
        );
        disk.asked_once();
    }

    /// The answers die with the call: an install that appears between two
    /// requests is seen by the second.
    #[test]
    fn nothing_is_kept_for_the_next_request() {
        let tree = Tree::new("request-local");
        let root = tree.install("");
        tree.dir("packages/a/src");
        let files = vec![
            tree.file("packages/a/src/x.ts"),
            tree.file("packages/a/y.ts"),
        ];
        let before = lookup(&files, &mut Counting::default());
        assert!(before.values().all(|resolved| *resolved == root));
        let nearer = tree.install("packages/a");
        let after = lookup(&files, &mut Counting::default());
        assert!(after.values().all(|resolved| *resolved == nearer));
        assert_eq!(after, per_file(&files));
        fs::remove_dir_all(tree.path("packages/a/node_modules")).unwrap();
        let removed = lookup(&files, &mut Counting::default());
        assert_eq!(removed, before);
    }

    /// Over generated trees (installs at any depth, directories that do not
    /// exist, symlinks to other directories) the shared lookup answers what
    /// the per-file lookup did.
    #[test]
    fn the_shared_lookup_answers_what_the_per_file_lookup_did() {
        for seed in 1..=40_u64 {
            let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut next = move |bound: u64| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state % bound
            };
            let tree = Tree::new(&format!("generated-{seed}"));
            let mut directories = vec![String::new()];
            let mut at = 0;
            while at < directories.len() && directories.len() < 40 {
                let parent = directories[at].clone();
                at += 1;
                let depth = parent.matches('/').count() + usize::from(!parent.is_empty());
                if depth >= 4 {
                    continue;
                }
                for child in 0..(1 + next(3)) {
                    let name = if parent.is_empty() {
                        format!("d{child}")
                    } else {
                        format!("{parent}/d{child}")
                    };
                    // One in six is never created: its files name a directory
                    // that is not there.
                    if next(6) != 0 {
                        tree.dir(&name);
                    }
                    if next(4) == 0 {
                        tree.install(&name);
                    }
                    directories.push(name);
                }
            }
            if next(2) == 0 {
                tree.install("");
            }
            #[cfg(unix)]
            for link in 0..3 {
                let target = tree.path(&directories[next(directories.len() as u64) as usize]);
                let from = tree.path(&format!("link{link}"));
                if target.is_dir() {
                    let _ = std::os::unix::fs::symlink(&target, &from);
                    directories.push(format!("link{link}"));
                }
            }
            let mut files = Vec::new();
            for directory in &directories {
                for file in 0..next(3) {
                    files.push(tree.file(&format!("{directory}/f{file}.ts")));
                }
                // Below a link, and below a directory that may not exist.
                if next(3) == 0 {
                    files.push(tree.file(&format!("{directory}/sub/g.ts")));
                }
            }
            assert_eq!(
                lookup(&files, &mut Counting::default()),
                per_file(&files),
                "seed {seed}"
            );
        }
    }
}
