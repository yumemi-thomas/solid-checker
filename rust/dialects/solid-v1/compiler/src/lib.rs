//! The Solid 1.x compiler fact domain: the solid-1x-compiler Oxc compiler
//! adapted to the checker's [`CompilerFactsProvider`] seam.
//!
//! Nothing outside this crate speaks the compiler's own types. This is the
//! Solid 1.x sibling of `solid-v2-compiler`: its own crate wrapping its own
//! compiler behind the same trait, so the pipeline selects a compiler by
//! selecting a dialect and never names either.

use solid_facts::compiler::{
    AnalysisRequest, CompilerFactsProvider, CompilerOperationInput, CompilerOperationKind,
    CompilerProducerInput, CompilerProviderError, CompilerTraceInput, ExecutionMap,
    LegacyCompilerDecision,
};
use solid_facts::core::{SourceHash, Span};
use solid1_dom_expressions_compiler as dom_expressions_compiler;

/// The trace schema version *this projection was written against*.
///
/// It is a consumer-owned literal on purpose. Comparing the trace's `version`
/// against the producer's own `SEMANTIC_TRACE_VERSION` would be tautological —
/// the producer fills the field from that same constant, so the runtime check
/// could never fire, for any producer, including a version-3 one arriving
/// through a pin move.
const READS_TRACE_VERSION: u32 = 2;
const PRODUCER_IMPLEMENTATION_REVISION: &str = "ca3bbfae7d1e00e28ef73f9af58bdb46e248b512";

/// Stable cache identity for the exact legacy semantic producer this adapter
/// reads. Trace v2 has no complete generated-operation identity, which is
/// represented in the normalized model rather than hidden in this string.
pub const COMPILER_FACTS_IDENTITY: &str =
    "solid-v1:trace2:ca3bbfae7d1e00e28ef73f9af58bdb46e248b512";

/// A pin move that changes the producer's schema version fails the build here
/// instead of silently making the runtime refusal below unreachable again. The
/// runtime check catches a trace that disagrees with its own producer; this
/// catches a producer that disagrees with this projection.
const _: () = assert!(dom_expressions_compiler::SEMANTIC_TRACE_VERSION == READS_TRACE_VERSION);

/// The in-process Solid 1.x compiler-facts provider.
#[derive(Default)]
pub struct NativeCompilerFacts;

impl CompilerFactsProvider for NativeCompilerFacts {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        use dom_expressions_compiler::{CompileOptions, Generate, Wrapper, compile};

        let requested = &request.compiler_options;
        // `None` leaves the compiler's own default wrapper in place; an
        // explicitly empty name is how the checker asks for no wrapper at all.
        let effect_wrapper = match requested.effect_wrapper.as_deref() {
            None => Wrapper::Default,
            Some("") => Wrapper::Disabled,
            Some(name) => Wrapper::Name(name.to_owned()),
        };
        let generate = match requested.generate.as_str() {
            "dom" => Generate::Dom,
            other => {
                return Err(CompilerProviderError::Native(format!(
                    "semantic tracing supports DOM output only, not `{other}`"
                )));
            }
        };
        // `CompilerOptions.dev` is deliberately not forwarded: the 1.x Babel
        // compiler treats `dev` as inert and this port matches it, so the
        // option does not exist on the 1.x `CompileOptions` at all.
        let options = CompileOptions {
            filename: Some(request.path.clone()),
            module_name: requested.module_name.clone(),
            generate,
            hydratable: requested.hydratable,
            effect_wrapper,
            wrap_conditionals: requested.wrap_conditionals.unwrap_or(true),
            static_marker: requested
                .static_marker
                .clone()
                .unwrap_or_else(|| CompileOptions::default().static_marker),
            built_ins: requested.built_ins.clone(),
            semantic_trace: true,
            ..CompileOptions::default()
        };
        // Whether the compiler kept its own effect wrapper is what makes an
        // owner claim auditable, and it is knowable only here, from the
        // request: see `execution_map_from_trace`.
        let default_effect_wrapper = matches!(options.effect_wrapper, Wrapper::Default);
        let output = compile(&request.source, &options)
            .map_err(|error| CompilerProviderError::Native(format!("{}: {error}", request.path)))?;
        let trace = output
            .semantic_trace
            .ok_or(CompilerProviderError::MissingExecutionMap)?;
        execution_map_from_trace(
            &trace,
            &request.source,
            default_effect_wrapper,
            output.code,
            output.source_map,
            serde_json::json!({
                "filename": request.path,
                "moduleName": requested.module_name,
                "generate": requested.generate,
                "hydratable": requested.hydratable,
                "effectWrapper": requested.effect_wrapper,
                "wrapConditionals": requested.wrap_conditionals.unwrap_or(true),
                "staticMarker": requested.static_marker,
                "builtIns": requested.built_ins,
            })
            .to_string(),
        )
    }
}

/// Projects the compiler's semantic trace onto the checker's execution map.
///
/// The trace is total *over the census*: every censused JSX site carries a
/// terminal decision, so each one lands in exactly one of the tracked,
/// untracked, discarded, or callback categories, and
/// `ExecutionMap::uncovered_jsx_expressions` is empty by construction rather
/// than by luck. That is not the same as total over the JSX the source
/// contains — this compiler censuses what it lowers, and a nested
/// non-hydratable `<head>` is dropped before it is censused. The resulting hole
/// is handled downstream, in `solid-reactive-ir`'s `missing_jsx_census`; see
/// docs/compiler-facts.md, "Census gaps".
///
/// The current producer is transform-faithful to the Solid 1.x Babel compiler
/// for the audited child-content shapes. Child lists discarded by void and
/// `<noscript>` lowering are censused as one `Elided` range, so deletion is a
/// positive fact rather than compiler silence. A nested non-hydratable `<head>`
/// remains a genuine source/census hole and is handled downstream by
/// `missing_jsx_census`.
///
/// `default_effect_wrapper` reports whether the compile ran under the
/// compiler's own effect wrapper. It is not derivable from the trace, and it
/// is what makes the ownership derivation below auditable.
fn execution_map_from_trace(
    trace: &dom_expressions_compiler::SemanticTrace,
    source: &str,
    default_effect_wrapper: bool,
    output: String,
    source_map: Option<String>,
    configuration_json: String,
) -> Result<ExecutionMap, CompilerProviderError> {
    // The trace schema is versioned and its meaning is not forward-compatible:
    // version 2 removed the producer's `ownership_sites` vocabulary, so a
    // trace of any other version must be refused rather than read as if the
    // fields it does carry mean what this projection assumes. The comparison is
    // against this crate's own `READS_TRACE_VERSION`, never the producer's
    // constant.
    if trace.version != READS_TRACE_VERSION {
        return Err(CompilerProviderError::Native(format!(
            "Solid 1.x compiler produced semantic trace version {}, but this checker reads version {READS_TRACE_VERSION}",
            trace.version
        )));
    }

    let source_hash = SourceHash::of(source);
    let source_sha256 = source_hash
        .as_str()
        .strip_prefix("sha256:")
        .expect("SourceHash is canonical")
        .to_string();
    let map = ExecutionMap::normalize(
        CompilerTraceInput {
            producer: CompilerProducerInput {
                dialect: "solid-v1".into(),
                trace_version: trace.version,
                package_version: None,
                upstream_revision: None,
                implementation_revision: PRODUCER_IMPLEMENTATION_REVISION.into(),
                source_sha256,
                output,
                source_map,
                claimed_output_sha256: None,
                claimed_source_map_sha256: None,
                configuration_json,
                identity_complete: false,
            },
            source_operations_complete: true,
            // Trace v2 did not bind generated wrapper observations to exact
            // generated operation identities. Preserve that as partial rather
            // than turning an empty list into negative proof.
            generated_operations_complete: false,
            operations: trace
                .sites
                .iter()
                .map(|site| {
                    CompilerOperationInput::legacy(
                        Span::new(site.span.start, site.span.end),
                        operation_kind(site.kind),
                        legacy_decision(site.decision),
                        default_effect_wrapper,
                    )
                })
                .collect(),
            generated_operations: Vec::new(),
        },
        source,
    )?;
    if !map.uncovered_jsx_expressions().is_empty() {
        return Err(CompilerProviderError::Native(
            "Solid 1.x compiler trace is incomplete: a JSX expression has no execution classification"
                .into(),
        ));
    }
    Ok(map)
}

fn operation_kind(kind: dom_expressions_compiler::ExecutionSiteKind) -> CompilerOperationKind {
    use dom_expressions_compiler::ExecutionSiteKind;

    match kind {
        ExecutionSiteKind::NativeAttribute => CompilerOperationKind::NativeAttribute,
        ExecutionSiteKind::NativeSpread => CompilerOperationKind::NativeSpread,
        ExecutionSiteKind::ComponentProperty => CompilerOperationKind::ComponentProperty,
        ExecutionSiteKind::ComponentSpread => CompilerOperationKind::ComponentSpread,
        ExecutionSiteKind::ComponentChild => CompilerOperationKind::ComponentChild,
        ExecutionSiteKind::JsxChild => CompilerOperationKind::JsxChild,
        ExecutionSiteKind::EventHandler => CompilerOperationKind::EventHandler,
        ExecutionSiteKind::Ref => CompilerOperationKind::Ref,
        ExecutionSiteKind::ControlFlowRender => CompilerOperationKind::ControlFlowRender,
    }
}

fn legacy_decision(decision: dom_expressions_compiler::TerminalDecision) -> LegacyCompilerDecision {
    use dom_expressions_compiler::{CallbackDecision, TerminalDecision, ValueDecision};
    match decision {
        TerminalDecision::Value(ValueDecision::EagerOnce) => LegacyCompilerDecision::EagerOnce,
        TerminalDecision::Value(ValueDecision::ReactiveRerun) => {
            LegacyCompilerDecision::ReactiveRerun
        }
        TerminalDecision::Value(ValueDecision::CallerContext) => {
            LegacyCompilerDecision::CallerContext
        }
        TerminalDecision::Value(ValueDecision::Elided) => LegacyCompilerDecision::Elided,
        TerminalDecision::Callback(CallbackDecision::LaterEvent) => {
            LegacyCompilerDecision::LaterEvent
        }
        TerminalDecision::Callback(CallbackDecision::LaterRender) => {
            LegacyCompilerDecision::LaterRender
        }
        TerminalDecision::Callback(CallbackDecision::RefApply) => LegacyCompilerDecision::RefApply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::compiler::{CompilerOptions, OwnershipRegionKind};

    fn facts(effect_wrapper: Option<&str>) -> ExecutionMap {
        let request = AnalysisRequest::new(
            "App.tsx",
            "const view = <div>{count()}</div>;",
            CompilerOptions {
                effect_wrapper: effect_wrapper.map(str::to_owned),
                ..CompilerOptions::default()
            },
        );
        NativeCompilerFacts
            .analyze(&request)
            .expect("compiler facts")
    }

    fn compile_source(source: &str) -> ExecutionMap {
        let request = AnalysisRequest::new("App.tsx", source, CompilerOptions::default());
        NativeCompilerFacts
            .analyze(&request)
            .expect("compiler facts")
    }

    // The 1.x twin of the 2.0 adapter's projection test: a value the emitter
    // deletes is a *discarded* region, not an untracked one. An untracked region
    // says the code runs once at render, which would make a reactive read inside
    // a deleted value a proven stale read.
    #[test]
    fn deleted_values_are_discarded_regions_rather_than_untracked_ones() {
        // A `children` attribute shadowed by real source children: dropped
        // during attribute planning, never lowered, and emitted by neither this
        // compiler nor Babel.
        let facts = compile_source("const view = <span children={ignored()}>{visible()}</span>;");
        assert_eq!(facts.discarded_regions.len(), 1, "{facts:#?}");
        assert!(facts.untracked_regions.is_empty(), "{facts:#?}");
        // The live sibling is untouched: deletion is per value, not per element.
        assert_eq!(facts.tracked_regions.len(), 1, "{facts:#?}");
        // Still classified, so the completeness invariant holds and the file is
        // not refused.
        assert!(facts.uncovered_jsx_expressions().is_empty(), "{facts:#?}");
    }

    // The other half of the split: `EagerOnce` really does execute, once, so it
    // stays an untracked region.
    #[test]
    fn one_shot_values_stay_untracked_regions() {
        let facts = compile_source("const view = <Widget value={CONSTANT} />;");
        assert!(facts.discarded_regions.is_empty(), "{facts:#?}");
        assert_eq!(facts.untracked_regions.len(), 1, "{facts:#?}");
    }

    #[test]
    fn default_effect_reruns_supply_owned_regions() {
        let facts = facts(None);
        assert_eq!(facts.ownership_regions.len(), 1);
        assert_eq!(facts.ownership_regions[0].kind, OwnershipRegionKind::Owned);
    }

    #[test]
    fn custom_effect_wrappers_make_no_owner_claim() {
        assert!(facts(Some("customEffect")).ownership_regions.is_empty());
    }

    // The pinned producer only ever emits the version this adapter reads, so
    // the refusal is unreachable through `analyze` and is pinned here instead.
    // It is the whole reason the ownership derivation above is allowed to
    // assume what the other fields mean.
    #[test]
    fn an_unreadable_trace_version_is_refused_rather_than_projected() {
        // Named through the 1.x crate rather than the module's
        // `dom_expressions_compiler` alias so the producer under test is
        // spelled out: this crate's dev-dependencies also carry the 2.0
        // compiler, and reading the 2.0 schema here would be testing a
        // different producer.
        use solid1_dom_expressions_compiler::SemanticTrace;

        let source = "const view = <div>{count()}</div>;";
        // A literal version, not `READS_TRACE_VERSION + 1`: the point is that
        // some *specific* other schema is refused, and a version derived from
        // the constant under test would move with it.
        //
        // Every field is named rather than filled from `..default()`, so a
        // producer that adds one fails this build instead of quietly widening
        // the schema this projection claims to read.
        let trace = SemanticTrace {
            version: 3,
            sites: Vec::new(),
            owner_establishments: Vec::new(),
            component_render_sites: Vec::new(),
            deferred_callback_sites: Vec::new(),
        };
        let error =
            execution_map_from_trace(&trace, source, true, String::new(), None, "{}".into())
                .expect_err("an unknown trace version must fail closed");
        assert!(
            matches!(&error, CompilerProviderError::Native(message)
                if message.contains("semantic trace version 3")
                    && message.contains("reads version 2")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn component_operation_kinds_preserve_property_and_child_roles() {
        let request = AnalysisRequest::new(
            "App.tsx",
            "const view = <Route component={Home} {...props}>{() => <Home />}</Route>;",
            CompilerOptions::default(),
        );
        let facts = NativeCompilerFacts.analyze(&request).unwrap();
        assert!(
            facts
                .jsx_operations
                .iter()
                .any(|operation| operation.kind == "component-property")
        );
        assert!(
            facts
                .jsx_operations
                .iter()
                .any(|operation| operation.kind == "component-child")
        );
        assert!(
            facts
                .jsx_operations
                .iter()
                .any(|operation| operation.kind == "component-spread")
        );
    }

    #[test]
    fn legacy_trace_keeps_generated_operations_explicitly_partial() {
        let facts = compile_source("const view = <div>{count()}</div>;");
        let producer = facts
            .semantic_model
            .producer
            .as_ref()
            .expect("legacy producer identity");
        assert!(!producer.identity_complete);
        assert!(facts.semantic_model.source_operations_complete);
        assert!(!facts.semantic_model.generated_operations_complete);
        assert!(facts.semantic_model.generated_operations.is_empty());
    }
}
