//! Stable diagnostic identities. Analysis decides whether a rule applies;
//! this catalog owns its externally visible code, name, and severity.

use solid_reactive_ir::RuleMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    StrictReadUntracked,
    ReactiveReadAfterAwait,
    // The decomposed upstream-`reactivity` rules shared with the 1.x
    // dialect: the defects — an accessor used where a value was meant, a
    // readonly proxy written through, a listener bound to a call's result, a
    // derivation nothing tracks, a source handed to an undescribed callee —
    // are version-independent, so both catalogs carry them under the same SC
    // codes and suppressions survive a migration. `no-async-tracked-scope`
    // stays 1.x-only: 2.0 models async computations as a feature
    // (SC5001–SC5003 and SC5005 own that surface).
    UncalledAccessor,
    UntrackedDerivedFunction,
    ExpectedFunctionGotExpression,
    NoDirectMutation,
    ReactiveSourceUncaptured,
    ComponentPropsDestructure,
    ComponentReturnsConditionally,
    PreferComponentSyntax,
    NoImplicitDraggable,
    ValidJsxNesting,
    ReactiveWriteInOwnedScope,
    ActionCalledInOwnedScope,
    CleanupInForbiddenScope,
    PrimitiveInLeafOwner,
    FlushInForbiddenScope,
    InvalidCleanupReturn,
    NoOwnerEffect,
    NoOwnerCleanup,
    NoOwnerBoundary,
    NoOwnerSettledCleanup,
    PendingAsyncUntrackedRead,
    PendingAsyncForbiddenScope,
    AsyncOutsideLoadingBoundary,
    SsrClientSourceOutsideLoadingBoundary,
    PrimitiveInDirectiveApplication,
    MissingEffectFunction,
    SyncNodeReceivedAsync,
    InvalidRefreshTarget,
    InvalidAffectsTarget,
    AffectsKeysOnAccessor,
    PackageContractExportMissing,
    PackageContractMissing,
    CleanupReturnUnresolved,
    RefreshTargetUnresolved,
    AffectsTargetUnresolved,
    ExecutionMapIncomplete,
    PackageContractCallbackMissing,
}

/// Base URL of the per-rule documentation pages in `docs/rules/`.
pub const DOCS_BASE_URL: &str = solid_reactive_ir::DOCS_BASE_URL;

/// The documentation page for a diagnostic, addressed by its externally
/// visible rule name so adapters that only carry the name (LSP, snapshots)
/// can link without a catalog lookup.
#[must_use]
pub fn docs_url(rule_name: &str) -> String {
    format!("{DOCS_BASE_URL}/{rule_name}.md")
}

impl Rule {
    pub const ALL: [Self; 39] = [
        Self::StrictReadUntracked,
        Self::ReactiveReadAfterAwait,
        Self::UncalledAccessor,
        Self::UntrackedDerivedFunction,
        Self::ExpectedFunctionGotExpression,
        Self::NoDirectMutation,
        Self::ReactiveSourceUncaptured,
        Self::ComponentPropsDestructure,
        Self::ComponentReturnsConditionally,
        Self::PreferComponentSyntax,
        Self::NoImplicitDraggable,
        Self::ValidJsxNesting,
        Self::ReactiveWriteInOwnedScope,
        Self::ActionCalledInOwnedScope,
        Self::CleanupInForbiddenScope,
        Self::PrimitiveInLeafOwner,
        Self::FlushInForbiddenScope,
        Self::InvalidCleanupReturn,
        Self::NoOwnerEffect,
        Self::NoOwnerCleanup,
        Self::NoOwnerBoundary,
        Self::NoOwnerSettledCleanup,
        Self::PendingAsyncUntrackedRead,
        Self::PendingAsyncForbiddenScope,
        Self::AsyncOutsideLoadingBoundary,
        Self::SsrClientSourceOutsideLoadingBoundary,
        Self::PrimitiveInDirectiveApplication,
        Self::MissingEffectFunction,
        Self::SyncNodeReceivedAsync,
        Self::InvalidRefreshTarget,
        Self::InvalidAffectsTarget,
        Self::AffectsKeysOnAccessor,
        Self::PackageContractExportMissing,
        Self::PackageContractMissing,
        Self::CleanupReturnUnresolved,
        Self::RefreshTargetUnresolved,
        Self::AffectsTargetUnresolved,
        Self::ExecutionMapIncomplete,
        Self::PackageContractCallbackMissing,
    ];

    #[must_use]
    pub const fn metadata(self) -> RuleMetadata {
        let (code, name, severity, uncertifiable) = match self {
            Self::StrictReadUntracked => ("SC1001", "strict-read-untracked", "warning", false),
            Self::ReactiveReadAfterAwait => ("SC1002", "reactive-read-after-await", "error", false),
            Self::UncalledAccessor => ("SC1005", "uncalled-accessor", "warning", false),
            Self::UntrackedDerivedFunction => {
                ("SC1006", "untracked-derived-function", "warning", false)
            }
            Self::ExpectedFunctionGotExpression => (
                "SC1007",
                "expected-function-got-expression",
                "warning",
                false,
            ),
            Self::NoDirectMutation => ("SC2003", "no-direct-mutation", "warning", false),
            Self::ReactiveSourceUncaptured => {
                ("SC9011", "reactive-source-uncaptured", "warning", true)
            }
            Self::ComponentPropsDestructure => {
                ("SC1003", "component-props-destructure", "error", false)
            }
            Self::ComponentReturnsConditionally => {
                ("SC1004", "component-returns-conditionally", "error", false)
            }
            Self::PreferComponentSyntax => ("SC8018", "prefer-component-syntax", "warning", false),
            Self::NoImplicitDraggable => ("SC8019", "no-implicit-draggable", "error", false),
            Self::ValidJsxNesting => ("SC8020", "valid-jsx-nesting", "error", false),
            Self::ReactiveWriteInOwnedScope => {
                ("SC2001", "reactive-write-in-owned-scope", "error", false)
            }
            Self::ActionCalledInOwnedScope => {
                ("SC2002", "action-called-in-owned-scope", "error", false)
            }
            Self::CleanupInForbiddenScope => {
                ("SC3001", "cleanup-in-forbidden-scope", "error", false)
            }
            Self::PrimitiveInLeafOwner => ("SC3002", "primitive-in-leaf-owner", "error", false),
            Self::FlushInForbiddenScope => ("SC3003", "flush-in-forbidden-scope", "error", false),
            Self::InvalidCleanupReturn => ("SC3004", "invalid-cleanup-return", "error", false),
            Self::NoOwnerEffect => ("SC4001", "no-owner-effect", "warning", false),
            Self::NoOwnerCleanup => ("SC4002", "no-owner-cleanup", "warning", false),
            Self::NoOwnerBoundary => ("SC4003", "no-owner-boundary", "warning", false),
            Self::NoOwnerSettledCleanup => ("SC4004", "no-owner-settled-cleanup", "warning", false),
            Self::PendingAsyncUntrackedRead => {
                ("SC5001", "pending-async-untracked-read", "error", false)
            }
            Self::PendingAsyncForbiddenScope => {
                ("SC5002", "pending-async-forbidden-scope", "warning", false)
            }
            Self::AsyncOutsideLoadingBoundary => {
                ("SC5003", "async-outside-loading-boundary", "warning", false)
            }
            // SC5004 belongs to v1/no-async-tracked-scope, a different defect
            // concept; per the shared-code-by-concept policy this new 2.0-only
            // rule takes the next free code in the async family.
            Self::SsrClientSourceOutsideLoadingBoundary => (
                "SC5005",
                "ssr-client-source-outside-loading-boundary",
                "error",
                false,
            ),
            Self::PrimitiveInDirectiveApplication => (
                "SC6001",
                "primitive-in-directive-application",
                "error",
                false,
            ),
            Self::MissingEffectFunction => ("SC7001", "missing-effect-function", "error", false),
            Self::SyncNodeReceivedAsync => ("SC7002", "sync-node-received-async", "error", false),
            // SC7003 and SC9003 each carry two rule names on purpose: the
            // code identifies the defect (an invalid or unresolved target),
            // while the name identifies the surface it was found on
            // (`refresh` versus `affects`).
            Self::InvalidRefreshTarget => ("SC7003", "invalid-refresh-target", "error", false),
            Self::InvalidAffectsTarget => ("SC7003", "invalid-affects-target", "error", false),
            Self::AffectsKeysOnAccessor => ("SC7004", "affects-keys-on-accessor", "error", false),
            Self::PackageContractExportMissing => {
                ("SC9001", "package-contract-export-missing", "error", true)
            }
            Self::PackageContractMissing => ("SC9005", "package-contract-missing", "error", true),
            Self::CleanupReturnUnresolved => ("SC9002", "cleanup-return-unresolved", "error", true),
            Self::RefreshTargetUnresolved => ("SC9003", "refresh-target-unresolved", "error", true),
            Self::AffectsTargetUnresolved => ("SC9003", "affects-target-unresolved", "error", true),
            Self::ExecutionMapIncomplete => ("SC9004", "execution-map-incomplete", "error", true),
            Self::PackageContractCallbackMissing => {
                ("SC9006", "package-contract-callback-missing", "error", true)
            }
        };
        RuleMetadata {
            code,
            name,
            severity,
            uncertifiable,
        }
    }

    #[must_use]
    pub fn from_identity(code: &str, name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|rule| rule.metadata().code == code && rule.metadata().name == name)
    }
}

/// The catalog as the npm plugin consumes it: one JSON entry per rule, in
/// catalog order. Generated into `packages/cli/lib/rules-solid-v2.json` by the test
/// below; `SOLID_RULES_UPDATE=1 cargo test -p solid-v2-rules` rewrites it, a plain
/// test run fails on drift. The JS surface must never hand-maintain rule
/// facts the catalog already owns.
#[must_use]
pub fn manifest_json() -> String {
    solid_reactive_ir::rule_manifest_json(
        solid_reactive_ir::RuleManifestIdentity {
            dialect: "solid-v2",
            config: "v2",
            namespace: "",
        },
        DOCS_BASE_URL,
        Rule::ALL.into_iter().map(Rule::metadata),
    )
}

#[cfg(test)]
mod tests {
    /// The npm plugin reads the catalog from a checked-in JSON file; this is
    /// what keeps that file the catalog. `SOLID_RULES_UPDATE=1` rewrites it.
    #[test]
    fn the_shipped_manifest_is_the_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../packages/cli/lib/rules-solid-v2.json");
        let expected = super::manifest_json();
        if std::env::var_os("SOLID_RULES_UPDATE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &expected).unwrap();
            return;
        }
        let shipped = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            shipped, expected,
            "packages/cli/lib/rules-solid-v2.json has drifted from the catalog; run SOLID_RULES_UPDATE=1 cargo test -p solid-v2-rules to rewrite it"
        );
    }

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
                assert!(!metadata.name.is_empty());
                (metadata.code, metadata.name)
            })
            .collect::<HashSet<_>>();
        assert_eq!(identities.len(), Rule::ALL.len());
    }

    #[test]
    fn every_rule_has_a_documentation_page() {
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/rules");
        solid_reactive_ir::assert_rules_have_documentation(
            &docs,
            Rule::ALL.into_iter().map(|rule| rule.metadata().name),
        );
    }

    /// Every identity the reactive IR emits as a *static violation* on the
    /// 2.0 static-API surface must resolve here — a miss panics in `solve`.
    #[test]
    fn every_v2_static_violation_identity_resolves() {
        for (code, name) in [
            ("SC7002", "sync-node-received-async"),
            ("SC7003", "invalid-refresh-target"),
            ("SC7003", "invalid-affects-target"),
            ("SC7004", "affects-keys-on-accessor"),
            ("SC9003", "refresh-target-unresolved"),
            ("SC9003", "affects-target-unresolved"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_some(),
                "IR static-violation identity {code}/{name} does not resolve in the v2 catalog"
            );
        }
    }

    #[test]
    fn runtime_mirrored_severities_match_solid_two() {
        assert_eq!(
            Rule::AsyncOutsideLoadingBoundary.metadata().severity,
            "warning"
        );
        assert_eq!(Rule::PendingAsyncUntrackedRead.metadata().severity, "error");
        assert_eq!(
            Rule::PendingAsyncForbiddenScope.metadata().severity,
            "warning"
        );
        // The server throw for a bare ssrSource: "client" read outside a
        // Loading boundary is unconditional (rc.0 dist/server.js), so the
        // static rule mirrors it as an error.
        assert_eq!(
            Rule::SsrClientSourceOutsideLoadingBoundary.metadata().severity,
            "error"
        );
    }
}
