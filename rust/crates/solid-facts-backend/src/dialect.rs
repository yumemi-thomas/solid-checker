//! The composition seam between dialect-independent infrastructure and one
//! Solid dialect.
//!
//! A [`Dialect`] bundles everything a Solid version contributes to the
//! checker: its vocabulary, its compiler adapter, its rule catalog, its rule
//! documentation, and its bundled package contracts, plus the stable identity
//! that keys every cache and retained session. The analysis pipeline receives
//! the selected `&Dialect` from its entry point — the CLI's `--dialect` flag,
//! the wasm request, or [`detect`] when a request names none — and never
//! names a dialect crate directly.

use std::path::Path;

use solid_facts::compiler::CompilerFactsProvider;
use solid_reactive_ir::{Finding, PackageContract, PackageContractIssue, Program, SolveTimings};

use crate::BackendError;

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
/// old name disables the current target. Entries land atomically with the
/// identity change that creates the target, so this table begins empty.
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
    pub server_argument_library_types: bool,
}

impl SemanticDemandCapabilities {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
        server_argument_library_types: false,
    };
    /// Only the 2.0 catalog carries `server-function-rich-argument`, so only it
    /// pays for the library-type identities that rule reads.
    #[cfg(feature = "dialect-v2")]
    const SOLID_2: Self = Self {
        array_map_receiver_types: false,
        server_argument_library_types: true,
    };
    #[cfg(feature = "dialect-v1")]
    const SOLID_1: Self = Self {
        array_map_receiver_types: true,
        server_argument_library_types: false,
    };
}

pub struct Dialect {
    /// Stable identity, folded into every cache key and retained session
    /// identity so artifacts from different dialects can never collide.
    pub id: &'static str,
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
    /// Typed fact-acquisition requirements of the catalog.
    pub semantic_demands: SemanticDemandCapabilities,
    /// Projects a package-contract issue through this dialect's catalog so
    /// SC9005 identity and every sentence remain catalog-owned.
    pub package_contract_finding: fn(&PackageContractIssue) -> Finding,
    /// Package roots the dialect ships a bundled contract for; answers the
    /// cheap membership question without decoding any contract.
    pub bundled_packages: &'static [&'static str],
    /// The contract the dialect bundles for a package root, if any.
    pub bundled_contract: fn(&str) -> Result<Option<PackageContract>, BackendError>,
}

impl Dialect {
    pub fn solve(&self, program: &Program) -> Vec<Finding> {
        (self.solve_measured)(program).0
    }
}

/// Every dialect the checker can run with. A new dialect registers here and
/// becomes selectable by id everywhere a dialect can be named.
pub static ALL: &[&Dialect] = &[
    #[cfg(feature = "dialect-v2")]
    &SOLID_V2,
    #[cfg(feature = "dialect-v1")]
    &SOLID_V1,
];

/// Resolves a dialect by its stable id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    ALL.iter().copied().find(|dialect| dialect.id == id)
}

/// The dialect entry points fall back to when a request names none and
/// nothing resolves.
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    #[cfg(feature = "dialect-v2")]
    {
        &SOLID_V2
    }
    #[cfg(all(not(feature = "dialect-v2"), feature = "dialect-v1"))]
    {
        &SOLID_V1
    }
}

/// The dialect for a Solid language version, if this build includes it.
#[must_use]
pub fn by_version(version: solid_dialect::Version) -> Option<&'static Dialect> {
    ALL.iter()
        .copied()
        .find(|dialect| dialect.vocabulary.version() == version)
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
#[must_use]
pub fn detect(project: &Path) -> &'static Dialect {
    resolved_solid_version(project)
        .and_then(by_version)
        .unwrap_or_else(default_dialect)
}

fn resolved_solid_version(project: &Path) -> Option<solid_dialect::Version> {
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
        return solid_dialect::Version::for_solid_js(&version);
    }
    None
}

#[cfg(feature = "dialect-v2")]
static SOLID_V2: Dialect = Dialect {
    id: "solid-v2",
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
    semantic_demands: SemanticDemandCapabilities::SOLID_2,
    package_contract_finding: solid_v2_rules::package_contract_finding,
    bundled_packages: &["solid-js", "@solidjs/web"],
    bundled_contract: crate::diagnostics::bundled_contract_v2,
};

#[cfg(feature = "dialect-v1")]
static SOLID_V1: Dialect = Dialect {
    id: "solid-v1",
    vocabulary: &solid_dialect::Solid1x,
    rule_count: solid_v1_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v1_compiler::NativeCompilerFacts),
    solve_measured: solid_v1_rules::solve_measured,
    docs_url: solid_v1_rules::docs_url,
    has_rule: |name| {
        solid_v1_rules::Rule::ALL
            .into_iter()
            .any(|rule| rule.metadata().name == name)
    },
    semantic_demands: SemanticDemandCapabilities::SOLID_1,
    package_contract_finding: solid_v1_rules::package_contract_finding,
    bundled_packages: &["solid-js", "@solid-primitives/scheduled"],
    bundled_contract: crate::diagnostics::bundled_contract_v1,
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
            // Both halves of "models": the vocabulary owns the module's
            // exports, or the backend compiles a contract in for it. Either is
            // a claim about the package that the manifest has to carry.
            let modeled = dialect
                .vocabulary
                .modules()
                .iter()
                .copied()
                .chain(dialect.bundled_packages.iter().copied())
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
            let mut unmodeled = declared.difference(&modeled).collect::<Vec<_>>();
            unmodeled.sort();
            assert!(
                unmodeled.is_empty(),
                "{} declares a contract for {unmodeled:?}, which its vocabulary does not own and \
                 the backend does not bundle; the entry in {} is dead weight",
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

    /// The documentation and suppression model both depend on this exact
    /// ownership split. Keep it derived from the two catalogs rather than
    /// maintaining an unaudited second list in prose. The one test that must
    /// see both catalogs at once; every other test asks the registry, so
    /// single-dialect feature builds still compile the suite.
    #[cfg(all(feature = "dialect-v1", feature = "dialect-v2"))]
    #[test]
    fn rule_catalogs_keep_the_shared_and_version_only_split() {
        let v1 = solid_v1_rules::Rule::ALL
            .into_iter()
            .map(|rule| rule.metadata().code)
            .collect::<HashSet<_>>();
        let v2 = solid_v2_rules::Rule::ALL
            .into_iter()
            .map(|rule| rule.metadata().code)
            .collect::<HashSet<_>>();
        let shared = v1.intersection(&v2).copied().collect::<HashSet<_>>();
        let expected = HashSet::from([
            "SC1001", "SC1002", "SC1003", "SC1004", "SC1005", "SC1007", "SC2001", "SC2003",
            "SC4001", "SC7001", "SC9005", "SC9011", "SC9012",
        ]);
        assert_eq!(shared, expected);
        assert_eq!(
            solid_v1_rules::Rule::ALL
                .into_iter()
                .filter(|rule| shared.contains(rule.metadata().code))
                .count(),
            13
        );
        assert_eq!(
            solid_v2_rules::Rule::ALL
                .into_iter()
                .filter(|rule| shared.contains(rule.metadata().code))
                .count(),
            13
        );
        assert_eq!(
            solid_v1_rules::Rule::ALL.len() - 13,
            5,
            "the 1.x catalog size moved; update the counts in docs/rules/README.md and rust/ARCHITECTURE.md alongside this test"
        );
        assert_eq!(
            solid_v2_rules::Rule::ALL.len() - 13,
            10,
            "the 2.0 catalog size moved; update the counts in docs/rules/README.md and rust/ARCHITECTURE.md alongside this test"
        );
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
        assert_eq!(detect(&project).id, "solid-v1");

        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, "solid-v2");

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
        std::fs::write(inner.join("package.json"), "{ not json").unwrap();
        assert_eq!(detect(&project).id, "solid-v1");
        std::fs::write(inner.join("package.json"), r#"{"name":"solid-js"}"#).unwrap();
        assert_eq!(detect(&project).id, "solid-v1");

        // A parseable version that classifies stops the walk at the nearest
        // manifest, masking the outer 1.x -- resolution order, not breakage.
        std::fs::write(
            inner.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, "solid-v2");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
