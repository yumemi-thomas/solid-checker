//! The overlay file system (Go: `tsgo/overlay.go`). Client-sent edits and the
//! `-project-config` bytes are overlaid on the real file system, so the
//! program sees an edited buffer exactly where it would be on disk while
//! nothing is written. An overlay is immutable once a program is built from it:
//! `update` clones it, applies the change set to the clone, and builds the
//! candidate program from the clone, so a failed update leaves the previous
//! overlay untouched.

use crate::gocompat::{clean, dir};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tsrs_vfs::{DirEntry, Entries, FS, FileInfo, FileMode, FsError, WalkDirError};

#[derive(Default)]
struct State {
    files: BTreeMap<String, String>,
    deleted: BTreeSet<String>,
}

/// The callback of [`OverlayFs::walk_dir`] (tsrs `walk_dir`'s own shape).
pub type WalkFn<'a> =
    dyn FnMut(&str, Option<&DirEntry>, Option<&FsError>) -> Result<(), WalkDirError> + 'a;

pub struct OverlayFs {
    base: Arc<dyn FS>,
    state: RwLock<State>,
}

impl OverlayFs {
    #[must_use]
    pub fn new(base: Arc<dyn FS>) -> Self {
        Self {
            base,
            state: RwLock::new(State::default()),
        }
    }

    /// Go: `clone`.
    #[must_use]
    pub fn duplicate(&self) -> Self {
        let state = self.read();
        Self {
            base: Arc::clone(&self.base),
            state: RwLock::new(State {
                files: state.files.clone(),
                deleted: state.deleted.clone(),
            }),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Go: `set`.
    pub fn set(&self, path: &str, source: &str) {
        let path = clean_overlay_path(path);
        let mut state = self.write();
        state.deleted.remove(&path);
        state.files.insert(path, source.to_owned());
    }

    /// Go: `delete`.
    pub fn delete(&self, path: &str) {
        let path = clean_overlay_path(path);
        let mut state = self.write();
        state.files.remove(&path);
        state.deleted.insert(path);
    }

    /// Go: `WalkDir`. The tsrs file-system trait has no walk method; the free
    /// `walk_dir` walks any `FS` through `stat` and `get_accessible_entries`,
    /// both of which this overlay answers, so Go's hand-merged walk (base
    /// entries minus deletions, then overlay-only entries in sorted order) is
    /// the same traversal here.
    pub fn walk_dir(&self, root: &str, walk: &mut WalkFn<'_>) -> Result<(), FsError> {
        tsrs_vfs::walk_dir(self, root, walk)
    }
}

/// Go: `cleanOverlayPath`. URL-like names (`bundled:///...`) are not file
/// paths and pass through.
#[must_use]
pub fn clean_overlay_path(path: &str) -> String {
    if path.contains("://") {
        path.to_owned()
    } else {
        clean(path)
    }
}

/// Go: `isDescendant`: `path` is strictly inside `directory`.
fn is_descendant(path: &str, directory: &str) -> bool {
    relative(directory, path)
        .is_some_and(|relative| relative != "." && relative != ".." && !relative.starts_with("../"))
}

/// `filepath.Rel` for the cleaned absolute or cleaned relative paths the
/// overlay holds; `None` where Go returns an error (mixed absolute and
/// relative).
fn relative(base: &str, target: &str) -> Option<String> {
    let base = clean(base);
    let target = clean(target);
    if base.starts_with('/') != target.starts_with('/') {
        return None;
    }
    if base == target {
        return Some(".".to_owned());
    }
    let split = |path: &str| -> Vec<String> {
        path.split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .map(str::to_owned)
            .collect()
    };
    let (base_parts, target_parts) = (split(&base), split(&target));
    let common = base_parts
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();
    if base_parts[common..].iter().any(|part| part == "..") {
        return None;
    }
    let mut parts: Vec<String> = base_parts[common..]
        .iter()
        .map(|_| "..".to_owned())
        .collect();
    parts.extend(target_parts[common..].iter().cloned());
    Some(if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    })
}

fn base_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn file_info(path: &str, size: usize) -> FileInfo {
    FileInfo {
        name: base_of(path).to_owned(),
        size: i64::try_from(size).unwrap_or(i64::MAX),
        mode: FileMode::None,
        mod_time: None,
    }
}

impl FS for OverlayFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.base.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        let path = clean_overlay_path(path);
        {
            let state = self.read();
            if state.deleted.contains(&path) {
                return false;
            }
            if state.files.contains_key(&path) {
                return true;
            }
        }
        self.base.file_exists(&path)
    }

    fn read_file(&self, path: &str) -> Option<String> {
        let path = clean_overlay_path(path);
        {
            let state = self.read();
            if state.deleted.contains(&path) {
                return None;
            }
            if let Some(source) = state.files.get(&path) {
                return Some(source.clone());
            }
        }
        self.base.read_file(&path)
    }

    fn write_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.base.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.base.append_file(path, data)
    }

    fn remove(&self, path: &str) -> Result<(), String> {
        self.base.remove(path)
    }

    fn chtimes(&self, path: &str, a_time: SystemTime, m_time: SystemTime) -> Result<(), String> {
        self.base.chtimes(path, a_time, m_time)
    }

    fn directory_exists(&self, path: &str) -> bool {
        if self.base.directory_exists(path) {
            return true;
        }
        let directory = clean_overlay_path(path);
        self.read()
            .files
            .keys()
            .any(|file| is_descendant(file, &directory))
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        let mut entries = self.base.get_accessible_entries(path);
        let mut files: BTreeSet<String> = entries.files.iter().cloned().collect();
        let mut directories: BTreeSet<String> = entries.directories.iter().cloned().collect();
        let directory = clean_overlay_path(path);
        {
            let state = self.read();
            for file in &state.deleted {
                if dir(file) == directory {
                    files.remove(base_of(file));
                }
            }
            for file in state.files.keys() {
                let Some(relative) = relative(&directory, file) else {
                    continue;
                };
                if relative == "." || relative == ".." || relative.starts_with("../") {
                    continue;
                }
                match relative.split_once('/') {
                    None => {
                        files.insert(relative);
                    }
                    Some((first, _)) => {
                        directories.insert(first.to_owned());
                    }
                }
            }
        }
        entries.files = files.iter().cloned().collect();
        entries.directories = directories.iter().cloned().collect();
        if let Some(symlinks) = &mut entries.symlinks {
            symlinks.retain(|name| files.contains(name) || directories.contains(name));
        }
        entries
    }

    fn stat(&self, path: &str) -> Option<FileInfo> {
        let path = clean_overlay_path(path);
        {
            let state = self.read();
            if state.deleted.contains(&path) {
                return None;
            }
            if let Some(source) = state.files.get(&path) {
                return Some(file_info(&path, source.len()));
            }
        }
        self.base.stat(&path)
    }

    fn realpath(&self, path: &str) -> String {
        let path = clean_overlay_path(path);
        if self.read().files.contains_key(&path) {
            return path;
        }
        self.base.realpath(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A base file system with fixed files, standing in for the disk.
    struct Base {
        files: Vec<(&'static str, &'static str)>,
    }

    impl FS for Base {
        fn use_case_sensitive_file_names(&self) -> bool {
            true
        }
        fn file_exists(&self, path: &str) -> bool {
            self.files.iter().any(|(name, _)| *name == path)
        }
        fn read_file(&self, path: &str) -> Option<String> {
            self.files
                .iter()
                .find(|(name, _)| *name == path)
                .map(|(_, text)| (*text).to_owned())
        }
        fn write_file(&self, _: &str, _: &str) -> Result<(), String> {
            Err("read only".to_owned())
        }
        fn append_file(&self, _: &str, _: &str) -> Result<(), String> {
            Err("read only".to_owned())
        }
        fn remove(&self, _: &str) -> Result<(), String> {
            Err("read only".to_owned())
        }
        fn chtimes(&self, _: &str, _: SystemTime, _: SystemTime) -> Result<(), String> {
            Ok(())
        }
        fn directory_exists(&self, path: &str) -> bool {
            self.files
                .iter()
                .any(|(name, _)| is_descendant(name, &clean(path)))
        }
        fn get_accessible_entries(&self, path: &str) -> Entries {
            let directory = clean(path);
            let mut entries = Entries::default();
            for (name, _) in &self.files {
                if let Some(relative) = relative(&directory, name)
                    && !relative.starts_with("..")
                    && relative != "."
                {
                    match relative.split_once('/') {
                        None => entries.files.push(relative),
                        Some((first, _)) => {
                            if !entries.directories.iter().any(|known| known == first) {
                                entries.directories.push(first.to_owned());
                            }
                        }
                    }
                }
            }
            entries
        }
        fn stat(&self, path: &str) -> Option<FileInfo> {
            self.file_exists(path)
                .then(|| file_info(path, 0))
                .or_else(|| {
                    self.directory_exists(path).then(|| FileInfo {
                        name: base_of(path).to_owned(),
                        size: 0,
                        mode: FileMode::Dir,
                        mod_time: None,
                    })
                })
        }
        fn realpath(&self, path: &str) -> String {
            path.to_owned()
        }
    }

    fn overlay() -> OverlayFs {
        OverlayFs::new(Arc::new(Base {
            files: vec![("/p/a.ts", "a"), ("/p/sub/b.ts", "b"), ("/p/gone.ts", "g")],
        }))
    }

    #[test]
    fn set_overrides_base_and_delete_hides_it() {
        let fs = overlay();
        assert_eq!(fs.read_file("/p/a.ts").as_deref(), Some("a"));
        fs.set("/p//a.ts", "edited");
        assert_eq!(fs.read_file("/p/a.ts").as_deref(), Some("edited"));
        assert_eq!(fs.stat("/p/a.ts").unwrap().size, 6);
        fs.delete("/p/gone.ts");
        assert!(!fs.file_exists("/p/gone.ts"));
        assert_eq!(fs.read_file("/p/gone.ts"), None);
        assert!(fs.stat("/p/gone.ts").is_none());
        // Setting again resurrects it.
        fs.set("/p/gone.ts", "back");
        assert_eq!(fs.read_file("/p/gone.ts").as_deref(), Some("back"));
    }

    #[test]
    fn overlay_only_files_and_directories_are_visible() {
        let fs = overlay();
        fs.set("/p/new/deep/c.ts", "c");
        fs.set("/p/d.ts", "d");
        assert!(fs.directory_exists("/p/new"));
        assert!(fs.directory_exists("/p/new/deep"));
        assert!(!fs.directory_exists("/p/missing"));
        let entries = fs.get_accessible_entries("/p");
        assert_eq!(entries.files, ["a.ts", "d.ts", "gone.ts"]);
        assert_eq!(entries.directories, ["new", "sub"]);
        fs.delete("/p/gone.ts");
        assert_eq!(fs.get_accessible_entries("/p").files, ["a.ts", "d.ts"]);
        assert_eq!(fs.realpath("/p/d.ts"), "/p/d.ts");
    }

    #[test]
    fn urls_pass_through_unclean() {
        let fs = overlay();
        fs.set("bundled:///libs/x.d.ts", "x");
        assert_eq!(fs.read_file("bundled:///libs/x.d.ts").as_deref(), Some("x"));
        assert_eq!(
            clean_overlay_path("bundled:///libs/../x"),
            "bundled:///libs/../x"
        );
        assert_eq!(clean_overlay_path("/a/./b/../c"), "/a/c");
    }

    #[test]
    fn duplicate_is_independent() {
        let fs = overlay();
        fs.set("/p/a.ts", "one");
        let copy = fs.duplicate();
        copy.set("/p/a.ts", "two");
        copy.delete("/p/sub/b.ts");
        assert_eq!(fs.read_file("/p/a.ts").as_deref(), Some("one"));
        assert_eq!(fs.read_file("/p/sub/b.ts").as_deref(), Some("b"));
        assert_eq!(copy.read_file("/p/a.ts").as_deref(), Some("two"));
        assert_eq!(copy.read_file("/p/sub/b.ts"), None);
    }

    #[test]
    fn walk_visits_base_minus_deleted_then_overlay_only_entries() {
        let fs = overlay();
        fs.set("/p/new/c.ts", "c");
        fs.delete("/p/gone.ts");
        let mut seen = Vec::new();
        fs.walk_dir("/p", &mut |path, _, _| {
            seen.push(path.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            seen,
            [
                "/p",
                "/p/a.ts",
                "/p/new",
                "/p/new/c.ts",
                "/p/sub",
                "/p/sub/b.ts"
            ]
        );
    }

    #[test]
    fn symlink_names_are_pruned_to_surviving_entries() {
        struct Linked(Base);
        impl FS for Linked {
            fn use_case_sensitive_file_names(&self) -> bool {
                true
            }
            fn file_exists(&self, p: &str) -> bool {
                self.0.file_exists(p)
            }
            fn read_file(&self, p: &str) -> Option<String> {
                self.0.read_file(p)
            }
            fn write_file(&self, p: &str, d: &str) -> Result<(), String> {
                self.0.write_file(p, d)
            }
            fn append_file(&self, p: &str, d: &str) -> Result<(), String> {
                self.0.append_file(p, d)
            }
            fn remove(&self, p: &str) -> Result<(), String> {
                self.0.remove(p)
            }
            fn chtimes(&self, p: &str, a: SystemTime, m: SystemTime) -> Result<(), String> {
                self.0.chtimes(p, a, m)
            }
            fn directory_exists(&self, p: &str) -> bool {
                self.0.directory_exists(p)
            }
            fn get_accessible_entries(&self, p: &str) -> Entries {
                let mut entries = self.0.get_accessible_entries(p);
                entries.symlinks = Some(
                    ["a.ts", "gone.ts"]
                        .iter()
                        .map(|s| (*s).to_owned())
                        .collect(),
                );
                entries
            }
            fn stat(&self, p: &str) -> Option<FileInfo> {
                self.0.stat(p)
            }
            fn realpath(&self, p: &str) -> String {
                self.0.realpath(p)
            }
        }
        let fs = OverlayFs::new(Arc::new(Linked(Base {
            files: vec![("/p/a.ts", "a"), ("/p/gone.ts", "g")],
        })));
        fs.delete("/p/gone.ts");
        let symlinks = fs.get_accessible_entries("/p").symlinks.unwrap();
        assert!(symlinks.contains("a.ts") && !symlinks.contains("gone.ts"));
    }
}
