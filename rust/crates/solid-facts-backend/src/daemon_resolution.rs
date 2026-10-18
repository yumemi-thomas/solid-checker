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
//! - every file the program holds, by content (a file outside the tracked
//!   sources -- a package's declarations -- edited in place);
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

/// Whether `path` still is what `recorded` saw. A file whose metadata is
/// unchanged is not reread.
fn still(path: &Path, recorded: &Observed) -> bool {
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

pub(crate) struct ResolutionWitness {
    observed: BTreeMap<PathBuf, Observed>,
}

impl ResolutionWitness {
    pub(crate) fn of(graph: &typefacts::ModuleGraph) -> Self {
        let mut observed = BTreeMap::new();
        let mut record = |path: PathBuf| {
            observed
                .entry(path)
                .or_insert_with_key(|path| observe(path));
        };
        for module in &graph.modules {
            record(PathBuf::from(module.path.as_ref()));
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
        Self { observed }
    }

    /// Whether every recorded entry is still as it was.
    pub(crate) fn current(&self) -> bool {
        self.observed
            .iter()
            .all(|(path, recorded)| still(path, recorded))
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
}
