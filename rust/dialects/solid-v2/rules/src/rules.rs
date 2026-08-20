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
    ExpectedFunctionGotExpression,
    NoDirectMutation,
    ReactiveSourceUncaptured,
    ReactiveDispatchUnresolved,
    ComponentPropsDestructure,
    ComponentReturnsConditionally,
    ReactiveWriteInOwnedScope,
    ActionCalledInOwnedScope,
    ResolveInReactiveScope,
    LeafOwnerForbiddenCall,
    MissingOwner,
    PendingAsyncUntrackedRead,
    PendingAsyncForbiddenScope,
    AsyncOutsideLoadingBoundary,
    SsrClientSourceOutsideLoadingBoundary,
    PrimitiveInDirectiveApplication,
    MissingEffectFunction,
    SyncNodeReceivedAsync,
    HttpResponseAfterFlush,
    ServerFunctionModuleDirective,
    ServerFunctionRichArgument,
    PackageContractExportMissing,
    PackageContractMissing,
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
    pub const ALL: [Self; 27] = [
        Self::StrictReadUntracked,
        Self::ReactiveReadAfterAwait,
        Self::UncalledAccessor,
        Self::ExpectedFunctionGotExpression,
        Self::NoDirectMutation,
        Self::ReactiveSourceUncaptured,
        Self::ReactiveDispatchUnresolved,
        Self::ComponentPropsDestructure,
        Self::ComponentReturnsConditionally,
        Self::ReactiveWriteInOwnedScope,
        Self::ActionCalledInOwnedScope,
        Self::ResolveInReactiveScope,
        Self::LeafOwnerForbiddenCall,
        Self::MissingOwner,
        Self::PendingAsyncUntrackedRead,
        Self::PendingAsyncForbiddenScope,
        Self::AsyncOutsideLoadingBoundary,
        Self::SsrClientSourceOutsideLoadingBoundary,
        Self::PrimitiveInDirectiveApplication,
        Self::MissingEffectFunction,
        Self::SyncNodeReceivedAsync,
        Self::HttpResponseAfterFlush,
        Self::ServerFunctionModuleDirective,
        Self::ServerFunctionRichArgument,
        Self::PackageContractExportMissing,
        Self::PackageContractMissing,
        Self::PackageContractCallbackMissing,
    ];

    #[must_use]
    pub const fn metadata(self) -> RuleMetadata {
        let (code, name, severity, uncertifiable) = match self {
            Self::StrictReadUntracked => ("SC1001", "strict-read-untracked", "warning", false),
            Self::ReactiveReadAfterAwait => ("SC1002", "reactive-read-after-await", "error", false),
            Self::UncalledAccessor => ("SC1005", "uncalled-accessor", "warning", false),
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
            Self::ReactiveDispatchUnresolved => {
                ("SC9012", "reactive-dispatch-unresolved", "warning", true)
            }
            Self::ComponentPropsDestructure => {
                ("SC1003", "component-props-destructure", "error", false)
            }
            Self::ComponentReturnsConditionally => {
                ("SC1004", "component-returns-conditionally", "error", false)
            }
            Self::ReactiveWriteInOwnedScope => {
                ("SC2001", "reactive-write-in-owned-scope", "error", false)
            }
            Self::ActionCalledInOwnedScope => {
                ("SC2002", "action-called-in-owned-scope", "error", false)
            }
            // SC2003 is the shared no-direct-mutation concept; this new
            // 2.0-only rule takes the next free code in the writes/actions
            // family. The rc.0 dev guard throws on an active observer
            // (probed), so the proven tracked-scope form mirrors it as an
            // error; production has no guard and silently takes a one-shot
            // snapshot.
            Self::ResolveInReactiveScope => ("SC2004", "resolve-in-reactive-scope", "error", false),
            Self::LeafOwnerForbiddenCall => ("SC3001", "leaf-owner-forbidden-call", "error", false),
            // Settled-cleanup findings override this family default to error:
            // the rc.0 dev runtime throws SETTLED_CLEANUP_UNOWNED, while the
            // other missing-owner operations leak silently.
            Self::MissingOwner => ("SC4001", "missing-owner", "warning", false),
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
            // The apply callback runs with no owner (`@solidjs/web` rc.0's
            // `ref()` is `runWithOwner(null, ...)`), so an owner-attaching
            // primitive created there is the SC4001-family defect: a real,
            // per-element leak the dev runtime answers with a NO_OWNER_*
            // *warning* (probed), never a throw. The catalog mirrors that —
            // warning severity, violation kind (the leak is proven).
            Self::PrimitiveInDirectiveApplication => (
                "SC6001",
                "primitive-in-directive-application",
                "warning",
                false,
            ),
            Self::MissingEffectFunction => ("SC7001", "missing-effect-function", "error", false),
            Self::SyncNodeReceivedAsync => ("SC7002", "sync-node-received-async", "error", false),
            // SC7003 and SC9003 each carry two rule names on purpose: the
            // code identifies the defect (an invalid or unresolved target),
            // while the name identifies the surface it was found on
            // (`refresh` versus `affects`).
            // The post-flush drop is real but request-time ordering decides
            // whether it occurs, so this is an uncertifiable hazard.
            Self::HttpResponseAfterFlush => {
                ("SC7005", "http-response-after-flush", "warning", true)
            }
            // The client build silently loses the export (RFC 10 §Compiler
            // implications — "Minimum: a diagnostic"), so this is an error.
            Self::ServerFunctionModuleDirective => {
                ("SC7006", "server-function-module-directive", "error", false)
            }
            // The default transport throws at the call site (probed, rc.0
            // server-functions client), so the proven form is an error.
            Self::ServerFunctionRichArgument => {
                ("SC7007", "server-function-rich-argument", "error", false)
            }
            Self::PackageContractExportMissing => {
                ("SC9001", "package-contract-export-missing", "error", true)
            }
            Self::PackageContractMissing => ("SC9005", "package-contract-missing", "error", true),
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
            ("SC2004", "resolve-in-reactive-scope"),
            ("SC7002", "sync-node-received-async"),
            ("SC7005", "http-response-after-flush"),
            ("SC7006", "server-function-module-directive"),
            ("SC7007", "server-function-rich-argument"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_some(),
                "IR static-violation identity {code}/{name} does not resolve in the v2 catalog"
            );
        }
        // The refresh/affects target identities were removed on 2026-08-17:
        // `Refreshable<T>` brands the target *in the type system*, so every
        // invalid target is TS2345 and every "cannot prove the brand"
        // obligation asks a question the type already answers. Asserted absent
        // so a reintroduction has to argue with this comment.
        for (code, name) in [
            ("SC7003", "invalid-refresh-target"),
            ("SC7003", "invalid-affects-target"),
            ("SC7004", "affects-keys-on-accessor"),
            ("SC9003", "refresh-target-unresolved"),
            ("SC9003", "affects-target-unresolved"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_none(),
                "{code}/{name} duplicates a TypeScript diagnostic and was removed"
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
            Rule::SsrClientSourceOutsideLoadingBoundary
                .metadata()
                .severity,
            "error"
        );
        // SETTLED_CLEANUP_UNOWNED is a dev *throw* (rc.0 dev bundle emits an
        // error diagnostic and throws), not a warning: an onSettled callback
        // returning a cleanup in an unowned scope halts in dev and drops the
        // cleanup in production. The catalog mirrors the throw as an error.
        assert_eq!(Rule::MissingOwner.metadata().severity, "warning");
        // resolve() under an active observer is a dev *throw* ("Cannot call
        // resolve inside a reactive scope", probed on the rc.0 signals dev
        // bundle), mirrored as an error like the other owned/tracked-scope
        // throws.
        assert_eq!(Rule::ResolveInReactiveScope.metadata().severity, "error");
        // The rich-argument transport throw is unconditional at the default
        // client (probed) — error; the post-flush header drop only occurs
        // when the boundary settles after the shell flush — warning.
        assert_eq!(
            Rule::ServerFunctionRichArgument.metadata().severity,
            "error"
        );
        assert_eq!(Rule::HttpResponseAfterFlush.metadata().severity, "warning");
        assert!(Rule::HttpResponseAfterFlush.metadata().uncertifiable);
    }
}
