//! ADR 0972: whether code a declaration describes may reach a Solid action, decided per installed package from its source by the backend
//! (`solid-facts-backend/src/package_reach.rs`) and read by the analysis. The analysis never reads the filesystem; this is its view.

use std::collections::BTreeMap;

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
        }
    }
}

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

/// The verdicts the backend decided, by installed package root (the directory under the last `node_modules` component of a declaration's path, see
/// [`package_root_of`]).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageReachIndex {
    pub by_root: BTreeMap<String, PackageReach>,
}

impl PackageReachIndex {
    /// The verdict for the code a declaration at `path` describes. A path the backend did not decide is undecided, never clear.
    #[must_use]
    pub fn for_declaration(&self, path: &str) -> PackageReach {
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
    fn an_undecided_path_is_never_clear() {
        let index = PackageReachIndex::default();
        assert!(!index.for_declaration("/p/node_modules/x/i.d.ts").is_clear());
    }
}
