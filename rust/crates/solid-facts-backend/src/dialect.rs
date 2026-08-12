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
    package_contract_finding: solid_v1_rules::package_contract_finding,
    bundled_packages: &["solid-js"],
    bundled_contract: crate::diagnostics::bundled_contract_v1,
};

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "dialect-v1", feature = "dialect-v2"))]
    use std::collections::HashSet;

    use solid_reactive_ir::{
        AsyncRead, DirectMutationTarget, ExecutionRole, OwnerRequirement,
        OwnerRequirementOperation, ReactiveRead, ReactiveWrite, ReactiveWriteOperation,
        StaticDefect, StaticDefectKind,
    };
    use typefacts::Location;

    use super::*;

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
            StaticDefectKind::ExecutionMapIncomplete,
            StaticDefectKind::ComponentPropsDestructure,
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
            StaticDefectKind::UntrackedDerivedFunction {
                name: "sampleFunction".into(),
            },
            StaticDefectKind::ReactiveSourceUncaptured {
                source: "sampleAccessor".into(),
                callee: "sampleCallee".into(),
            },
            StaticDefectKind::ReactiveHandlerRead {
                attribute: "onClick".into(),
                expression: "sampleHandler".into(),
            },
            StaticDefectKind::HandlerCallResult {
                attribute: "onClick".into(),
                callee: "sampleHandler".into(),
                call: "sampleHandler()".into(),
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
                conditional_owner: false,
                report: true,
            }],
            async_reads: vec![AsyncRead {
                accessor: "sampleAsyncAccessor".into(),
                location: location(6),
                declaration: location(7),
                execution: ExecutionRole::TrackedJsx,
                leaf_owner: None,
                under_loading: false,
            }],
            static_defects: defect_kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| StaticDefect {
                    kind,
                    location: location(index as u64 + 10),
                    analysis_context: String::new(),
                    fixes: vec![],
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
            "SC1001", "SC1002", "SC1003", "SC1004", "SC1005", "SC1006", "SC1007", "SC2001",
            "SC2003", "SC3001", "SC3002", "SC4001", "SC4002", "SC4003", "SC6001", "SC7001",
            "SC9001", "SC9004", "SC9005", "SC9011",
        ]);
        assert_eq!(shared, expected);
        assert_eq!(
            solid_v1_rules::Rule::ALL
                .into_iter()
                .filter(|rule| shared.contains(rule.metadata().code))
                .count(),
            20
        );
        assert_eq!(
            solid_v2_rules::Rule::ALL
                .into_iter()
                .filter(|rule| shared.contains(rule.metadata().code))
                .count(),
            20
        );
        assert_eq!(
            solid_v1_rules::Rule::ALL.len() - 20,
            18,
            "the 1.x catalog size moved; update the counts in docs/rules/README.md and rust/ARCHITECTURE.md alongside this test"
        );
        assert_eq!(
            solid_v2_rules::Rule::ALL.len() - 20,
            14,
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
            r#"{"name":"solid-js","version":"2.0.0-beta.31"}"#,
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
            r#"{"name":"solid-js","version":"2.0.0-beta.31"}"#,
        )
        .unwrap();
        assert_eq!(detect(&project).id, "solid-v2");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
