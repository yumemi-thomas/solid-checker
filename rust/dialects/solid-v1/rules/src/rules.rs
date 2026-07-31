//! Stable diagnostic identities for the Solid 1.x dialect. Analysis decides
//! whether a rule applies; this catalog owns its externally visible code,
//! name, and severity.
//!
//! # Naming
//!
//! Every external name carries the `v1/` namespace — `v1/no-destructure`,
//! `v1/strict-read-untracked` — so a project can configure the two dialects'
//! rules side by side and a finding names the dialect that produced it. The
//! part after the namespace follows eslint-plugin-solid 0.14.5 where the rule
//! reproduces one of its identities (`no-destructure`,
//! `components-return-once`) and the checker's own vocabulary everywhere else.
//!
//! # Codes
//!
//! A rule that shares a concept with the Solid 2.0 dialect keeps its `SC`
//! code — `SC1001` is strict-read-untracked in both — so a suppression
//! comment stays portable across a 1.x → 2.0 migration. Codes are labels, not
//! identities: the variant is the identity.

use solid_reactive_ir::RuleMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    StrictReadUntracked,
    ReactiveReadAfterAwait,
    NoDestructure,
    ComponentsReturnOnce,
    ReactiveWriteInOwnedScope,
    CleanupInForbiddenScope,
    PrimitiveInLeafOwner,
    NoOwnerEffect,
    NoOwnerCleanup,
    NoOwnerBoundary,
    PrimitiveInDirectiveApplication,
    MissingEffectFunction,
    PackageContractExportMissing,
    PackageContractMissing,
    ExecutionMapIncomplete,
}

/// Base URL of the per-rule documentation pages in `docs/rules/v1/`.
pub const DOCS_BASE_URL: &str =
    "https://github.com/yumemi-thomas/solid-checker/blob/main/docs/rules";

/// The documentation page for a diagnostic, addressed by its externally
/// visible rule name so adapters that only carry the name (snapshots, the
/// ESLint plugin) can link without a catalog lookup. The `v1/` namespace in
/// the rule name is the `v1/` directory in the docs tree.
#[must_use]
pub fn docs_url(rule_name: &str) -> String {
    format!("{DOCS_BASE_URL}/{rule_name}.md")
}

impl Rule {
    pub const ALL: [Self; 15] = [
        Self::StrictReadUntracked,
        Self::ReactiveReadAfterAwait,
        Self::NoDestructure,
        Self::ComponentsReturnOnce,
        Self::ReactiveWriteInOwnedScope,
        Self::CleanupInForbiddenScope,
        Self::PrimitiveInLeafOwner,
        Self::NoOwnerEffect,
        Self::NoOwnerCleanup,
        Self::NoOwnerBoundary,
        Self::PrimitiveInDirectiveApplication,
        Self::MissingEffectFunction,
        Self::PackageContractExportMissing,
        Self::PackageContractMissing,
        Self::ExecutionMapIncomplete,
    ];

    #[must_use]
    pub const fn metadata(self) -> RuleMetadata {
        let (code, name, severity, uncertifiable) = match self {
            Self::StrictReadUntracked => ("SC1001", "v1/strict-read-untracked", "warning", false),
            Self::ReactiveReadAfterAwait => {
                ("SC1002", "v1/reactive-read-after-await", "error", false)
            }
            Self::NoDestructure => ("SC1003", "v1/no-destructure", "error", false),
            Self::ComponentsReturnOnce => ("SC1004", "v1/components-return-once", "warning", false),
            Self::ReactiveWriteInOwnedScope => {
                ("SC2001", "v1/reactive-write-in-owned-scope", "error", false)
            }
            Self::CleanupInForbiddenScope => {
                ("SC3001", "v1/cleanup-in-forbidden-scope", "error", false)
            }
            Self::PrimitiveInLeafOwner => ("SC3002", "v1/primitive-in-leaf-owner", "error", false),
            Self::NoOwnerEffect => ("SC4001", "v1/no-owner-effect", "warning", false),
            Self::NoOwnerCleanup => ("SC4002", "v1/no-owner-cleanup", "warning", false),
            Self::NoOwnerBoundary => ("SC4003", "v1/no-owner-boundary", "warning", false),
            Self::PrimitiveInDirectiveApplication => (
                "SC6001",
                "v1/primitive-in-directive-application",
                "error",
                false,
            ),
            Self::MissingEffectFunction => ("SC7001", "v1/missing-effect-function", "error", false),
            Self::PackageContractExportMissing => {
                ("SC9001", "v1/package-contract-export-missing", "error", true)
            }
            Self::PackageContractMissing => {
                ("SC9005", "v1/package-contract-missing", "error", true)
            }
            Self::ExecutionMapIncomplete => {
                ("SC9004", "v1/execution-map-incomplete", "error", true)
            }
        };
        RuleMetadata {
            code,
            name,
            severity,
            uncertifiable,
        }
    }

    /// Resolves a static violation raised inside the shared reactive IR to
    /// this catalog's identity.
    ///
    /// The IR names its diagnostics in its own vocabulary — the identity it
    /// has carried since before dialects existed — and each dialect's catalog
    /// projects that onto its external surface. `SC1003` is
    /// `component-props-destructure` to the engine and `v1/no-destructure`
    /// here, because eslint-plugin-solid users know the defect by that name.
    #[must_use]
    pub fn from_identity(code: &str, name: &str) -> Option<Self> {
        let rule = match (code, name) {
            ("SC1002", "reactive-read-after-await") => Self::ReactiveReadAfterAwait,
            ("SC1003", "component-props-destructure") => Self::NoDestructure,
            ("SC1004", "component-returns-conditionally") => Self::ComponentsReturnOnce,
            ("SC7001", "missing-effect-function") => Self::MissingEffectFunction,
            ("SC9001", "package-contract-export-missing") => Self::PackageContractExportMissing,
            ("SC9004", "execution-map-incomplete") => Self::ExecutionMapIncomplete,
            _ => return None,
        };
        Some(rule)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::Rule;

    #[test]
    fn diagnostic_identities_are_unique_and_well_formed() {
        let identities = Rule::ALL
            .into_iter()
            .map(|rule| {
                let metadata = rule.metadata();
                assert!(metadata.code.starts_with("SC"));
                assert_eq!(metadata.code.len(), 6);
                assert!(
                    metadata.name.starts_with("v1/"),
                    "{} must carry the v1/ namespace",
                    metadata.name
                );
                (metadata.code, metadata.name)
            })
            .collect::<HashSet<_>>();
        assert_eq!(identities.len(), Rule::ALL.len());
    }

    #[test]
    fn every_rule_has_a_documentation_page() {
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/rules");
        for rule in Rule::ALL {
            let page = docs.join(format!("{}.md", rule.metadata().name));
            assert!(
                page.is_file(),
                "rule {} has no documentation page at {}",
                rule.metadata().name,
                page.display()
            );
        }
    }

    /// Codes shared with the Solid 2.0 catalog stay on the shared concept, so
    /// suppression comments survive a dialect migration (a rule that applies
    /// to both keeps one code).
    #[test]
    fn shared_concepts_keep_their_codes() {
        assert_eq!(Rule::StrictReadUntracked.metadata().code, "SC1001");
        assert_eq!(Rule::NoDestructure.metadata().code, "SC1003");
        assert_eq!(Rule::ComponentsReturnOnce.metadata().code, "SC1004");
        assert_eq!(Rule::PackageContractMissing.metadata().code, "SC9005");
    }
}
