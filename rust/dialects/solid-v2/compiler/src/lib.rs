//! The Solid 2.0 compiler fact domain: the Solid Oxc compiler
//! adapted to the checker's [`CompilerFactsProvider`] seam.
//!
//! Nothing outside this crate speaks the compiler's own types. A Solid 1.x
//! dialect plugs in the same way: its own crate wrapping its own compiler
//! behind the same trait.

use solid_facts::compiler::{
    AnalysisRequest, CompilerExecutionCardinality, CompilerExecutionDisposition,
    CompilerExecutionSchedule, CompilerExecutionSemantics, CompilerExecutionTrigger,
    CompilerFactsProvider, CompilerOperationEvidence, CompilerOperationInput,
    CompilerOperationKind, CompilerOwnerRelation, CompilerProducerInput, CompilerProviderError,
    CompilerTraceInput, CompilerTrackingRelation, ExecutionMap, GeneratedCompilerOperation,
    GeneratedCompilerOperationKind,
};
use solid_facts::core::Span;

/// The trace schema version *this projection was written against*.
///
/// It is a consumer-owned literal on purpose. Comparing the trace's `version`
/// against the producer's own `SEMANTIC_TRACE_VERSION` would be tautological —
/// the producer fills the field from that same constant, so the runtime check
/// could never fire, for any producer, including a version-3 one arriving
/// through a pin move.
const READS_TRACE_VERSION: u32 = 3;

/// Consumer-owned provenance literals. These deliberately do not reuse the
/// producer constants: otherwise a typo in the producer could certify itself.
const EXPECTED_COMPILER_UPSTREAM_REVISION: &str = "a10cf1a147209d8da50697896742d2b1d4afad75";
const EXPECTED_COMPILER_IMPLEMENTATION_REVISION: &str = "7f4e1135943c1fb01231d1bda707b4a1856a5607";
pub const COMPILER_DISTRIBUTION_REVISION: &str = "9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4";

/// Stable cache identity for the exact semantic producer this adapter reads.
/// Keep this synchronized with the pinned compiler revision and trace
/// implementation when either changes.
pub const COMPILER_FACTS_IDENTITY: &str =
    "solid-v2:trace3:7f4e1135943c1fb01231d1bda707b4a1856a5607";

/// Digest of the exact Git-owned semantic compiler source identity consumed by
/// this adapter. The independent identity gate recomputes this value from the
/// upstream, implementation, distribution, trace, and protocol pins.
pub const COMPILER_SOURCE_MANIFEST_SHA256: &str =
    "sha256:613049ba60fa514c662bd9350adb4b0ed9c3031e4f80f2bd1ecb23d56846fde0";

/// A pin move that changes the producer's schema version fails the build here
/// instead of silently making the runtime refusal below unreachable again. The
/// runtime check catches a trace that disagrees with its own producer; this
/// catches a producer that disagrees with this projection.
const _: () = assert!(solidjs_compiler::SEMANTIC_TRACE_VERSION == READS_TRACE_VERSION);

/// The in-process Solid 2.0 compiler-facts provider.
#[derive(Default)]
pub struct NativeCompilerFacts;

impl CompilerFactsProvider for NativeCompilerFacts {
    fn analyze(
        &mut self,
        request: &AnalysisRequest,
    ) -> Result<ExecutionMap, CompilerProviderError> {
        analyze_traced(request).map(|compilation| compilation.execution_map)
    }
}

/// One compiler run retained for policy-2 certification.
///
/// This is deliberately ordinary data, not proof authority. The backend only
/// treats it as evidence after obtaining it from its directly launched private
/// compiler session and wrapping that response in a non-serializable token.
pub struct MaterializedCompilation {
    pub execution_map: ExecutionMap,
    pub output: String,
    pub source_map: Option<String>,
}

/// Compiles the exact request twice and requires semantic tracing to be output
/// neutral before returning the trace-bearing run.
///
/// The compiler fork stays semantic-facts-only: this is a consumer-side replay
/// of the existing trace-on/trace-off invariant and does not add or alter a
/// lowering hook.
pub fn analyze_with_materialized_output(
    request: &AnalysisRequest,
) -> Result<MaterializedCompilation, CompilerProviderError> {
    use solidjs_compiler::compile;

    let traced = analyze_traced(request)?;
    let untraced = compile(&request.source, &compile_options(request, false)?)
        .map_err(|error| CompilerProviderError::Native(format!("{}: {error}", request.path)))?;
    if traced.output != untraced.code || traced.source_map != untraced.source_map {
        return Err(CompilerProviderError::Native(
            "semantic tracing changed generated output or source-map bytes".into(),
        ));
    }
    if untraced.semantic_trace.is_some() {
        return Err(CompilerProviderError::Native(
            "trace-disabled compiler run unexpectedly returned a semantic trace".into(),
        ));
    }
    Ok(traced)
}

fn analyze_traced(
    request: &AnalysisRequest,
) -> Result<MaterializedCompilation, CompilerProviderError> {
    use solidjs_compiler::compile;

    let traced = compile(&request.source, &compile_options(request, true)?)
        .map_err(|error| CompilerProviderError::Native(format!("{}: {error}", request.path)))?;
    let trace = traced
        .semantic_trace
        .ok_or(CompilerProviderError::MissingExecutionMap)?;
    let output = traced.code;
    let source_map = traced.source_map;
    let execution_map =
        execution_map_from_trace(&trace, &request.source, output.clone(), source_map.clone())?;
    Ok(MaterializedCompilation {
        execution_map,
        output,
        source_map,
    })
}

fn compile_options(
    request: &AnalysisRequest,
    semantic_trace: bool,
) -> Result<solidjs_compiler::CompileOptions, CompilerProviderError> {
    use solidjs_compiler::{CompileOptions, Generate, Wrapper};

    let requested = &request.compiler_options;
    // `None` leaves the compiler's own default wrapper in place; an explicitly
    // empty name is how the checker asks for no wrapper at all.
    let effect_wrapper = match requested.effect_wrapper.as_deref() {
        None => Wrapper::Default,
        Some("") => Wrapper::Disabled,
        Some(name) => Wrapper::Name(name.to_owned()),
    };
    let generate = match requested.generate.as_str() {
        "dom" => Generate::Dom,
        "ssr" => Generate::Ssr,
        other => {
            return Err(CompilerProviderError::Native(format!(
                "semantic tracing supports DOM or SSR output only, not `{other}`"
            )));
        }
    };
    Ok(CompileOptions {
        filename: Some(request.path.clone()),
        module_name: requested.module_name.clone(),
        generate,
        hydratable: requested.hydratable,
        dev: requested.dev,
        effect_wrapper,
        wrap_conditionals: requested.wrap_conditionals.unwrap_or(true),
        static_marker: requested
            .static_marker
            .clone()
            .unwrap_or_else(|| CompileOptions::default().static_marker),
        built_ins: requested.built_ins.clone(),
        semantic_trace,
        ..CompileOptions::default()
    })
}

/// Projects the compiler's semantic trace onto the checker's execution map.
///
/// The trace is total *over the census*: every censused JSX site carries a
/// terminal decision, so each one lands in exactly one of the tracked,
/// untracked, discarded, or callback categories, and
/// `ExecutionMap::uncovered_jsx_expressions` is empty by construction rather
/// than by luck. That is not the same as total over the JSX the source
/// contains — this compiler censuses what it lowers. Two consequences are
/// documented in docs/compiler-facts.md:
///
/// - A source expression this compiler lowers and then retracts without an
///   `Elided` decision (the textarea `value` fold or inert `<noscript>` fast
///   path) leaves a hole handled by `solid-reactive-ir`'s
///   `missing_jsx_census`.
/// - Ryan's `next` transform semantics are authoritative for Solid 2. Nested
///   native void children remain live and are reported as tracked; a
///   template-root void child list is discarded and reported as one `Elided`
///   range. The checker consumes both facts without a Babel-parity mitigation.
fn execution_map_from_trace(
    trace: &solidjs_compiler::SemanticTrace,
    source: &str,
    output: String,
    source_map: Option<String>,
) -> Result<ExecutionMap, CompilerProviderError> {
    if trace.version != READS_TRACE_VERSION {
        return Err(CompilerProviderError::Native(format!(
            "Solid 2.0 compiler produced semantic trace version {}, but this checker reads version {READS_TRACE_VERSION}",
            trace.version
        )));
    }
    if trace.identity.compiler.package_version != solidjs_compiler::COMPILER_VERSION
        || trace.identity.compiler.upstream_revision != EXPECTED_COMPILER_UPSTREAM_REVISION
        || solidjs_compiler::SEMANTIC_TRACE_UPSTREAM_REVISION != EXPECTED_COMPILER_UPSTREAM_REVISION
        || trace.identity.compiler.implementation_revision
            != EXPECTED_COMPILER_IMPLEMENTATION_REVISION
        || solidjs_compiler::SEMANTIC_TRACE_IMPLEMENTATION_REVISION
            != EXPECTED_COMPILER_IMPLEMENTATION_REVISION
    {
        return Err(CompilerProviderError::Native(
            "Solid 2.0 compiler semantic identity disagrees with the pinned producer".into(),
        ));
    }
    let input = CompilerTraceInput {
        producer: CompilerProducerInput {
            dialect: "solid-v2".into(),
            trace_version: trace.version,
            package_version: Some(trace.identity.compiler.package_version.clone()),
            upstream_revision: Some(trace.identity.compiler.upstream_revision.clone()),
            implementation_revision: trace.identity.compiler.implementation_revision.clone(),
            source_sha256: trace.identity.source_sha256.clone(),
            output,
            source_map,
            claimed_output_sha256: Some(trace.identity.output_sha256.clone()),
            claimed_source_map_sha256: trace.identity.source_map_sha256.clone(),
            configuration_json: serde_json::to_string(&trace.identity.config).map_err(|error| {
                CompilerProviderError::Native(format!(
                    "serialize Solid 2.0 compiler configuration: {error}"
                ))
            })?,
            identity_complete: true,
        },
        source_operations_complete: true,
        // Trace v3 gives every reported generated operation an exact identity
        // and reconciles source sites, but it does not yet run an independent
        // census over every wrapper emission. Keep positive operations without
        // turning a missing recorder hook into negative proof.
        generated_operations_complete: false,
        operations: trace
            .sites
            .iter()
            .map(|site| CompilerOperationInput {
                id: site.id.clone(),
                span: Span::new(site.span.start, site.span.end),
                kind: operation_kind(site.kind),
                evidence: CompilerOperationEvidence::Rich(execution_semantics(&site.semantics)),
            })
            .collect(),
        generated_operations: trace
            .generated_operations
            .iter()
            .map(|operation| GeneratedCompilerOperation {
                id: operation.id.clone(),
                source_id: operation.source_id.clone(),
                source_span: Span::new(operation.source_span.start, operation.source_span.end),
                kind: generated_kind(operation.kind),
                trigger: execution_trigger(operation.trigger),
                schedule: execution_schedule(operation.schedule),
                tracking: tracking_relation(operation.tracking),
                cardinality: execution_cardinality(operation.cardinality),
                owner: owner_relation(operation.owner),
                receiver_span: operation
                    .receiver_span
                    .map(|span| Span::new(span.start, span.end)),
                group_id: operation.group_id,
                wrapper: operation.wrapper.clone(),
            })
            .collect(),
    };
    let map = ExecutionMap::normalize(input, source)?;
    if !map.uncovered_jsx_expressions().is_empty() {
        return Err(CompilerProviderError::Native(
            "Solid 2.0 compiler trace is incomplete: a JSX expression has no execution classification"
                .into(),
        ));
    }
    Ok(map)
}

fn operation_kind(kind: solidjs_compiler::ExecutionSiteKind) -> CompilerOperationKind {
    use solidjs_compiler::ExecutionSiteKind;
    match kind {
        ExecutionSiteKind::JsxChild => CompilerOperationKind::JsxChild,
        ExecutionSiteKind::NativeAttribute => CompilerOperationKind::NativeAttribute,
        ExecutionSiteKind::NativeSpread => CompilerOperationKind::NativeSpread,
        ExecutionSiteKind::ComponentProperty => CompilerOperationKind::ComponentProperty,
        ExecutionSiteKind::ComponentSpread => CompilerOperationKind::ComponentSpread,
        ExecutionSiteKind::ComponentChild => CompilerOperationKind::ComponentChild,
        ExecutionSiteKind::EventHandler => CompilerOperationKind::EventHandler,
        ExecutionSiteKind::Ref => CompilerOperationKind::Ref,
        ExecutionSiteKind::ControlFlowRender => CompilerOperationKind::ControlFlowRender,
    }
}

fn generated_kind(
    kind: solidjs_compiler::GeneratedOperationKind,
) -> GeneratedCompilerOperationKind {
    use solidjs_compiler::GeneratedOperationKind;
    match kind {
        GeneratedOperationKind::Effect => GeneratedCompilerOperationKind::Effect,
        GeneratedOperationKind::Insert => GeneratedCompilerOperationKind::Insert,
        GeneratedOperationKind::Memo => GeneratedCompilerOperationKind::Memo,
        GeneratedOperationKind::Scope => GeneratedCompilerOperationKind::Scope,
        GeneratedOperationKind::ComponentInvocation => {
            GeneratedCompilerOperationKind::ComponentInvocation
        }
        GeneratedOperationKind::DeferredCallback => {
            GeneratedCompilerOperationKind::DeferredCallback
        }
        GeneratedOperationKind::DelegatedEvent => GeneratedCompilerOperationKind::DelegatedEvent,
        GeneratedOperationKind::RefApplication => GeneratedCompilerOperationKind::RefApplication,
        GeneratedOperationKind::SsrClaim => GeneratedCompilerOperationKind::SsrClaim,
        GeneratedOperationKind::RuntimeWrapper => GeneratedCompilerOperationKind::RuntimeWrapper,
    }
}

fn execution_semantics(value: &solidjs_compiler::ExecutionSemantics) -> CompilerExecutionSemantics {
    CompilerExecutionSemantics {
        disposition: execution_disposition(value.disposition),
        trigger: execution_trigger(value.trigger),
        schedule: execution_schedule(value.schedule),
        tracking: tracking_relation(value.tracking),
        cardinality: execution_cardinality(value.cardinality),
        owner: owner_relation(value.owner),
        generated_operations: value.generated_operations.clone(),
    }
}

macro_rules! map_enum {
    ($name:ident, $source:ty, $target:ty, {$($variant:ident),+ $(,)?}) => {
        fn $name(value: $source) -> $target {
            match value {
                $(<$source>::$variant => <$target>::$variant,)+
            }
        }
    };
}

map_enum!(execution_disposition, solidjs_compiler::ExecutionDisposition, CompilerExecutionDisposition, {
    Unknown, Discarded, EagerOnce, Deferred, ReactiveRerun, EventTriggered, RefFactory,
    RefApplication, ComponentPropertyGetter, ControlFlowRender, SsrEvaluation, SsrRenderCallback
});
map_enum!(execution_trigger, solidjs_compiler::ExecutionTrigger, CompilerExecutionTrigger, {
    Unknown, None, Render, Dependency, Event, RefApplication, Caller
});
map_enum!(execution_schedule, solidjs_compiler::ExecutionSchedule, CompilerExecutionSchedule, {
    Unknown, None, Inline, Render, Deferred
});
map_enum!(tracking_relation, solidjs_compiler::TrackingRelation, CompilerTrackingRelation, {
    Unknown, None, Tracked, Untracked, Inherited
});
map_enum!(execution_cardinality, solidjs_compiler::ExecutionCardinality, CompilerExecutionCardinality, {
    Never, ZeroOrOne, ExactlyOnce, ZeroOrMore, OneOrMore, Unknown
});
map_enum!(owner_relation, solidjs_compiler::OwnerRelation, CompilerOwnerRelation, {
    None, AmbientAtTransformSite, AmbientAtGeneratedInvocation, CapturedGeneratedOwner,
    CreatedGeneratedOwner, Unknown
});

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

    #[test]
    fn certification_replay_retains_the_exact_output_and_is_trace_neutral() {
        let request = AnalysisRequest::new(
            "App.tsx",
            "const view = <div>{count()}</div>;",
            CompilerOptions::default(),
        );
        let compilation =
            analyze_with_materialized_output(&request).expect("materialized compilation");
        let producer = compilation
            .execution_map
            .semantic_model
            .producer
            .as_ref()
            .expect("complete producer identity");
        assert_eq!(
            producer.output_sha256,
            solid_facts::core::SourceHash::of(&compilation.output)
                .as_str()
                .strip_prefix("sha256:")
                .expect("canonical source hash")
        );
        assert!(
            compilation
                .execution_map
                .semantic_model
                .source_operations_complete
        );
        assert!(
            !compilation
                .execution_map
                .semantic_model
                .generated_operations_complete
        );
    }

    // The projection this adapter exists to get right, and the one it got wrong:
    // a value the emitter deletes is a *discarded* region, not an untracked one.
    // An untracked region says the code runs once at render, which would make a
    // reactive read inside a deleted value a proven stale read.
    #[test]
    fn deleted_values_are_discarded_regions_rather_than_untracked_ones() {
        // A `children` attribute shadowed by real source children: dropped in
        // `plan_attributes`, never lowered, and emitted by neither this
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

    // Ryan's authoritative `next` semantics promote a template-root
    // `<noscript children={c()}/>` to a real child and lower it. This assertion
    // pins the positive tracked fact the checker consumes; it is not a request
    // to force Babel output parity into this fork.
    #[test]
    fn a_template_root_noscript_children_attribute_is_lowered_not_deleted() {
        let facts = compile_source("const view = <noscript children={c()} />;");
        assert_eq!(facts.tracked_regions.len(), 1, "{facts:#?}");
        assert!(facts.discarded_regions.is_empty(), "{facts:#?}");
        assert_eq!(
            facts
                .jsx_operations
                .iter()
                .map(|operation| operation.kind.as_str())
                .collect::<Vec<_>>(),
            ["jsx-expression"],
            "{facts:#?}"
        );
    }

    // The nested position is the opposite, for the reason the fork states in
    // `children.rs`: promoting there would add an insert Babel does not emit,
    // so the capture is discarded instead and the two compilers agree.
    #[test]
    fn a_nested_noscript_children_attribute_is_deleted_not_lowered() {
        let facts = compile_source("const view = <div><noscript children={c()} /></div>;");
        assert_eq!(facts.discarded_regions.len(), 1, "{facts:#?}");
        assert!(facts.tracked_regions.is_empty(), "{facts:#?}");
    }

    // With a spread on the element the `children` member is censused as a
    // **child**
    // (`ExecutionSiteKind::JsxChild` -> `jsx-expression`) and decided
    // `ReactiveRerun`, which is truthful -- `spread()` assigns it as the
    // element's children through a `mergeProps` getter -- and yet neither this
    // compiler promotes it (both keep it in the merged props). This assertion
    // pins that distinct positive execution path.
    #[test]
    fn a_spread_carrying_children_attribute_is_censused_as_a_reactive_child() {
        for source in [
            "const view = <noscript {...p} children={c()} />;",
            "const view = <div><noscript {...p} children={c()} /></div>;",
        ] {
            let facts = compile_source(source);
            assert!(
                facts
                    .jsx_operations
                    .iter()
                    .any(|operation| operation.kind.as_str() == "jsx-expression"),
                "{source}: {facts:#?}"
            );
            assert_eq!(facts.tracked_regions.len(), 1, "{source}: {facts:#?}");
            assert!(facts.discarded_regions.is_empty(), "{source}: {facts:#?}");
        }
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
    // It is what keeps a future pin move from reinterpreting a field whose
    // meaning changed.
    #[test]
    fn an_unreadable_trace_version_is_refused_rather_than_projected() {
        use solidjs_compiler::{CompileOptions, compile};

        let source = "const view = <div>{count()}</div>;";
        // A literal version, not `READS_TRACE_VERSION + 1`: the point is that
        // some *specific* other schema is refused, and a version derived from
        // the constant under test would move with it.
        //
        let mut output = compile(
            source,
            &CompileOptions {
                semantic_trace: true,
                ..CompileOptions::default()
            },
        )
        .expect("compiler output");
        let mut trace = output.semantic_trace.take().expect("semantic trace");
        trace.version = 4;
        let error = execution_map_from_trace(&trace, source, output.code, output.source_map)
            .expect_err("an unknown trace version must fail closed");
        assert!(
            matches!(&error, CompilerProviderError::Native(message)
                if message.contains("semantic trace version 4")
                    && message.contains("reads version 3")),
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
    fn ref_factories_preserve_the_returned_directive_apply_role() {
        let facts = compile_source("const view = <button ref={makeDirective()} />;");
        let factory = facts
            .semantic_model
            .operations
            .iter()
            .find(|operation| {
                operation.execution.disposition == CompilerExecutionDisposition::RefFactory
            })
            .expect("the ref expression is evaluated as a factory");

        assert!(
            facts.callback_roles.iter().any(|callback| {
                callback.span == factory.span
                    && callback.role == solid_facts::compiler::CallbackRoleKind::DirectiveApply
            }),
            "the factory result is later invoked in the directive-apply phase: {facts:#?}"
        );
    }

    #[test]
    fn normalized_identity_is_complete_and_bound_to_trace_three() {
        let facts = compile_source("const view = <div>{count()}</div>;");
        let producer = facts
            .semantic_model
            .producer
            .as_ref()
            .expect("the native compiler supplies producer identity");
        assert!(producer.identity_complete);
        assert_eq!(producer.trace_version, READS_TRACE_VERSION);
        assert_eq!(
            producer.upstream_revision.as_deref(),
            Some(EXPECTED_COMPILER_UPSTREAM_REVISION)
        );
        assert_eq!(
            producer.implementation_revision,
            EXPECTED_COMPILER_IMPLEMENTATION_REVISION
        );
        assert_eq!(producer.output_sha256.len(), 64);
        assert_eq!(producer.configuration_sha256.len(), 64);
        assert!(facts.semantic_model.source_operations_complete);
        assert!(!facts.semantic_model.generated_operations_complete);
    }

    #[test]
    fn ssr_execution_remains_inherited_instead_of_becoming_legacy_untracked() {
        let request = AnalysisRequest::new(
            "App.tsx",
            "const view = <div title={title()}>{count()}</div>;",
            CompilerOptions {
                generate: "ssr".into(),
                ..CompilerOptions::default()
            },
        );
        let facts = NativeCompilerFacts.analyze(&request).expect("SSR facts");
        assert!(
            facts.semantic_model.operations.iter().any(|operation| {
                matches!(
                    operation.execution.disposition,
                    CompilerExecutionDisposition::SsrEvaluation
                        | CompilerExecutionDisposition::SsrRenderCallback
                ) && operation.execution.tracking == CompilerTrackingRelation::Inherited
            }),
            "{facts:#?}"
        );
        assert!(facts.untracked_regions.is_empty(), "{facts:#?}");
    }
}
