//! The composition seam between dialect-independent infrastructure and one
//! Solid dialect.
//!
//! A [`Dialect`] bundles everything a Solid version contributes to the
//! checker: its vocabulary, its compiler adapter, its rule catalog, its rule
//! documentation, and its built-in runtime model, plus the stable identity
//! that keys every cache and retained session. The analysis pipeline receives
//! the selected `&Dialect` from its entry point — the CLI's `--dialect` flag,
//! the wasm request, or [`detect`] when a request names none — and never
//! names a dialect crate directly.

use std::path::{Path, PathBuf};

use solid_facts::compiler::CompilerFactsProvider;
use solid_reactive_ir::{Finding, Program, RuleMetadata, SolveTimings};

/// Rule identities this checker used to publish and has since removed, with the
/// reason, so a project's existing `.solid-checker/rule-options.json` does not
/// hard-fail on an id that no longer exists.
///
/// A rule name in that document is validated against the catalogs, and an
/// unknown name fails the whole analysis rather than silently changing policy —
/// which is right for a typo and wrong for a rule the checker itself deleted.
/// Accepting a retired id is **not** demoting the rule or hiding it behind an
/// option (AGENTS.md forbids both): the rule cannot fire, no catalog declares
/// it, and disabling it is a no-op. Only the stale key is tolerated.
///
/// Entries are permanent. Removing one turns a tolerated config back into a
/// fatal error for the same user, so this list only grows.
pub const RETIRED_RULES: &[(&str, &str)] = &[
    (
        "invalid-cleanup-return",
        "removed 2026-08-17: every illegal return is a TypeScript error against `EffectFunction`'s `(() => void) | void` return type",
    ),
    (
        "cleanup-return-unresolved",
        "removed 2026-08-17: the obligation's whole domain was the legality of a returned value, which the same type closes",
    ),
    (
        "invalid-refresh-target",
        "removed 2026-08-17: `Refreshable<T>` brands the target in the type system, so every invalid target is a TypeScript error",
    ),
    (
        "invalid-affects-target",
        "removed 2026-08-17: same, against `Accessor<unknown> | Store<object>`",
    ),
    (
        "affects-keys-on-accessor",
        "removed 2026-08-17: a key on an accessor target selects the one-argument overload, so the key is a TypeScript error",
    ),
    (
        "refresh-target-unresolved",
        "removed 2026-08-17: asked whether the target carries the source brand, which is a question the type answers",
    ),
    ("affects-target-unresolved", "removed 2026-08-17: same"),
    (
        "v1/imports",
        "removed 2026-08-17: its one condition — the named module does not export the name — is exactly TS2305's",
    ),
    (
        "v1/untracked-derived-function",
        "removed 2026-08-20: SC1001 follows helper-call chains and owns the same untracked reactive-read failure",
    ),
    (
        "untracked-derived-function",
        "removed 2026-08-20: SC1001 follows helper-call chains and owns the same runtime STRICT_READ_UNTRACKED failure",
    ),
    (
        "v1/cleanup-in-forbidden-scope",
        "removed 2026-08-20: Solid 1.x createReaction callbacks run under the reaction's own disposing computation",
    ),
    (
        "v1/primitive-in-leaf-owner",
        "removed 2026-08-20: Solid 1.x createReaction owns and disposes primitives created by its invalidation callback",
    ),
    (
        "v1/primitive-in-directive-application",
        "removed 2026-08-20: Solid 1.x directive and ref application preserve the surrounding owner",
    ),
    (
        "v1/no-implicit-draggable",
        "removed 2026-08-20: its inverted shorthand check was generic HTML attribute-state validation, outside the checker domain",
    ),
    (
        "no-implicit-draggable",
        "removed 2026-08-20: the remaining claim was generic HTML draggable-state validation, outside the checker domain",
    ),
    (
        "v1/no-array-handlers",
        "removed 2026-08-20: Solid 1.x intentionally supports [handler, data] pairs, and the available facts cannot prove that a matched pair is defective",
    ),
    (
        "v1/no-react-deps",
        "removed 2026-08-20: Solid 1.x intentionally accepts an array seed and passes it to the reactive callback",
    ),
    (
        "v1/event-handlers",
        "removed 2026-08-20: its surviving arms enforced spelling and intent conventions rather than proven runtime defects",
    ),
    (
        "v1/no-react-specific-props",
        "removed 2026-08-20: intrinsic uses are TypeScript errors and component props are passed through without React-specific lowering",
    ),
    (
        "v1/no-unknown-namespaces",
        "removed 2026-08-20: namespaced component props are delivered verbatim and intrinsic invalid names are TypeScript-owned",
    ),
    (
        "v1/no-innerhtml",
        "removed 2026-08-20: its component and injection-policy arms were unproven; content competition remains SC8003",
    ),
    (
        "v1/style-prop",
        "removed 2026-08-20: its component arm was false and its intrinsic residue was CSS style policy or TypeScript-owned",
    ),
    (
        "v1/no-async-tracked-scope",
        "removed 2026-08-20: an async tracked callback is not itself defective; SC1002 reports only proven reactive reads after await",
    ),
    (
        "v1/jsx-no-script-url",
        "removed 2026-08-20: generic injection-sink policy is outside the checker domain",
    ),
    (
        "v1/jsx-uses-vars",
        "removed 2026-08-20: it never emitted a diagnostic because semantic reference facts already model JSX uses",
    ),
    (
        "v1/no-proxy-apis",
        "removed 2026-08-20: runtime target compatibility is project policy and cannot be proven from source",
    ),
    (
        "v1/self-closing-comp",
        "removed 2026-08-20: self-closing syntax is formatting, not a runtime defect",
    ),
    (
        "v1/prefer-component-syntax",
        "removed 2026-08-20: imperative calls of JSX-returning functions are runtime-valid and the rule enforced a naming convention",
    ),
    (
        "prefer-component-syntax",
        "removed 2026-08-20: imperative calls of JSX-returning functions are runtime-valid and the rule enforced a naming convention",
    ),
    (
        "v1/execution-map-incomplete",
        "removed 2026-08-20: compiler-fact completeness is a producer-integrity invariant, not a project diagnostic",
    ),
    (
        "execution-map-incomplete",
        "removed 2026-08-20: compiler-fact completeness is a producer-integrity invariant, not a project diagnostic",
    ),
    (
        "v1/valid-jsx-nesting",
        "removed 2026-08-20: generic HTML parser conformance is outside the Solid semantic checker domain",
    ),
    (
        "valid-jsx-nesting",
        "removed 2026-08-20: generic HTML parser conformance is outside the Solid semantic checker domain",
    ),
    (
        "cleanup-in-forbidden-scope",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "primitive-in-leaf-owner",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "flush-in-forbidden-scope",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "pending-async-untracked-read",
        "merged 2026-08-20 into pending-async-unsuspendable-read; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "pending-async-forbidden-scope",
        "merged 2026-08-20 into pending-async-unsuspendable-read; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "ssr-client-source-outside-loading-boundary",
        "merged 2026-08-20 into async-outside-loading-boundary; existing disables intentionally do not transfer to the wider rule",
    ),
];

/// Former external rule identities that canonicalize onto a current rule.
///
/// Unlike [`RETIRED_RULES`], an alias transfers configuration: disabling its
/// old name disables the current target. Each entry landed atomically with
/// the identity change that created its target.
pub const RULE_ALIASES: &[(&str, &str)] = &[
    ("v1/no-owner-effect", "v1/missing-owner"),
    ("v1/no-owner-cleanup", "v1/missing-owner"),
    ("v1/no-owner-boundary", "v1/missing-owner"),
    ("no-owner-effect", "missing-owner"),
    ("no-owner-cleanup", "missing-owner"),
    ("no-owner-boundary", "missing-owner"),
    ("no-owner-settled-cleanup", "missing-owner"),
    (
        "v1/package-contract-export-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "v1/package-contract-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "v1/package-contract-callback-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "package-contract-export-missing",
        "package-contract-incomplete",
    ),
    ("package-contract-missing", "package-contract-incomplete"),
    (
        "package-contract-callback-missing",
        "package-contract-incomplete",
    ),
    ("component-props-destructure", "no-destructure"),
    ("component-returns-conditionally", "components-return-once"),
    (
        "expected-function-got-expression",
        "reactive-handler-frozen",
    ),
    (
        "v1/expected-function-got-expression",
        "v1/reactive-handler-frozen",
    ),
    ("resolve-in-reactive-scope", "resolve-in-tracked-scope"),
    (
        "sync-node-received-async",
        "sync-computation-received-async",
    ),
];

/// The removal note for a retired rule identity, or `None` if the checker never
/// published that name.
#[must_use]
pub fn retired_rule(name: &str) -> Option<&'static str> {
    RETIRED_RULES
        .iter()
        .find(|(retired, _)| *retired == name)
        .map(|(_, reason)| *reason)
}

/// The current catalog identity for a former external name.
#[must_use]
pub fn rule_alias(name: &str) -> Option<&'static str> {
    RULE_ALIASES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, current)| *current)
}

/// Semantic evidence a dialect's catalog needs Type Facts to acquire.
///
/// These are analysis capabilities, not external rule identities. Renaming a
/// rule therefore cannot silently change the fact plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticDemandCapabilities {
    pub array_map_receiver_types: bool,
    pub async_array_map_callbacks: bool,
    pub server_argument_library_types: bool,
}

impl SemanticDemandCapabilities {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
        async_array_map_callbacks: false,
        server_argument_library_types: false,
    };
    /// Only the 2.0 catalog carries `server-function-rich-argument`, so only it
    /// pays for the library-type identities that rule reads.
    const SOLID_2: Self = Self {
        array_map_receiver_types: true,
        async_array_map_callbacks: true,
        server_argument_library_types: true,
    };
}

pub struct Dialect {
    /// Stable identity, folded into every cache key and retained session
    /// identity so artifacts from different dialects can never collide.
    pub id: &'static str,
    /// Exact compiler semantic producer identity. This is separate from the
    /// dialect because a producer pin or trace revision can change answers
    /// while the language dialect remains the same.
    pub compiler_facts_identity: &'static str,
    /// The Solid-version vocabulary the reactive IR analyzes with: which
    /// names are primitives, where their callbacks sit, which JSX tags open
    /// boundaries. The engine asks this table; it never names a version.
    pub vocabulary: &'static dyn solid_dialect::Dialect,
    /// Size of the rule catalog; reporting only.
    pub rule_count: usize,
    /// Constructs the dialect's in-process compiler-facts provider.
    pub compiler: fn() -> Box<dyn CompilerFactsProvider>,
    /// Runs the dialect's rule catalog over a program.
    pub solve_measured: fn(&Program) -> (Vec<Finding>, SolveTimings),
    /// Documentation page for a rule, addressed by its externally visible
    /// name.
    pub docs_url: fn(&str) -> String,
    /// Whether the catalog carries a rule with this externally visible name.
    /// Lets shared backend code condition work on catalog capability
    /// (for example, which type facts to demand) instead of naming a
    /// version.
    pub has_rule: fn(&str) -> bool,
    /// Catalog metadata for one exact external rule identity.
    pub rule_metadata: fn(&str) -> Option<RuleMetadata>,
    /// Typed fact-acquisition requirements of the catalog.
    pub semantic_demands: SemanticDemandCapabilities,
    /// Dialect-owned projection policy used after rule enablement filtering.
    pub catalog_capabilities: solid_reactive_ir::CatalogCapabilities,
}

impl Dialect {
    pub fn solve(&self, program: &Program) -> Vec<Finding> {
        (self.solve_measured)(program).0
    }
}

/// The stable id and nothing else. A dialect is function pointers, a `&dyn`
/// vocabulary table and a rule catalog; the id is the only part of it that
/// means anything in a diagnostic, and it is the part that keys every cache.
impl std::fmt::Debug for Dialect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Dialect").field(&self.id).finish()
    }
}

/// Every dialect the checker can run with. A new dialect registers here and
/// becomes selectable by id everywhere a dialect can be named.
pub static ALL: &[&Dialect] = &[&SOLID_V2];

/// Resolves a dialect by its stable id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    ALL.iter().copied().find(|dialect| dialect.id == id)
}

/// The dialect entry points fall back to when a request names none and
/// nothing resolves.
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    &SOLID_V2
}

/// The dialect for a Solid language version, if this build includes it.
#[must_use]
pub fn by_version(version: solid_dialect::Version) -> Option<&'static Dialect> {
    ALL.iter()
        .copied()
        .find(|dialect| dialect.vocabulary.version() == version)
}

/// The diagnostic identity of the unsupported-runtime refusal.
///
/// Held here, not read from a catalog, because the refusal is decided
/// **before** a dialect is chosen: it has to be emittable in any feature
/// configuration, including one whose default catalog does not declare it.
/// The Solid 2 catalog declares it too -- that is where adapters, suppression
/// configuration and `docs/rules/` look it up -- and
/// `the_refusal_identity_is_the_one_the_catalog_publishes` pins the two
/// together so they cannot drift. See ADR 0110.
pub const UNSUPPORTED_RUNTIME_CODE: &str = "SC9013";
/// The rule name paired with [`UNSUPPORTED_RUNTIME_CODE`].
pub const UNSUPPORTED_RUNTIME_RULE: &str = "unsupported-solid-runtime";

/// What the dialect walk found, and where it found it.
///
/// [`detect`] collapses this to a single dialect for callers that only need
/// one. The three cases are kept apart here because they are **not** the same
/// answer, and a caller that must refuse an unsupported runtime cannot tell
/// them apart from a dialect alone:
///
/// - an installed `solid-js` whose major this build carries,
/// - an installed `solid-js` whose major this build has **no** dialect for,
/// - nothing installed, or a version naming no released major.
///
/// Each case that read a manifest carries the exact path it read, because a
/// refusal has to say *which* `package.json` decided it — the walk is
/// unbounded and the deciding file is frequently not the one beside the
/// project.
#[derive(Clone, Debug)]
pub enum Detection {
    /// The nearest installed `solid-js` names a released major this build
    /// carries a dialect for.
    Installed {
        dialect: &'static Dialect,
        version: solid_dialect::Version,
        manifest: PathBuf,
    },
    /// The nearest installed `solid-js` names a released major this build has
    /// no dialect for. **Never a dialect**: there is no correct one to pick,
    /// and picking the default would analyze the project under a language it
    /// does not run. The caller refuses.
    Unsupported {
        version: solid_dialect::Version,
        /// The `version` field exactly as the manifest spelled it. The
        /// refusal quotes this rather than the classified major, because
        /// "1.9.14" tells the reader which install to go and change and
        /// "Solid 1.x" does not.
        installed: String,
        manifest: PathBuf,
    },
    /// Nothing resolved, or the nearest manifest names no released major
    /// (`workspace:*`, `0.5.0`, `3.0.0`). `manifest` is that unclassifiable
    /// manifest when the walk stopped at one, and `None` when no
    /// `node_modules/solid-js` was found at all.
    Defaulted { manifest: Option<PathBuf> },
}

/// Resolves the dialect a project speaks from the `solid-js` it would
/// actually import: the nearest `node_modules/solid-js/package.json` above
/// the project file, walked the way a bundler resolves.
///
/// Deliberately **not** read from any loaded contract — a bundled contract
/// carries the version the checker ships, not the one the project installed.
/// Falls back to the default dialect when nothing resolves (no node_modules,
/// a non-version like `workspace:*`, or a major nobody has released), which
/// is what every request without an installed solid-js got before detection
/// existed.
///
/// **This collapses [`Detection::Unsupported`] onto the default dialect, and
/// that is a hole, not a design.** It is unreachable in a build carrying every
/// released major's dialect, and it is reachable today in the
/// `--no-default-features --features dialect-v2` arm `scripts/verify.sh` runs:
/// there, a 1.x project is analyzed under the 2.0 catalog and told nothing.
/// Retiring the 1.x dialect makes that the *only* build, which is why step 4
/// of `docs/2026-09-16-retire-solid-1x-plan.md` must land with step 3 and not
/// after it. Callers that can emit a finding should read [`detect_detailed`]
/// and refuse `Unsupported` rather than calling this.
#[must_use]
pub fn detect(project: &Path) -> &'static Dialect {
    match detect_detailed(project) {
        Detection::Installed { dialect, .. } => dialect,
        Detection::Unsupported { .. } | Detection::Defaulted { .. } => default_dialect(),
    }
}

/// [`detect`] with its reasoning intact. See [`Detection`].
#[must_use]
pub fn detect_detailed(project: &Path) -> Detection {
    let Some((version, installed, manifest)) = resolved_solid_version(project) else {
        return Detection::Defaulted { manifest: None };
    };
    let Some(version) = version else {
        return Detection::Defaulted {
            manifest: Some(manifest),
        };
    };
    match by_version(version) {
        Some(dialect) => Detection::Installed {
            dialect,
            version,
            manifest,
        },
        None => Detection::Unsupported {
            version,
            installed,
            manifest,
        },
    }
}

/// The nearest installed `solid-js`, as
/// `(classification, version as written, manifest path)`.
///
/// The outer `Option` is "did the walk find a manifest carrying a version
/// string at all"; the inner one is whether that string names a released
/// major. They are separate answers and the caller needs both: a missing
/// install and an install spelled `workspace:*` both default, but only the
/// second can name the file that decided it. The raw version string rides
/// along because a refusal has to quote what it actually read.
fn resolved_solid_version(
    project: &Path,
) -> Option<(Option<solid_dialect::Version>, String, PathBuf)> {
    let start = if project.is_dir() {
        project
    } else {
        project.parent()?
    };
    for directory in start.ancestors() {
        let manifest = directory
            .join("node_modules")
            .join("solid-js")
            .join("package.json");
        let Ok(encoded) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        // A manifest that parses to no version string -- broken JSON, or no
        // "version" field -- is treated exactly like an unreadable one: the
        // walk continues, because a broken stub (a half-written install, an
        // empty placeholder) must not mask a real installation higher up.
        let Some(version) = serde_json::from_str::<serde_json::Value>(&encoded)
            .ok()
            .and_then(|manifest| Some(manifest.get("version")?.as_str()?.to_owned()))
        else {
            continue;
        };
        // A version string that names no released major ("workspace:*",
        // "0.5.0", "3.0.0") stops the walk and answers `None` -- per
        // `Version::for_solid_js`'s docs, refusing to classify is deliberate,
        // and the caller falls back to the v2 default. This is the nearest
        // `solid-js` the project would import; a resolvable-but-unclassifiable
        // install is an answer, not an absence.
        return Some((
            solid_dialect::Version::for_solid_js(&version),
            version,
            manifest,
        ));
    }
    None
}

#[cfg(feature = "dialect-v2")]
static SOLID_V2: Dialect = Dialect {
    id: "solid-v2",
    compiler_facts_identity: solid_v2_compiler::COMPILER_FACTS_IDENTITY,
    vocabulary: &solid_dialect::Solid2,
    rule_count: solid_v2_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v2_compiler::NativeCompilerFacts),
    solve_measured: solid_v2_rules::solve_measured,
    docs_url: solid_v2_rules::docs_url,
    has_rule: |name| {
        solid_v2_rules::Rule::ALL
            .into_iter()
            .any(|rule| rule.metadata().name == name)
    },
    rule_metadata: |name| {
        solid_v2_rules::Rule::ALL
            .into_iter()
            .find(|rule| rule.metadata().name == name)
            .map(solid_v2_rules::Rule::metadata)
    },
    semantic_demands: SemanticDemandCapabilities::SOLID_2,
    catalog_capabilities: solid_v2_rules::CATALOG_CAPABILITIES,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use solid_reactive_ir::{
        AsyncRead, DirectMutationTarget, ExecutionRole, OwnerRequirement,
        OwnerRequirementOperation, ReactiveRead, ReactiveWrite, ReactiveWriteOperation,
        StaticDefect, StaticDefectKind,
    };
    use typefacts::Location;

    use super::*;

    /// The package root of a module specifier, matching contract discovery:
    /// `solid-js/store` and `@solidjs/web/frames` are subpaths of one installed
    /// package, not packages of their own.
    fn package_root(module: &str) -> &str {
        if module.starts_with('@') {
            module
                .match_indices('/')
                .nth(1)
                .map_or(module, |(index, _)| &module[..index])
        } else {
            module.split('/').next().unwrap_or(module)
        }
    }

    /// Every package a dialect models is declared in its assembly manifest, and
    /// every declaration models something.
    ///
    /// The manifest drives contract generation, the drift check, runtime
    /// conformance, and the composed-artifact check -- all of which enumerate
    /// what it *declares*. A package the vocabulary owns or the backend bundles
    /// but no entry names is therefore covered by no gate at all: it silently
    /// has no contract, and every project importing it reports SC9005 forever.
    /// This check closes that hole, so it derives the expected set from the
    /// dialect itself rather than from the manifest it is checking.
    #[test]
    fn every_modeled_package_is_declared_in_the_assembly_manifest() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for dialect in ALL {
            let source = root
                .join("rust/dialects")
                .join(dialect.id)
                .join("dialect.json");
            let read = std::fs::read(&source)
                .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
            let manifest: serde_json::Value = serde_json::from_slice(&read)
                .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
            let declared = manifest["contracts"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no contracts array", source.display()))
                .iter()
                .map(|contract| {
                    contract["package"]
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("{} has a contract without a package", source.display())
                        })
                        .to_owned()
                })
                .collect::<HashSet<_>>();
            let bundle_index = root.join(
                manifest["bundleIndex"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} has no bundleIndex", source.display())),
            );
            let bundle: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&bundle_index)
                    .unwrap_or_else(|error| panic!("{}: {error}", bundle_index.display())),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", bundle_index.display()));
            let bundled = bundle["contracts"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no contracts array", bundle_index.display()))
                .iter()
                .map(|case| {
                    case["package"]
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("{} has a case without a package", bundle_index.display())
                        })
                        .to_owned()
                })
                .collect::<HashSet<_>>();
            assert!(
                bundled.is_subset(&declared),
                "{} receipt-issued bundle index names an undeclared package",
                dialect.id
            );
            let modeled = dialect
                .vocabulary
                .modules()
                .iter()
                .copied()
                .map(|module| package_root(module).to_owned())
                .collect::<HashSet<_>>();
            let mut undeclared = modeled.difference(&declared).collect::<Vec<_>>();
            undeclared.sort();
            assert!(
                undeclared.is_empty(),
                "{} models {undeclared:?} but declares no contract for them; add an entry to {} \
                 (a hand-authored bundled overlay sets \"generated\": false)",
                dialect.id,
                source.display()
            );
        }
    }

    fn location(index: u64) -> Location {
        Location {
            path: "catalog-prose.tsx".into(),
            start_byte: index,
            end_byte: index + 1,
        }
    }

    /// Materializes every shared static-defect wording branch plus the
    /// catalog findings that consumed the old dialect prose helpers: a strict
    /// read, an owned write, an ownerless cleanup (the owner arm whose hint
    /// diverges most between the versions), and a pending async read (a table
    /// only the 2.0 catalog projects). Dynamic subjects use a reserved prefix
    /// so the API-name assertion below can distinguish user code quoted by a
    /// finding from catalog-owned advice.
    fn catalog_prose_program() -> Program {
        let defect_kinds = [
            StaticDefectKind::ReactiveObjectDestructure {
                source: "props".into(),
                component_props: true,
            },
            StaticDefectKind::ReactiveReadAfterAwait {
                accessor: "sampleAccessor".into(),
            },
            StaticDefectKind::ComponentReturnsConditionally,
            StaticDefectKind::PackageContractExportMissing {
                module: "sample-package".into(),
                export: "sampleExport".into(),
                reexported: false,
                site: solid_reactive_ir::ContractDefectSite::Import,
            },
            StaticDefectKind::MissingEffectFunction,
            StaticDefectKind::ReactiveSourceUncaptured {
                source: "sampleAccessor".into(),
                callee: "sampleCallee".into(),
            },
            StaticDefectKind::ReactiveHandlerRead {
                attribute: "onClick".into(),
                expression: "sampleHandler".into(),
            },
            StaticDefectKind::UncalledAccessor {
                name: "sampleAccessor".into(),
                position: "sample expression".into(),
            },
            StaticDefectKind::DirectMutation {
                name: "sampleAccessor".into(),
                target: DirectMutationTarget::AccessorBinding,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleStore".into(),
                target: DirectMutationTarget::Store,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleProps".into(),
                target: DirectMutationTarget::Props,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleValue".into(),
                target: DirectMutationTarget::ReactiveValue,
            },
        ];
        Program {
            reads: vec![ReactiveRead {
                kind: "signal".into(),
                accessor: "sampleAccessor".into(),
                location: location(1),
                declaration: location(2),
                execution: ExecutionRole::UntrackedRendering,
                context: "sample component".into(),
                via: "".into(),
                origin: None,
                origin_context: "".into(),
                uncertain: false,
                missing_jsx_census: false,
            }],
            writes: vec![ReactiveWrite {
                setter: "sampleSetter".into(),
                operation: ReactiveWriteOperation::Setter,
                source_kind: solid_reactive_ir::ReactiveSourceKind::Accessor,
                location: location(3),
                declaration: location(4),
                execution: ExecutionRole::TrackedJsx,
                allowed_by_option: false,
                context: "sample computation".into(),
            }],
            missing_owners: vec![OwnerRequirement {
                operation: OwnerRequirementOperation::Cleanup,
                location: location(5),
                uncertain: false,
                runtime_uncertain: false,
                caller_uncertain: false,
                conditional_owner: false,
                component_uncertain: false,
                missing_jsx_census: false,
                report: true,
            }],
            async_reads: vec![AsyncRead {
                accessor: "sampleAsyncAccessor".into(),
                location: location(6),
                declaration: location(7),
                execution: ExecutionRole::TrackedJsx,
                leaf_owner: None,
                under_loading: false,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
            }],
            static_defects: defect_kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| StaticDefect {
                    kind,
                    location: location(index as u64 + 10),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                })
                .collect(),
            ..Program::default()
        }
    }

    fn called_names(text: &str) -> impl Iterator<Item = String> + '_ {
        text.match_indices('(').filter_map(|(index, _)| {
            let name = text[..index]
                .chars()
                .rev()
                .take_while(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '$')
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            (!name.is_empty()).then_some(name)
        })
    }

    fn contains_identifier(text: &str, expected: &str) -> bool {
        text.split(|character: char| {
            !(character.is_alphanumeric() || matches!(character, '_' | '$'))
        })
        .any(|identifier| identifier == expected)
    }

    #[test]
    fn dialect_ids_are_unique_and_resolvable() {
        for dialect in ALL {
            assert_eq!(by_id(dialect.id).map(|found| found.id), Some(dialect.id));
        }
        assert!(by_id("solid-v3").is_none());
    }

    #[test]
    fn compatibility_registry_counts_match_the_catalog_migration() {
        assert_eq!(
            RULE_ALIASES.len(),
            19,
            "the migration note must list every transferred configuration key"
        );
        assert_eq!(
            RETIRED_RULES.len(),
            39,
            "eight pre-existing TypeScript redundancies plus 31 catalog-reduction identities"
        );
    }

    #[test]
    fn every_catalog_identity_resolves_to_its_metadata() {
        for dialect in ALL {
            if dialect.id == "solid-v2" {
                for rule in solid_v2_rules::Rule::ALL {
                    assert_eq!(
                        (dialect.rule_metadata)(rule.metadata().name),
                        Some(rule.metadata())
                    );
                }
            }
        }
    }

    /// Catalogs own user-facing wording, but the generated export index still
    /// owns the fact of which APIs exist. Exercise the real findings so moving
    /// prose out of [`solid_dialect::Dialect`] cannot also remove the guard
    /// that stopped 2.0 advice leaking into 1.x diagnostics (and vice versa).
    #[test]
    fn rule_catalog_prose_only_names_apis_exported_by_its_dialect() {
        const NON_SOLID_CALLS: &[&str] =
            &["dispose", "log", "queueMicrotask", "setStore", "setTimeout"];
        let program = catalog_prose_program();
        for (version, forbidden) in [
            (
                solid_dialect::Version::V1,
                &["action", "actions", "onSettled", "ownedWrite"][..],
            ),
            (
                solid_dialect::Version::V2,
                &[
                    "onMount",
                    "mergeProps",
                    "splitProps",
                    "produce",
                    "Suspense",
                    "SuspenseList",
                ][..],
            ),
        ] {
            // A single-dialect feature build simply has nothing to check for
            // the absent version.
            let Some(dialect) = by_version(version) else {
                continue;
            };
            let findings = dialect.solve(&program);
            // Beyond the defects: the strict read, the owned write, and the
            // ownerless cleanup. The pending async read joins them only in
            // 2.0 — the 1.x catalog deliberately never reads that table.
            let catalog_findings = match version {
                solid_dialect::Version::V1 => 3,
                solid_dialect::Version::V2 => 4,
            };
            assert_eq!(
                findings.len(),
                program.static_defects.len() + catalog_findings
            );

            let mut checked_calls = 0;
            let mut prose = String::new();
            for finding in &findings {
                for (field, text) in std::iter::once(("message", finding.message.as_str()))
                    .chain(std::iter::once(("hint", finding.hint.as_str())))
                    .chain(
                        finding
                            .evidence
                            .iter()
                            .map(|step| ("evidence", step.message.as_str())),
                    )
                {
                    prose.push_str(text);
                    prose.push('\n');
                    for name in called_names(text) {
                        if name.starts_with("sample") || NON_SOLID_CALLS.contains(&name.as_str()) {
                            continue;
                        }
                        assert!(
                            !dialect
                                .vocabulary
                                .export_modules(&name, solid_dialect::ExportPosition::Value)
                                .is_empty(),
                            "{version:?} catalog {field} names {name}(), which that dialect does not export: {text:?}"
                        );
                        checked_calls += 1;
                    }
                }
            }
            assert!(
                checked_calls >= 5,
                "{version:?}: only {checked_calls} API calls checked"
            );
            for name in forbidden {
                assert!(
                    !contains_identifier(&prose, name),
                    "{version:?} catalog names the other dialect's {name}: {prose}"
                );
            }

            match version {
                solid_dialect::Version::V1 => {
                    assert!(prose.contains("createEffect(fn, value?)"));
                    assert!(prose.contains("splitProps(props"));
                    assert!(prose.contains("mergeProps(defaults"));
                    assert_eq!(
                        dialect
                            .vocabulary
                            .callback_positions(solid_dialect::Primitive::CreateEffect),
                        &[0]
                    );
                }
                solid_dialect::Version::V2 => {
                    assert!(prose.contains("createEffect(compute, apply)"));
                    assert!(prose.contains("omit(props"));
                    assert!(prose.contains("merge(defaults"));
                    assert_eq!(
                        dialect
                            .vocabulary
                            .callback_positions(solid_dialect::Primitive::CreateEffect),
                        &[1]
                    );
                }
            }
        }
    }

    #[test]
    fn detection_reads_the_resolved_solid_js_version() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-detect-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();
        // The classification, not the dialect id: a 1.x install resolves to
        // `Version::V1` in every build, and only a build carrying the 1.x
        // dialect can turn that into one.
        assert!(matches!(
            detect_detailed(&project),
            Detection::Installed {
                version: solid_dialect::Version::V1,
                ..
            } | Detection::Unsupported {
                version: solid_dialect::Version::V1,
                ..
            }
        ));

        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert!(matches!(
            detect_detailed(&project),
            Detection::Installed {
                version: solid_dialect::Version::V2,
                ..
            } | Detection::Unsupported {
                version: solid_dialect::Version::V2,
                ..
            }
        ));

        // No resolvable version answers the default rather than guessing.
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"workspace:*"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, default_dialect().id);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_broken_nearer_manifest_does_not_mask_an_installation_higher_up() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-broken-stub-{}",
            std::process::id()
        ));
        let outer = root.join("node_modules/solid-js");
        let inner = root.join("workspace/node_modules/solid-js");
        std::fs::create_dir_all(&outer).unwrap();
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::create_dir_all(root.join("workspace/src")).unwrap();
        std::fs::write(
            outer.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("workspace/src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        // Unparseable JSON and a version-less manifest are both the walk
        // continuing, exactly like an unreadable file.
        //
        // Asserted as *which manifest the walk selected*, which is the claim.
        // Reading it off a dialect id instead made this test depend on the 1.x
        // dialect being compiled in, and it failed in the
        // `--features dialect-v2` arm for a reason that has nothing to do with
        // the walk.
        let selected = |detection: Detection| match detection {
            Detection::Installed { manifest, .. } | Detection::Unsupported { manifest, .. } => {
                manifest
            }
            Detection::Defaulted { manifest } => {
                panic!("the outer 1.x install is a resolution, not a default: {manifest:?}")
            }
        };
        std::fs::write(inner.join("package.json"), "{ not json").unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            outer.join("package.json")
        );
        std::fs::write(inner.join("package.json"), r#"{"name":"solid-js"}"#).unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            outer.join("package.json")
        );

        // A parseable version that classifies stops the walk at the nearest
        // manifest, masking the outer 1.x -- resolution order, not breakage.
        std::fs::write(
            inner.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            inner.join("package.json")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The three outcomes `detect` collapses, kept apart -- and the manifest
    /// path a refusal has to name. The walk is unbounded, so the deciding
    /// `package.json` is frequently not the one beside the project, and a
    /// refusal that cannot say which file decided it is not actionable.
    #[test]
    fn detection_separates_an_install_from_a_default_and_names_the_manifest() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-detection-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let manifest = package.join("package.json");
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        std::fs::write(&manifest, r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#).unwrap();
        match detect_detailed(&project) {
            Detection::Installed {
                dialect,
                version,
                manifest: read,
            } => {
                assert_eq!(dialect.id, "solid-v2");
                assert_eq!(version, solid_dialect::Version::V2);
                // Named from two directories up, not from beside the project.
                assert_eq!(read, manifest);
            }
            // A build without the 2.0 dialect still resolves the install and
            // still names the file; only the dialect is missing.
            Detection::Unsupported {
                version: solid_dialect::Version::V2,
                installed,
                manifest: read,
            } => {
                assert_eq!(read, manifest);
                assert_eq!(installed, "2.0.0-rc.0");
            }
            other => panic!("an installed 2.0 is a resolution: {other:?}"),
        }

        // Resolvable but unclassifiable: a default that can still say which
        // file it read, which is what separates it from nothing installed.
        std::fs::write(&manifest, r#"{"name":"solid-js","version":"workspace:*"}"#).unwrap();
        match detect_detailed(&project) {
            Detection::Defaulted {
                manifest: Some(read),
            } => assert_eq!(read, manifest),
            other => panic!("an unclassifiable version defaults, and names its file: {other:?}"),
        }

        // Nothing installed anywhere above: a default with no file to name.
        std::fs::remove_dir_all(root.join("node_modules")).unwrap();
        assert!(matches!(
            detect_detailed(&project),
            Detection::Defaulted { manifest: None }
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The refusal's identity is held in two places on purpose; this is what
    /// stops them drifting.
    ///
    /// [`UNSUPPORTED_RUNTIME_CODE`] is what the emission actually writes, and
    /// it cannot read a catalog because the refusal happens before a dialect
    /// is chosen. The Solid 2 catalog is where every *consumer* resolves the
    /// identity -- the npm rules manifest, suppression configuration, the
    /// docs URL -- so a disagreement between them would publish a code no
    /// adapter recognizes while the tool emitted it anyway.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn the_refusal_identity_is_the_one_the_catalog_publishes() {
        let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
        let metadata = (dialect.rule_metadata)(UNSUPPORTED_RUNTIME_RULE)
            .unwrap_or_else(|| panic!("the 2.0 catalog must declare {UNSUPPORTED_RUNTIME_RULE}"));
        assert_eq!(metadata.code, UNSUPPORTED_RUNTIME_CODE);
        assert_eq!(metadata.name, UNSUPPORTED_RUNTIME_RULE);
        assert!(
            metadata.uncertifiable,
            "the refusal asserts nothing about the project's source, so it is an \
             uncertifiable result and not a violation"
        );
        assert_eq!(metadata.severity, "error");
    }

    /// The refusal snapshot is the *whole* result, and its shape is the claim.
    #[test]
    fn the_refusal_snapshot_carries_one_finding_and_measures_nothing() {
        let snapshot = crate::diagnostics::unsupported_runtime_snapshot(
            "1.9.14",
            Path::new("/tmp/app/node_modules/solid-js/package.json"),
        );
        assert_eq!(snapshot.status, "uncertifiable");
        assert_eq!(
            snapshot.findings.len(),
            1,
            "a second finding would assert something about source that was \
             never analyzed under the language it runs"
        );
        let finding = &snapshot.findings[0];
        assert_eq!(finding.id, UNSUPPORTED_RUNTIME_CODE);
        assert_eq!(finding.rule, UNSUPPORTED_RUNTIME_RULE);
        assert_eq!(finding.kind, "uncertifiable");
        assert!(
            finding.message.contains("1.9.14"),
            "the refusal quotes the version it read, not the classified major: {}",
            finding.message
        );
        assert_eq!(
            finding.primary_location.path, "/tmp/app/node_modules/solid-js/package.json",
            "the deciding manifest is the location, because it is the file to change"
        );
        assert_eq!(
            snapshot.metrics.files_analyzed, 0,
            "nothing was read; reporting otherwise would overstate what happened"
        );
        assert!(snapshot.package_summaries.is_empty());
    }

    /// The step-4 hole, asserted rather than described.
    ///
    /// A build carrying every released major's dialect can never answer
    /// `Unsupported`, so this asserts the *reachable* half: with the 1.x
    /// dialect compiled in, a 1.x install is `Installed`. The
    /// `--no-default-features --features dialect-v2` arm compiles the other
    /// half, where the same tree answers `Unsupported` and `detect` collapses
    /// it onto the 2.0 default -- a 1.x project analyzed under the 2.0 catalog
    /// and told nothing. Retiring the 1.x dialect makes that the only build,
    /// which is why the refusal has to land in the same slice as the deletion.
    #[test]
    fn a_one_x_install_is_supported_exactly_while_its_dialect_is_compiled_in() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-unsupported-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        let detection = detect_detailed(&project);
        {
            assert!(
                matches!(
                    &detection,
                    Detection::Unsupported {
                        version: solid_dialect::Version::V1,
                        ..
                    }
                ),
                "without the 1.x dialect, a 1.x install is unsupported, not a 2.0 project: {detection:?}"
            );
            // And this is the hole: `detect` answers the 2.0 default anyway.
            assert_eq!(detect(&project).id, "solid-v2");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
