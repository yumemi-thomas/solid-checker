//! The overlay file system (Go: `tsgo/overlay.go`). Client-sent edits and the
//! `-project-config` bytes are overlaid on the real file system, so the
//! program sees an edited buffer exactly where it would be on disk while
//! nothing is written. An overlay is immutable once a program is built from it:
//! `update` clones it, applies the change set to the clone, and builds the
//! candidate program from the clone, so a failed update leaves the previous
//! overlay untouched.

use crate::gocompat::{clean, clean_cow, dir};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tsrs_core::collections::SyncMap;
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
    /// What the base answered during the program build in progress, if one
    /// is: present exactly while a [`BuildScope`] is open.
    lookups: RwLock<Option<Lookups>>,
}

/// The base file system's answers during one program build, by operation.
///
/// A program build asks the same questions of the disk again and again: every
/// file that imports `./util` probes the same candidate paths, and every
/// bare specifier walks the same `node_modules` ancestors. TypeScript assumes
/// one file system snapshot for a build, so within the build the first answer
/// stands. The answers live only as long as the [`BuildScope`] that created
/// them (nothing is kept between builds: the next build asks the disk again),
/// and they sit below the overlay's own state, which is consulted first, so an
/// edited or deleted file never reaches them.
#[derive(Default)]
struct Lookups {
    /// Whether to count hits and misses (a trace is listening).
    counting: bool,
    files: SyncMap<String, bool>,
    directories: SyncMap<String, bool>,
    stats: SyncMap<String, Option<FileInfo>>,
    realpaths: SyncMap<String, String>,
    entries: SyncMap<String, Entries>,
    hits: AtomicU64,
    misses: AtomicU64,
    /// `read_file` calls that reached the base (contents are not kept).
    reads: AtomicU64,
}

impl Lookups {
    fn remember<V: Clone>(
        &self,
        map: &SyncMap<String, V>,
        key: &str,
        ask: impl FnOnce() -> V,
    ) -> V {
        if let Some(answer) = map.load(key) {
            self.count(&self.hits);
            return answer;
        }
        self.count(&self.misses);
        let answer = ask();
        map.store(key.to_owned(), answer.clone());
        answer
    }

    fn count(&self, counter: &AtomicU64) {
        if self.counting {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn stats(&self) -> LookupStats {
        LookupStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            reads: self.reads.load(Ordering::Relaxed),
        }
    }
}

/// What a [`BuildScope`] answered from its lookups and what it asked the disk
/// (all zero unless the scope was counting).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LookupStats {
    pub hits: u64,
    pub misses: u64,
    pub reads: u64,
}

/// One program build's lookups: open from [`OverlayFs::build_scope`] until it
/// is finished or dropped. A scope opened while another is open on the same
/// overlay is inert and leaves the outer one's lookups to it.
#[must_use = "the lookups last exactly as long as the scope"]
pub struct BuildScope<'a> {
    fs: Option<&'a OverlayFs>,
}

impl BuildScope<'_> {
    /// Ends the scope, returning its counters.
    pub fn finish(mut self) -> LookupStats {
        self.fs
            .take()
            .and_then(OverlayFs::end_build)
            .map_or_else(LookupStats::default, |lookups| lookups.stats())
    }
}

impl Drop for BuildScope<'_> {
    fn drop(&mut self) {
        if let Some(fs) = self.fs.take() {
            drop(fs.end_build());
        }
    }
}

impl OverlayFs {
    #[must_use]
    pub fn new(base: Arc<dyn FS>) -> Self {
        Self {
            base,
            state: RwLock::new(State::default()),
            lookups: RwLock::new(None),
        }
    }

    /// Go: `clone`. The copy has no lookups: every build makes its own.
    #[must_use]
    pub fn duplicate(&self) -> Self {
        let state = self.read();
        Self {
            base: Arc::clone(&self.base),
            state: RwLock::new(State {
                files: state.files.clone(),
                deleted: state.deleted.clone(),
            }),
            lookups: RwLock::new(None),
        }
    }

    /// Opens the lookups of one program build: until the scope ends, the
    /// base's answers are asked once each. `counting` keeps the hit and miss
    /// counters (only a listening trace needs them).
    pub fn build_scope(&self, counting: bool) -> BuildScope<'_> {
        let mut lookups = self
            .lookups
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if lookups.is_some() || typefacts::audit_off("fs_lookups") {
            return BuildScope { fs: None };
        }
        *lookups = Some(Lookups {
            counting,
            ..Lookups::default()
        });
        BuildScope { fs: Some(self) }
    }

    fn end_build(&self) -> Option<Lookups> {
        self.lookups
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// Asks the base through the build's lookups, if a build is open.
    fn through<V: Clone>(
        &self,
        pick: impl FnOnce(&Lookups) -> &SyncMap<String, V>,
        key: &str,
        ask: impl FnOnce() -> V,
    ) -> V {
        let lookups = self
            .lookups
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match lookups.as_ref() {
            Some(lookups) => lookups.remember(pick(lookups), key, ask),
            None => ask(),
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

/// The first path element of `target` below `base`, and whether anything
/// follows it, when `target` is strictly inside `base`: the answer
/// `filepath.Rel(base, target)` gives for cleaned paths of the same rootedness,
/// read as "a relative path that is not `.` and does not start with `..`",
/// without building the relative path. Both arguments are cleaned first.
fn inside<'t>(base: &str, target: &'t str) -> Option<(&'t str, bool)> {
    if base.starts_with('/') != target.starts_with('/') {
        return None;
    }
    let mut base_parts = base
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".");
    let mut target_parts = target
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".");
    loop {
        match (base_parts.next(), target_parts.next()) {
            (Some(left), Some(right)) if left == right => {}
            // `target` ends first or leaves `base`: `.`, `..` or `../x`.
            (Some(_), _) | (None, None) => return None,
            (None, Some(first)) => {
                return (first != "..").then(|| (first, target_parts.next().is_some()));
            }
        }
    }
}

/// Go: `isDescendant`: `path` is strictly inside `directory`.
#[cfg(test)]
fn is_descendant(path: &str, directory: &str) -> bool {
    inside(&clean_cow(directory), &clean_cow(path)).is_some()
}

/// `filepath.Rel` for the cleaned absolute or cleaned relative paths the
/// overlay holds; `None` where Go returns an error (mixed absolute and
/// relative). The overlay itself now answers through [`inside`]; this is the
/// literal port its tests compare against.
#[cfg(test)]
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
        self.through(|l| &l.files, &path, || self.base.file_exists(&path))
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
        if let Some(lookups) = self
            .lookups
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            lookups.count(&lookups.reads);
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
        if self.through(
            |l| &l.directories,
            path,
            || self.base.directory_exists(path),
        ) {
            return true;
        }
        let directory = clean_overlay_path(path);
        let directory = clean_cow(&directory);
        self.read()
            .files
            .keys()
            .any(|file| inside(&directory, &clean_cow(file)).is_some())
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        let mut entries = self.through(
            |l| &l.entries,
            path,
            || self.base.get_accessible_entries(path),
        );
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
            let base = clean_cow(&directory);
            for file in state.files.keys() {
                let file = clean_cow(file);
                match inside(&base, &file) {
                    None => {}
                    Some((name, false)) => {
                        files.insert(name.to_owned());
                    }
                    Some((first, true)) => {
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
        self.through(|l| &l.stats, &path, || self.base.stat(&path))
    }

    fn realpath(&self, path: &str) -> String {
        let path = clean_overlay_path(path);
        if self.read().files.contains_key(&path) {
            return path;
        }
        self.through(|l| &l.realpaths, &path, || self.base.realpath(&path))
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

    /// The allocation-free containment test equals the literal `filepath.Rel`
    /// port on every pair: absolute and relative, `.`/`..` leaders, equal
    /// paths, siblings sharing a prefix, a parent, the root, unclean spellings
    /// and URL-like names.
    #[test]
    fn inside_equals_the_filepath_rel_port() {
        let paths = [
            "",
            ".",
            "..",
            "../..",
            "../a",
            "../a/b",
            "a",
            "a/b",
            "a/b/c",
            "ab",
            "ab/c",
            "/",
            "/a",
            "/a/",
            "/a/b",
            "/a/b/c",
            "/ab",
            "/ab/c",
            "//a",
            "/a//b",
            "/a/./b",
            "/a/b/../c",
            "/a/../..",
            "bundled:///libs/x.d.ts",
            "bundled:/libs",
            "/p/new/deep/c.ts",
            "/p/new",
            "/p",
            "/p/sub/b.ts",
            "a/..",
            "./a",
            "../../a/b",
        ];
        let mut inside_count = 0;
        for base in paths {
            for target in paths {
                let want = relative(base, target).filter(|relative| {
                    relative != "." && relative != ".." && !relative.starts_with("../")
                });
                let (cleaned_base, cleaned_target) = (clean(base), clean(target));
                let got = inside(&cleaned_base, &cleaned_target);
                assert_eq!(
                    got.map(|(first, nested)| (first.to_owned(), nested)),
                    want.as_ref()
                        .map(|relative| match relative.split_once('/') {
                            None => (relative.clone(), false),
                            Some((first, _)) => (first.to_owned(), true),
                        }),
                    "{base:?} / {target:?}"
                );
                assert_eq!(
                    is_descendant(target, base),
                    want.is_some(),
                    "{base:?} / {target:?}"
                );
                inside_count += usize::from(want.is_some());
            }
        }
        assert!(inside_count > 40, "{inside_count}");
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

    /// A disk that counts what reaches it, and whose files can change under
    /// the overlay (a package installed, a file created).
    struct Disk {
        files: RwLock<Vec<&'static str>>,
        asked: std::sync::Mutex<std::collections::HashMap<&'static str, u32>>,
    }

    impl Disk {
        fn new(files: &[&'static str]) -> Arc<Self> {
            Arc::new(Self {
                files: RwLock::new(files.to_vec()),
                asked: std::sync::Mutex::default(),
            })
        }

        fn ask(&self, operation: &'static str) {
            *self.asked.lock().unwrap().entry(operation).or_default() += 1;
        }

        fn asked(&self, operation: &'static str) -> u32 {
            self.asked
                .lock()
                .unwrap()
                .get(operation)
                .copied()
                .unwrap_or(0)
        }

        fn create(&self, path: &'static str) {
            self.files.write().unwrap().push(path);
        }

        fn has(&self, path: &str) -> bool {
            self.files.read().unwrap().contains(&path)
        }
    }

    impl FS for Disk {
        fn use_case_sensitive_file_names(&self) -> bool {
            true
        }
        fn file_exists(&self, path: &str) -> bool {
            self.ask("file_exists");
            self.has(path)
        }
        fn read_file(&self, path: &str) -> Option<String> {
            self.ask("read_file");
            self.has(path).then(|| path.to_owned())
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
            self.ask("directory_exists");
            self.files
                .read()
                .unwrap()
                .iter()
                .any(|file| is_descendant(file, path))
        }
        fn get_accessible_entries(&self, path: &str) -> Entries {
            self.ask("get_accessible_entries");
            let mut entries = Entries::default();
            for file in self.files.read().unwrap().iter() {
                if let Some(relative) = relative(path, file)
                    && !relative.starts_with("..")
                    && relative != "."
                    && !relative.contains('/')
                {
                    entries.files.push(relative);
                }
            }
            entries
        }
        fn stat(&self, path: &str) -> Option<FileInfo> {
            self.ask("stat");
            self.has(path).then(|| file_info(path, 1))
        }
        fn realpath(&self, path: &str) -> String {
            self.ask("realpath");
            path.to_owned()
        }
    }

    fn over(disk: &Arc<Disk>) -> OverlayFs {
        OverlayFs::new(Arc::new(Arc::clone(disk)))
    }

    /// Inside a build the disk answers each question once, whichever way it
    /// answers: a repeated probe of a missing file (the common case in module
    /// resolution) is as free as a repeated hit.
    #[test]
    fn a_build_asks_the_disk_each_question_once() {
        let disk = Disk::new(&["/p/a.ts"]);
        let fs = over(&disk);
        let scope = fs.build_scope(true);
        for _ in 0..4 {
            assert!(fs.file_exists("/p/a.ts"));
            assert!(!fs.file_exists("/p/missing.ts"));
            assert!(fs.directory_exists("/p"));
            assert!(!fs.directory_exists("/q"));
            assert_eq!(fs.stat("/p/a.ts").unwrap().size, 1);
            assert!(fs.stat("/p/missing.ts").is_none());
            assert_eq!(fs.realpath("/p/a.ts"), "/p/a.ts");
            assert_eq!(fs.get_accessible_entries("/p").files, ["a.ts"]);
        }
        assert_eq!(disk.asked("file_exists"), 2);
        assert_eq!(disk.asked("directory_exists"), 2);
        assert_eq!(disk.asked("stat"), 2);
        assert_eq!(disk.asked("realpath"), 1);
        assert_eq!(disk.asked("get_accessible_entries"), 1);
        let stats = scope.finish();
        assert_eq!((stats.misses, stats.hits), (8, 24));
    }

    /// Nothing is kept between builds, and nothing is kept outside one: a
    /// file created after a build's failed probe is seen by the next build,
    /// and a probe made with no build open reaches the disk every time.
    #[test]
    fn answers_last_one_build() {
        let disk = Disk::new(&["/p/a.ts"]);
        let fs = over(&disk);
        assert!(!fs.file_exists("/p/new.ts"));
        assert!(!fs.file_exists("/p/new.ts"));
        assert_eq!(disk.asked("file_exists"), 2, "no build open: no lookups");

        let scope = fs.build_scope(false);
        assert!(!fs.file_exists("/p/new.ts"));
        assert!(!fs.directory_exists("/p/new"));
        disk.create("/p/new.ts");
        assert!(
            !fs.file_exists("/p/new.ts"),
            "within the build the first answer stands"
        );
        drop(scope);

        assert!(fs.file_exists("/p/new.ts"), "the build's answers are gone");
        let scope = fs.build_scope(false);
        assert!(fs.file_exists("/p/new.ts"));
        assert_eq!(scope.finish(), LookupStats::default(), "not counting");
    }

    /// The lookups sit below the overlay: an edit or a deletion made before or
    /// during a build is answered by the overlay, never by a stale lookup.
    #[test]
    fn the_overlay_is_consulted_before_the_lookups() {
        let disk = Disk::new(&["/p/a.ts", "/p/gone.ts"]);
        let fs = over(&disk);
        let _scope = fs.build_scope(false);
        assert!(fs.file_exists("/p/gone.ts"));
        assert!(!fs.file_exists("/p/fresh.ts"));
        assert_eq!(fs.stat("/p/gone.ts").unwrap().size, 1);
        fs.delete("/p/gone.ts");
        fs.set("/p/fresh.ts", "fresh");
        assert!(!fs.file_exists("/p/gone.ts"));
        assert!(fs.stat("/p/gone.ts").is_none());
        assert!(fs.file_exists("/p/fresh.ts"));
        assert_eq!(fs.stat("/p/fresh.ts").unwrap().size, 5);
        assert_eq!(fs.read_file("/p/fresh.ts").as_deref(), Some("fresh"));
        assert_eq!(fs.get_accessible_entries("/p").files, ["a.ts", "fresh.ts"]);
    }

    /// A scope opened inside another leaves the outer lookups alone, and a
    /// duplicate starts with none: each build makes its own.
    #[test]
    fn scopes_nest_and_duplicates_start_empty() {
        let disk = Disk::new(&["/p/a.ts"]);
        let fs = over(&disk);
        let outer = fs.build_scope(true);
        assert!(fs.file_exists("/p/a.ts"));
        {
            let inner = fs.build_scope(true);
            assert!(fs.file_exists("/p/a.ts"));
            assert_eq!(inner.finish(), LookupStats::default(), "inert");
        }
        assert!(fs.file_exists("/p/a.ts"));
        assert_eq!(disk.asked("file_exists"), 1, "the outer lookups survived");
        let copy = fs.duplicate();
        assert!(copy.file_exists("/p/a.ts"));
        assert_eq!(disk.asked("file_exists"), 2, "a duplicate has no lookups");
        assert_eq!(outer.finish().hits, 2);
    }
}
