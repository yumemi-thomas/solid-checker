//! The Solid 1.x compiler fact domain: the solid-1x-compiler Oxc compiler
//! adapted to the checker's [`CompilerFactsProvider`] seam.
//!
//! Nothing outside this crate speaks the compiler's own types. This is the
//! Solid 1.x sibling of `solid-v2-compiler`: its own crate wrapping its own
//! compiler behind the same trait, so the pipeline selects a compiler by
//! selecting a dialect and never names either.

use solid_facts::compiler::{
    AnalysisRequest, COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind,
    CompilerFactsProvider, CompilerProviderError, ExecutionMap, ExecutionRegion, JsxOperation,
    OwnershipRegion, OwnershipRegionKind, RegionReason,
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
        execution_map_from_trace(&trace, &request.source, default_effect_wrapper)
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
) -> Result<ExecutionMap, CompilerProviderError> {
    use dom_expressions_compiler::{
        CallbackDecision, ExecutionSiteKind, TerminalDecision, ValueDecision,
    };

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

    let mut map = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(source),
        tracked_regions: Vec::new(),
        untracked_regions: Vec::new(),
        discarded_regions: Vec::new(),
        ownership_regions: Vec::new(),
        callback_roles: Vec::new(),
        jsx_operations: Vec::new(),
    };

    // Version 2 of the 1.x trace no longer carries ownership decisions, so the
    // checker derives them from the sites, applying exactly the rule the
    // producer used to apply: a value the compiler re-runs reactively is
    // re-run under an owner the compiler's own runtime established, which is
    // an auditable claim only while that runtime is the compiler's own. A
    // configured effect wrapper is an unaudited runtime, so no owner is
    // claimed for it at all — absence here is "not proven", never "unowned".
    //
    // `trace.sites` is ordered by (span, kind), so appending in iteration
    // order already satisfies the non-decreasing span order `validate`
    // requires.
    if default_effect_wrapper {
        for site in &trace.sites {
            if matches!(
                site.decision,
                TerminalDecision::Value(ValueDecision::ReactiveRerun)
            ) {
                map.ownership_regions.push(OwnershipRegion {
                    span: Span::new(site.span.start, site.span.end),
                    kind: OwnershipRegionKind::Owned,
                });
            }
        }
    }

    // Sites arrive ordered by (span, kind), so appending in iteration order
    // keeps every category in the canonical span order `validate` requires.
    for site in &trace.sites {
        let span = Span::new(site.span.start, site.span.end);
        let kind = match site.kind {
            ExecutionSiteKind::JsxChild => "jsx-expression",
            ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                "dynamic-attribute"
            }
            ExecutionSiteKind::ComponentProperty => "component-property",
            ExecutionSiteKind::ComponentSpread => "component-spread",
            ExecutionSiteKind::ComponentChild => "component-child",
            ExecutionSiteKind::EventHandler => "event-listener",
            ExecutionSiteKind::Ref => "directive-apply",
            ExecutionSiteKind::ControlFlowRender => "control-flow-render",
        };
        map.jsx_operations.push(JsxOperation {
            span,
            kind: kind.into(),
        });

        match site.decision {
            TerminalDecision::Value(ValueDecision::ReactiveRerun) => {
                let reason = match site.kind {
                    ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                        RegionReason::JsxAttribute
                    }
                    _ => RegionReason::JsxChild,
                };
                map.tracked_regions.push(ExecutionRegion { span, reason });
            }
            // `CallerContext` is the dynamic component prop: the expression is
            // handed to the child as a getter and re-evaluated in the child's
            // tracking context. It is deferred, not untracked — treating it as
            // an untracked region would report every `when={count()}` as a
            // stale read.
            TerminalDecision::Value(ValueDecision::CallerContext) => {
                map.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            // A component child is handed to the component and invoked from
            // the component's own render, not from here — a deferred callback
            // even when the value itself is built once.
            TerminalDecision::Value(ValueDecision::EagerOnce)
                if site.kind == ExecutionSiteKind::ComponentChild =>
            {
                map.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            // `EagerOnce` settles at render and never re-runs. It does execute:
            // exactly once, outside any tracking scope, which is what an
            // untracked region claims.
            TerminalDecision::Value(ValueDecision::EagerOnce) => {
                map.untracked_regions.push(ExecutionRegion {
                    span,
                    reason: region_reason(site.kind),
                });
            }
            // `Elided` is the opposite: the value is decided and then emitted
            // nowhere. Every one of this producer's `Elided` sites is a value
            // the emitter deletes — a confidently-foldable constant baked into
            // the template (`children.rs`, `static_template.rs`, the folded
            // attribute plans in `attrs.rs`), or a value discarded unlowered (a
            // `children` attribute or component `children` prop shadowed by
            // real children, a spread's skipped `children`). Projecting it as
            // an untracked region reported the read inside it as a proven stale
            // read, a claim whose every clause is false of code neither
            // compiler emits.
            TerminalDecision::Value(ValueDecision::Elided) => {
                map.discarded_regions.push(ExecutionRegion {
                    span,
                    reason: region_reason(site.kind),
                });
            }
            TerminalDecision::Callback(decision) => {
                let role = match decision {
                    CallbackDecision::LaterEvent => CallbackRoleKind::EventHandler,
                    CallbackDecision::RefApply => CallbackRoleKind::DirectiveApply,
                    // A render callback runs at render time under no tracking
                    // scope of its own.
                    CallbackDecision::LaterRender => CallbackRoleKind::Render,
                };
                map.callback_roles.push(CallbackRole { span, role });
            }
        }
    }

    map.validate(source)?;
    if !map.uncovered_jsx_expressions().is_empty() {
        return Err(CompilerProviderError::Native(
            "Solid 1.x compiler trace is incomplete: a JSX expression has no execution classification"
                .into(),
        ));
    }
    Ok(map)
}

/// Which JSX position a one-shot or deleted value sat in.
///
/// Shared by the untracked (`EagerOnce`) and discarded (`Elided`) arms: the
/// reason describes where the value was written, which does not depend on
/// whether the emitter kept it.
fn region_reason(kind: dom_expressions_compiler::ExecutionSiteKind) -> RegionReason {
    use dom_expressions_compiler::ExecutionSiteKind;

    match kind {
        ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
            RegionReason::JsxAttribute
        }
        ExecutionSiteKind::ComponentProperty
        | ExecutionSiteKind::ComponentSpread
        | ExecutionSiteKind::ComponentChild => RegionReason::ComponentGetter,
        _ => RegionReason::JsxChild,
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
        let error = execution_map_from_trace(&trace, source, true)
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
}
