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
    // The fine-grained decomposition of eslint-plugin-solid's monolithic
    // `reactivity` rule. Untracked reads and after-await reads land on the
    // engine's own SC1001/SC1002 above; these are the remaining distinct
    // defects that rule bundled. See docs/rules/README.md for the mapping.
    UncalledAccessor,
    UntrackedDerivedFunction,
    ExpectedFunctionGotExpression,
    NoDirectMutation,
    NoAsyncTrackedScope,
    ReactiveSourceUncaptured,
    // The eslint-plugin-solid 0.14.5 rule surface, one identity per upstream
    // rule. `jsx-uses-vars` is catalog-only: upstream exists to mark JSX
    // identifiers used for no-unused-vars, and TypeScript reference facts
    // already model those uses, so nothing here ever emits it.
    EventHandlers,
    Imports,
    JsxNoDuplicateProps,
    JsxNoScriptUrl,
    JsxNoUndef,
    JsxUsesVars,
    NoArrayHandlers,
    NoInnerhtml,
    NoProxyApis,
    NoReactDeps,
    NoReactSpecificProps,
    NoUnknownNamespaces,
    PreferClasslist,
    PreferFor,
    PreferShow,
    SelfClosingComp,
    StyleProp,
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
    pub const ALL: [Self; 38] = [
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
        Self::UncalledAccessor,
        Self::UntrackedDerivedFunction,
        Self::ExpectedFunctionGotExpression,
        Self::NoDirectMutation,
        Self::NoAsyncTrackedScope,
        Self::ReactiveSourceUncaptured,
        Self::EventHandlers,
        Self::Imports,
        Self::JsxNoDuplicateProps,
        Self::JsxNoScriptUrl,
        Self::JsxNoUndef,
        Self::JsxUsesVars,
        Self::NoArrayHandlers,
        Self::NoInnerhtml,
        Self::NoProxyApis,
        Self::NoReactDeps,
        Self::NoReactSpecificProps,
        Self::NoUnknownNamespaces,
        Self::PreferClasslist,
        Self::PreferFor,
        Self::PreferShow,
        Self::SelfClosingComp,
        Self::StyleProp,
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
            Self::UncalledAccessor => ("SC1005", "v1/uncalled-accessor", "warning", false),
            Self::UntrackedDerivedFunction => {
                ("SC1006", "v1/untracked-derived-function", "warning", false)
            }
            Self::ExpectedFunctionGotExpression => (
                "SC1007",
                "v1/expected-function-got-expression",
                "warning",
                false,
            ),
            Self::NoDirectMutation => ("SC2003", "v1/no-direct-mutation", "warning", false),
            Self::NoAsyncTrackedScope => ("SC5004", "v1/no-async-tracked-scope", "warning", false),
            Self::ReactiveSourceUncaptured => {
                ("SC9011", "v1/reactive-source-uncaptured", "warning", true)
            }
            Self::EventHandlers => ("SC8001", "v1/event-handlers", "warning", false),
            Self::Imports => ("SC8002", "v1/imports", "warning", false),
            Self::JsxNoDuplicateProps => ("SC8003", "v1/jsx-no-duplicate-props", "error", false),
            Self::JsxNoScriptUrl => ("SC8004", "v1/jsx-no-script-url", "error", false),
            Self::JsxNoUndef => ("SC8005", "v1/jsx-no-undef", "error", false),
            Self::JsxUsesVars => ("SC8006", "v1/jsx-uses-vars", "error", false),
            Self::NoArrayHandlers => ("SC8007", "v1/no-array-handlers", "error", false),
            Self::NoInnerhtml => ("SC8008", "v1/no-innerhtml", "error", false),
            Self::NoProxyApis => ("SC8009", "v1/no-proxy-apis", "error", false),
            Self::NoReactDeps => ("SC8010", "v1/no-react-deps", "warning", false),
            Self::NoReactSpecificProps => {
                ("SC8011", "v1/no-react-specific-props", "warning", false)
            }
            Self::NoUnknownNamespaces => ("SC8012", "v1/no-unknown-namespaces", "error", false),
            Self::PreferClasslist => ("SC8013", "v1/prefer-classlist", "warning", false),
            Self::PreferFor => ("SC8014", "v1/prefer-for", "error", false),
            Self::PreferShow => ("SC8015", "v1/prefer-show", "warning", false),
            Self::SelfClosingComp => ("SC8016", "v1/self-closing-comp", "warning", false),
            Self::StyleProp => ("SC8017", "v1/style-prop", "warning", false),
            Self::PackageContractExportMissing => (
                "SC9001",
                "v1/package-contract-export-missing",
                "error",
                true,
            ),
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
            ("SC1005", "uncalled-accessor") => Self::UncalledAccessor,
            ("SC1006", "untracked-derived-function") => Self::UntrackedDerivedFunction,
            ("SC1007", "expected-function-got-expression") => Self::ExpectedFunctionGotExpression,
            ("SC2003", "no-direct-mutation") => Self::NoDirectMutation,
            ("SC5004", "no-async-tracked-scope") => Self::NoAsyncTrackedScope,
            ("SC9011", "reactive-source-uncaptured") => Self::ReactiveSourceUncaptured,
            ("SC8001", "event-handlers") => Self::EventHandlers,
            ("SC8002", "imports") => Self::Imports,
            ("SC8003", "jsx-no-duplicate-props") => Self::JsxNoDuplicateProps,
            ("SC8004", "jsx-no-script-url") => Self::JsxNoScriptUrl,
            ("SC8005", "jsx-no-undef") => Self::JsxNoUndef,
            ("SC8007", "no-array-handlers") => Self::NoArrayHandlers,
            ("SC8008", "no-innerhtml") => Self::NoInnerhtml,
            ("SC8009", "no-proxy-apis") => Self::NoProxyApis,
            ("SC8010", "no-react-deps") => Self::NoReactDeps,
            ("SC8011", "no-react-specific-props") => Self::NoReactSpecificProps,
            ("SC8012", "no-unknown-namespaces") => Self::NoUnknownNamespaces,
            ("SC8013", "prefer-classlist") => Self::PreferClasslist,
            ("SC8014", "prefer-for") => Self::PreferFor,
            ("SC8015", "prefer-show") => Self::PreferShow,
            ("SC8016", "self-closing-comp") => Self::SelfClosingComp,
            ("SC8017", "style-prop") => Self::StyleProp,
            ("SC9001", "package-contract-export-missing") => Self::PackageContractExportMissing,
            ("SC9004", "execution-map-incomplete") => Self::ExecutionMapIncomplete,
            _ => return None,
        };
        Some(rule)
    }
}

/// The catalog as the npm plugin consumes it: one JSON entry per rule, in
/// catalog order. Generated into `packages/cli/lib/rules-v1.json` by the test
/// below; `SOLID_RULES_UPDATE=1 cargo test -p solid-v1-rules` rewrites it, a plain
/// test run fails on drift. The JS surface must never hand-maintain rule
/// facts the catalog already owns.
#[must_use]
pub fn manifest_json() -> String {
    let mut out = format!("{{\n  \"docsBaseUrl\": \"{DOCS_BASE_URL}\",\n  \"rules\": [\n");
    for (index, rule) in Rule::ALL.into_iter().enumerate() {
        let metadata = rule.metadata();
        out.push_str(&format!(
            "    {{ \"code\": \"{}\", \"name\": \"{}\", \"severity\": \"{}\", \"uncertifiable\": {} }}{}\n",
            metadata.code,
            metadata.name,
            metadata.severity,
            metadata.uncertifiable,
            if index + 1 == Rule::ALL.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

#[cfg(test)]
mod tests {
    /// The npm plugin reads the catalog from a checked-in JSON file; this is
    /// what keeps that file the catalog. `SOLID_RULES_UPDATE=1` rewrites it.
    #[test]
    fn the_shipped_manifest_is_the_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../packages/cli/lib/rules-v1.json");
        let expected = super::manifest_json();
        if std::env::var_os("SOLID_RULES_UPDATE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &expected).unwrap();
            return;
        }
        let shipped = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            shipped, expected,
            "packages/cli/lib/rules-v1.json has drifted from the catalog; run SOLID_RULES_UPDATE=1 cargo test -p solid-v1-rules to rewrite it"
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
