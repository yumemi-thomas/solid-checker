//! ADR 0972: whether code a declaration describes may reach a Solid action, decided per installed package from its source by the backend
//! (`solid-facts-backend/src/package_reach.rs`) and read by the analysis. The analysis never reads the filesystem; this is its view.

use std::collections::{BTreeMap, BTreeSet};

/// Why a package's closure is not decided.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReachBlocker {
    /// A load no installed file answers, or a package with no runtime file (a declaration-only package with no runtime package beside it).
    MissingSource,
    /// A native addon or a WebAssembly module (`.node`, `.wasm`).
    NativeModule,
    /// A file Oxc does not parse.
    Unparseable,
    /// A load by a computed name.
    DynamicLoad,
    /// `eval`, the `Function` constructor, a `.constructor` call.
    CodeGeneration,
    /// A global the pinned TypeScript libraries do not declare.
    UnknownGlobal,
    /// The global object read by a computed key or handed on.
    GlobalObjectEscape,
    /// A `package.json` the walk does not read (unparseable, an `exports` form it does not read).
    ManifestUnread,
    /// The closure is larger than the walk follows.
    ClosureLimit,
    /// The adapter reads no filesystem (the WASM `analyze` entry): nothing is decided.
    NoFilesystem,
    /// A declaration file of the project with runtime JavaScript beside it that the program does not analyze (`allowJs` is off): nobody reads that code.
    UnanalyzedProjectJs,
    /// The walk that finds where a value came from stopped at a limit (an array of more elements than it reads, a chain of more hops than it follows): what runs
    /// for the value is not decided, so the call of it is a site rather than a silence (ADR 0973).
    OriginLimit,
}

impl ReachBlocker {
    /// The kebab-case word a row's blocker detail names it by.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::MissingSource => "missing-source",
            Self::NativeModule => "native-module",
            Self::Unparseable => "unparseable",
            Self::DynamicLoad => "dynamic-load",
            Self::CodeGeneration => "code-generation",
            Self::UnknownGlobal => "unknown-global",
            Self::GlobalObjectEscape => "global-object-escape",
            Self::ManifestUnread => "manifest-unread",
            Self::ClosureLimit => "closure-limit",
            Self::NoFilesystem => "no-filesystem",
            Self::UnanalyzedProjectJs => "unanalyzed-project-js",
            Self::OriginLimit => "origin-limit",
        }
    }
}

/// The prefix of a site's pseudo-path for a callee that is a global declared by no installed package and no standard-library file (a project declaration
/// file: `declare function gtag(…)`), whose provider the declaration does not say: the host, a script or a package may write it (ADR 0973).
pub const AMBIENT_PREFIX: &str = "ambient:";

/// The prefix of a site's pseudo-path for a value whose origin the walk could not follow to its end: it stopped at a limit (`origin-limit:array of more than 256 elements`),
/// so what runs for the value is not decided (ADR 0973).
pub const LIMIT_PREFIX: &str = "origin-limit:";

/// The prefix of a site's pseudo-path for a module loaded by a name the syntax does not fix (`lazy(() => import(name))`): any module may be the component.
pub const COMPUTED_LOAD_PREFIX: &str = "computed-load:";

/// What a package's closure was found to reach.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageReach {
    /// Nothing in the closure names a Solid package, and nothing in it loads code the syntax does not show.
    Clear,
    /// The closure loads a Solid package: `via` names the load (`pkg -> solid-js`).
    ReachesSolid { via: String },
    /// The closure is not decided: why, and where.
    Undecided {
        blocker: ReachBlocker,
        detail: String,
    },
}

impl PackageReach {
    #[must_use]
    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }

    /// The blocker detail a row names (`reaches-solid`, `unknown-global`, …).
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::ReachesSolid { .. } => "reaches-solid",
            Self::Undecided { blocker, .. } => blocker.word(),
        }
    }

    /// What the row's message says about it.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Clear => "nothing in its closure names a Solid package".to_owned(),
            Self::ReachesSolid { via } => format!("its closure loads a Solid package ({via})"),
            Self::Undecided { blocker, detail } => {
                format!("its closure is not decided ({}: {detail})", blocker.word())
            }
        }
    }
}

/// Where the closure of a package may define an accessor (a `get` or `set`, a `Proxy`): the code a member read or assignment of an object the package hands out may run
/// (ADR 0973). Read from the whole closure, so it answers only where the whole closure was read.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccessorFacts {
    /// Whether the whole closure was read and holds nothing the syntax cannot see through: no unread file, load by a computed name, generated code or value from
    /// outside the closure (a global the libraries do not declare). A load of Solid's runtime is known and keeps it closed. Open, any member of an object the package
    /// hands out may be an accessor of code nobody read.
    pub closed: bool,
    /// The names accessors are defined under in the closure.
    pub names: BTreeSet<String>,
    /// An accessor, or a construct that may define one (a `Proxy`, a computed key), is defined under a name the syntax does not fix: it matches every name.
    pub any_name: bool,
}

impl AccessorFacts {
    /// Nothing is known: every member may be an accessor.
    #[must_use]
    pub fn open() -> Self {
        Self::default()
    }

    /// Whether reading or assigning a member by one of `chain` (the names read off the package's object, `None` for a computed key) may run package code: the
    /// closure was not read whole, defines an accessor under an unfixed name, or defines one under a name of the chain (a computed key matches any accessor).
    #[must_use]
    pub fn may_run(&self, chain: &[Option<&str>]) -> bool {
        if !self.closed || self.any_name {
            return true;
        }
        chain.iter().any(|name| match name {
            Some(name) => self.names.contains(*name),
            None => !self.names.is_empty(),
        })
    }
}

/// The verdicts the backend decided, by installed package root (the directory under the last `node_modules` component of a declaration's path, see
/// [`package_root_of`]).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageReachIndex {
    pub by_root: BTreeMap<String, PackageReach>,
    /// The `name` of each root's `package.json`: what the package is called, whatever directory a linker put it in (`.store/<slot>/package`).
    pub names: BTreeMap<String, String>,
    /// Each linked workspace package the project imports: the real directory a `node_modules` link points to (outside every `node_modules`) keyed to the
    /// link's own package root. The compiler gives a linked package's declarations their real paths, so they read as no installed code until the link names
    /// them ([`Self::linked_spelling`]).
    pub linked: BTreeMap<String, String>,
    /// The project's own declaration files (`vendor/step.d.ts`) with runtime JavaScript beside them that the program does not analyze: a declaration there
    /// describes code nobody reads, so it is undecided ([`ReachBlocker::UnanalyzedProjectJs`]).
    pub project_js: std::collections::BTreeSet<String>,
    /// Where each decided package's closure may define an accessor, by the same root.
    pub accessors: BTreeMap<String, AccessorFacts>,
    /// The reviewed premises that hold for each root: a reading of the package's source for one API (ADR 0973), pinned to digests of the files read, so it holds
    /// only where the installed bytes are those. The ids are the analysis's (`reviewed_api`).
    pub reviewed: BTreeMap<String, std::collections::BTreeSet<String>>,
}

impl PackageReachIndex {
    /// The name the manifest at `root` gives its package; `None` where the backend read no manifest.
    #[must_use]
    pub fn name_of(&self, root: &str) -> Option<&str> {
        self.names.get(root).map(String::as_str)
    }

    /// The `node_modules` spelling of a declaration `path` inside a linked workspace package (the link's root, then the path below the real directory), when the
    /// backend found the link; `None` for any other path. The longest real directory that holds the path names it.
    #[must_use]
    pub fn linked_spelling(&self, path: &str) -> Option<String> {
        self.linked
            .iter()
            .filter_map(|(real, link)| {
                let below = path.strip_prefix(real.as_str())?.strip_prefix('/')?;
                Some((real.len(), format!("{link}/{below}")))
            })
            .max_by_key(|(length, _)| *length)
            .map(|(_, spelled)| spelled)
    }

    /// Whether `path` is a project declaration file whose runtime JavaScript the program does not analyze.
    #[must_use]
    pub fn is_project_js(&self, path: &str) -> bool {
        self.project_js.contains(path)
    }

    /// Whether the reviewed premise `id` holds for the package a declaration at `path` is in.
    #[must_use]
    pub fn premise_holds(&self, path: &str, id: &str) -> bool {
        package_root_of(path)
            .and_then(|root| self.reviewed.get(root))
            .is_some_and(|ids| ids.contains(id))
    }

    /// Where the closure of the package a declaration at `path` describes may define an accessor. A path the backend did not decide, and a pseudo-path of a site
    /// ([`AMBIENT_PREFIX`], [`COMPUTED_LOAD_PREFIX`]), is open: any member may be one.
    #[must_use]
    pub fn accessors_for_declaration(&self, path: &str) -> AccessorFacts {
        if path.starts_with(AMBIENT_PREFIX)
            || path.starts_with(COMPUTED_LOAD_PREFIX)
            || path.starts_with(LIMIT_PREFIX)
            || self.project_js.contains(path)
        {
            return AccessorFacts::open();
        }
        let key = package_root_of(path).unwrap_or(path);
        self.accessors.get(key).cloned().unwrap_or_default()
    }

    /// The verdict for the code a declaration at `path` describes. A path the backend did not decide is undecided, never clear.
    #[must_use]
    pub fn for_declaration(&self, path: &str) -> PackageReach {
        if let Some(declared) = path.strip_prefix(AMBIENT_PREFIX) {
            return PackageReach::Undecided {
                blocker: ReachBlocker::UnknownGlobal,
                detail: format!(
                    "{declared} declares a global, and the host, a script or an installed package may provide it"
                ),
            };
        }
        if let Some(what) = path.strip_prefix(LIMIT_PREFIX) {
            return PackageReach::Undecided {
                blocker: ReachBlocker::OriginLimit,
                detail: format!(
                    "{what}: the walk that finds where the value came from stopped at its limit"
                ),
            };
        }
        if let Some(specifier) = path.strip_prefix(COMPUTED_LOAD_PREFIX) {
            return PackageReach::Undecided {
                blocker: ReachBlocker::DynamicLoad,
                detail: format!("{specifier}: a module loaded by a name the syntax does not fix"),
            };
        }
        if self.project_js.contains(path) {
            return PackageReach::Undecided {
                blocker: ReachBlocker::UnanalyzedProjectJs,
                detail: format!(
                    "{path} has runtime code beside it that the program does not analyze"
                ),
            };
        }
        let key = package_root_of(path).unwrap_or(path);
        self.by_root
            .get(key)
            .cloned()
            .unwrap_or_else(|| PackageReach::Undecided {
                blocker: ReachBlocker::MissingSource,
                detail: format!("no verdict for {key}"),
            })
    }
}

/// The path a relative `specifier` names from the directory of `importer`, normalized lexically (`.` and `..` resolved, no filesystem read).
#[must_use]
pub fn join_relative(importer: &str, specifier: &str) -> String {
    let mut parts = importer.split('/').collect::<Vec<_>>();
    parts.pop();
    // A query or a fragment (`./step.vendor.js?fz`) is the bundler's: the file is the one the path names.
    let specifier = specifier.split(['?', '#']).next().unwrap_or(specifier);
    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// The runtime files a bundler tries for the path `base` an import names: the path itself when it carries a script extension, else each extension, then the
/// directory's `index`.
#[must_use]
pub fn script_candidates(base: &str) -> Vec<String> {
    const EXTENSIONS: [&str; 4] = [".js", ".mjs", ".cjs", ".jsx"];
    if EXTENSIONS.iter().any(|extension| base.ends_with(extension)) {
        return vec![base.to_owned()];
    }
    EXTENSIONS
        .iter()
        .map(|extension| format!("{base}{extension}"))
        .chain(
            EXTENSIONS
                .iter()
                .map(|extension| format!("{base}/index{extension}")),
        )
        .collect()
}

/// The installed package directory a path is in: the prefix through the package name after the last `node_modules` component
/// (`/p/node_modules/@scope/name/dist/x.d.ts` is `/p/node_modules/@scope/name`). A yarn `pnpm`-linker store keeps each package in
/// `node_modules/.store/<slot>/package`, so that is the root there. `None` for a path in no `node_modules`.
#[must_use]
pub fn package_root_of(path: &str) -> Option<&str> {
    let marker = "/node_modules/";
    let start = path.rfind(marker)? + marker.len();
    let rest = &path[start..];
    let mut parts = rest.splitn(4, '/');
    let first = parts.next().filter(|part| !part.is_empty())?;
    let length = if first == ".store" {
        let slot = parts.next().filter(|part| !part.is_empty())?;
        parts.next().filter(|part| *part == "package")?;
        first.len() + 1 + slot.len() + 1 + "package".len()
    } else if first.starts_with('@') {
        let second = parts.next().filter(|part| !part.is_empty())?;
        first.len() + 1 + second.len()
    } else {
        first.len()
    };
    Some(&path[..start + length])
}

/// The name a package root goes by in a report: the directory under its last `node_modules` (`@scope/name`), and for a `.store` slot
/// (`@babel-code-frame-npm-7.29.7-cc910f9962`) the package name it is named for (`@babel/code-frame`).
#[must_use]
pub fn package_label(root: &str) -> String {
    let after = root
        .rsplit_once("/node_modules/")
        .map_or(root, |(_, name)| name);
    let Some(slot) = after
        .strip_prefix(".store/")
        .and_then(|rest| rest.strip_suffix("/package"))
    else {
        return after.to_owned();
    };
    let name = slot.split_once("-npm-").map_or(slot, |(name, _)| name);
    match name.strip_prefix('@') {
        Some(scoped) => format!("@{}", scoped.replacen('-', "/", 1)),
        None => name.to_owned(),
    }
}

/// Whether `path` is a TypeScript declaration file.
#[must_use]
pub fn is_declaration_file(path: &str) -> bool {
    path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_root_is_the_last_node_modules_package() {
        assert_eq!(
            package_root_of("/p/node_modules/x/dist/i.d.ts"),
            Some("/p/node_modules/x")
        );
        assert_eq!(
            package_root_of("/p/node_modules/@s/x/i.d.ts"),
            Some("/p/node_modules/@s/x")
        );
        assert_eq!(
            package_root_of("/p/node_modules/.pnpm/x@1/node_modules/x/i.d.ts"),
            Some("/p/node_modules/.pnpm/x@1/node_modules/x")
        );
        assert_eq!(
            package_root_of(
                "/p/node_modules/.store/relay-compiler-npm-21.0.1-1e06a71766/package/lib/x.d.ts"
            ),
            Some("/p/node_modules/.store/relay-compiler-npm-21.0.1-1e06a71766/package")
        );
        assert_eq!(
            package_root_of("/p/node_modules/.store/x-npm-1.0.0-ab/node_modules/dep/a.d.ts"),
            Some("/p/node_modules/.store/x-npm-1.0.0-ab/node_modules/dep")
        );
        assert_eq!(package_root_of("/p/src/env.d.ts"), None);
        assert_eq!(package_root_of("/p/node_modules/@s"), None);
    }

    #[test]
    fn a_store_slot_is_labelled_by_the_package_it_is_named_for() {
        assert_eq!(package_label("/p/node_modules/@a/b"), "@a/b");
        assert_eq!(package_label("/p/node_modules/x"), "x");
        assert_eq!(
            package_label("/p/node_modules/.store/@babel-code-frame-npm-7.29.7-cc910f9962/package"),
            "@babel/code-frame"
        );
        assert_eq!(
            package_label("/p/node_modules/.store/relay-compiler-npm-21.0.1-1e06a71766/package"),
            "relay-compiler"
        );
    }

    #[test]
    fn a_declaration_in_a_linked_package_is_spelled_by_its_link() {
        let mut index = PackageReachIndex::default();
        index.linked.insert(
            "/ws/packages/kernel".to_owned(),
            "/ws/apps/web/node_modules/@ws/kernel".to_owned(),
        );
        index.linked.insert(
            "/ws/packages/kernel/vendor".to_owned(),
            "/ws/apps/web/node_modules/vendor".to_owned(),
        );
        assert_eq!(
            index.linked_spelling("/ws/packages/kernel/dist/index.d.ts"),
            Some("/ws/apps/web/node_modules/@ws/kernel/dist/index.d.ts".to_owned())
        );
        // The longest real directory names the path.
        assert_eq!(
            index.linked_spelling("/ws/packages/kernel/vendor/x.d.ts"),
            Some("/ws/apps/web/node_modules/vendor/x.d.ts".to_owned())
        );
        // A sibling that only shares the prefix, the directory itself and any other path are not in a link.
        assert_eq!(
            index.linked_spelling("/ws/packages/kernel-extra/x.d.ts"),
            None
        );
        assert_eq!(index.linked_spelling("/ws/packages/kernel"), None);
        assert_eq!(index.linked_spelling("/ws/apps/web/ambient.d.ts"), None);
    }

    #[test]
    fn a_project_declaration_with_unread_javascript_is_undecided() {
        let mut index = PackageReachIndex::default();
        index.project_js.insert("/p/vendor/step.d.ts".to_owned());
        assert!(index.is_project_js("/p/vendor/step.d.ts"));
        assert!(!index.is_project_js("/p/vendor/other.d.ts"));
        let reach = index.for_declaration("/p/vendor/step.d.ts");
        assert_eq!(reach.word(), "unanalyzed-project-js");
        assert!(!reach.is_clear());
    }

    #[test]
    fn a_global_or_a_computed_load_is_undecided_by_its_pseudo_path() {
        let index = PackageReachIndex::default();
        let ambient = index.for_declaration("ambient:/p/globals.d.ts");
        assert_eq!(ambient.word(), "unknown-global");
        assert!(!ambient.is_clear());
        let computed = index.for_declaration("computed-load:import()");
        assert_eq!(computed.word(), "dynamic-load");
        assert!(!computed.is_clear());
        let limit = index.for_declaration("origin-limit:an array of more than 256 elements");
        assert_eq!(limit.word(), "origin-limit");
        assert!(!limit.is_clear());
    }

    #[test]
    fn a_member_may_run_code_unless_the_whole_closure_names_no_accessor_for_it() {
        let mut facts = AccessorFacts {
            closed: true,
            ..AccessorFacts::default()
        };
        // Closed, no accessor: nothing runs.
        assert!(!facts.may_run(&[Some("now"), Some("later")]));
        assert!(!facts.may_run(&[None]));
        // An accessor under one name: that name, and a computed key, may run it; another name may not.
        facts.names.insert("now".to_owned());
        assert!(facts.may_run(&[Some("now")]));
        assert!(facts.may_run(&[Some("other"), Some("now")]));
        assert!(facts.may_run(&[None]));
        assert!(!facts.may_run(&[Some("later")]));
        // An accessor under an unfixed name matches every name.
        facts.any_name = true;
        assert!(facts.may_run(&[Some("later")]));
        // Open (a closure not read whole, a path nobody decided): every name.
        assert!(AccessorFacts::open().may_run(&[Some("later")]));
        assert!(
            PackageReachIndex::default()
                .accessors_for_declaration("/p/node_modules/x/i.d.ts")
                .may_run(&[Some("later")])
        );
    }

    #[test]
    fn a_relative_specifier_joins_lexically() {
        assert_eq!(
            join_relative("/p/src/App.tsx", "./vendor/step.js"),
            "/p/src/vendor/step.js"
        );
        assert_eq!(join_relative("/p/src/a/App.tsx", "../x/./y"), "/p/src/x/y");
        // A query or a fragment is no part of the path.
        assert_eq!(
            join_relative("/p/src/App.tsx", "./step.vendor.js?fz"),
            "/p/src/step.vendor.js"
        );
        assert_eq!(join_relative("/p/src/App.tsx", "./a.js#x"), "/p/src/a.js");
        assert_eq!(
            script_candidates("/p/src/step").first().map(String::as_str),
            Some("/p/src/step.js")
        );
        assert_eq!(
            script_candidates("/p/src/step.vendor.js"),
            vec!["/p/src/step.vendor.js"]
        );
    }

    #[test]
    fn an_undecided_path_is_never_clear() {
        let index = PackageReachIndex::default();
        assert!(!index.for_declaration("/p/node_modules/x/i.d.ts").is_clear());
    }
}
