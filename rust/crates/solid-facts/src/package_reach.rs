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
    /// for the value is not decided, so the call of it is a site rather than a silence (ADR 0974).
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
/// file: `declare function gtag(…)`), whose provider the declaration does not say: the host, a script or a package may write it (ADR 0974).
pub const AMBIENT_PREFIX: &str = "ambient:";

/// The prefix of a site's pseudo-path for a global the standard library declares that an installed package writes at run time (`global-write:<name>:<writer root>`): a call
/// of it runs the writer's code, whatever type the library declares the global with. Its verdict is the writer's (ADR 0974).
pub const WRITE_PREFIX: &str = "global-write:";

/// [`WRITE_PREFIX`] for a global the writer may write under a key the scan does not decide (`global-write-any:<name>:<writer root>`): the writer's code is what the call may
/// run, and the row says the package may write it, not that it does.
pub const WRITE_ANY_PREFIX: &str = "global-write-any:";

/// ADR 1108: the prefix of a site's pseudo-path for a call of a method the standard library declares that an installed package writes on a built-in's prototype at run
/// time (`prototype-write:<name>:<writer root>`): the call runs the writer's code, whatever type the library declares the method with. Its verdict is the writer's.
pub const PROTOTYPE_WRITE_PREFIX: &str = "prototype-write:";

/// ADR 1108: [`PROTOTYPE_WRITE_PREFIX`] for a writer that may write a built-in's prototype under a key nothing decides (`prototype-write-any:<name>:<writer root>`).
pub const PROTOTYPE_WRITE_ANY_PREFIX: &str = "prototype-write-any:";

/// The prefix of a site's pseudo-path for a member of the global object read by a key the program does not spell (`window[key]`, `const { [key]: value } = window`, a rest
/// element of a destructuring of it): any global may be the one read, so the code is that of a package that writes any (`global-read-any:<writer root>`). Its verdict is the
/// writer's (ADR 0974).
pub const READ_ANY_PREFIX: &str = "global-read-any:";

/// The prefix of a site's pseudo-path for a value whose origin the walk could not follow to its end: it stopped at a limit (`origin-limit:array of more than 256 elements`),
/// so what runs for the value is not decided (ADR 0974).
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
/// (ADR 0974). Read from the whole closure, so it answers only where the whole closure was read.
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
    /// The reviewed premises that hold for each root: a reading of the package's source for one API (ADR 0974), pinned to digests of the files read, so it holds
    /// only where the installed bytes are those. The ids are the analysis's (`reviewed_api`).
    pub reviewed: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The packages whose closure writes the global of each name (`globalThis.x = …`, `Object.defineProperty(window, "x", …)`), by root (ADR 0974): the installed code the
    /// program loads that may be what a call of that global runs.
    pub global_writers: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The packages whose closure writes a global under a key nothing in the file decides (`window[key] = v` with a key from a call or a parameter,
    /// `Object.assign(globalThis, source)` of a source the file does not hold): any global may be one of theirs (see [`PackageReachIndex::global_writers_of`]).
    pub global_writers_any: std::collections::BTreeSet<String>,
    /// The packages whose closure defines an accessor (a getter or a setter, or a descriptor the scan does not read) on the global object under each name
    /// (`Object.defineProperty(window, "devicePixelRatio", { get })`), by root: a read of that global runs their code, which a data write does not.
    pub global_accessor_writers: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The packages whose closure may define an accessor on the global object under a name nothing in the file decides, or change its prototype: a read of any global
    /// may run their code (see [`PackageReachIndex::global_accessor_writers_of`]).
    pub global_accessor_writers_any: std::collections::BTreeSet<String>,
    /// ADR 1108: the packages whose closure writes a member of each name on a built-in's prototype (`Float32Array.prototype.reverse = …`), by root: every object of that
    /// built-in inherits it, so a call of a method the standard library declares under that name may run their code.
    pub prototype_writers: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// ADR 1108: the packages whose closure may write a built-in's prototype under a name nothing decides (a key the syntax does not fix, the prototype or the global
    /// object handed on, generated code, code nobody read): a call of any method the standard library declares may run their code.
    pub prototype_writers_any: std::collections::BTreeSet<String>,
    /// Why the code a package's closure runs is not all seen, by installed package root (the second re-check of #232): a file of the closure the build reached names
    /// `Object.prototype` or builds code as a string (`names Object.prototype`, `generates code`), or the closure is not read to its end (a file or a load the walk cannot
    /// read: `undecided: <blocker>`). A package that gives a shared prototype an accessor reads any key through it, and nothing in the project shows the read; an
    /// unread package may do the same. Solid's own packages are the dialect's and are not listed. Empty where every package the program loads was read and none does.
    pub unseen_code: BTreeMap<String, String>,
    /// The roots of the packages the program loads, read to the end of their closures, that hold a `this` or a `super` anywhere in it: a getter such a package writes reads
    /// a member through the object it runs against, and no file of the program shows the read. Asked of a program that can name a shared prototype (`unseen_code` is
    /// the answer for a package that does itself).
    pub reads_receiver: std::collections::BTreeSet<String>,
    /// ADR 1061 (the blocker census, internal only): everything each root's closure holds, not only its verdict. Filled only where [`blocker_census`] is on;
    /// no analysis reads it for a verdict.
    pub census: BTreeMap<String, RootCensus>,
    /// ADR 1145: where the product's real entries are known, the files they load and the prototype writers among what those files (and the declared host) load.
    /// `None` is every site judged against every writer above, as before.
    pub product: Option<ProductWriters>,
}

/// ADR 1145: the product realm of a program whose real entries are read from its build configuration (an HTML page's module scripts, a package's `exports`): the
/// program's files the entries load (a closure over every static and literal load) and, of the packages that write a built-in's prototype, those the closure's
/// files or the declared host load. A site in one of `files` runs in the product's realm, which no other entry's loads (a test, a script, a config) share, so it is
/// judged against these writers only ([`PackageReachIndex::product_scope`]); a site anywhere else is judged against every writer, as every site was.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductWriters {
    /// The program's files the entries load, by path.
    pub files: BTreeSet<String>,
    /// [`PackageReachIndex::prototype_writers`], kept where a package the product loads writes the name.
    pub prototype_writers: BTreeMap<String, BTreeSet<String>>,
    /// [`PackageReachIndex::prototype_writers_any`], kept where the product loads the package.
    pub prototype_writers_any: BTreeSet<String>,
    /// Whether the realm holds fewer writers than the index (a realm that keeps every writer changes no site).
    pub narrows: bool,
}

/// ADR 1145: the prototype writers one site is judged against ([`PackageReachIndex::full_scope`], [`PackageReachIndex::product_scope`]): the index's, or the product
/// realm's for a site in a file the product's entries load. The clear filter of every query is the index's own (`by_root`).
#[derive(Clone, Copy, Debug)]
pub struct WriterScope<'a> {
    index: &'a PackageReachIndex,
    named: &'a BTreeMap<String, BTreeSet<String>>,
    any: &'a BTreeSet<String>,
}

impl<'a> WriterScope<'a> {
    fn counts(&self, root: &str) -> bool {
        self.index
            .by_root
            .get(root)
            .is_none_or(|reach| !reach.is_clear())
    }

    /// Whether no package writes a built-in's prototype, by name or under a name nothing decides, in this scope (clear packages included).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && self.any.is_empty()
    }

    /// [`PackageReachIndex::prototype_writers_by_name`] in this scope.
    pub fn prototype_writers_by_name(&self, name: &str) -> impl Iterator<Item = &'a str> + 'a {
        let scope = *self;
        self.named
            .get(name)
            .into_iter()
            .flatten()
            .filter(move |root| scope.counts(root))
            .map(String::as_str)
    }

    /// [`PackageReachIndex::prototype_writers_any_name`] in this scope.
    pub fn prototype_writers_any_name(&self) -> impl Iterator<Item = &'a str> + 'a {
        let scope = *self;
        self.any
            .iter()
            .filter(move |root| scope.counts(root))
            .map(String::as_str)
    }

    /// [`PackageReachIndex::prototype_writers_of_unspelled`] in this scope.
    pub fn prototype_writers_of_unspelled(&self) -> impl Iterator<Item = &'a str> + 'a {
        let scope = *self;
        self.named
            .values()
            .flatten()
            .chain(self.any.iter())
            .filter(move |root| scope.counts(root))
            .map(String::as_str)
    }

    /// [`PackageReachIndex::prototype_writers_of`] in this scope: the roots that write `name` by name (`true`) or may under a name nothing decides (`false`).
    pub fn prototype_writers_of(&self, name: &str) -> impl Iterator<Item = (&'a str, bool)> + 'a {
        let scope = *self;
        let by_name = self.named.get(name);
        let any = self
            .any
            .iter()
            .filter(move |root| !by_name.is_some_and(|roots| roots.contains(*root)));
        by_name
            .into_iter()
            .flatten()
            .map(|root| (root, true))
            .chain(any.map(|root| (root, false)))
            .filter(move |(root, _)| scope.counts(root))
            .map(|(root, named)| (root.as_str(), named))
    }

    /// The first package (by name, then by root) that writes a member by name and may reach a Solid action: the member and the root.
    #[must_use]
    pub fn first_named_writer(&self) -> Option<(&'a str, &'a str)> {
        self.named.iter().find_map(|(name, roots)| {
            roots
                .iter()
                .find(|root| self.counts(root))
                .map(|root| (name.as_str(), root.as_str()))
        })
    }
}

/// ADR 1061: what the blocker census records of one root's closure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RootCensus {
    /// Every problem the walk met, each once, in walk order (the verdict first).
    pub problems: Vec<PackageReach>,
    /// The files of the closure that write a global under a key nothing in the file decides, and whether code nobody read may.
    pub write_any_files: Vec<String>,
    pub write_any_unread: bool,
}

/// ADR 1061: whether the blocker census is on (`SOLID_CHECKER_BLOCKER_CENSUS=1`). Internal only: it adds the `blockerCensus` table to the internal JSON and
/// changes no finding, verdict, text or exit code.
#[must_use]
pub fn blocker_census() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var_os("SOLID_CHECKER_BLOCKER_CENSUS").is_some_and(|value| value == "1")
    })
}

impl PackageReachIndex {
    /// ADR 1145: every prototype writer the index holds, the scope of a site the product realm does not cover (and of every site, where it is not used).
    #[must_use]
    pub fn full_scope(&self) -> WriterScope<'_> {
        WriterScope {
            index: self,
            named: &self.prototype_writers,
            any: &self.prototype_writers_any,
        }
    }

    /// ADR 1145: the writers a site in the file at `path` is judged against where the product's entries load the file and the realm holds fewer writers than the
    /// index; `None` where the site is judged against [`Self::full_scope`].
    #[must_use]
    pub fn product_scope(&self, path: &str) -> Option<WriterScope<'_>> {
        let product = self
            .product
            .as_ref()
            .filter(|product| product.narrows && product.files.contains(path))?;
        Some(WriterScope {
            index: self,
            named: &product.prototype_writers,
            any: &product.prototype_writers_any,
        })
    }

    /// The first package (by root) whose closure runs code the build does not see, and why; `None` where none does.
    #[must_use]
    pub fn unseen_code(&self) -> Option<(&str, &str)> {
        self.unseen_code
            .iter()
            .next()
            .map(|(root, why)| (root.as_str(), why.as_str()))
    }

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

    /// The roots of the installed packages whose closure writes the global `name` and may reach a Solid action, with whether each writes it by that name (the names its
    /// constants decide included) or may write it under a key nothing decides: the first, by name then by root, answers for a call of the global. Empty when no installed
    /// package the program loads writes it, or each that does is clear (a clear package is silent: its closure names no Solid package and has no hazard).
    pub fn global_writers_of<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = (&'a str, bool)> + 'a {
        let by_name = self.global_writers.get(name);
        let any = self
            .global_writers_any
            .iter()
            .filter(move |root| !by_name.is_some_and(|roots| roots.contains(*root)));
        by_name
            .into_iter()
            .flatten()
            .map(|root| (root, true))
            .chain(any.map(|root| (root, false)))
            .filter(|(root, _)| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(|(root, named)| (root.as_str(), named))
    }

    /// The roots of the installed packages whose closure may define an accessor on the global object under `name` and may reach a Solid action, with whether each does
    /// it under that name (the names its constants decide included) or under a key nothing decides: a read of the global (a member of the global object, a bare name, a
    /// pattern key, `typeof`) runs the code of the first, by name then by root. Empty when no installed package the program loads defines one, or each that does is clear.
    pub fn global_accessor_writers_of<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = (&'a str, bool)> + 'a {
        let by_name = self.global_accessor_writers.get(name);
        let any = self
            .global_accessor_writers_any
            .iter()
            .filter(move |root| !by_name.is_some_and(|roots| roots.contains(*root)));
        by_name
            .into_iter()
            .flatten()
            .map(|root| (root, true))
            .chain(any.map(|root| (root, false)))
            .filter(|(root, _)| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(|(root, named)| (root.as_str(), named))
    }

    /// The roots of the installed packages whose closure writes any global and may reach a Solid action, by whatever name: the packages a call of the global object's
    /// member under a key the program does not spell (`window[key]()`, a pattern key that is computed, a rest) may run the code of. A clear package is silent.
    pub fn global_writers_of_unspelled<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.global_writers
            .values()
            .flatten()
            .chain(self.global_writers_any.iter())
            .filter(|root| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(String::as_str)
    }

    /// ADR 1108: the roots of the installed packages whose closure writes a member `name` on a built-in's prototype and may reach a Solid action, with whether each writes
    /// it by that name or may write a prototype under a key nothing decides: a call of a method the standard library declares under `name` runs the code of the first, by name
    /// then by root. Empty when no installed package the program loads writes one, or each that does is clear.
    pub fn prototype_writers_of<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = (&'a str, bool)> + 'a {
        let by_name = self.prototype_writers.get(name);
        let any = self
            .prototype_writers_any
            .iter()
            .filter(move |root| !by_name.is_some_and(|roots| roots.contains(*root)));
        by_name
            .into_iter()
            .flatten()
            .map(|root| (root, true))
            .chain(any.map(|root| (root, false)))
            .filter(|(root, _)| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(|(root, named)| (root.as_str(), named))
    }

    /// ADR 1129 (review F2): the roots of the installed packages whose closure writes a member `name` on a built-in's prototype, or may write one under a key nothing
    /// decides (an unread closure included), clear or not: a premise that a standard method behaves as the library says fails wherever one is loaded, whether or not
    /// the writer can reach Solid.
    pub fn prototype_member_writers<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.prototype_writers
            .get(name)
            .into_iter()
            .flatten()
            .chain(self.prototype_writers_any.iter())
            .map(String::as_str)
    }

    /// ADR 1108 part 2: the roots of the installed packages that write `name` on a built-in's prototype by that name and may reach a Solid action (no any-name writer).
    pub fn prototype_writers_by_name<'a>(
        &'a self,
        name: &str,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.prototype_writers
            .get(name)
            .into_iter()
            .flatten()
            .filter(|root| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(String::as_str)
    }

    /// ADR 1108 part 2: the roots of the installed packages that may write a built-in's prototype under a name nothing decides and may reach a Solid action.
    pub fn prototype_writers_any_name<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.prototype_writers_any
            .iter()
            .filter(|root| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(String::as_str)
    }

    /// ADR 1108: the roots of the installed packages whose closure writes any member on a built-in's prototype and may reach a Solid action: a call of a standard-library
    /// method by a key the program does not spell may run their code.
    pub fn prototype_writers_of_unspelled<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.prototype_writers
            .values()
            .flatten()
            .chain(self.prototype_writers_any.iter())
            .filter(|root| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(String::as_str)
    }

    /// Whether any installed package the program loads defines, or may define, an accessor on the global object: where none does, no read of a global runs a package's code
    /// and the questions [`Self::global_accessor_writers_of`] and [`Self::global_accessor_writers_of_unspelled`] answer nothing.
    #[must_use]
    pub fn defines_global_accessors(&self) -> bool {
        !self.global_accessor_writers.is_empty() || !self.global_accessor_writers_any.is_empty()
    }

    /// The roots of the installed packages whose closure may define an accessor on the global object under any name and may reach a Solid action: the packages a read
    /// of the global object's member under a key the program does not spell may run the code of.
    pub fn global_accessor_writers_of_unspelled<'a>(
        &'a self,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.global_accessor_writers
            .values()
            .flatten()
            .chain(self.global_accessor_writers_any.iter())
            .filter(|root| {
                self.by_root
                    .get(root.as_str())
                    .is_none_or(|reach| !reach.is_clear())
            })
            .map(String::as_str)
    }

    /// Where the closure of the package a declaration at `path` describes may define an accessor. A path the backend did not decide, and a pseudo-path of a site
    /// ([`AMBIENT_PREFIX`], [`COMPUTED_LOAD_PREFIX`]), is open: any member may be one.
    #[must_use]
    pub fn accessors_for_declaration(&self, path: &str) -> AccessorFacts {
        if path.starts_with(AMBIENT_PREFIX)
            || path.starts_with(COMPUTED_LOAD_PREFIX)
            || path.starts_with(LIMIT_PREFIX)
            || path.starts_with(WRITE_PREFIX)
            || path.starts_with(WRITE_ANY_PREFIX)
            || path.starts_with(READ_ANY_PREFIX)
            || path.starts_with(PROTOTYPE_WRITE_PREFIX)
            || path.starts_with(PROTOTYPE_WRITE_ANY_PREFIX)
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
        if let Some(written) = path
            .strip_prefix(WRITE_PREFIX)
            .or_else(|| path.strip_prefix(WRITE_ANY_PREFIX))
            .or_else(|| path.strip_prefix(PROTOTYPE_WRITE_PREFIX))
            .or_else(|| path.strip_prefix(PROTOTYPE_WRITE_ANY_PREFIX))
        {
            // `global-write:<name>:<writer root>`: the writer's closure decides.
            let (name, root) = written.split_once(':').unwrap_or((written, ""));
            return match self.by_root.get(root) {
                Some(reach) => reach.clone(),
                None => PackageReach::Undecided {
                    blocker: ReachBlocker::UnknownGlobal,
                    detail: format!(
                        "{root} writes the global {name}, and no verdict is recorded for it"
                    ),
                },
            };
        }
        if let Some(root) = path.strip_prefix(READ_ANY_PREFIX) {
            // `global-read-any:<writer root>`: the writer's closure decides.
            return match self.by_root.get(root) {
                Some(reach) => reach.clone(),
                None => PackageReach::Undecided {
                    blocker: ReachBlocker::UnknownGlobal,
                    detail: format!("{root} writes globals, and no verdict is recorded for it"),
                },
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

/// ADR 1082/1086: `path` spelled as a plain install places it (`…/node_modules/<name>/<rest>`) when it lies in a linker's store slot
/// (`…/node_modules/.store/<slot>/package/<rest>`, [`package_root_of`]) whose `package.json` the backend read ([`PackageReachIndex::name_of`]):
/// a package is what its manifest names, whatever directory the linker gave it. Every check that recognises a package by the path
/// segments of a declaration or file asks this first. Any other path, a slot whose manifest was not read, and an adapter with no
/// filesystem (`index` is `None`) keep `path` as written, so nothing unread is taken for a named package.
#[must_use]
pub fn installed_spelling<'p>(
    index: Option<&PackageReachIndex>,
    path: &'p str,
) -> std::borrow::Cow<'p, str> {
    let slot = package_root_of(path).and_then(|root| {
        let (prefix, below) = root.rsplit_once("/node_modules/")?;
        below.starts_with(".store/").then_some(())?;
        let name = index?.name_of(root)?;
        Some(format!(
            "{prefix}/node_modules/{name}{}",
            &path[root.len()..]
        ))
    });
    slot.map_or(std::borrow::Cow::Borrowed(path), std::borrow::Cow::Owned)
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

    // ADR 1086: a store slot reads as the package its manifest names; an unread manifest, a plain install and no index keep the path.
    #[test]
    fn a_store_slot_is_spelled_as_its_manifest_names_it() {
        let slot = "/p/node_modules/.store/vitest-virtual-0a1b/package";
        let path = format!("{slot}/dist/index.d.ts");
        let mut index = PackageReachIndex::default();
        assert_eq!(installed_spelling(Some(&index), &path), path.as_str());
        assert_eq!(installed_spelling(None, &path), path.as_str());
        index.names.insert(slot.to_owned(), "vitest".to_owned());
        assert_eq!(
            installed_spelling(Some(&index), &path),
            "/p/node_modules/vitest/dist/index.d.ts"
        );
        assert_eq!(installed_spelling(None, &path), path.as_str());
        let plain = "/p/node_modules/vitest/dist/index.d.ts";
        assert_eq!(installed_spelling(Some(&index), plain), plain);
        index.names.insert(
            "/p/node_modules/.store/x/package".to_owned(),
            "@vitest/runner".to_owned(),
        );
        assert_eq!(
            installed_spelling(Some(&index), "/p/node_modules/.store/x/package/dist/a.d.ts"),
            "/p/node_modules/@vitest/runner/dist/a.d.ts"
        );
    }

    // ADR 1145: a site in a file the product's entries load is judged against the realm's writers (only where the realm holds fewer than the index), every other
    // site against all of them; a clear package is no writer in either.
    #[test]
    fn a_product_file_is_judged_against_the_realms_writers_and_any_other_against_every_writer() {
        let reach = |clear: bool| {
            if clear {
                PackageReach::Clear
            } else {
                PackageReach::ReachesSolid { via: "x".into() }
            }
        };
        let mut index = PackageReachIndex::default();
        for (root, clear) in [("/n/page", false), ("/n/runner", false), ("/n/quiet", true)] {
            index.by_root.insert(root.to_owned(), reach(clear));
        }
        index.prototype_writers.insert(
            "reverse".to_owned(),
            ["/n/page", "/n/runner", "/n/quiet"]
                .map(str::to_owned)
                .into(),
        );
        index
            .prototype_writers
            .insert("fill".to_owned(), ["/n/runner".to_owned()].into());
        index.prototype_writers_any.insert("/n/runner".to_owned());
        // No realm: every site is judged against every writer.
        assert!(index.product_scope("/p/App.tsx").is_none());
        let every = index.full_scope();
        assert_eq!(
            every.prototype_writers_by_name("fill").collect::<Vec<_>>(),
            ["/n/runner"]
        );
        assert_eq!(
            every
                .prototype_writers_by_name("reverse")
                .collect::<Vec<_>>(),
            ["/n/page", "/n/runner"],
            "a clear package is no writer"
        );
        assert_eq!(
            every.prototype_writers_any_name().collect::<Vec<_>>(),
            ["/n/runner"]
        );
        assert_eq!(every.first_named_writer(), Some(("fill", "/n/runner")));
        // A realm that keeps only the page's writer.
        index.product = Some(ProductWriters {
            files: ["/p/App.tsx".to_owned()].into(),
            prototype_writers: [("reverse".to_owned(), ["/n/page".to_owned()].into())].into(),
            prototype_writers_any: BTreeSet::new(),
            narrows: true,
        });
        let product = index
            .product_scope("/p/App.tsx")
            .expect("a file the entries load");
        assert!(!product.is_empty());
        assert_eq!(
            product
                .prototype_writers_by_name("reverse")
                .collect::<Vec<_>>(),
            ["/n/page"]
        );
        assert_eq!(product.prototype_writers_by_name("fill").count(), 0);
        assert_eq!(product.prototype_writers_any_name().count(), 0);
        assert_eq!(
            product.prototype_writers_of("reverse").collect::<Vec<_>>(),
            [("/n/page", true)]
        );
        assert_eq!(product.first_named_writer(), Some(("reverse", "/n/page")));
        // A file the entries do not load keeps every writer, and so does a realm that narrows nothing.
        assert!(index.product_scope("/p/App.test.tsx").is_none());
        index.product.as_mut().unwrap().narrows = false;
        assert!(index.product_scope("/p/App.tsx").is_none());
        // A realm that holds no writer is empty.
        index.product = Some(ProductWriters {
            files: ["/p/App.tsx".to_owned()].into(),
            narrows: true,
            ..ProductWriters::default()
        });
        assert!(index.product_scope("/p/App.tsx").unwrap().is_empty());
    }

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
        // A global a package writes answers with the writer's verdict.
        let mut written = PackageReachIndex::default();
        written.by_root.insert(
            "/p/node_modules/w".to_owned(),
            PackageReach::ReachesSolid {
                via: "solid-js".to_owned(),
            },
        );
        written
            .by_root
            .insert("/p/node_modules/c".to_owned(), PackageReach::Clear);
        written
            .global_writers
            .entry("setTimeout".to_owned())
            .or_default()
            .extend([
                "/p/node_modules/c".to_owned(),
                "/p/node_modules/w".to_owned(),
            ]);
        assert_eq!(
            written.global_writers_of("setTimeout").collect::<Vec<_>>(),
            [("/p/node_modules/w", true)]
        );
        assert_eq!(written.global_writers_of("fetch").count(), 0);
        assert_eq!(
            written
                .for_declaration("global-write:setTimeout:/p/node_modules/w")
                .word(),
            "reaches-solid"
        );
        // A writer under a key nothing decides may write any global: it answers for every name it does not write by name, unless it is clear.
        written
            .global_writers_any
            .insert("/p/node_modules/w".to_owned());
        written
            .global_writers_any
            .insert("/p/node_modules/c".to_owned());
        assert_eq!(
            written.global_writers_of("fetch").collect::<Vec<_>>(),
            [("/p/node_modules/w", false)]
        );
        assert_eq!(
            written.global_writers_of("setTimeout").collect::<Vec<_>>(),
            [("/p/node_modules/w", true)]
        );
        assert_eq!(
            written
                .for_declaration("global-write-any:fetch:/p/node_modules/w")
                .word(),
            "reaches-solid"
        );
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
