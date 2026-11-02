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
//! - for a `#` import, every `package.json` on the importer's walk up;
//! - every question the program's builds asked the file system (the
//!   producer's `buildInputs`, ADR 0543), each observed at least as finely
//!   as tsrs's answer depends on it: a path's kind following links
//!   (`file_exists`, `directory_exists`), its kind and a file's size
//!   (`stat`: tsrs reads no other part of it), its canonical path
//!   (`realpath`), a directory's entries with each one's kind following
//!   links (`get_accessible_entries`), a file's bytes (`read_file`). This is
//!   what a relative or `paths`-mapped resolution read, which the walks above
//!   do not model: a directory's `package.json` edited in place, a file that
//!   now shadows the resolved one, a link swapped, anywhere the build looked.
//!   A tracked source's bytes and size are left to the daemon's own overlay
//!   updates, as above.
//!
//! Any difference rebuilds the program from a new Type Facts session.
//! Configuration is already the tracked tsconfig (`paths`, `baseUrl`,
//! `moduleResolution`); a tsconfig it `extends` is not observed here.
//!
//! The observations are made once the producer has answered, a little after
//! its build asked: a change in between is missed until the next build,
//! as for every observation here.

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

/// What a path is, following links: the part of a `file_exists` or
/// `directory_exists` answer, and of a `stat`'s, that depends on the path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Absent,
    File,
    Directory,
    Other,
}

fn kind_of(metadata: &fs::Metadata) -> Kind {
    if metadata.is_file() {
        Kind::File
    } else if metadata.is_dir() {
        Kind::Directory
    } else {
        Kind::Other
    }
}

fn followed_kind(path: &Path) -> Kind {
    fs::metadata(path).map_or(Kind::Absent, |metadata| kind_of(&metadata))
}

/// A question of the producer's `buildInputs`, by kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Query {
    /// `file_exists` and `directory_exists`.
    Kind,
    Stat,
    Realpath,
    Entries,
    Read,
}

/// What a [`Query`] observed.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Answer {
    Kind(Kind),
    /// The kind, and a file's size.
    Stat(Kind, Option<u64>),
    Realpath(Option<PathBuf>),
    /// The directory's own stamp (by `lstat`, so a rewritten link or entry
    /// moves it), and every entry's name with its kind following links;
    /// `None` when it cannot be listed.
    Entries(Option<Listing>),
    /// The bytes behind the path, links followed, with the stamp they were
    /// read under (`None`: unreadable).
    Read(Option<(FileStampKey, [u8; 32])>),
}

type FileStampKey = (u64, u64, u64, i64, i64, i64, i64);

/// A directory's `lstat` stamp, and each entry's name, kind following links,
/// and whether it is a link.
type Listing = (FileStampKey, Vec<(std::ffi::OsString, Kind, bool)>);

fn entries_of(path: &Path) -> Option<Listing> {
    let directory = stamp(&fs::symlink_metadata(path).ok()?);
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).ok()? {
        let entry = entry.ok()?;
        let file_type = entry.file_type().ok()?;
        let link = file_type.is_symlink();
        let kind = if link {
            followed_kind(&entry.path())
        } else if file_type.is_dir() {
            Kind::Directory
        } else if file_type.is_file() {
            Kind::File
        } else {
            Kind::Other
        };
        entries.push((entry.file_name(), kind, link));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Some((directory, entries))
}

fn read_of(path: &Path) -> Option<(FileStampKey, [u8; 32])> {
    let metadata = fs::metadata(path).ok()?;
    let bytes = fs::read(path).ok()?;
    Some((stamp(&metadata), hash(&bytes)))
}

fn answer(query: Query, path: &Path, real: &RealPaths) -> Answer {
    match query {
        Query::Kind => Answer::Kind(followed_kind(path)),
        Query::Stat => match fs::metadata(path) {
            Ok(metadata) => {
                let kind = kind_of(&metadata);
                Answer::Stat(kind, (kind == Kind::File).then_some(metadata.len()))
            }
            Err(_) => Answer::Stat(Kind::Absent, None),
        },
        Query::Realpath => Answer::Realpath(real.of(path)),
        Query::Entries => Answer::Entries(entries_of(path)),
        Query::Read => Answer::Read(read_of(path)),
    }
}

/// Whether `path` still answers `query` as `recorded` saw. A listing whose
/// directory stamp is unchanged holds the same names (an entry created,
/// removed, renamed or replaced moves it), so only its links are followed
/// again; bytes whose stamp is unchanged are not reread.
fn still_answers(query: Query, path: &Path, recorded: &Answer, real: &RealPaths) -> bool {
    match (query, recorded) {
        (Query::Entries, Answer::Entries(Some((directory, entries)))) => {
            let Ok(metadata) = fs::symlink_metadata(path) else {
                return false;
            };
            if stamp(&metadata) == *directory {
                return entries
                    .iter()
                    .all(|(name, kind, link)| !*link || followed_kind(&path.join(name)) == *kind);
            }
            matches!(entries_of(path), Some((_, now)) if now == *entries)
        }
        (Query::Read, Answer::Read(Some((recorded_stamp, recorded_hash)))) => {
            match fs::metadata(path) {
                Ok(metadata) if stamp(&metadata) == *recorded_stamp => true,
                Ok(_) => fs::read(path).is_ok_and(|bytes| hash(&bytes) == *recorded_hash),
                Err(_) => false,
            }
        }
        // An unreadable directory or file is never the same again.
        (Query::Entries, Answer::Entries(None)) | (Query::Read, Answer::Read(None)) => false,
        _ => answer(query, path, real) == *recorded,
    }
}

#[derive(Clone)]
pub(crate) struct ResolutionWitness {
    observed: BTreeMap<PathBuf, Observed>,
    /// The producer's `buildInputs`, each as it answered when recorded.
    queried: BTreeMap<(Query, PathBuf), Answer>,
    /// The `buildInputs` probes that found no entry in a directory that is
    /// there, by directory, with the directory's `lstat` stamp when they
    /// were recorded: while the stamp is unchanged no entry was created,
    /// removed or renamed in it and it was not replaced, so each is still
    /// absent. A resolution's candidate files are mostly such probes.
    absent: BTreeMap<PathBuf, (FileStampKey, Vec<PathBuf>)>,
    /// Every path of `absent`, so a probe recorded there is not observed
    /// again when the next answer names it too.
    absent_paths: std::collections::HashSet<PathBuf>,
}

/// Whether every path of `children` of `directory` is still absent.
fn still_absent(directory: &Path, recorded: &FileStampKey, children: &[PathBuf]) -> bool {
    let unchanged = fs::symlink_metadata(directory)
        .is_ok_and(|metadata| metadata.is_dir() && stamp(&metadata) == *recorded);
    unchanged
        || children
            .iter()
            .all(|child| followed_kind(child) == Kind::Absent)
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
            queried: BTreeMap::new(),
            absent: BTreeMap::new(),
            absent_paths: std::collections::HashSet::new(),
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
        if let Some(inputs) = &graph.build_inputs {
            self.record_inputs(inputs, &tracked);
        }
    }

    /// Records the producer's `buildInputs` (see the module docs). An answer
    /// another observation already decides is not observed twice: a read
    /// file's kind and size follow from its bytes' stamp, and a file the
    /// module walk observes by content is that read.
    fn record_inputs(&mut self, inputs: &typefacts::BuildInputs, tracked: &impl Fn(&str) -> bool) {
        let real = RealPaths::default();
        let path_of = |path: &std::sync::Arc<str>| PathBuf::from(path.as_ref());
        let observed_file = |observed: &BTreeMap<PathBuf, Observed>, path: &Path| {
            matches!(observed.get(path), Some(Observed::File { .. }))
        };
        // A tracked source's bytes, and so its size, are the overlay's: its
        // edits reach the producer as updates. Its kind is observed.
        let mut read = std::collections::HashSet::new();
        for path in inputs.reads.iter().filter(|path| !tracked(path)) {
            let path = path_of(path);
            if observed_file(&self.observed, &path) {
                read.insert(path);
                continue;
            }
            let recorded = self
                .queried
                .entry((Query::Read, path.clone()))
                .or_insert_with_key(|(query, path)| answer(*query, path, &real));
            if matches!(recorded, Answer::Read(Some(_))) {
                read.insert(path);
            }
        }
        let decided = |path: &Path| read.contains(path);
        let mut record = |query: Query, path: PathBuf| {
            if self.queried.contains_key(&(query, path.clone()))
                || (query == Query::Kind && self.absent_paths.contains(&path))
            {
                return;
            }
            if query == Query::Kind
                && let Some(parent) = path.parent()
                && followed_kind(&path) == Kind::Absent
                && let Ok(metadata) = fs::symlink_metadata(parent)
                && metadata.is_dir()
            {
                let (directory, children) = self
                    .absent
                    .entry(parent.to_path_buf())
                    .or_insert_with(|| (stamp(&metadata), Vec::new()));
                // A directory recorded under another stamp has moved since:
                // the probe is observed on its own.
                if *directory == stamp(&metadata) {
                    self.absent_paths.insert(path.clone());
                    children.push(path);
                    return;
                }
            }
            let recorded = answer(query, &path, &real);
            self.queried.insert((query, path), recorded);
        };
        for path in inputs.files.iter().chain(&inputs.directories) {
            let path = path_of(path);
            if !decided(&path) {
                record(Query::Kind, path);
            }
        }
        for path in &inputs.stats {
            let path = path_of(path);
            if decided(&path) {
                continue;
            }
            record(
                if tracked(path.to_str().unwrap_or_default()) {
                    Query::Kind
                } else {
                    Query::Stat
                },
                path,
            );
        }
        for path in &inputs.realpaths {
            record(Query::Realpath, path_of(path));
        }
        for path in &inputs.entries {
            record(Query::Entries, path_of(path));
        }
    }

    /// Whether every recorded entry is still as it was. The entries are
    /// independent filesystem reads (tens of thousands for a monorepo's
    /// `node_modules` walks), checked on six workers; any change answers no.
    pub(crate) fn current(&self) -> bool {
        const WORKERS: usize = 6;
        enum Entry<'a> {
            Observed(&'a Path, &'a Observed),
            Queried(Query, &'a Path, &'a Answer),
            Absent(&'a Path, &'a FileStampKey, &'a [PathBuf]),
        }
        let still = |entry: &Entry<'_>, real: &RealPaths| match entry {
            Entry::Observed(path, recorded) => still_with(path, recorded, real),
            Entry::Queried(query, path, recorded) => still_answers(*query, path, recorded, real),
            Entry::Absent(directory, recorded, children) => {
                still_absent(directory, recorded, children)
            }
        };
        let entries = self
            .observed
            .iter()
            .map(|(path, recorded)| Entry::Observed(path, recorded))
            .chain(
                self.queried
                    .iter()
                    .map(|((query, path), recorded)| Entry::Queried(*query, path, recorded)),
            )
            .chain(self.absent.iter().map(|(directory, (recorded, children))| {
                Entry::Absent(directory, recorded, children)
            }))
            .collect::<Vec<_>>();
        let real = RealPaths::default();
        if entries.len() < 512 {
            return entries.iter().all(|entry| still(entry, &real));
        }
        let moved = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            for worker in 0..WORKERS {
                let (entries, moved, real, still) = (&entries, &moved, &real, &still);
                scope.spawn(move || {
                    for entry in entries.iter().skip(worker).step_by(WORKERS) {
                        if moved.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        if !still(entry, real) {
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
