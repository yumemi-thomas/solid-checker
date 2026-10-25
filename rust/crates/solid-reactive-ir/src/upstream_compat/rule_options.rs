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
//! by an ancestor walk from the project. There is
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

/// Dialect-neutral project rule configuration.
///
/// Enablement belongs here because every catalog uses it. Dialect-specific
/// option shapes remain nested in their owning implementation instead of
/// becoming the shared pipeline's interface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleOptions {
    overrides: BTreeMap<String, RuleOverride>,
    /// Overrides of a single finding code, set through a former rule name that
    /// covered one code of a merged rule (ADR 0384): `resolve-in-tracked-scope`
    /// reaches `SC2004` alone, where the merged name reaches both codes.
    code_overrides: BTreeMap<String, RuleOverride>,
    requested_presets: BTreeSet<String>,
    requested_rules: BTreeSet<String>,
    /// Host-selected runtime evidence is part of the retained analysis
    /// identity, but is not a rule-owned option. Keeping it beside the
    /// catalog options lets the shared pipeline thread one immutable selector
    /// through every proof stage without making dialect code guess globals.
    pub runtime: RuntimeEnvironment,
    /// Optional source models for development instrumentation; included in
    /// cache identity and never inferred from project rule configuration.
    pub development_feedback: bool,
    /// `valid-use-server`'s `clientWrappers` entries, in upstream's name
    /// grammar (ADR 0385): names the project itself declares to be client-side
    /// declaration wrappers, beside the built-in list.
    client_wrappers: Vec<String>,
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
    pub fn parse(encoded: &str, has_rule: impl Fn(&str) -> bool) -> Result<Self, String> {
        Self::parse_with_aliases(encoded, has_rule, |_| None)
    }

    /// Parses a document while canonicalizing former external names onto
    /// their current catalog identities. Alias lookup stays outside this
    /// crate so the backend's permanent compatibility table remains the one
    /// source of truth.
    pub fn parse_with_aliases(
        encoded: &str,
        has_rule: impl Fn(&str) -> bool,
        alias: impl Fn(&str) -> Option<&'static str>,
    ) -> Result<Self, String> {
        Self::parse_with_scoped_aliases(encoded, has_rule, alias, |_| None)
    }

    /// [`Self::parse_with_aliases`] where a former name may also be limited to
    /// one finding code (`scope`): its `enabled` then applies to that code
    /// alone and not to the rest of the rule it canonicalizes onto.
    pub fn parse_with_scoped_aliases(
        encoded: &str,
        has_rule: impl Fn(&str) -> bool,
        alias: impl Fn(&str) -> Option<&'static str>,
        scope: impl Fn(&str) -> Option<&'static str>,
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
            let requested = if enabled {
                RuleOverride::Enabled
            } else {
                RuleOverride::Disabled
            };
            if let Some(code) = scope(&configured_rule) {
                // Two former names of one code (`resolve-in-tracked-scope`,
                // `resolve-in-reactive-scope`) may disagree: a disable stays.
                let slot = options
                    .code_overrides
                    .entry(code.to_owned())
                    .or_insert(requested);
                if requested == RuleOverride::Disabled {
                    *slot = requested;
                }
            } else {
                options.overrides.insert(rule.to_owned(), requested);
            }
            if rule == "valid-use-server"
                && let Some(entries) = fields.remove("clientWrappers")
            {
                options.client_wrappers = entries
                    .as_array()
                    .and_then(|entries| {
                        entries
                            .iter()
                            .map(|entry| entry.as_str().map(str::to_owned))
                            .collect::<Option<Vec<_>>>()
                    })
                    .ok_or_else(|| {
                        format!(
                            "rule options for {configured_rule:?}: clientWrappers must be an array of strings"
                        )
                    })?;
            }
            let value = serde_json::Value::Object(fields);
            // This module owns 1.x option shapes, not the catalog's external
            // namespace. Match the final, stable upstream rule key after the
            // catalog has validated the full external name.
            let result = if value.as_object().is_some_and(serde_json::Map::is_empty) {
                Ok(())
            } else {
                // Only `valid-use-server` takes an option beyond `enabled` (its
                // `clientWrappers`, parsed above); the other that did was 1.x's
                // `prefer-classlist`, whose `classnames` list went with the 1.x
                // catalog (ADR 0110). The refusal stays because this module's
                // whole point is that a typo must not silently mean
                // "defaults" -- when a rule grows an option, it is this arm
                // that grows a parser.
                Err(if rule == "valid-use-server" {
                    "this rule takes only `enabled` and `clientWrappers`".to_owned()
                } else {
                    "this rule takes only `enabled`; no other rule in this catalog takes additional options"
                        .to_owned()
                })
            };
            result.map_err(|error| format!("rule options for {configured_rule:?}: {error}"))?;
        }
        Ok(options)
    }

    /// The project's declared `valid-use-server` client wrappers.
    #[must_use]
    pub fn client_wrappers(&self) -> &[String] {
        &self.client_wrappers
    }

    pub fn request_presets(&mut self, presets: impl IntoIterator<Item = String>) {
        self.requested_presets.extend(presets);
    }

    pub fn request_rules(&mut self, rules: impl IntoIterator<Item = String>) {
        self.requested_rules.extend(rules);
    }

    /// [`Self::is_enabled`] for a finding of `code`: an override the project set
    /// through a former name limited to that code wins over the rule's.
    #[must_use]
    pub fn is_enabled_for_code(
        &self,
        rule: &str,
        code: &str,
        default_enabled: bool,
        presets: &[&str],
    ) -> bool {
        match self.code_overrides.get(code) {
            Some(RuleOverride::Disabled) => false,
            Some(RuleOverride::Enabled) => true,
            None | Some(RuleOverride::Unset) => self.is_enabled(rule, default_enabled, presets),
        }
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
            "prefer-classlist"
                | "no-destructure"
                | "no-store-mutation-outside-setter"
                | "no-owner-cleanup"
                | "no-owner-settled-cleanup"
                | "strict-read-untracked"
                | "valid-use-server"
        )
    }

    #[test]
    fn an_empty_document_is_upstreams_defaults() {
        let options = RuleOptions::parse(r#"{ "schemaVersion": 1 }"#, known_rule).unwrap();
        assert_eq!(options, RuleOptions::default());
    }

    /// No rule ships an option beyond `enabled`, and a document that tries to
    /// set one is refused rather than ignored.
    #[test]
    fn a_rule_option_other_than_enabled_is_refused() {
        let error = RuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "strict-read-untracked": { "classnames": ["cx"] }
              }
            }"#,
            known_rule,
        )
        .expect_err("an unknown option must not be read as defaults");
        assert!(error.contains("takes only `enabled`"), "{error}");
    }

    #[test]
    fn parses_enablement_for_rules_with_and_without_options() {
        let options = RuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "no-store-mutation-outside-setter": { "enabled": false },
                "no-owner-cleanup": { "enabled": false },
                "strict-read-untracked": { "enabled": true }
              }
            }"#,
            known_rule,
        )
        .unwrap();
        assert!(!options.is_enabled("no-store-mutation-outside-setter", true, &[]));
        assert!(options.is_enabled("strict-read-untracked", true, &[]));
        assert!(options.is_enabled("no-destructure", true, &[]));
        assert!(!options.is_enabled("no-owner-cleanup", true, &[]));
        assert!(options.is_enabled("no-owner-settled-cleanup", true, &[]));
    }

    #[test]
    fn rejects_unknown_rules_keys_and_schema_versions() {
        assert!(RuleOptions::parse(r#"{ "schemaVersion": 2 }"#, known_rule).is_err());
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "not-a-rule": {} } }"#,
                known_rule,
            )
            .is_err()
        );
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "no-destructure": { "severity": "off" } } }"#,
                known_rule,
            )
            .is_err()
        );
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "no-destructure": { "enabled": "no" } } }"#,
                known_rule,
            )
            .is_err()
        );
    }

    /// ADR 0384: a former name limited to one code disables that code alone;
    /// the merged name still governs both, and a code override wins for its own
    /// code.
    #[test]
    fn a_scoped_alias_governs_its_own_code_only() {
        let parse = |document: &str| {
            RuleOptions::parse_with_scoped_aliases(
                document,
                |rule| rule == "merged",
                |rule| matches!(rule, "old-a" | "old-b").then_some("merged"),
                |rule| match rule {
                    "old-a" => Some("SC1"),
                    "old-b" => Some("SC2"),
                    _ => None,
                },
            )
            .unwrap()
        };
        let enabled = |options: &RuleOptions, code: &str| {
            options.is_enabled_for_code("merged", code, true, &[])
        };
        let old_a = parse(r#"{ "schemaVersion": 1, "rules": { "old-a": { "enabled": false } } }"#);
        assert!(!enabled(&old_a, "SC1") && enabled(&old_a, "SC2"));
        assert!(old_a.is_enabled("merged", true, &[]));
        let merged =
            parse(r#"{ "schemaVersion": 1, "rules": { "merged": { "enabled": false } } }"#);
        assert!(!enabled(&merged, "SC1") && !enabled(&merged, "SC2"));
        let both = parse(
            r#"{ "schemaVersion": 1, "rules": {
                 "merged": { "enabled": false }, "old-b": { "enabled": true } } }"#,
        );
        assert!(!enabled(&both, "SC1") && enabled(&both, "SC2"));
    }

    #[test]
    fn aliases_transfer_disable_to_the_canonical_rule() {
        let options = RuleOptions::parse_with_aliases(
            r#"{
              "schemaVersion": 1,
              "rules": { "old-missing-owner": { "enabled": false } }
            }"#,
            |rule| rule == "missing-owner",
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
        assert!(options.is_enabled("prefer-show", true, &["preferences"]));
        assert!(options.is_enabled("strict-read-untracked", true, &[]));

        options.request_presets(["preferences".into()]);
        assert!(options.is_enabled("prefer-show", false, &["preferences"]));

        options.request_rules(["prefer-for".into()]);
        assert!(options.is_enabled("prefer-for", false, &["preferences"]));

        let mut disabled = RuleOptions::parse(
            r#"{ "schemaVersion": 1, "rules": { "prefer-classlist": { "enabled": false } } }"#,
            known_rule,
        )
        .unwrap();
        disabled.request_presets(["preferences".into()]);
        assert!(!disabled.is_enabled("prefer-classlist", true, &["preferences"]));
    }

    #[test]
    fn valid_use_server_takes_client_wrappers_and_nothing_else() {
        let options = RuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "valid-use-server": { "clientWrappers": ["define*", "/^use[A-Z]/"] }
              }
            }"#,
            known_rule,
        )
        .unwrap();
        assert_eq!(options.client_wrappers(), ["define*", "/^use[A-Z]/"]);
        assert!(options.is_enabled("valid-use-server", true, &[]));
        assert!(RuleOptions::default().client_wrappers().is_empty());

        for bad in [
            r#"{ "clientWrappers": "define*" }"#,
            r#"{ "clientWrappers": ["ok", 1] }"#,
            r#"{ "clientWrappers": ["ok"], "wrappers": ["x"] }"#,
        ] {
            let error = RuleOptions::parse(
                &format!(r#"{{ "schemaVersion": 1, "rules": {{ "valid-use-server": {bad} }} }}"#),
                known_rule,
            )
            .expect_err(bad);
            assert!(
                error.contains("clientWrappers must be an array of strings")
                    || error.contains("takes only `enabled` and `clientWrappers`"),
                "{error}"
            );
        }
        // The option belongs to this rule alone.
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "no-destructure": { "clientWrappers": ["x"] } } }"#,
                known_rule,
            )
            .is_err()
        );
    }

    #[test]
    fn the_former_rule_name_carries_the_option_to_the_canonical_rule() {
        let options = RuleOptions::parse_with_aliases(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "server-function-module-directive": { "clientWrappers": ["define*"] }
              }
            }"#,
            |rule| rule == "valid-use-server",
            |rule| (rule == "server-function-module-directive").then_some("valid-use-server"),
        )
        .unwrap();
        assert_eq!(options.client_wrappers(), ["define*"]);
    }
}
