//! Project-wide rule configuration and Solid 1.x rule options.
//!
//! eslint-plugin-solid ships user-configurable options on several of the
//! rules this pass reproduces. The checker carries exactly the options whose
//! behaviour upstream's own test corpus exercises — every field here is
//! proven by a `fixtures/upstream-parity` case — and nothing speculative.
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
//! [`Solid1xRuleOptions::parse`] rejects unknown rule names, unknown option keys,
//! and unsupported schema versions rather than ignoring them: a typo in a
//! config must not silently mean "defaults".

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// Options for `v1/event-handlers` (SC8001).
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct EventHandlersOptions {
    /// Upstream's `ignoreCase`: treat handler names case-insensitively and
    /// stop suggesting canonical spellings, so `onclick` and `only` are
    /// accepted as written.
    pub ignore_case: bool,
    /// Upstream's `warnOnSpread`: report handler-named properties carried
    /// into a DOM element through a JSX spread, which Solid does not attach.
    pub warn_on_spread: bool,
}

/// Options for `v1/no-innerhtml` (SC8008).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct NoInnerhtmlOptions {
    /// Upstream's `allowStatic` (default `true`): accept a provably-static
    /// HTML string. With `false`, every `innerHTML` value is reported.
    pub allow_static: bool,
}

impl Default for NoInnerhtmlOptions {
    fn default() -> Self {
        Self { allow_static: true }
    }
}

/// Which empty elements of one category `v1/self-closing-comp` (SC8016)
/// wants self-closed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SelfClosePolicy {
    /// Every childless element of the category must self-close.
    #[default]
    All,
    /// Only void HTML elements (`br`, `img`, ...) must self-close; other
    /// childless elements must not. Only meaningful for the `html` category.
    Void,
    /// No element of the category may self-close.
    None,
}

/// Options for `v1/self-closing-comp` (SC8016).
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct SelfClosingCompOptions {
    /// Upstream's `component`: policy for components. `void` is not a
    /// meaningful component policy and is treated as `all`, as upstream's
    /// schema forbids it outright.
    pub component: SelfClosePolicy,
    /// Upstream's `html`: policy for native elements.
    pub html: SelfClosePolicy,
}

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

/// Options for `v1/style-prop` (SC8017).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct StylePropOptions {
    /// Upstream's `styleProps`: the prop names the rule inspects.
    pub style_props: Vec<String>,
    /// Upstream's `allowString`: accept string-valued style props instead of
    /// asking for an object.
    pub allow_string: bool,
}

impl Default for StylePropOptions {
    fn default() -> Self {
        Self {
            style_props: vec!["style".to_owned()],
            allow_string: false,
        }
    }
}

/// Options for `v1/no-unknown-namespaces` (SC8012).
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct NoUnknownNamespacesOptions {
    /// Upstream's `allowedNamespaces`: extra namespace prefixes to accept on
    /// top of the dialect's own vocabulary.
    pub allowed_namespaces: Vec<String>,
}

/// Project-wide rule enablement plus the per-rule options the checker carries.
///
/// Constructed from `.solid-checker/rule-options.json` by [`Self::parse`],
/// or defaulted when the project has none. Part of the build identity: two
/// runs with different options never share a retained program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Solid1xRuleOptions {
    disabled: BTreeSet<String>,
    pub event_handlers: EventHandlersOptions,
    pub no_innerhtml: NoInnerhtmlOptions,
    pub self_closing_comp: SelfClosingCompOptions,
    pub prefer_classlist: PreferClasslistOptions,
    pub style_prop: StylePropOptions,
    pub no_unknown_namespaces: NoUnknownNamespacesOptions,
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

impl Solid1xRuleOptions {
    /// Parses a rule-options document, failing closed on anything it does
    /// not understand. `has_rule` keeps the dialect catalogs as the source of
    /// truth for names instead of duplicating their identity tables here.
    pub fn parse(encoded: &str, has_rule: impl Fn(&str) -> bool) -> Result<Self, String> {
        let document: Document = serde_json::from_str(encoded)
            .map_err(|error| format!("rule options are not a valid document: {error}"))?;
        if document.schema_version != 1 {
            return Err(format!(
                "rule options schema version {} is unsupported; this checker reads version 1",
                document.schema_version
            ));
        }
        let mut options = Self::default();
        for (rule, value) in document.rules {
            if !has_rule(&rule) {
                return Err(format!("the rule catalog has no rule named {rule:?}"));
            }
            let mut fields = value
                .as_object()
                .cloned()
                .ok_or_else(|| format!("rule options for {rule:?} must be a JSON object"))?;
            let enabled = fields.remove("enabled").map_or(Ok(true), |value| {
                value
                    .as_bool()
                    .ok_or_else(|| format!("rule options for {rule:?}: enabled must be a boolean"))
            })?;
            if !enabled {
                options.disabled.insert(rule.clone());
            }
            let value = serde_json::Value::Object(fields);
            // This module owns 1.x option shapes, not the catalog's external
            // namespace. Match the final, stable upstream rule key after the
            // catalog has validated the full external name.
            let local_rule = rule.rsplit('/').next().unwrap_or(&rule);
            let result = match local_rule {
                "event-handlers" => serde_json::from_value(value)
                    .map(|parsed| options.event_handlers = parsed)
                    .map_err(|error| error.to_string()),
                "no-innerhtml" => serde_json::from_value(value)
                    .map(|parsed| options.no_innerhtml = parsed)
                    .map_err(|error| error.to_string()),
                "self-closing-comp" => serde_json::from_value(value)
                    .map(|parsed| options.self_closing_comp = parsed)
                    .map_err(|error| error.to_string()),
                "prefer-classlist" => serde_json::from_value(value)
                    .map(|parsed| options.prefer_classlist = parsed)
                    .map_err(|error| error.to_string()),
                "style-prop" => serde_json::from_value(value)
                    .map(|parsed| options.style_prop = parsed)
                    .map_err(|error| error.to_string()),
                "no-unknown-namespaces" => serde_json::from_value(value)
                    .map(|parsed| options.no_unknown_namespaces = parsed)
                    .map_err(|error| error.to_string()),
                _ if value.as_object().is_some_and(serde_json::Map::is_empty) => Ok(()),
                _ => Err(
                    "this rule takes only `enabled`; the rules with additional options are \
                     event-handlers, no-innerhtml, self-closing-comp, prefer-classlist, \
                     style-prop, and no-unknown-namespaces"
                        .into(),
                ),
            };
            result.map_err(|error| format!("rule options for {rule:?}: {error}"))?;
        }
        Ok(options)
    }

    /// Whether a catalog finding participates in certification. Rules are
    /// enabled unless the project document explicitly disables their exact
    /// external name.
    #[must_use]
    pub fn is_enabled(&self, rule: &str) -> bool {
        !self.disabled.contains(rule)
    }
}

#[cfg(test)]
mod tests {
    use super::{SelfClosePolicy, Solid1xRuleOptions};

    fn known_rule(rule: &str) -> bool {
        matches!(
            rule,
            "v1/event-handlers"
                | "v1/no-innerhtml"
                | "v1/self-closing-comp"
                | "v1/prefer-classlist"
                | "v1/style-prop"
                | "v1/no-unknown-namespaces"
                | "v1/no-destructure"
                | "v1/no-array-handlers"
                | "invalid-refresh-target"
                | "invalid-affects-target"
                | "strict-read-untracked"
        )
    }

    #[test]
    fn an_empty_document_is_upstreams_defaults() {
        let options = Solid1xRuleOptions::parse(r#"{ "schemaVersion": 1 }"#, known_rule).unwrap();
        assert_eq!(options, Solid1xRuleOptions::default());
        assert!(options.no_innerhtml.allow_static);
        assert!(!options.event_handlers.ignore_case);
        assert!(!options.event_handlers.warn_on_spread);
        assert_eq!(options.self_closing_comp.html, SelfClosePolicy::All);
        assert_eq!(
            options.prefer_classlist.classnames,
            ["cn", "clsx", "classnames"]
        );
        assert_eq!(options.style_prop.style_props, ["style"]);
        assert!(!options.style_prop.allow_string);
        assert!(options.no_unknown_namespaces.allowed_namespaces.is_empty());
    }

    #[test]
    fn parses_every_configurable_rule() {
        let options = Solid1xRuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "v1/event-handlers": { "ignoreCase": true, "warnOnSpread": true },
                "v1/no-innerhtml": { "allowStatic": false },
                "v1/self-closing-comp": { "component": "none", "html": "void" },
                "v1/prefer-classlist": { "classnames": ["cx"] },
                "v1/style-prop": { "styleProps": ["css"], "allowString": true },
                "v1/no-unknown-namespaces": { "allowedNamespaces": ["foo"] }
              }
            }"#,
            known_rule,
        )
        .unwrap();
        assert!(options.event_handlers.ignore_case);
        assert!(options.event_handlers.warn_on_spread);
        assert!(!options.no_innerhtml.allow_static);
        assert_eq!(options.self_closing_comp.component, SelfClosePolicy::None);
        assert_eq!(options.self_closing_comp.html, SelfClosePolicy::Void);
        assert_eq!(options.prefer_classlist.classnames, ["cx"]);
        assert_eq!(options.style_prop.style_props, ["css"]);
        assert!(options.style_prop.allow_string);
        assert_eq!(options.no_unknown_namespaces.allowed_namespaces, ["foo"]);
    }

    #[test]
    fn configurable_shapes_do_not_own_the_catalog_namespace() {
        let options = Solid1xRuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "legacy/no-innerhtml": { "allowStatic": false }
              }
            }"#,
            |rule| rule == "legacy/no-innerhtml",
        )
        .unwrap();

        assert!(!options.no_innerhtml.allow_static);
    }

    #[test]
    fn parses_enablement_for_rules_with_and_without_options() {
        let options = Solid1xRuleOptions::parse(
            r#"{
              "schemaVersion": 1,
              "rules": {
                "v1/no-array-handlers": { "enabled": false },
                "v1/no-innerhtml": { "enabled": false, "allowStatic": false },
                "invalid-refresh-target": { "enabled": false },
                "strict-read-untracked": { "enabled": true }
              }
            }"#,
            known_rule,
        )
        .unwrap();
        assert!(!options.is_enabled("v1/no-array-handlers"));
        assert!(!options.is_enabled("v1/no-innerhtml"));
        assert!(options.is_enabled("strict-read-untracked"));
        assert!(options.is_enabled("v1/no-destructure"));
        assert!(!options.is_enabled("invalid-refresh-target"));
        assert!(options.is_enabled("invalid-affects-target"));
        assert!(!options.no_innerhtml.allow_static);
    }

    #[test]
    fn rejects_unknown_rules_keys_and_schema_versions() {
        assert!(Solid1xRuleOptions::parse(r#"{ "schemaVersion": 2 }"#, known_rule).is_err());
        assert!(
            Solid1xRuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/not-a-rule": {} } }"#,
                known_rule,
            )
            .is_err()
        );
        assert!(
            Solid1xRuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-innerhtml": { "allowStatick": false } } }"#,
                known_rule,
            )
            .is_err()
        );
        assert!(
            Solid1xRuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-destructure": { "severity": "off" } } }"#,
                known_rule,
            )
            .is_err()
        );
        assert!(
            Solid1xRuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-destructure": { "enabled": "no" } } }"#,
                known_rule,
            )
            .is_err()
        );
    }
}
