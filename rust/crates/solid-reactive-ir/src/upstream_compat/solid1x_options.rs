//! Project-wide rule configuration and Solid 1.x rule options.
//!
//! eslint-plugin-solid ships user-configurable options on several of the
//! rules this pass reproduces. The checker carries exactly the options whose
//! behaviour upstream's own test corpus exercises — every field here is
//! proven by a product-owned case in `fixtures/ownership-cases` — and nothing speculative.
//! Option fields use upstream's defaults. Rule enablement defaults to the
//! checker's existing catalog policy (enabled), so an absent file reproduces
//! the behaviour the pass has always had.
//!
//! # Where options come from
//!
//! One project-level document, `.solid-checker/rule-options.json`, discovered
//! by the same ancestor walk as `.solid-checker/contracts/`. There is
//! deliberately no per-file or per-ESLint-config channel: the npm adapter
//! spawns one analysis per project, and ESLint hands each rule its options
//! lazily per file, so options arriving through ESLint could never be
//! aggregated into that one run without racing it. A single discovered file
//! keeps the CLI, the daemon, and every editor integration reading the same
//! configuration.
//!
//! # Fail-closed parsing
//!
//! [`RuleOptions::parse`] rejects unknown rule names, unknown option keys,
//! and unsupported schema versions rather than ignoring them: a typo in a
//! config must not silently mean "defaults".

use std::collections::{BTreeMap, BTreeSet};

use crate::RuntimeEnvironment;
use serde::Deserialize;

/// Options for `v1/prefer-classlist` (SC8013).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct PreferClasslistOptions {
    /// Upstream's `classnames`: the helper names whose object-literal call
    /// in a `class` prop the rule rewrites to `classlist`.
    pub classnames: Vec<String>,
}

impl Default for PreferClasslistOptions {
    fn default() -> Self {
        Self {
            classnames: ["cn", "clsx", "classnames"]
                .map(str::to_owned)
                .into_iter()
                .collect(),
        }
    }
}

/// Options owned specifically by the Solid 1.x compatibility implementation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Solid1xRuleOptions {
    pub prefer_classlist: PreferClasslistOptions,
}

impl Solid1xRuleOptions {
    const CONFIGURABLE_RULES: [&'static str; 1] = ["prefer-classlist"];

    /// Applies an option object owned by the Solid 1.x compatibility layer.
    ///
    /// `None` means this is a catalog rule without dialect-specific options;
    /// the shared project parser therefore never needs to know these names.
    fn parse_rule(&mut self, rule: &str, value: &serde_json::Value) -> Option<Result<(), String>> {
        let parsed = match rule {
            "prefer-classlist" => {
                serde_json::from_value(value.clone()).map(|parsed| self.prefer_classlist = parsed)
            }
            _ => return None,
        };
        Some(parsed.map_err(|error| error.to_string()))
    }
}

/// Dialect-neutral project rule configuration.
///
/// Enablement belongs here because every catalog uses it. Dialect-specific
/// option shapes remain nested in their owning implementation instead of
/// becoming the shared pipeline's interface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleOptions {
    overrides: BTreeMap<String, RuleOverride>,
    requested_presets: BTreeSet<String>,
    requested_rules: BTreeSet<String>,
    pub solid1x: Solid1xRuleOptions,
    /// Host-selected runtime evidence is part of the retained analysis
    /// identity, but is not a rule-owned option. Keeping it beside the
    /// catalog options lets the shared pipeline thread one immutable selector
    /// through every proof stage without making dialect code guess globals.
    pub runtime: RuntimeEnvironment,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuleOverride {
    #[default]
    Unset,
    Enabled,
    Disabled,
}

/// The document shape of `.solid-checker/rule-options.json`: a schema
/// version plus per-rule objects keyed by the catalog's external rule name.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    schema_version: u32,
    #[serde(default)]
    rules: BTreeMap<String, serde_json::Value>,
}

impl RuleOptions {
    /// Parses a rule-options document, failing closed on anything it does
    /// not understand. `has_rule` keeps the dialect catalogs as the source of
    /// truth for names instead of duplicating their identity tables here.
    pub fn parse(
        encoded: &str,
        has_rule: impl Fn(&str) -> bool,
        owns_solid1x_options: impl Fn(&str) -> bool,
    ) -> Result<Self, String> {
        Self::parse_with_aliases(encoded, has_rule, owns_solid1x_options, |_| None)
    }

    /// Parses a document while canonicalizing former external names onto
    /// their current catalog identities. Alias lookup stays outside this
    /// crate so the backend's permanent compatibility table remains the one
    /// source of truth.
    pub fn parse_with_aliases(
        encoded: &str,
        has_rule: impl Fn(&str) -> bool,
        owns_solid1x_options: impl Fn(&str) -> bool,
        alias: impl Fn(&str) -> Option<&'static str>,
    ) -> Result<Self, String> {
        let document: Document = serde_json::from_str(encoded)
            .map_err(|error| format!("rule options are not a valid document: {error}"))?;
        if document.schema_version != 1 {
            return Err(format!(
                "rule options schema version {} is unsupported; this checker reads version 1",
                document.schema_version
            ));
        }
        let mut options = Self::default();
        for (configured_rule, value) in document.rules {
            let rule = alias(&configured_rule).unwrap_or(&configured_rule);
            if !has_rule(rule) {
                return Err(format!(
                    "the rule catalog has no rule named {configured_rule:?}"
                ));
            }
            let mut fields = value.as_object().cloned().ok_or_else(|| {
                format!("rule options for {configured_rule:?} must be a JSON object")
            })?;
            let enabled = fields.remove("enabled").map_or(Ok(true), |value| {
                value.as_bool().ok_or_else(|| {
                    format!("rule options for {configured_rule:?}: enabled must be a boolean")
                })
            })?;
            options.overrides.insert(
                rule.to_owned(),
                if enabled {
                    RuleOverride::Enabled
                } else {
                    RuleOverride::Disabled
                },
            );
            let value = serde_json::Value::Object(fields);
            // This module owns 1.x option shapes, not the catalog's external
            // namespace. Match the final, stable upstream rule key after the
            // catalog has validated the full external name.
            let local_rule = rule.rsplit('/').next().unwrap_or(rule);
            let result = owns_solid1x_options(rule)
                .then(|| options.solid1x.parse_rule(local_rule, &value))
                .flatten()
                .unwrap_or_else(|| {
                    if value.as_object().is_some_and(serde_json::Map::is_empty) {
                        Ok(())
                    } else {
                        Err(format!(
                            "this rule takes only `enabled`; the rules with additional options \
                             are {}",
                            Solid1xRuleOptions::CONFIGURABLE_RULES.join(", ")
                        ))
                    }
                });
            result.map_err(|error| format!("rule options for {configured_rule:?}: {error}"))?;
        }
        Ok(options)
    }

    pub fn request_presets(&mut self, presets: impl IntoIterator<Item = String>) {
        self.requested_presets.extend(presets);
    }

    pub fn request_rules(&mut self, rules: impl IntoIterator<Item = String>) {
        self.requested_rules.extend(rules);
    }

    /// Whether a catalog finding participates in certification.
    #[must_use]
    pub fn is_enabled(&self, rule: &str, default_enabled: bool, presets: &[&str]) -> bool {
        match self.overrides.get(rule) {
            Some(RuleOverride::Disabled) => false,
            Some(RuleOverride::Enabled) => true,
            None | Some(RuleOverride::Unset) => {
                default_enabled
                    || self.requested_rules.contains(rule)
                    || presets
                        .iter()
                        .any(|preset| self.requested_presets.contains(*preset))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuleOptions;

    fn known_rule(rule: &str) -> bool {
        matches!(
            rule,
            "v1/prefer-classlist"
                | "v1/no-destructure"
                | "v1/no-direct-mutation"
                | "no-owner-cleanup"
                | "no-owner-settled-cleanup"
                | "strict-read-untracked"
        )
    }

    fn solid1x_rule(rule: &str) -> bool {
        rule.starts_with("v1/") || rule.starts_with("legacy/")
    }

    #[test]
    fn an_empty_document_is_upstreams_defaults() {
        let options =
            RuleOptions::parse(r#"{ "schemaVersion": 1 }"#, known_rule, solid1x_rule).unwrap();
        assert_eq!(options, RuleOptions::default());
        let options = options.solid1x;
        assert_eq!(
            options.prefer_classlist.classnames,
            ["cn", "clsx", "classnames"]
        );
    }

    #[test]
    fn parses_every_configurable_rule() {
        let options = RuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "v1/prefer-classlist": { "classnames": ["cx"] }
              }
            }"#,
            known_rule,
            solid1x_rule,
        )
        .unwrap();
        let options = options.solid1x;
        assert_eq!(options.prefer_classlist.classnames, ["cx"]);
    }

    #[test]
    fn parses_enablement_for_rules_with_and_without_options() {
        let options = RuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "v1/no-direct-mutation": { "enabled": false },
                "no-owner-cleanup": { "enabled": false },
                "strict-read-untracked": { "enabled": true }
              }
            }"#,
            known_rule,
            solid1x_rule,
        )
        .unwrap();
        assert!(!options.is_enabled("v1/no-direct-mutation", true, &[]));
        assert!(options.is_enabled("strict-read-untracked", true, &[]));
        assert!(options.is_enabled("v1/no-destructure", true, &[]));
        assert!(!options.is_enabled("no-owner-cleanup", true, &[]));
        assert!(options.is_enabled("no-owner-settled-cleanup", true, &[]));
    }

    #[test]
    fn rejects_unknown_rules_keys_and_schema_versions() {
        assert!(RuleOptions::parse(r#"{ "schemaVersion": 2 }"#, known_rule, solid1x_rule).is_err());
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/not-a-rule": {} } }"#,
                known_rule,
                solid1x_rule,
            )
            .is_err()
        );
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-destructure": { "severity": "off" } } }"#,
                known_rule,
                solid1x_rule,
            )
            .is_err()
        );
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-destructure": { "enabled": "no" } } }"#,
                known_rule,
                solid1x_rule,
            )
            .is_err()
        );
    }

    #[test]
    fn aliases_transfer_disable_to_the_canonical_rule() {
        let options = RuleOptions::parse_with_aliases(
            r#"{
              "schemaVersion": 1,
              "rules": { "old-missing-owner": { "enabled": false } }
            }"#,
            |rule| rule == "missing-owner",
            |_| false,
            |rule| (rule == "old-missing-owner").then_some("missing-owner"),
        )
        .unwrap();

        assert!(!options.is_enabled("missing-owner", true, &[]));
        assert!(options.is_enabled("old-missing-owner", true, &[]));
    }

    #[test]
    fn catalog_defaults_presets_requests_and_overrides_have_stable_precedence() {
        let mut options = RuleOptions::default();
        assert!(!options.is_enabled("prefer-show", false, &["preferences"]));
        assert!(options.is_enabled("strict-read-untracked", true, &[]));

        options.request_presets(["preferences".into()]);
        assert!(options.is_enabled("prefer-show", false, &["preferences"]));

        options.request_rules(["prefer-for".into()]);
        assert!(options.is_enabled("prefer-for", false, &["preferences"]));

        let mut disabled = RuleOptions::parse(
            r#"{ "schemaVersion": 1, "rules": { "v1/prefer-classlist": { "enabled": false } } }"#,
            known_rule,
            solid1x_rule,
        )
        .unwrap();
        disabled.request_presets(["preferences".into()]);
        assert!(!disabled.is_enabled("v1/prefer-classlist", false, &["preferences"]));
    }
}
