//! What a lookup step on disk observed (ADR 0915).
//!
//! Node's resolver treats every failed `stat` at a lookup step as "not here,
//! try the next place": a missing path, a dangling symlink and a permission
//! error alike. The checker certifies the application as it is built and run,
//! not this checkout, so it draws the line at `lstat` instead. A path is
//! absent only when nothing exists there: `lstat` answers `ENOENT` or
//! `ENOTDIR` and no existing ancestor is a symlink that does not resolve.
//! Anything else (a dangling link at the path or above it, a permission
//! error, a manifest that does not parse) is unobservable: an input exists
//! that the checker could not read, which is a missing fact and never
//! evidence of absence.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// `lstat` found nothing at the path for a reason that is an absence.
pub(crate) fn absent_kind(kind: ErrorKind) -> bool {
    matches!(kind, ErrorKind::NotFound | ErrorKind::NotADirectory)
}

/// One ancestor directory as a lookup step sees it.
#[derive(Clone)]
enum Step {
    /// Nothing there: the walk goes on up.
    Missing,
    /// An ordinary directory or file, or a link that resolves: the walk stops.
    Ordinary,
    /// A link that does not resolve, or an `lstat` error that is no absence.
    Broken(String),
}

/// The ancestor walk of many paths of one check: each ancestor directory is
/// classified once. Absent inputs share their parents (one fingerprint pass
/// holds thousands), so the walk costs one `lstat` per directory, not per
/// path. Made per check and dropped with it: never reused across requests.
#[derive(Default)]
pub(crate) struct Ancestors(HashMap<PathBuf, Step>);

impl Ancestors {
    /// The nearest existing ancestor of `path` that makes it unobservable: a
    /// symlink that does not resolve, or a directory `lstat` cannot see past.
    /// `None` when the nearest existing ancestor is an ordinary file or
    /// directory, so an `ENOENT` at `path` is a real absence.
    pub(crate) fn broken(&mut self, path: &Path) -> Option<(PathBuf, String)> {
        for ancestor in path.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                return None;
            }
            let step = match self.0.get(ancestor) {
                Some(step) => step.clone(),
                None => {
                    // Nothing exists under a directory with nothing there: no
                    // `lstat` for it (a package's absent `node_modules/<dep>`).
                    let step = if self.missing_parent(ancestor) {
                        Step::Missing
                    } else {
                        classify(ancestor)
                    };
                    self.0.insert(ancestor.to_owned(), step.clone());
                    step
                }
            };
            match step {
                Step::Missing => {}
                Step::Ordinary => return None,
                Step::Broken(why) => return Some((ancestor.to_owned(), why)),
            }
        }
        None
    }

    /// Whether `path`'s parent is already known to hold nothing, so nothing
    /// is at `path` either.
    pub(crate) fn missing_parent(&self, path: &Path) -> bool {
        path.parent()
            .and_then(|parent| self.0.get(parent))
            .is_some_and(|step| matches!(step, Step::Missing))
    }
}

fn classify(ancestor: &Path) -> Step {
    match fs::symlink_metadata(ancestor) {
        Ok(metadata) if metadata.file_type().is_symlink() => match fs::metadata(ancestor) {
            Ok(_) => Step::Ordinary,
            Err(error) => Step::Broken(link_failure(ancestor, &error)),
        },
        Ok(_) => Step::Ordinary,
        Err(error) if absent_kind(error.kind()) => Step::Missing,
        Err(error) => Step::Broken(error.to_string()),
    }
}

fn link_failure(path: &Path, error: &std::io::Error) -> String {
    match fs::read_link(path) {
        Ok(target) if error.kind() == ErrorKind::NotFound => {
            format!("a symlink to {}, which does not resolve", target.display())
        }
        Ok(target) => format!(
            "a symlink to {}, which cannot be followed: {error}",
            target.display()
        ),
        Err(_) => format!("a symlink that cannot be followed: {error}"),
    }
}

/// Why `path` is unobservable, or `None` when it is absent or readable.
/// Diagnosis only: callers decide unobservability from their own read and
/// ask this for the words.
pub(crate) fn unobservable(path: &Path) -> Option<(PathBuf, String)> {
    unobservable_in(path, &mut Ancestors::default())
}

/// [`unobservable`] sharing one check's [`Ancestors`].
pub(crate) fn unobservable_in(path: &Path, ancestors: &mut Ancestors) -> Option<(PathBuf, String)> {
    if ancestors.missing_parent(path) {
        return ancestors.broken(path);
    }
    match fs::symlink_metadata(path) {
        Err(error) if absent_kind(error.kind()) => ancestors.broken(path),
        Err(error) => Some((path.to_owned(), error.to_string())),
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                && let Err(error) = fs::metadata(path)
            {
                return Some((path.to_owned(), link_failure(path, &error)));
            }
            let followed = fs::metadata(path).ok()?;
            if followed.is_dir() {
                return match fs::read_dir(path) {
                    Ok(_) => None,
                    Err(error) => Some((path.to_owned(), error.to_string())),
                };
            }
            match fs::read(path) {
                Err(error) => Some((path.to_owned(), error.to_string())),
                Ok(bytes) if manifest(path) && !parses(&bytes) => {
                    Some((path.to_owned(), "not valid JSON".to_owned()))
                }
                Ok(_) => None,
            }
        }
    }
}

/// A package manifest: Node and every bundler parse it at the lookup step
/// that reaches it and stop with an error when it does not parse.
pub(crate) fn manifest(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "package.json")
}

/// Whether manifest bytes parse as JSON, as Node reads them (a leading UTF-8
/// byte-order mark is stripped first).
pub(crate) fn parses(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    serde_json::from_slice::<serde::de::IgnoredAny>(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-observed-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn nothing_there_is_absent_and_a_dangling_link_above_is_not() {
        let root = scratch("absent");
        // Nothing at all: absent, as Node sees it.
        assert_eq!(
            unobservable(&root.join("node_modules/x/package.json")),
            None
        );
        // A file where a directory was expected: ENOTDIR, also absent.
        fs::write(root.join("file"), "").unwrap();
        assert_eq!(unobservable(&root.join("file/package.json")), None);
        // A link to a directory that was deleted: Node skips it, the checker
        // cannot read what the application installs there.
        std::os::unix::fs::symlink(root.join("gone"), root.join("node_modules")).unwrap();
        let (at, why) = unobservable(&root.join("node_modules/x/package.json")).unwrap();
        assert_eq!(at, root.join("node_modules"));
        assert!(why.contains("does not resolve"), "{why}");
        let (at, _) = unobservable(&root.join("node_modules")).unwrap();
        assert_eq!(at, root.join("node_modules"));
        // A link that resolves is an ordinary path.
        fs::remove_file(root.join("node_modules")).unwrap();
        fs::create_dir(root.join("real")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("node_modules")).unwrap();
        assert_eq!(
            unobservable(&root.join("node_modules/x/package.json")),
            None
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_unparsable_manifest_is_unobservable_and_other_json_is_not_judged() {
        let root = scratch("manifest");
        fs::write(root.join("package.json"), "{ not json").unwrap();
        fs::write(root.join("data.json"), "{ not json").unwrap();
        let (at, why) = unobservable(&root.join("package.json")).unwrap();
        assert_eq!(
            (at, why.as_str()),
            (root.join("package.json"), "not valid JSON")
        );
        assert_eq!(unobservable(&root.join("data.json")), None);
        fs::write(root.join("package.json"), "{}").unwrap();
        assert_eq!(unobservable(&root.join("package.json")), None);
        fs::remove_dir_all(&root).unwrap();
    }
}
