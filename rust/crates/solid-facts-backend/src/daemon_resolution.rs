//! What module resolution read, observed so a retained daemon notices when
//! the filesystem would now resolve its program differently.
//!
//! A retained Type Facts session resolves each import once, when it opens.
//! Its tracked sources are the program's own files, so a relinked workspace
//! package, a `package.json` whose `types`/`exports`/`main` moved, a package
//! reinstalled at a different version in place, or an edited declaration file
//! under `node_modules` changes no tracked byte. Each of those makes a fresh
//! session resolve or read differently, and the retained one would keep
//! answering for the old program.
//!
//! The witness records, from the open program's own module graph:
//!
//! - every file the program holds outside the tracked sources, by content (a
//!   package's declarations edited in place). A tracked source's edit is the
//!   daemon's own overlay update, so observing its bytes here would turn
//!   every edit into a rebuild;
//! - for every non-relative import, each `node_modules/<name>` and
//!   `node_modules/@types/<name>` candidate on the walk from the importing
//!   file's directory to the root: absent, a directory (with its realpath),
//!   or a symlink (with its target and realpath), so creating, removing or
//!   relinking any of them, at any depth of a pnpm-style chain, moves it; and
//!   the `package.json` inside each present candidate, by content;
//! - for a `#` import, every `package.json` on the importer's walk up.
//!
//! Any difference rebuilds the program from a new Type Facts session.
//! Configuration is already the tracked tsconfig (`paths`, `baseUrl`,
//! `moduleResolution`); a tsconfig it `extends` is not observed here.

use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Observed {
    Absent,
    Directory {
        realpath: Option<PathBuf>,
    },
    Link {
        target: PathBuf,
        realpath: Option<PathBuf>,
    },
    File {
        stamp: (u64, u64, u64, i64, i64, i64, i64),
        hash: [u8; 32],
    },
    /// Something the walk cannot read: never equal to a later observation.
    Unreadable,
}

fn stamp(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// An entry as `lstat` sees it; a regular file (or a symlink's file target,
/// for a path the walk reads through) by content.
fn observe(path: &Path) -> Observed {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Observed::Absent;
    };
    if metadata.file_type().is_symlink() {
        return match fs::read_link(path) {
            Ok(target) => Observed::Link {
                target,
                realpath: fs::canonicalize(path).ok(),
            },
            Err(_) => Observed::Unreadable,
        };
    }
    if metadata.is_dir() {
        return Observed::Directory {
            realpath: fs::canonicalize(path).ok(),
        };
    }
    match fs::read(path) {
        Ok(bytes) => Observed::File {
            stamp: stamp(&metadata),
            hash: hash(&bytes),
        },
        Err(_) => Observed::Unreadable,
    }
}

/// Canonical paths of directories, each resolved once per check. Most
/// entries of a witness are `node_modules` candidates sharing a few parents,
/// and `canonicalize` walks every component.
#[derive(Default)]
struct RealPaths {
    known: std::sync::Mutex<std::collections::HashMap<PathBuf, Option<PathBuf>>>,
}

impl RealPaths {
    fn of(&self, directory: &Path) -> Option<PathBuf> {
        if let Ok(known) = self.known.lock()
            && let Some(real) = known.get(directory)
        {
            return real.clone();
        }
        let real = fs::canonicalize(directory).ok();
        if let Ok(mut known) = self.known.lock() {
            known.insert(directory.to_path_buf(), real.clone());
        }
        real
    }

    /// What [`observe`] records for a directory `path` that is no symlink: a
    /// real directory's canonical path is its parent's with its own name.
    fn directory(&self, path: &Path) -> Option<PathBuf> {
        match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                self.of(parent).map(|real| real.join(name))
            }
            _ => fs::canonicalize(path).ok(),
        }
    }
}

/// Whether `path` still is what `recorded` saw. A file whose metadata is
/// unchanged is not reread.
#[cfg(test)]
fn still(path: &Path, recorded: &Observed) -> bool {
    still_with(path, recorded, &RealPaths::default())
}

fn still_with(path: &Path, recorded: &Observed, real: &RealPaths) -> bool {
    if let Observed::File {
        stamp: recorded_stamp,
        hash: recorded_hash,
    } = recorded
    {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return false;
        };
        if metadata.file_type().is_symlink() || metadata.is_dir() {
            return false;
        }
        if stamp(&metadata) == *recorded_stamp {
            return true;
        }
        return fs::read(path).is_ok_and(|bytes| hash(&bytes) == *recorded_hash);
    }
    if let Observed::Directory {
        realpath: recorded_real,
    } = recorded
    {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return false;
        }
        return real.directory(path) == *recorded_real;
    }
    if *recorded == Observed::Unreadable {
        return false;
    }
    observe(path) == *recorded
}

/// The package directory name a bare specifier looks up: `@scope/name` or
/// `name`, without a subpath.
fn package_name(specifier: &str) -> Option<String> {
    let mut parts = specifier.split('/');
    let first = parts.next()?;
    if first.is_empty() {
        return None;
    }
    if first.starts_with('@') {
        let second = parts.next()?;
        return Some(format!("{first}/{second}"));
    }
    Some(first.to_owned())
}

/// The `@types` directory name TypeScript looks up for `name`.
fn types_name(name: &str) -> String {
    match name.strip_prefix('@') {
        Some(scoped) => format!("@types/{}", scoped.replacen('/', "__", 1)),
        None => format!("@types/{name}"),
    }
}

#[derive(Clone)]
pub(crate) struct ResolutionWitness {
    observed: BTreeMap<PathBuf, Observed>,
}

/// Drops every absent entry that lies under another absent entry. A path under
/// a missing ancestor is missing for the same reason, and the ancestor's own
/// entry already fails the moment that ancestor appears, so the descendant
/// adds a filesystem read per check and no information. (`BTreeMap` orders
/// paths by component, so a path's descendants follow it contiguously.)
///
/// Only absent descendants go. One recorded present, by an earlier request,
/// under an ancestor observed absent now, says the ancestor was removed since:
/// it stays, and fails the next check (review R1 finding 2).
fn prune_shadowed_absent(observed: &mut BTreeMap<PathBuf, Observed>) {
    let mut shadow: Option<PathBuf> = None;
    let mut shadowed = Vec::new();
    for (path, seen) in observed.iter() {
        if let Some(absent) = &shadow
            && path.starts_with(absent)
        {
            if *seen == Observed::Absent {
                shadowed.push(path.clone());
            }
            continue;
        }
        shadow = (*seen == Observed::Absent).then(|| path.clone());
    }
    for path in shadowed {
        observed.remove(&path);
    }
}

impl ResolutionWitness {
    /// `tracked` names the sources the caller reconciles by content itself.
    /// A tracked path spelled differently from the module's is observed, so
    /// a mismatch costs a rebuild, never a missed edit.
    pub(crate) fn of(graph: &typefacts::ModuleGraph, tracked: impl Fn(&str) -> bool) -> Self {
        let mut witness = Self {
            observed: BTreeMap::new(),
        };
        witness.extend(graph, tracked);
        witness
    }

    /// Adds what `graph` read to a witness that is still [`Self::current`],
    /// keeping every entry already recorded (each still as it was). After an
    /// overlay update that kept the program's files, `graph` scoped to the
    /// edited files makes a superset of a fresh witness: an import the edit
    /// dropped stays observed, which can only cost a rebuild.
    pub(crate) fn extend(
        &mut self,
        graph: &typefacts::ModuleGraph,
        tracked: impl Fn(&str) -> bool,
    ) {
        let observed = &mut self.observed;
        let mut record = |path: PathBuf| {
            observed
                .entry(path)
                .or_insert_with_key(|path| observe(path));
        };
        for module in &graph.modules {
            if !tracked(module.path.as_ref()) {
                record(PathBuf::from(module.path.as_ref()));
            }
        }
        for import in &graph.imports {
            let text = import.text.as_ref();
            if text.starts_with('.') || text.starts_with('/') {
                continue;
            }
            for path in [
                &import.resolved_path,
                &import.included_path,
                &import.symlink_path,
            ] {
                if !path.is_empty() {
                    record(PathBuf::from(path.as_ref()));
                }
            }
            let importer = Path::new(import.specifier.path.as_ref());
            let Some(directory) = importer.parent() else {
                continue;
            };
            if text.starts_with('#') {
                for ancestor in directory.ancestors() {
                    record(ancestor.join("package.json"));
                }
                continue;
            }
            let Some(name) = package_name(text) else {
                continue;
            };
            let types = types_name(&name);
            for ancestor in directory.ancestors() {
                for candidate in [
                    ancestor.join("node_modules").join(&name),
                    ancestor.join("node_modules").join(&types),
                ] {
                    record(candidate.join("package.json"));
                    record(candidate);
                }
                // A scope directory that is itself a link moves every
                // package under it.
                if let Some(scope) = name.split_once('/').map(|(scope, _)| scope) {
                    record(ancestor.join("node_modules").join(scope));
                }
                record(ancestor.join("node_modules"));
            }
        }
        prune_shadowed_absent(&mut self.observed);
    }

    /// Whether every recorded entry is still as it was. The entries are
    /// independent filesystem reads (tens of thousands for a monorepo's
    /// `node_modules` walks), checked on six workers; any change answers no.
    pub(crate) fn current(&self) -> bool {
        const WORKERS: usize = 6;
        let entries = self.observed.iter().collect::<Vec<_>>();
        let real = RealPaths::default();
        if entries.len() < 512 {
            return entries
                .iter()
                .all(|(path, recorded)| still_with(path, recorded, &real));
        }
        let moved = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            for worker in 0..WORKERS {
                let (entries, moved, real) = (&entries, &moved, &real);
                scope.spawn(move || {
                    for (path, recorded) in entries.iter().skip(worker).step_by(WORKERS) {
                        if moved.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        if !still_with(path, recorded, real) {
                            moved.store(true, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }
                    }
                });
            }
        });
        !moved.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_strip_subpaths_and_keep_scopes() {
        assert_eq!(package_name("solid-js/web").as_deref(), Some("solid-js"));
        assert_eq!(package_name("@ws/lib/sub").as_deref(), Some("@ws/lib"));
        assert_eq!(types_name("@ws/lib"), "@types/ws__lib");
        assert_eq!(types_name("lodash"), "@types/lodash");
    }

    #[test]
    fn a_relink_an_edit_and_a_new_entry_each_move_the_observation() {
        let root = std::env::temp_dir().join(format!("sc-resolution-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("one")).unwrap();
        fs::create_dir_all(root.join("two")).unwrap();
        fs::write(root.join("one/package.json"), "{}").unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(root.join("one"), &link).unwrap();
        let manifest = root.join("one/package.json");
        let absent = root.join("absent");
        let observations = [
            (link.clone(), observe(&link)),
            (manifest.clone(), observe(&manifest)),
            (absent.clone(), observe(&absent)),
        ];
        assert!(observations.iter().all(|(path, seen)| still(path, seen)));
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(root.join("two"), &link).unwrap();
        assert!(!still(&link, &observations[0].1));
        fs::write(&manifest, r#"{"types":"x.d.ts"}"#).unwrap();
        assert!(!still(&manifest, &observations[1].1));
        fs::create_dir_all(&absent).unwrap();
        assert!(!still(&absent, &observations[2].1));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_absent_entry_under_an_absent_one_adds_nothing() {
        let mut observed = BTreeMap::new();
        let absent = || Observed::Absent;
        for path in ["/a", "/a/x", "/a/x/y", "/a2", "/b/c", "/b/c/d"] {
            observed.insert(PathBuf::from(path), absent());
        }
        observed.insert(
            PathBuf::from("/b"),
            Observed::Directory {
                realpath: Some(PathBuf::from("/b")),
            },
        );
        prune_shadowed_absent(&mut observed);
        let kept = observed
            .keys()
            .map(|p| p.to_str().unwrap())
            .collect::<Vec<_>>();
        // `/a2` is not under `/a`; `/b/c/d` is under the absent `/b/c`.
        assert_eq!(kept, ["/a", "/a2", "/b", "/b/c"]);
    }

    #[test]
    fn an_entry_recorded_present_under_one_now_absent_is_kept() {
        // An earlier request recorded the package's declarations; this one
        // first observes the package directory, after it was removed.
        let mut observed = BTreeMap::new();
        observed.insert(PathBuf::from("/m/node_modules/@s/pkg"), Observed::Absent);
        let declarations = Observed::File {
            stamp: (1, 2, 3, 4, 5, 6, 7),
            hash: [0; 32],
        };
        observed.insert(
            PathBuf::from("/m/node_modules/@s/pkg/index.d.ts"),
            declarations.clone(),
        );
        observed.insert(
            PathBuf::from("/m/node_modules/@s/pkg/package.json"),
            Observed::Absent,
        );
        prune_shadowed_absent(&mut observed);
        assert_eq!(
            observed
                .keys()
                .map(|p| p.to_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "/m/node_modules/@s/pkg",
                "/m/node_modules/@s/pkg/index.d.ts"
            ]
        );
        assert!(!still(
            Path::new("/m/node_modules/@s/pkg/index.d.ts"),
            &declarations
        ));
    }

    #[test]
    fn a_directory_whose_ancestor_is_relinked_moves_the_observation() {
        let root = std::env::temp_dir().join(format!("sc-resolution-dirs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let root = {
            fs::create_dir_all(&root).unwrap();
            root.canonicalize().unwrap()
        };
        fs::create_dir_all(root.join("p/d")).unwrap();
        fs::create_dir_all(root.join("elsewhere/d")).unwrap();
        let entry = root.join("p/d");
        let seen = observe(&entry);
        let real = RealPaths::default();
        assert!(still_with(&entry, &seen, &real));
        // A fresh memo, as the next check has.
        fs::rename(root.join("p"), root.join("moved")).unwrap();
        std::os::unix::fs::symlink(root.join("moved"), root.join("p")).unwrap();
        assert!(!still_with(&entry, &seen, &RealPaths::default()));
        // The directory itself replaced by a link, or by a file.
        fs::remove_file(root.join("p")).unwrap();
        fs::rename(root.join("moved"), root.join("p")).unwrap();
        assert!(still_with(&entry, &seen, &RealPaths::default()));
        fs::remove_dir(&entry).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere/d"), &entry).unwrap();
        assert!(!still_with(&entry, &seen, &RealPaths::default()));
        fs::remove_file(&entry).unwrap();
        fs::write(&entry, b"x").unwrap();
        assert!(!still_with(&entry, &seen, &RealPaths::default()));
        fs::remove_dir_all(&root).unwrap();
    }
}
