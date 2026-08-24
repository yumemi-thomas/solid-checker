//! The Solid 2.0 compiler fact domain: the dom-expressions Oxc compiler
//! adapted to the checker's [`CompilerFactsProvider`] seam.
//!
//! Nothing outside this crate speaks the compiler's own types. A Solid 1.x
//! dialect plugs in the same way: its own crate wrapping its own compiler
//! behind the same trait.

use solid_facts::compiler::{
    AnalysisRequest, COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind,
    CompilerFactsProvider, CompilerProviderError, ExecutionMap, ExecutionRegion, JsxOperation,
    OwnershipRegion, OwnershipRegionKind, RegionReason,
};
use solid_facts::core::{SourceHash, Span};

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

/// The in-process Solid 2.0 compiler-facts provider.
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
        let options = CompileOptions {
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
            semantic_trace: true,
            ..CompileOptions::default()
        };
        let output = compile(&request.source, &options)
            .map_err(|error| CompilerProviderError::Native(format!("{}: {error}", request.path)))?;
        let trace = output
            .semantic_trace
            .ok_or(CompilerProviderError::MissingExecutionMap)?;
        execution_map_from_trace(&trace, &request.source)
    }
}

/// Projects the compiler's semantic trace onto the checker's execution map.
///
/// The trace is total *over the census*: every censused JSX site carries a
/// terminal decision, so each one lands in exactly one of the tracked,
/// untracked, or callback categories, and
/// `ExecutionMap::uncovered_jsx_expressions` is empty by construction rather
/// than by luck. That is not the same as total over the JSX the source
/// contains — this compiler censuses what it lowers — and it is not the same as
/// truthful about the compiler Solid *ships*. Two consequences, both handled
/// downstream and both documented in docs/compiler-facts.md:
///
/// - A source expression this compiler does not lower (a template-root void
///   element's children) or lowers and then retracts (a nested dynamic
///   `textContent`, the textarea `value` fold, the inert `<noscript>` fast
///   path) leaves a hole — `solid-reactive-ir`'s `missing_jsx_census`, under
///   "Census gaps".
/// - A site this compiler lowers that Babel emits nothing for (a *nested* void
///   element's children; a `<noscript>`'s children where it is its own template
///   root or its attributes force it off the fast path) is a present, truthful
///   fact that is still not evidence about the user's build —
///   `divergent_lowered_child`, under "Divergent lowering". The fork's own
///   docs/execution-contract.md declares that consumer rule binding.
fn execution_map_from_trace(
    trace: &dom_expressions_compiler::SemanticTrace,
    source: &str,
) -> Result<ExecutionMap, CompilerProviderError> {
    use dom_expressions_compiler::{
        CallbackDecision, ExecutionSiteKind, TerminalDecision, ValueDecision,
    };

    // The trace schema is versioned and its meaning is not forward-compatible:
    // `ownership_sites` is emitted by version 2 but is the producer's legacy
    // vocabulary, so a trace of any other version must be refused rather than
    // read as if the fields it does carry mean what this projection assumes.
    // The comparison is against this crate's own `READS_TRACE_VERSION`, never
    // the producer's constant.
    if trace.version != READS_TRACE_VERSION {
        return Err(CompilerProviderError::Native(format!(
            "Solid 2.0 compiler produced semantic trace version {}, but this checker reads version {READS_TRACE_VERSION}",
            trace.version
        )));
    }

    let mut map = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(source),
        tracked_regions: Vec::new(),
        untracked_regions: Vec::new(),
        ownership_regions: Vec::new(),
        callback_roles: Vec::new(),
        jsx_operations: Vec::new(),
    };

    for site in &trace.ownership_sites {
        use dom_expressions_compiler::OwnershipDecision;

        map.ownership_regions.push(OwnershipRegion {
            span: Span::new(site.span.start, site.span.end),
            kind: match site.decision {
                OwnershipDecision::Owned => OwnershipRegionKind::Owned,
                OwnershipDecision::Unowned => OwnershipRegionKind::Unowned,
                OwnershipDecision::Leaf => OwnershipRegionKind::Leaf,
            },
        });
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
            // `EagerOnce` and `Elided` settle at render and never re-run.
            TerminalDecision::Value(ValueDecision::EagerOnce | ValueDecision::Elided) => {
                let reason = match site.kind {
                    ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                        RegionReason::JsxAttribute
                    }
                    ExecutionSiteKind::ComponentProperty
                    | ExecutionSiteKind::ComponentSpread
                    | ExecutionSiteKind::ComponentChild => RegionReason::ComponentGetter,
                    _ => RegionReason::JsxChild,
                };
                map.untracked_regions.push(ExecutionRegion { span, reason });
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
            "Solid 2.0 compiler trace is incomplete: a JSX expression has no execution classification"
                .into(),
        ));
    }
    Ok(map)
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
        use dom_expressions_compiler::SemanticTrace;

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
            ownership_sites: Vec::new(),
            owner_establishments: Vec::new(),
            component_render_sites: Vec::new(),
            deferred_callback_sites: Vec::new(),
        };
        let error = execution_map_from_trace(&trace, source)
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
