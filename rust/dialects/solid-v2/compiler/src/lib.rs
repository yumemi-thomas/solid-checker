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
const EXPECTED_COMPILER_UPSTREAM_REVISION: &str = "5b1b777648a89b6e207e8bb78514d5c03420e08b";
const EXPECTED_COMPILER_IMPLEMENTATION_REVISION: &str = "a4983a7130daa210bfc00f079dce57939fe14502";
pub const COMPILER_DISTRIBUTION_REVISION: &str = "9f5fff59c2abc4465058d96080771c82dfd91990";

/// Stable cache identity for the exact semantic producer this adapter reads.
/// Keep this synchronized with the pinned compiler revision and trace
/// implementation when either changes.
pub const COMPILER_FACTS_IDENTITY: &str =
    "solid-v2:trace3:a4983a7130daa210bfc00f079dce57939fe14502";

/// Digest of the exact Git-owned semantic compiler source identity consumed by
/// this adapter. The independent identity gate recomputes this value from the
/// upstream, implementation, distribution, trace, and protocol pins.
pub const COMPILER_SOURCE_MANIFEST_SHA256: &str =
    "sha256:b5dbbed3d12508b88fae544f1f4c41cc6e70bdda4e6e7ecca28434fd4fe85fdf";

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

    // ADR 0360: the compiler declining the source is a refusal of this file,
    // not a producer failure. The trace-off replay and every identity check
    // below stay `Native` (fatal): they mean the pinned producer misbehaved.
    let traced = compile(&request.source, &compile_options(request, true)?)
        .map_err(|error| CompilerProviderError::Refused(format!("{}: {error}", request.path)))?;
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
mod tests;
