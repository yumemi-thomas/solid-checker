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
    MissingOwner,
    MissingEffectFunction,
    // The fine-grained decomposition of eslint-plugin-solid's monolithic
    // `reactivity` rule. Untracked reads and after-await reads land on the
    // engine's own SC1001/SC1002 above; these are the remaining distinct
    // defects that rule bundled. See docs/rules/README.md for the mapping.
    UncalledAccessor,
    ExpectedFunctionGotExpression,
    NoDirectMutation,
    ReactiveSourceUncaptured,
    ReactiveDispatchUnresolved,
    // The eslint-plugin-solid 0.14.5 rule surface, one identity per upstream
    // rule.
    JsxNoDuplicateProps,
    JsxNoUndef,
    PreferClasslist,
    PreferFor,
    PreferShow,
    PackageContractIncomplete,
}

/// Base URL of the per-rule documentation pages in `docs/rules/v1/`.
pub const DOCS_BASE_URL: &str = solid_reactive_ir::DOCS_BASE_URL;

/// The documentation page for a diagnostic, addressed by its externally
/// visible rule name so adapters that only carry the name (snapshots, the
/// ESLint plugin) can link without a catalog lookup. The `v1/` namespace in
/// the rule name is the `v1/` directory in the docs tree.
#[must_use]
pub fn docs_url(rule_name: &str) -> String {
    format!("{DOCS_BASE_URL}/{rule_name}.md")
}

impl Rule {
    pub const ALL: [Self; 18] = [
        Self::StrictReadUntracked,
        Self::ReactiveReadAfterAwait,
        Self::NoDestructure,
        Self::ComponentsReturnOnce,
        Self::ReactiveWriteInOwnedScope,
        Self::MissingOwner,
        Self::MissingEffectFunction,
        Self::UncalledAccessor,
        Self::ExpectedFunctionGotExpression,
        Self::NoDirectMutation,
        Self::ReactiveSourceUncaptured,
        Self::ReactiveDispatchUnresolved,
        Self::JsxNoDuplicateProps,
        Self::JsxNoUndef,
        Self::PreferClasslist,
        Self::PreferFor,
        Self::PreferShow,
        Self::PackageContractIncomplete,
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
            Self::MissingOwner => ("SC4001", "v1/missing-owner", "warning", false),
            Self::MissingEffectFunction => ("SC7001", "v1/missing-effect-function", "error", false),
            Self::UncalledAccessor => ("SC1005", "v1/uncalled-accessor", "warning", false),
            Self::ExpectedFunctionGotExpression => {
                ("SC1007", "v1/reactive-handler-frozen", "warning", false)
            }
            Self::NoDirectMutation => ("SC2003", "v1/no-direct-mutation", "warning", false),
            Self::ReactiveSourceUncaptured => {
                ("SC9011", "v1/reactive-source-uncaptured", "warning", true)
            }
            Self::ReactiveDispatchUnresolved => {
                ("SC9012", "v1/reactive-dispatch-unresolved", "warning", true)
            }
            Self::JsxNoDuplicateProps => ("SC8003", "v1/jsx-no-duplicate-props", "error", false),
            Self::JsxNoUndef => ("SC8005", "v1/jsx-no-undef", "error", false),
            Self::PreferClasslist => ("SC8013", "v1/prefer-classlist", "warning", false),
            Self::PreferFor => ("SC8014", "v1/prefer-for", "error", false),
            Self::PreferShow => ("SC8015", "v1/prefer-show", "warning", false),
            Self::PackageContractIncomplete => {
                ("SC9005", "v1/package-contract-incomplete", "error", true)
            }
        };
        let preference = matches!(
            self,
            Self::PreferClasslist | Self::PreferFor | Self::PreferShow
        );
        RuleMetadata {
            code,
            name,
            severity,
            uncertifiable,
            default_enabled: true,
            presets: if preference { &["preferences"] } else { &[] },
        }
    }

    /// Resolves a static violation raised inside the shared reactive IR to
    /// this catalog's identity.
    ///
    /// The IR names its diagnostics in its own vocabulary — the identity it
    /// has carried since before dialects existed. Every identity that
    /// reaches this catalog as a *static violation* (the upstream-compat
    /// surface) keeps its name here, so the
    /// catalog scan below is the whole projection. Identities whose v1 rule
    /// is renamed (`component-props-destructure` → `v1/no-destructure`)
    /// arrive as static *defects* and are worded by `static_defect_finding`
    /// instead, never through this path.
    #[must_use]
    pub fn from_identity(code: &str, name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| {
            let metadata = rule.metadata();
            metadata.code == code && metadata.name.strip_prefix("v1/") == Some(name)
        })
    }
}

/// The catalog as the npm plugin consumes it: one JSON entry per rule, in
/// catalog order. Generated into `packages/cli/lib/rules-solid-v1.json` by the test
/// below; `SOLID_RULES_UPDATE=1 cargo test -p solid-v1-rules` rewrites it, a plain
/// test run fails on drift. The JS surface must never hand-maintain rule
/// facts the catalog already owns.
#[must_use]
pub fn manifest_json() -> String {
    solid_reactive_ir::rule_manifest_json(
        solid_reactive_ir::RuleManifestIdentity {
            dialect: "solid-v1",
            config: "v1",
            namespace: "v1",
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
            .join("../../../../packages/cli/lib/rules-solid-v1.json");
        let expected = super::manifest_json();
        if std::env::var_os("SOLID_RULES_UPDATE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &expected).unwrap();
            return;
        }
        let shipped = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            shipped, expected,
            "packages/cli/lib/rules-solid-v1.json has drifted from the catalog; run SOLID_RULES_UPDATE=1 cargo test -p solid-v1-rules to rewrite it"
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
        solid_reactive_ir::assert_rules_have_documentation(
            &docs,
            Rule::ALL.into_iter().map(|rule| rule.metadata().name),
        );
    }

    #[test]
    fn every_rule_page_contains_substantive_guidance() {
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/rules");
        for rule in Rule::ALL {
            let name = rule.metadata().name;
            let page = docs.join(format!("{name}.md"));
            let text = std::fs::read_to_string(&page)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", page.display()));
            assert!(
                text.len() >= 400,
                "{} is only {} bytes; every rule page must explain behavior, motivation, and remediation",
                page.display(),
                text.len()
            );
        }
    }

    /// Every identity the reactive IR emits as a *static violation* under
    /// `Version::V1` (the retained upstream-compat surface) must resolve
    /// here — a miss panics in `solve`.
    #[test]
    fn every_v1_static_violation_identity_resolves() {
        for (code, name) in [
            ("SC8003", "jsx-no-duplicate-props"),
            ("SC8005", "jsx-no-undef"),
            ("SC8013", "prefer-classlist"),
            ("SC8014", "prefer-for"),
            ("SC8015", "prefer-show"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_some(),
                "IR static-violation identity {code}/{name} does not resolve in the v1 catalog"
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
        assert_eq!(Rule::PackageContractIncomplete.metadata().code, "SC9005");
    }
}
