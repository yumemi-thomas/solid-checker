//! The program boundary a check uses when the user selected none (ADR 0193,
//! ADR 0444).
//!
//! An application is the whole program: every caller of its exported
//! components and hooks is one of its own call sites. A published library is
//! not, and neither is a package a sibling of its workspace imports. The
//! nearest `package.json` at or above the project decides, then the
//! workspaces around it:
//!
//! - `"private": true` is an application;
//! - otherwise, a manifest that publishes an entry (`exports`, `main`,
//!   `module`, `types`, `typings` or `bin`) is a library;
//! - a manifest that publishes nothing is an application;
//! - an application that is a member of a workspace (or its root) is open when
//!   a package of that workspace declares a dependency on it, or when the
//!   workspace cannot be read exactly.
//!
//! No manifest, or one that cannot be read, keeps the open boundary: a build
//! that cannot tell stays fail-closed.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};
use solid_reactive_ir::ProgramBoundary;

/// The dependency fields through which a sibling package reaches another.
const DEPENDENCY_FIELDS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// The protocols whose value after the colon may be a path to the package.
const PATH_PROTOCOLS: [&str; 4] = ["file:", "link:", "portal:", "workspace:"];

/// A workspace this walk will list at most this many directories of before it
/// refuses to name its members.
const DIRECTORY_BUDGET: usize = 20_000;

/// The boundary the nearest manifest and its workspaces give `project`, and a
/// digest of every file and directory listing the answer read (a daemon keeps
/// an answer only while the digest holds).
pub(crate) struct Decision {
    pub(crate) boundary: ProgramBoundary,
    pub(crate) digest: [u8; 32],
}

/// A digest of everything [`default_program_boundary`] reads for `project`.
#[must_use]
pub fn program_boundary_digest(project: &Path) -> [u8; 32] {
    decide(project).digest
}

/// The default boundary and the digest of what deciding it read, from one
/// read of the disk: a daemon request that needs both (the answer's inputs
/// and the analysis' options) decides once and takes the digest of the very
/// state the analysis used, so a manifest edited meanwhile moves the next
/// request's digest.
#[derive(Clone, Copy, Debug)]
pub struct BoundaryDecision {
    pub boundary: ProgramBoundary,
    pub digest: [u8; 32],
}

pub(crate) fn default_program_boundary(project: &Path) -> ProgramBoundary {
    decide(project).boundary
}

pub(crate) fn decide(project: &Path) -> Decision {
    decide_observed(project).0
}

/// [`decide`], and what the disk showed while it ran (`None` where the
/// observation cannot be re-checked by a stat: a symbolic link was involved).
fn decide_observed(project: &Path) -> (Decision, Option<Vec<Seen>>) {
    let mut reads = Reads::default();
    let boundary = decide_with(project, &mut reads);
    let witness = reads.cacheable.then_some(reads.seen);
    (
        Decision {
            boundary,
            digest: reads.transcript.finalize().into(),
        },
        witness,
    )
}

/// One thing the disk showed a decision: a path that was absent, a path with
/// the stamp it had when read (a file's, or a directory whose listing was
/// read; any entry added, removed or renamed in a directory moves its stamp),
/// or the real path a directory canonicalized to. The stamp is the daemon's
/// own file stamp: device, inode, length, modification and change times, the
/// last of which a rewrite cannot set back.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Absent(PathBuf),
    Stamp(PathBuf, [i64; 7]),
    Canonical(PathBuf, PathBuf),
}

impl Seen {
    fn holds(&self) -> bool {
        match self {
            Self::Absent(path) => fs::symlink_metadata(path).is_err(),
            Self::Stamp(path, stamp) => fs::symlink_metadata(path).is_ok_and(|metadata| {
                !metadata.file_type().is_symlink() && stamp_of(&metadata) == Some(*stamp)
            }),
            Self::Canonical(path, real) => fs::canonicalize(path).is_ok_and(|now| now == *real),
        }
    }
}

/// The daemon's file stamp of `metadata`; `None` where the platform has no
/// change time to stamp with (a decision is then never kept).
#[cfg(unix)]
#[allow(clippy::cast_possible_wrap)]
fn stamp_of(metadata: &fs::Metadata) -> Option<[i64; 7]> {
    use std::os::unix::fs::MetadataExt;
    Some([
        metadata.dev() as i64,
        metadata.ino() as i64,
        metadata.len() as i64,
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ])
}

#[cfg(not(unix))]
fn stamp_of(_: &fs::Metadata) -> Option<[i64; 7]> {
    None
}

/// The decision a daemon keeps between requests while the disk still shows
/// what it showed (ADR 0444): a request that finds every observation as
/// recorded takes the kept decision and its digest, and any other decides
/// again. A decision that involved a symbolic link is never kept, since a
/// link's target can change under unchanged stamps.
#[derive(Default)]
pub struct BoundaryCache {
    kept: Option<(PathBuf, BoundaryDecision, Vec<Seen>)>,
}

impl BoundaryCache {
    #[must_use]
    pub fn decide(&mut self, project: &Path) -> BoundaryDecision {
        if let Some((kept_project, decision, witness)) = &self.kept
            && kept_project == project
            && witness.iter().all(Seen::holds)
        {
            return *decision;
        }
        let (decision, witness) = decide_observed(project);
        let decision = BoundaryDecision {
            boundary: decision.boundary,
            digest: decision.digest,
        };
        self.kept = witness.map(|witness| (project.to_path_buf(), decision, witness));
        decision
    }
}

fn decide_with(project: &Path, reads: &mut Reads) -> ProgramBoundary {
    let start = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or(project)
    };
    let raw_start = start;
    let start = fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    reads
        .seen
        .push(Seen::Canonical(raw_start.to_path_buf(), start.clone()));
    let Some((package_directory, read)) =
        start.ancestors().find_map(
            |directory| match reads.file(&directory.join("package.json")) {
                Read::Absent => None,
                read => Some((directory.to_path_buf(), read)),
            },
        )
    else {
        return ProgramBoundary::Open;
    };
    let Read::Text(text) = read else {
        return ProgramBoundary::Open;
    };
    let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&text) else {
        return ProgramBoundary::Open;
    };
    if fields.get("private") != Some(&Value::Bool(true)) {
        let publishes = ["exports", "main", "module", "types", "typings", "bin"]
            .iter()
            .any(|field| fields.contains_key(*field));
        if publishes {
            return ProgramBoundary::Open;
        }
    }
    let name = fields.get("name").and_then(Value::as_str);
    if workspace_may_import(reads, &package_directory, name) {
        ProgramBoundary::Open
    } else {
        ProgramBoundary::Closed
    }
}

/// Whether a package of a workspace that holds `package_directory` (or whose
/// root it is) may import it: a member declares a dependency on it, or the
/// workspace is not one this reads exactly.
fn workspace_may_import(reads: &mut Reads, package_directory: &Path, name: Option<&str>) -> bool {
    for root in package_directory.ancestors() {
        let patterns = match workspace_patterns(reads, root) {
            Declared::None => continue,
            Declared::Unreadable => return true,
            Declared::Patterns(patterns) => patterns,
        };
        let Some(members) = members(reads, root, &patterns) else {
            return true;
        };
        let is_root = root == package_directory;
        if !is_root && !members.iter().any(|member| member == package_directory) {
            continue;
        }
        // The root's own manifest is a dependent too: it may install a member.
        let root_package = (!is_root).then(|| root.to_path_buf());
        for sibling in members.iter().chain(root_package.as_ref()) {
            if sibling == package_directory {
                continue;
            }
            let text = match reads.file(&sibling.join("package.json")) {
                Read::Text(text) => text,
                // A workspace root may have no manifest (`pnpm-workspace.yaml`
                // alone); a member always has one.
                Read::Absent if Some(sibling) == root_package.as_ref() => continue,
                _ => return true,
            };
            let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&text) else {
                return true;
            };
            if depends_on(&fields, sibling, name, package_directory) {
                return true;
            }
        }
    }
    false
}

enum Declared {
    /// No workspace is declared at this directory.
    None,
    /// Patterns, relative to the directory, of its member packages.
    Patterns(Vec<String>),
    /// A workspace is declared that this does not read: an unsupported
    /// manager (`rush.json`, an `nx.json` with no package-manager workspace)
    /// or a declaration of an unknown shape.
    Unreadable,
}

/// The workspace patterns declared at `root`: `package.json`'s `workspaces`
/// (npm, yarn, bun), `pnpm-workspace.yaml`'s `packages`, `lerna.json`'s
/// `packages`. A project-graph manager that lists no packages (`rush.json`,
/// `nx.json` beside no patterns) imports through paths this does not read.
fn workspace_patterns(reads: &mut Reads, root: &Path) -> Declared {
    let mut patterns = Vec::new();
    let mut declared = false;
    // An unreadable file may hide a declaration, so it is one that is not read.
    match reads.file(&root.join("package.json")) {
        Read::Absent => {}
        Read::Unreadable => return Declared::Unreadable,
        Read::Text(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(fields)) => match fields.get("workspaces") {
                None => {}
                Some(Value::Array(items)) => {
                    declared = true;
                    if !push_strings(items, &mut patterns) {
                        return Declared::Unreadable;
                    }
                }
                Some(Value::Object(object)) => {
                    declared = true;
                    match object.get("packages") {
                        None => {}
                        Some(Value::Array(items)) => {
                            if !push_strings(items, &mut patterns) {
                                return Declared::Unreadable;
                            }
                        }
                        Some(_) => return Declared::Unreadable,
                    }
                }
                Some(_) => return Declared::Unreadable,
            },
            _ => return Declared::Unreadable,
        },
    }
    match reads.file(&root.join("pnpm-workspace.yaml")) {
        Read::Absent => {}
        Read::Unreadable => return Declared::Unreadable,
        Read::Text(text) => {
            declared = true;
            match pnpm_patterns(&text) {
                Some(listed) => patterns.extend(listed),
                None => return Declared::Unreadable,
            }
        }
    }
    match reads.file(&root.join("lerna.json")) {
        Read::Absent => {}
        Read::Unreadable => return Declared::Unreadable,
        Read::Text(text) => {
            declared = true;
            match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(fields)) => match fields.get("packages") {
                    None => patterns.push("packages/*".to_owned()),
                    Some(Value::Array(items)) => {
                        if !push_strings(items, &mut patterns) {
                            return Declared::Unreadable;
                        }
                    }
                    Some(_) => return Declared::Unreadable,
                },
                _ => return Declared::Unreadable,
            }
        }
    }
    if reads.file(&root.join("rush.json")) != Read::Absent {
        return Declared::Unreadable;
    }
    if !declared && reads.file(&root.join("nx.json")) != Read::Absent {
        return Declared::Unreadable;
    }
    if declared {
        Declared::Patterns(patterns)
    } else {
        Declared::None
    }
}

fn push_strings(items: &[Value], into: &mut Vec<String>) -> bool {
    for item in items {
        let Some(text) = item.as_str() else {
            return false;
        };
        into.push(text.to_owned());
    }
    true
}

/// The `packages` list of a `pnpm-workspace.yaml`: a block sequence of scalars
/// or a one-line flow sequence; without the key, pnpm's default of every
/// subdirectory. Anything else (anchors, multi-line flow sequences) is not
/// read.
fn pnpm_patterns(text: &str) -> Option<Vec<String>> {
    let unquote = |item: &str| {
        let item = item.trim();
        let item = match item.split_once(" #") {
            Some((before, _)) => before.trim_end(),
            None => item,
        };
        let quoted = item.len() >= 2
            && ((item.starts_with('\'') && item.ends_with('\''))
                || (item.starts_with('"') && item.ends_with('"')));
        let item = if quoted {
            &item[1..item.len() - 1]
        } else {
            item
        };
        // A leading `&`, `*`, `|` or `>` is an anchor, an alias or a block
        // scalar: not read.
        (!item.is_empty() && !item.starts_with(['&', '*', '|', '>', '{', '[']))
            .then(|| item.to_owned())
    };
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("packages:") else {
            continue;
        };
        let rest = rest.trim();
        if let Some(flow) = rest.strip_prefix('[') {
            let flow = flow.strip_suffix(']')?;
            return flow
                .split(',')
                .filter(|item| !item.trim().is_empty())
                .map(unquote)
                .collect();
        }
        if !(rest.is_empty() || rest.starts_with('#')) {
            return None;
        }
        let mut patterns = Vec::new();
        while let Some(next) = lines.peek() {
            let trimmed = next.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.next();
                continue;
            }
            if !next.starts_with(char::is_whitespace) && !next.starts_with('-') {
                break;
            }
            let item = trimmed.strip_prefix('-')?;
            patterns.push(unquote(item)?);
            lines.next();
        }
        return Some(patterns);
    }
    // A workspace file with no `packages` (only settings: `overrides`,
    // `onlyBuiltDependencies`) is pnpm's default workspace: the root and every
    // directory below it.
    Some(vec!["**".to_owned()])
}

/// The package directories the patterns name below `root`: a directory with a
/// `package.json` matching a positive pattern and no negated one. `None` when
/// a pattern uses syntax this does not read or the listing outgrows its
/// budget.
fn members(reads: &mut Reads, root: &Path, patterns: &[String]) -> Option<Vec<PathBuf>> {
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    let mut visited = 0_usize;
    for pattern in patterns {
        let (negated, pattern) = match pattern.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, pattern.as_str()),
        };
        let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
        let pattern = pattern.trim_end_matches('/');
        let segments = pattern.split('/').collect::<Vec<_>>();
        if segments.iter().any(|segment| {
            segment.is_empty()
                || *segment == ".."
                || segment.contains(['{', '}', '[', ']', '(', ')', '!', '+', '@', '\\'])
        }) {
            return None;
        }
        let matched = if negated {
            &mut excluded
        } else {
            &mut included
        };
        matching_directories(reads, root, &segments, &mut visited, matched)?;
    }
    included.sort();
    included.dedup();
    let mut members = Vec::new();
    for directory in included {
        if excluded.contains(&directory) {
            continue;
        }
        if reads.file(&directory.join("package.json")) != Read::Absent {
            members.push(directory);
        }
    }
    Some(members)
}

fn matching_directories(
    reads: &mut Reads,
    directory: &Path,
    segments: &[&str],
    visited: &mut usize,
    into: &mut Vec<PathBuf>,
) -> Option<()> {
    *visited += 1;
    if *visited > DIRECTORY_BUDGET {
        return None;
    }
    let Some((segment, rest)) = segments.split_first() else {
        into.push(directory.to_path_buf());
        return Some(());
    };
    if *segment == "**" {
        matching_directories(reads, directory, rest, visited, into)?;
        for child in reads.subdirectories(directory) {
            matching_directories(reads, &child, segments, visited, into)?;
        }
    } else if segment.contains(['*', '?']) {
        for child in reads.subdirectories(directory) {
            let name = child.file_name().and_then(|name| name.to_str());
            if name.is_some_and(|name| wildcard_matches(segment, name)) {
                matching_directories(reads, &child, rest, visited, into)?;
            }
        }
    } else {
        let child = directory.join(segment);
        reads.observe(&child);
        if child.is_dir() {
            matching_directories(reads, &child, rest, visited, into)?;
        }
    }
    Some(())
}

/// A path segment against a glob segment of `*` and `?` (never across `/`).
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    fn go(pattern: &[char], name: &[char]) -> bool {
        match pattern.split_first() {
            None => name.is_empty(),
            Some(('*', rest)) => (0..=name.len()).any(|skip| go(rest, &name[skip..])),
            Some(('?', rest)) => name.split_first().is_some_and(|(_, tail)| go(rest, tail)),
            Some((literal, rest)) => name
                .split_first()
                .is_some_and(|(first, tail)| first == literal && go(rest, tail)),
        }
    }
    let pattern = pattern.chars().collect::<Vec<_>>();
    let name = name.chars().collect::<Vec<_>>();
    go(&pattern, &name)
}

/// Whether `sibling`'s manifest `fields` name the package at `package` as a
/// dependency: by its name (a plain version, a `workspace:` or `npm:` alias
/// spelled with it) or by a path protocol resolving to its directory.
fn depends_on(
    fields: &serde_json::Map<String, Value>,
    sibling: &Path,
    name: Option<&str>,
    package: &Path,
) -> bool {
    for field in DEPENDENCY_FIELDS {
        let Some(Value::Object(dependencies)) = fields.get(field) else {
            continue;
        };
        for (key, value) in dependencies {
            if name == Some(key.as_str()) {
                return true;
            }
            let Some(spec) = value.as_str() else {
                continue;
            };
            if let Some(name) = name {
                for protocol in ["workspace:", "npm:"] {
                    let Some(target) = spec.strip_prefix(protocol) else {
                        continue;
                    };
                    let target = target
                        .strip_prefix(name)
                        .filter(|rest| rest.is_empty() || rest.starts_with('@'));
                    if target.is_some() {
                        return true;
                    }
                }
            }
            for protocol in PATH_PROTOCOLS {
                let Some(target) = spec.strip_prefix(protocol) else {
                    continue;
                };
                if (target.starts_with('.') || target.starts_with('/'))
                    && normalized(&sibling.join(target)) == package
                {
                    return true;
                }
            }
        }
    }
    false
}

/// A path with its `.` and `..` components resolved lexically.
fn normalized(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other.as_os_str()),
        }
    }
    resolved
}

/// What reading one file gave.
#[derive(Clone, PartialEq, Eq)]
enum Read {
    Absent,
    Unreadable,
    Text(String),
}

/// Everything one decision read, folded into a digest: a file's bytes or its
/// absence, a directory's subdirectory names. A file is read once per
/// decision (a member's manifest is asked for twice, to list the members and
/// to read their dependencies), so the decision sees one snapshot of it.
struct Reads {
    transcript: Sha256,
    files: std::collections::HashMap<PathBuf, Read>,
    /// What the disk showed, in the order asked ([`Seen`]).
    seen: Vec<Seen>,
    /// False once a symbolic link was involved.
    cacheable: bool,
}

impl Default for Reads {
    fn default() -> Self {
        Self {
            transcript: Sha256::new(),
            files: std::collections::HashMap::new(),
            seen: Vec::new(),
            cacheable: true,
        }
    }
}

impl Reads {
    /// Records what `path` is now: absent, or its stamp. The stamp is taken
    /// before the path is read, so a change during the read is a change.
    fn observe(&mut self, path: &Path) {
        match fs::symlink_metadata(path) {
            Err(_) => self.seen.push(Seen::Absent(path.to_path_buf())),
            Ok(metadata) if metadata.file_type().is_symlink() => self.cacheable = false,
            Ok(metadata) => match stamp_of(&metadata) {
                Some(stamp) => self.seen.push(Seen::Stamp(path.to_path_buf(), stamp)),
                None => self.cacheable = false,
            },
        }
    }

    fn file(&mut self, path: &Path) -> Read {
        if let Some(read) = self.files.get(path) {
            return read.clone();
        }
        let read = self.read_file(path);
        self.files.insert(path.to_path_buf(), read.clone());
        read
    }

    fn read_file(&mut self, path: &Path) -> Read {
        self.observe(path);
        self.transcript.update(b"file\0");
        self.transcript.update(path.as_os_str().as_encoded_bytes());
        if !path.is_file() {
            self.transcript.update(b"\0absent");
            return Read::Absent;
        }
        match fs::read_to_string(path) {
            Ok(text) => {
                self.transcript.update(b"\0present\0");
                self.transcript.update(Sha256::digest(text.as_bytes()));
                Read::Text(text)
            }
            Err(_) => {
                self.transcript.update(b"\0unreadable");
                Read::Unreadable
            }
        }
    }

    fn subdirectories(&mut self, directory: &Path) -> Vec<PathBuf> {
        self.observe(directory);
        let mut links = false;
        let mut children = fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                name != "node_modules" && name != ".git"
            })
            .filter(|entry| match entry.file_type() {
                Ok(kind) if kind.is_dir() => true,
                // A link is a directory if its target is.
                Ok(kind) if kind.is_symlink() => {
                    links = true;
                    entry.path().is_dir()
                }
                _ => false,
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        if links {
            self.cacheable = false;
        }
        children.sort();
        self.transcript.update(b"list\0");
        self.transcript
            .update(directory.as_os_str().as_encoded_bytes());
        for child in &children {
            self.transcript.update(b"\0");
            self.transcript.update(child.as_os_str().as_encoded_bytes());
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_reactive_ir::ProgramBoundary::{Closed, Open};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A directory tree of `(relative path, contents)` files, removed on drop.
    struct Tree(PathBuf);

    impl Tree {
        fn new(files: &[(&str, &str)]) -> Self {
            let root = std::env::temp_dir().join(format!(
                "solid-checker-boundary-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            for (path, text) in files {
                let path = root.join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text).unwrap();
            }
            Self(root)
        }

        fn boundary(&self, project: &str) -> ProgramBoundary {
            default_program_boundary(&self.0.join(project).join("tsconfig.json"))
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const ROOT: &str = r#"{"private": true, "workspaces": ["packages/*"]}"#;
    const KERNEL: &str = r#"{"name": "@x/kernel", "private": true}"#;

    #[test]
    fn a_private_workspace_member_a_sibling_depends_on_is_open() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            ("packages/kernel/package.json", KERNEL),
            (
                "packages/feature/package.json",
                r#"{"name": "@x/feature", "private": true,
                    "dependencies": {"@x/kernel": "workspace:*"}}"#,
            ),
            (
                "packages/shell/package.json",
                r#"{"name": "@x/shell", "private": true}"#,
            ),
        ]);
        assert_eq!(
            tree.boundary("packages/kernel"),
            Open,
            "imported by feature"
        );
        assert_eq!(
            tree.boundary("packages/feature"),
            Closed,
            "nobody depends on it"
        );
        assert_eq!(tree.boundary("packages/shell"), Closed);
    }

    #[test]
    fn every_dependency_field_and_spelling_of_a_sibling_counts() {
        for dependent in [
            r#"{"devDependencies": {"@x/kernel": "^1"}}"#,
            r#"{"peerDependencies": {"@x/kernel": "*"}}"#,
            r#"{"optionalDependencies": {"@x/kernel": "1.0.0"}}"#,
            r#"{"dependencies": {"kernel": "workspace:@x/kernel@*"}}"#,
            r#"{"dependencies": {"kernel": "npm:@x/kernel@1"}}"#,
            r#"{"dependencies": {"kernel": "link:../kernel"}}"#,
            r#"{"dependencies": {"kernel": "file:../kernel/"}}"#,
            r#"{"dependencies": {"kernel": "workspace:../kernel"}}"#,
        ] {
            let tree = Tree::new(&[
                ("package.json", ROOT),
                ("packages/kernel/package.json", KERNEL),
                ("packages/other/package.json", dependent),
            ]);
            assert_eq!(tree.boundary("packages/kernel"), Open, "{dependent}");
        }
        for unrelated in [
            r#"{"dependencies": {"@x/kernels": "1"}}"#,
            r#"{"dependencies": {"kernel": "workspace:@x/kernelz@*"}}"#,
            r#"{"dependencies": {"kernel": "link:../elsewhere"}}"#,
            r#"{"dependencies": {"left-pad": "1"}}"#,
        ] {
            let tree = Tree::new(&[
                ("package.json", ROOT),
                ("packages/kernel/package.json", KERNEL),
                ("packages/other/package.json", unrelated),
            ]);
            assert_eq!(tree.boundary("packages/kernel"), Closed, "{unrelated}");
        }
    }

    #[test]
    fn the_workspace_root_manifest_is_a_dependent_too() {
        let tree = Tree::new(&[
            (
                "package.json",
                r#"{"private": true, "workspaces": ["packages/*"],
                    "dependencies": {"@x/kernel": "workspace:*"}}"#,
            ),
            ("packages/kernel/package.json", KERNEL),
        ]);
        assert_eq!(tree.boundary("packages/kernel"), Open);
    }

    #[test]
    fn a_workspace_root_application_is_open_when_a_member_depends_on_it() {
        let root = r#"{"name": "app", "private": true, "workspaces": ["packages/*"]}"#;
        let tree = Tree::new(&[
            ("package.json", root),
            ("packages/ui/package.json", r#"{"name": "@x/ui"}"#),
        ]);
        assert_eq!(tree.boundary("."), Closed);
        let tree = Tree::new(&[
            ("package.json", root),
            (
                "packages/ui/package.json",
                r#"{"name": "@x/ui", "dependencies": {"app": "*"}}"#,
            ),
        ]);
        assert_eq!(tree.boundary("."), Open);
    }

    #[test]
    fn a_package_the_workspace_does_not_list_is_not_a_member() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            (
                "packages/feature/package.json",
                r#"{"name": "@x/feature", "dependencies": {"@x/tool": "*"}}"#,
            ),
            (
                "tools/app/package.json",
                r#"{"name": "@x/tool", "private": true}"#,
            ),
        ]);
        assert_eq!(tree.boundary("tools/app"), Closed, "outside `packages/*`");
        let tree = Tree::new(&[
            (
                "package.json",
                r#"{"private": true, "workspaces": ["packages/*", "!packages/kernel"]}"#,
            ),
            ("packages/kernel/package.json", KERNEL),
            (
                "packages/feature/package.json",
                r#"{"dependencies": {"@x/kernel": "*"}}"#,
            ),
        ]);
        assert_eq!(tree.boundary("packages/kernel"), Closed, "negated pattern");
    }

    #[test]
    fn a_published_manifest_is_open_whatever_its_workspace() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            (
                "packages/lib/package.json",
                r#"{"name": "@x/lib", "exports": "./index.js"}"#,
            ),
        ]);
        assert_eq!(tree.boundary("packages/lib"), Open);
    }

    #[test]
    fn pnpm_and_lerna_workspaces_list_members_as_npm_does() {
        let pnpm =
            "# comment\npackages:\n  - 'apps/*'\n  - \"libs/**\" # all\n\ncatalog:\n  a: 1\n";
        let tree = Tree::new(&[
            ("package.json", r#"{"private": true}"#),
            ("pnpm-workspace.yaml", pnpm),
            ("libs/deep/kernel/package.json", KERNEL),
            (
                "apps/web/package.json",
                r#"{"private": true, "dependencies": {"@x/kernel": "workspace:^"}}"#,
            ),
        ]);
        assert_eq!(tree.boundary("libs/deep/kernel"), Open);
        assert_eq!(tree.boundary("apps/web"), Closed);
        let flow = Tree::new(&[
            ("pnpm-workspace.yaml", "packages: ['libs/*']\n"),
            ("libs/kernel/package.json", KERNEL),
            (
                "libs/a/package.json",
                r#"{"dependencies": {"@x/kernel": "*"}}"#,
            ),
        ]);
        assert_eq!(flow.boundary("libs/kernel"), Open);
        let lerna = Tree::new(&[
            ("lerna.json", r#"{"version": "1.0.0"}"#),
            ("packages/kernel/package.json", KERNEL),
            (
                "packages/a/package.json",
                r#"{"dependencies": {"@x/kernel": "*"}}"#,
            ),
        ]);
        assert_eq!(
            lerna.boundary("packages/kernel"),
            Open,
            "lerna's default packages"
        );
        // pnpm's workspace file with settings only: every subdirectory is a member.
        let settings = "overrides:\n  a: 1\n";
        let alone = Tree::new(&[
            ("package.json", r#"{"name": "app", "private": true}"#),
            ("pnpm-workspace.yaml", settings),
            ("examples/demo/package.json", r#"{"name": "demo"}"#),
        ]);
        assert_eq!(alone.boundary("."), Closed, "no subdirectory depends on it");
        let used = Tree::new(&[
            ("package.json", r#"{"name": "app", "private": true}"#),
            ("pnpm-workspace.yaml", settings),
            (
                "examples/demo/package.json",
                r#"{"dependencies": {"app": "workspace:*"}}"#,
            ),
        ]);
        assert_eq!(used.boundary("."), Open, "a nested package depends on it");
        let rootless = Tree::new(&[
            ("pnpm-workspace.yaml", "packages:\n  - 'libs/*'\n"),
            ("libs/kernel/package.json", KERNEL),
        ]);
        assert_eq!(
            rootless.boundary("libs/kernel"),
            Closed,
            "a root with no manifest"
        );
    }

    #[test]
    fn a_workspace_that_cannot_be_read_exactly_is_open() {
        let isolated = [("packages/kernel/package.json", KERNEL)];
        for (label, extra) in [
            (
                "brace pattern",
                ("package.json", r#"{"workspaces": ["{packages,libs}/*"]}"#),
            ),
            ("number in list", ("package.json", r#"{"workspaces": [1]}"#)),
            (
                "odd shape",
                ("package.json", r#"{"workspaces": "packages/*"}"#),
            ),
            (
                "anchored yaml",
                ("pnpm-workspace.yaml", "packages: *shared\n"),
            ),
            (
                "multi-line flow list",
                ("pnpm-workspace.yaml", "packages: [\n  a,\n]\n"),
            ),
            ("rush", ("rush.json", "{}")),
            ("nx without workspaces", ("nx.json", "{}")),
            ("invalid ancestor manifest", ("package.json", "not json")),
        ] {
            let mut files = isolated.to_vec();
            files.push(extra);
            assert_eq!(
                Tree::new(&files).boundary("packages/kernel"),
                Open,
                "{label}"
            );
        }
        let tree = Tree::new(&[
            ("package.json", ROOT),
            ("packages/kernel/package.json", KERNEL),
            ("packages/broken/package.json", "{"),
        ]);
        assert_eq!(
            tree.boundary("packages/kernel"),
            Open,
            "a sibling manifest unreadable"
        );
    }

    #[test]
    fn the_digest_follows_every_file_the_answer_read() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            ("packages/kernel/package.json", KERNEL),
            ("packages/a/package.json", r#"{"name": "a"}"#),
        ]);
        let project = tree.0.join("packages/kernel/tsconfig.json");
        let before = program_boundary_digest(&project);
        assert_eq!(program_boundary_digest(&project), before);
        fs::write(
            tree.0.join("packages/a/package.json"),
            r#"{"dependencies": {"@x/kernel": "*"}}"#,
        )
        .unwrap();
        assert_ne!(program_boundary_digest(&project), before, "a sibling edit");
        let after = program_boundary_digest(&project);
        fs::create_dir_all(tree.0.join("packages/new")).unwrap();
        fs::write(tree.0.join("packages/new/package.json"), "{}").unwrap();
        assert_ne!(program_boundary_digest(&project), after, "a new member");
    }

    /// What an uncached decision says now, beside what the kept one says.
    fn both(
        cache: &mut BoundaryCache,
        project: &Path,
    ) -> ((ProgramBoundary, [u8; 32]), (ProgramBoundary, [u8; 32])) {
        let kept = cache.decide(project);
        let fresh = decide(project);
        ((kept.boundary, kept.digest), (fresh.boundary, fresh.digest))
    }

    /// The kept decision is the uncached one after every edit of the files a
    /// decision reads, including a rewrite of the same length whose
    /// modification time is put back (the change time still moves).
    #[test]
    fn a_kept_decision_follows_the_disk_as_an_uncached_one_does() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            ("packages/kernel/package.json", KERNEL),
            (
                "packages/shell/package.json",
                r#"{"name": "@x/shell", "private": true}"#,
            ),
        ]);
        let project = tree.0.join("packages/kernel/tsconfig.json");
        let mut cache = BoundaryCache::default();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh);
        assert_eq!(kept.0, Closed);
        assert!(cache.kept.is_some(), "an unchanged disk keeps the decision");
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "a request that finds the disk as it was");

        let feature = tree.0.join("packages/feature/package.json");
        let depends =
            r#"{"name": "@x/feature", "private": true, "dependencies": {"@x/kernel": "1"}}"#;
        let plain =
            r#"{"name": "@x/feature", "private": true, "dependencies": {"@y/kernel": "1"}}"#;
        assert_eq!(depends.len(), plain.len());
        fs::create_dir_all(feature.parent().unwrap()).unwrap();
        fs::write(&feature, depends).unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "a new member that depends on the project");
        assert_eq!(kept.0, Open);

        let modified = fs::metadata(&feature).unwrap().modified().unwrap();
        fs::write(&feature, plain).unwrap();
        fs::File::options()
            .write(true)
            .open(&feature)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "same length, same modification time");
        assert_eq!(kept.0, Closed);

        fs::remove_dir_all(feature.parent().unwrap()).unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "the member's directory is gone");

        fs::write(
            tree.0.join("package.json"),
            r#"{"private": true, "workspaces": ["packages/*", "extra/*"]}"#,
        )
        .unwrap();
        fs::create_dir_all(tree.0.join("extra/app")).unwrap();
        fs::write(tree.0.join("extra/app/package.json"), depends).unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "a new workspace pattern with a dependent");
        assert_eq!(kept.0, Open);

        fs::remove_file(tree.0.join("package.json")).unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh, "the workspace root manifest is gone");
    }

    /// A symbolic link's target can change under unchanged stamps, so a
    /// decision that listed one is decided again every time.
    #[cfg(unix)]
    #[test]
    fn a_decision_that_involved_a_link_is_never_kept() {
        let tree = Tree::new(&[
            ("package.json", ROOT),
            ("packages/kernel/package.json", KERNEL),
            (
                "elsewhere/shell/package.json",
                r#"{"name": "@x/shell", "private": true}"#,
            ),
        ]);
        std::os::unix::fs::symlink(
            tree.0.join("elsewhere/shell"),
            tree.0.join("packages/shell"),
        )
        .unwrap();
        let project = tree.0.join("packages/kernel/tsconfig.json");
        let mut cache = BoundaryCache::default();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh);
        assert!(cache.kept.is_none(), "a listed link: not kept");
        fs::write(
            tree.0.join("elsewhere/shell/package.json"),
            r#"{"name": "@x/shell", "private": true, "dependencies": {"@x/kernel": "1"}}"#,
        )
        .unwrap();
        let (kept, fresh) = both(&mut cache, &project);
        assert_eq!(kept, fresh);
        assert_eq!(kept.0, Open, "through the link");
    }
}
