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
    let compilation = analyze_with_materialized_output(&request).expect("materialized compilation");
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
