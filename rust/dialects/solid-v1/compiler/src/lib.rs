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
    RegionReason,
};
use solid_facts::core::{SourceHash, Span};
use solid1_dom_expressions_compiler as dom_expressions_compiler;

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
/// The trace is total: every censused JSX site carries a terminal decision, so
/// each one lands in exactly one of the tracked, untracked, or callback
/// categories, and `ExecutionMap::uncovered_jsx_expressions` is empty by
/// construction rather than by luck.
fn execution_map_from_trace(
    trace: &dom_expressions_compiler::SemanticTrace,
    source: &str,
) -> Result<ExecutionMap, CompilerProviderError> {
    use dom_expressions_compiler::{
        CallbackDecision, ExecutionSiteKind, TerminalDecision, ValueDecision,
    };

    let mut map = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: SourceHash::of(source),
        tracked_regions: Vec::new(),
        untracked_regions: Vec::new(),
        // The compiler has never emitted ownership regions; ownership is
        // derived from the AST and type facts instead.
        ownership_regions: Vec::new(),
        callback_roles: Vec::new(),
        jsx_operations: Vec::new(),
    };

    // Sites arrive ordered by (span, kind), so appending in iteration order
    // keeps every category in the canonical span order `validate` requires.
    for site in &trace.sites {
        let span = Span::new(site.span.start, site.span.end);
        let kind = match site.kind {
            ExecutionSiteKind::JsxChild => "jsx-expression",
            ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                "dynamic-attribute"
            }
            ExecutionSiteKind::ComponentProperty
            | ExecutionSiteKind::ComponentSpread
            | ExecutionSiteKind::ComponentChild => "component-property",
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
    Ok(map)
}
