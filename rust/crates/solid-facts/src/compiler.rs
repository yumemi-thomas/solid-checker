//! Typed, validated Solid compiler execution facts.
//!
//! The controlled Oxc Solid compiler emits this model from original source.
//! This crate deliberately contains no compiler implementation and no AST:
//! it is the stable boundary consumed by the future Rust analysis engine.

use crate::core::{SourceHash, Span};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// The compiler-facts wire version. [`ExecutionMap::validate`] refuses a map
/// that does not carry exactly this value.
///
/// It stayed at 1 when [`ExecutionMap::discarded_regions`] was added, and that
/// is safe only because of who fills the map in: every
/// [`CompilerFactsProvider`] is in-process and compiled from this same
/// workspace, so a map is never deserialized from a producer built against an
/// older definition. The field's `#[serde(default)]` is therefore only ever
/// exercised by this repository's own fixtures and caches, where an absent
/// array and an empty one mean the same thing. What would make it unsafe is a
/// provider on the other side of a process or package boundary: an older
/// producer would omit `discardedRegions` and its deletions would silently
/// arrive as *nothing at all*, which reads as "no code was deleted" rather
/// than as "this producer cannot say" — and a missing deletion fact turns a
/// deleted value back into an ordinary uncensused hole or, worse, an untracked
/// region. Adding such a boundary means bumping this constant, not adding
/// another defaulted field.
pub const COMPILER_FACTS_PROTOCOL: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerOptions {
    pub module_name: String,
    pub generate: String,
    #[serde(default)]
    pub hydratable: bool,
    #[serde(default)]
    pub dev: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_wrapper: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_conditionals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_marker: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub built_ins: Vec<String>,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            module_name: "dom".into(),
            generate: "dom".into(),
            hydratable: false,
            dev: false,
            effect_wrapper: None,
            wrap_conditionals: None,
            static_marker: None,
            built_ins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisRequest {
    pub compiler_facts_protocol: u32,
    pub path: String,
    pub source: Arc<str>,
    pub source_hash: SourceHash,
    pub compiler_options: CompilerOptions,
}

impl AnalysisRequest {
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        source: impl Into<Arc<str>>,
        mut options: CompilerOptions,
    ) -> Self {
        let source = source.into();
        options.built_ins.sort();
        options.built_ins.dedup();
        Self {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            path: path.into(),
            source_hash: SourceHash::of(source.as_ref()),
            source,
            compiler_options: options,
        }
    }
}

/// The checker's view of a Solid JSX compiler.
///
/// Each Solid dialect supplies its own adapter: the Solid 2.0 dialect wraps
/// the dom-expressions Oxc compiler, and a future Solid 1.x dialect would
/// wrap its own compiler the same way. Infrastructure only ever sees this
/// trait, so swapping compilers never touches the analysis pipeline.
pub trait CompilerFactsProvider {
    fn analyze(&mut self, request: &AnalysisRequest)
    -> Result<ExecutionMap, CompilerProviderError>;
}

/// What a [`CompilerFactsProvider`] can fail with, kept free of any single
/// compiler's error types so adapters stay interchangeable.
#[derive(Debug, Error)]
pub enum CompilerProviderError {
    #[error("the Solid compiler returned no semantic trace")]
    MissingExecutionMap,
    #[error("native Solid compiler facts error: {0}")]
    Native(String),
    #[error("compiler facts error: {0}")]
    Facts(#[from] CompilerFactsError),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarResponse {
    pub ok: bool,
    #[serde(default)]
    pub execution_map: Option<ExecutionMap>,
    #[serde(default)]
    pub measurement: Option<Measurement>,
    #[serde(default)]
    pub error: Option<SidecarError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Measurement {
    pub computation_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionMap {
    pub compiler_facts_protocol: u32,
    pub source_hash: SourceHash,
    #[serde(default)]
    pub tracked_regions: Vec<ExecutionRegion>,
    #[serde(default)]
    pub untracked_regions: Vec<ExecutionRegion>,
    /// Regions the compiler **deleted**: the value is censused, decided, and
    /// then emitted nowhere.
    ///
    /// Deliberately not part of [`Self::untracked_regions`], which is a claim
    /// about code that *executes* — once, at render, outside any tracking
    /// scope. A discarded region executes zero times, so every claim an
    /// untracked region licenses is false of it: the read does not see a stale
    /// value, the write does not run in the render phase, the callback is never
    /// attached. It is equally not a hole (see `missing_jsx_census` in
    /// solid-reactive-ir): the compiler reported on this JSX and said the code
    /// is gone.
    ///
    /// A discarded region proves no *positive* claim either. Nothing here is
    /// evidence that a reader is satisfied, that an owner was established, or
    /// that a value settles — dead code establishes nothing.
    #[serde(default)]
    pub discarded_regions: Vec<ExecutionRegion>,
    #[serde(default)]
    pub ownership_regions: Vec<OwnershipRegion>,
    #[serde(default)]
    pub callback_roles: Vec<CallbackRole>,
    #[serde(default)]
    pub jsx_operations: Vec<JsxOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRegion {
    pub span: Span,
    pub reason: RegionReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionReason {
    JsxChild,
    JsxAttribute,
    ComponentGetter,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnershipRegion {
    pub span: Span,
    pub kind: OwnershipRegionKind,
}

/// The owner state the compiler proves for a source region.
///
/// Known states are closed and typed. An older or newer protocol-1 adapter may
/// still send an unfamiliar string; it is retained as `Unknown` and supplies
/// no owner proof rather than making the whole fact payload unreadable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnershipRegionKind {
    Owned,
    Unowned,
    Leaf,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallbackRole {
    pub span: Span,
    pub role: CallbackRoleKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallbackRoleKind {
    EventHandler,
    Render,
    Deferred,
    DirectiveApply,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsxOperation {
    pub span: Span,
    pub kind: String,
}

#[derive(Debug, Error)]
pub enum CompilerFactsError {
    #[error("invalid compiler facts JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("compiler facts protocol {0} is unsupported")]
    Protocol(u32),
    #[error("compiler source hash {actual} does not match {expected}")]
    SourceHash {
        expected: SourceHash,
        actual: SourceHash,
    },
    #[error("invalid {category} span at index {index}: {source}")]
    Span {
        category: &'static str,
        index: usize,
        #[source]
        source: crate::core::FactIdentityError,
    },
    #[error("{category} facts are not in canonical order at index {index}")]
    Order {
        category: &'static str,
        index: usize,
    },
    #[error("JSX operation kind is empty at index {0}")]
    EmptyOperationKind(usize),
}

impl ExecutionMap {
    pub fn from_json(encoded: &str, source: &str) -> Result<Self, CompilerFactsError> {
        let facts: Self = serde_json::from_str(encoded)?;
        facts.validate(source)?;
        Ok(facts)
    }

    pub fn validate(&self, source: &str) -> Result<(), CompilerFactsError> {
        if self.compiler_facts_protocol != COMPILER_FACTS_PROTOCOL {
            return Err(CompilerFactsError::Protocol(self.compiler_facts_protocol));
        }
        let expected = SourceHash::of(source);
        if self.source_hash != expected {
            return Err(CompilerFactsError::SourceHash {
                expected,
                actual: self.source_hash.clone(),
            });
        }
        validate_spanned(
            "tracked regions",
            &self.tracked_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "untracked regions",
            &self.untracked_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "discarded regions",
            &self.discarded_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "ownership regions",
            &self.ownership_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "callback roles",
            &self.callback_roles,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "JSX operations",
            &self.jsx_operations,
            source.len(),
            |value| value.span,
        )?;
        if let Some(index) = self
            .jsx_operations
            .iter()
            .position(|operation| operation.kind.trim().is_empty())
        {
            return Err(CompilerFactsError::EmptyOperationKind(index));
        }
        Ok(())
    }

    /// Whether some region or role fact decides `candidate`.
    ///
    /// A discarded region counts: "the compiler deleted this" is a decision
    /// about the site, not an absence of one. Leaving it out would make
    /// [`Self::uncovered_jsx_expressions`] report every deleted value as an
    /// unclassified JSX expression, which the dialect adapters turn into a hard
    /// refusal of the whole file.
    #[must_use]
    pub fn classifies(&self, candidate: Span) -> bool {
        self.tracked_regions
            .iter()
            .any(|fact| fact.span.contains(candidate))
            || self
                .untracked_regions
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self
                .discarded_regions
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self
                .callback_roles
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self.jsx_operations.iter().any(|fact| {
                matches!(
                    fact.kind.as_str(),
                    "component-property" | "component-spread" | "component-child"
                ) && fact.span.contains(candidate)
            })
    }

    #[must_use]
    pub fn uncovered_jsx_expressions(&self) -> Vec<Span> {
        self.jsx_operations
            .iter()
            .filter(|operation| {
                operation.kind == "jsx-expression" && !self.classifies(operation.span)
            })
            .map(|operation| operation.span)
            .collect()
    }

    #[must_use]
    pub fn seed_spans(&self) -> Vec<Span> {
        let mut spans = self
            .callback_roles
            .iter()
            .map(|fact| fact.span)
            .chain(self.jsx_operations.iter().map(|fact| fact.span))
            .collect::<Vec<_>>();
        spans.sort_unstable();
        spans.dedup();
        spans
    }
}

fn validate_spanned<T>(
    category: &'static str,
    values: &[T],
    source_len: usize,
    get_span: impl Fn(&T) -> Span,
) -> Result<(), CompilerFactsError> {
    for (index, value) in values.iter().enumerate() {
        let current = get_span(value);
        current
            .validate(source_len)
            .map_err(|source| CompilerFactsError::Span {
                category,
                index,
                source,
            })?;
        if index > 0 && get_span(&values[index - 1]) > current {
            return Err(CompilerFactsError::Order { category, index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(source: &str) -> String {
        format!(
            r#"{{"compilerFactsProtocol":1,"sourceHash":"{}","trackedRegions":[{{"span":{{"start":0,"end":5}},"reason":"jsx-child"}}],"untrackedRegions":[],"ownershipRegions":[],"callbackRoles":[],"jsxOperations":[{{"span":{{"start":0,"end":5}},"kind":"jsx-expression"}}]}}"#,
            SourceHash::of(source)
        )
    }

    #[test]
    fn validates_execution_map_and_completeness() {
        let source = "value";
        let facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
        assert_eq!(facts.seed_spans(), vec![Span::new(0, 5)]);
    }

    #[test]
    fn exposes_unclassified_jsx_as_a_fail_closed_obligation() {
        let source = "value";
        let mut facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
        facts.tracked_regions.clear();
        facts.validate(source).unwrap();
        assert_eq!(facts.uncovered_jsx_expressions(), vec![Span::new(0, 5)]);
    }

    #[test]
    fn all_component_value_operations_cover_jsx_expressions() {
        for kind in ["component-property", "component-spread", "component-child"] {
            let source = "value";
            let encoded = encoded(source).replace("jsx-expression", kind);
            let mut facts = ExecutionMap::from_json(&encoded, source).unwrap();
            facts.tracked_regions.clear();
            assert!(facts.classifies(Span::new(1, 2)), "kind {kind}");
        }
    }

    #[test]
    fn unfamiliar_protocol_one_ownership_kinds_remain_non_proofs() {
        let source = "value";
        let encoded = encoded(source).replace(
            r#""ownershipRegions":[]"#,
            r#""ownershipRegions":[{"span":{"start":0,"end":5},"kind":"future-state"}]"#,
        );
        let facts = ExecutionMap::from_json(&encoded, source).unwrap();
        assert_eq!(
            facts.ownership_regions[0].kind,
            OwnershipRegionKind::Unknown
        );
    }

    #[test]
    fn rejects_stale_source() {
        assert!(matches!(
            ExecutionMap::from_json(&encoded("value"), "other"),
            Err(CompilerFactsError::SourceHash { .. })
        ));
    }
}
