//! Per-rule options for the Solid 1.x ESLint-surface rules.
//!
//! eslint-plugin-solid ships user-configurable options on several of the
//! rules this pass reproduces. The checker carries exactly the options whose
//! behaviour upstream's own test corpus exercises — every field here is
//! proven by a `fixtures/upstream-parity` case — and nothing speculative.
//! Defaults are upstream's defaults, so an absent options file reproduces
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

use std::collections::BTreeMap;

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

/// Every per-rule option the checker carries, with upstream's defaults.
///
/// Constructed from `.solid-checker/rule-options.json` by [`Self::parse`],
/// or defaulted when the project has none. Part of the build identity: two
/// runs with different options never share a retained program.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleOptions {
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

impl RuleOptions {
    /// Parses a rule-options document, failing closed on anything it does
    /// not understand.
    pub fn parse(encoded: &str) -> Result<Self, String> {
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
            let result = match rule.as_str() {
                "v1/event-handlers" => serde_json::from_value(value)
                    .map(|parsed| options.event_handlers = parsed)
                    .map_err(|error| error.to_string()),
                "v1/no-innerhtml" => serde_json::from_value(value)
                    .map(|parsed| options.no_innerhtml = parsed)
                    .map_err(|error| error.to_string()),
                "v1/self-closing-comp" => serde_json::from_value(value)
                    .map(|parsed| options.self_closing_comp = parsed)
                    .map_err(|error| error.to_string()),
                "v1/prefer-classlist" => serde_json::from_value(value)
                    .map(|parsed| options.prefer_classlist = parsed)
                    .map_err(|error| error.to_string()),
                "v1/style-prop" => serde_json::from_value(value)
                    .map(|parsed| options.style_prop = parsed)
                    .map_err(|error| error.to_string()),
                "v1/no-unknown-namespaces" => serde_json::from_value(value)
                    .map(|parsed| options.no_unknown_namespaces = parsed)
                    .map_err(|error| error.to_string()),
                unknown => Err(format!(
                    "no rule named {unknown:?} takes options; the configurable rules are \
                     v1/event-handlers, v1/no-innerhtml, v1/self-closing-comp, \
                     v1/prefer-classlist, v1/style-prop, and v1/no-unknown-namespaces"
                )),
            };
            result.map_err(|error| format!("rule options for {rule:?}: {error}"))?;
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::{RuleOptions, SelfClosePolicy};

    #[test]
    fn an_empty_document_is_upstreams_defaults() {
        let options = RuleOptions::parse(r#"{ "schemaVersion": 1 }"#).unwrap();
        assert_eq!(options, RuleOptions::default());
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
        let options = RuleOptions::parse(
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
    fn rejects_unknown_rules_keys_and_schema_versions() {
        assert!(RuleOptions::parse(r#"{ "schemaVersion": 2 }"#).is_err());
        assert!(
            RuleOptions::parse(r#"{ "schemaVersion": 1, "rules": { "v1/no-destructure": {} } }"#)
                .is_err()
        );
        assert!(
            RuleOptions::parse(
                r#"{ "schemaVersion": 1, "rules": { "v1/no-innerhtml": { "allowStatick": false } } }"#
            )
            .is_err()
        );
    }
}
