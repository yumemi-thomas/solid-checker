//! Static validation for Solid 2.0 API shapes and explicit refresh writes.

use crate::{HashMap, HashSet};

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::ArgumentValueKind;
use solid_facts::core::Span;
use typefacts::Location;

use crate::effect_api::{ProofStatus, classify_effect_call};
use crate::execution_role::{RootBodyGuard, allowed_callback_spans, semantic_write_execution_role};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use crate::owners::{analysis_context, computation_is_async};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_file_results};
use crate::{
    ReactiveSourceKind, ReactiveWrite, StaticDefect, StaticDefectKind, StaticViolation, location,
};

pub(super) struct StaticDirectiveFileResult {
    pub(super) defects: Vec<StaticDefect>,
    pub(super) violations: Vec<StaticViolation>,
    pub(super) writes: Vec<ReactiveWrite>,
    pub(super) write_action_obligations: Vec<(&'static str, String, u64, u64)>,
}

/// Runs the project-level static API stage and merges its independent file
/// results into the program draft.
pub(crate) fn check_project(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let checker = StaticApiContext {
        lookup: ctx.semantic_lookup,
        entities: ctx.entities,
        symbol_names: ctx.symbol_names,
        source_kinds: ctx.source_kinds,
        source_owned_write: ctx.source_owned_write,
        value_form_stores: ctx.value_form_stores,
        accessors: ctx.accessors,
        reachable_calls: ctx.reachable_calls,
        client_build: !ctx.rule_options.runtime.selects_server_build(),
        package_instances: &ctx.rule_options.runtime.package_instances,
    };
    for result in parallel_file_results(&ctx.facts.files, |file| checker.check_file(file)) {
        draft.static_defects.extend(result.defects);
        draft.static_violations.extend(result.violations);
        draft.writes.extend(result.writes);
        draft
            .write_action_obligations
            .extend(result.write_action_obligations);
    }
}

pub(super) struct StaticApiContext<'a> {
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) source_owned_write: &'a HashMap<SymbolId, crate::OwnedWriteOption>,
    pub(super) value_form_stores: &'a HashSet<SymbolId>,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) reachable_calls: &'a HashMap<Location, u64>,
    /// No server build of the Solid packages is selected.
    pub(super) client_build: bool,
    /// Which installed `solid-js` each file loads at runtime.
    pub(super) package_instances: &'a crate::PackageInstanceEvidence,
}

impl StaticApiContext<'_> {
    pub(super) fn check_file(&self, file: &FileFacts) -> StaticDirectiveFileResult {
        let mut result = StaticDirectiveFileResult {
            defects: Vec::new(),
            violations: Vec::new(),
            writes: Vec::new(),
            write_action_obligations: Vec::new(),
        };
        crate::installed_members::check_file(file, &mut result.defects);
        crate::listener_reach::check_file(file, self.lookup, &mut result.defects);
        let allowed = allowed_callback_spans(file, self.lookup);
        let dialect = self.lookup.dialect;
        for call in &file.ast.calls {
            let Some(primitive) = crate::call_primitive_name_cached(
                self.lookup,
                file,
                call,
                self.entities,
                self.symbol_names,
                dialect,
            ) else {
                continue;
            };
            // `primitive` stays as the spelling to report; `kind` is what the
            // rules below branch on. A name this dialect does not export
            // resolves to `None` and matches nothing.
            let kind = primitive.primitive();
            let effect_semantics = (kind == Some(Primitive::CreateEffect))
                .then(|| classify_effect_call(file, call, Primitive::CreateEffect, self.lookup));
            if effect_semantics
                .is_some_and(|semantics| semantics.missing_effect_function != ProofStatus::No)
            {
                let semantics = effect_semantics.expect("checked as some above");
                result.defects.push(StaticDefect {
                    kind: StaticDefectKind::MissingEffectFunction,
                    location: location(file.path.shared(), call.callee),
                    analysis_context: semantics
                        .missing_effect_uncertainty
                        .analysis_context()
                        .into(),
                    fixes: vec![],
                    uncertain: semantics.missing_effect_function == ProofStatus::Uncertain,
                });
            }
            // Where each primitive takes its options object -- a second index
            // vocabulary, and the dialect's to answer: 2.0's numbers read
            // 1.x's `createMemo(fn, seed)` seed as an options object.
            let options_index = kind
                .filter(|kind| dialect.supports_sync_option(*kind))
                .and_then(|kind| dialect.options_argument(kind));
            let sync_literal = options_index.map(|options_index| {
                (
                    call.arguments.get(options_index).is_some_and(|argument| {
                        argument.boolean_properties.iter().any(|property| {
                            file.source_text(property.name) == Some("sync") && property.value
                        })
                    }),
                    crate::call_option_literal(file, call, options_index, "sync"),
                )
            });
            // ADR 0655: what the computation does on a pass. A return typed
            // `any`, a type that may be sync and a value no fact describes are
            // open; only an async function or a Promise on every path proves
            // that a pass returns one.
            let compute = || {
                let argument = call.arguments.first()?;
                let lookup = self.lookup;
                // A spread of an exact one-element tuple has no options slot, so
                // nothing there can say `sync`.
                let spread_tuple = file
                    .ast
                    .spreads
                    .iter()
                    .find(|spread| argument.spread && spread.span == argument.span)
                    .and_then(|spread| lookup.entity_at(file.path.as_str(), spread.argument))
                    .and_then(|entity| entity.tuple_shape)
                    .and_then(typefacts::TupleShape::exact_length);
                if spread_tuple == Some(1) && call.arguments.len() == 1 {
                    return None;
                }
                // `createSignal(0, options)` has no computation: only a value
                // that may be a function can return anything.
                let entity = lookup.entity_at(file.path.as_str(), argument.span);
                if !argument.spread
                    && (matches!(
                        argument.runtime_value_kind,
                        solid_facts::ast::RuntimeValueKind::Primitive
                            | solid_facts::ast::RuntimeValueKind::Nullish
                    ) || entity.and_then(|entity| entity.callability)
                        == Some(typefacts::Callability::NonCallable))
                {
                    return None;
                }
                if computation_is_async(lookup, file, argument.span) {
                    let open =
                        crate::owners::computation_may_answer_sync(lookup, file, argument.span);
                    return Some(if open {
                        StaticSource::Open
                    } else {
                        StaticSource::Promise("")
                    });
                }
                Some(static_source_state(file, argument, self))
            };
            // Options the syntax cannot read (`createMemo(fn, opts)`, a spread,
            // a non-literal `sync:` value) may assert `sync: true`: with a
            // computation not proven synchronous that is an obligation, not a
            // silent clean.
            if let Some((false, solid_dialect::OptionLiteral::Unknown)) = sync_literal
                && matches!(
                    compute(),
                    Some(StaticSource::Promise(_) | StaticSource::Open)
                )
            {
                result.violations.push(StaticViolation {
                    id: "SC7002".into(),
                    rule: "sync-computation-received-async".into(),
                    message: format!(
                        "{primitive}'s computation is not proven synchronous, and its options cannot be read statically; if they set sync: true and it returns a Promise or AsyncIterable, dev throws SYNC_NODE_RECEIVED_ASYNC and production stores the unawaited value, and whether it does cannot be proven either way"
                    ),
                    hint: "Pass the options as an object literal with a literal sync value, so solid-checker can read it; drop sync: true for an async computation.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: crate::blockers::COMPUTE_RETURN_UNPROVEN.to_owned(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
            if let Some((true, _)) = sync_literal
                && let Some(state @ (StaticSource::Promise(_) | StaticSource::Open)) = compute()
            {
                let uncertain = matches!(state, StaticSource::Open);
                result.violations.push(StaticViolation {
                    id: "SC7002".into(),
                    rule: "sync-computation-received-async".into(),
                    message: format!(
                        "{primitive} is marked sync: true but its computation {}; sync: true asserts a synchronous result so the runtime can skip the async-shape probe — dev still probes and throws SYNC_NODE_RECEIVED_ASYNC, and production stores the unawaited value as-is",
                        if uncertain {
                            "may return a Promise or AsyncIterable, and nothing proves it does not"
                        } else {
                            "returns a Promise or AsyncIterable"
                        }
                    ),
                    hint: format!(
                        "Drop sync: true and let the read suspend to a <{}> boundary, or make the computation synchronous by moving the async work into its own computation and reading the settled accessor here.",
                        dialect.boundary_name(solid_dialect::Boundary::Async)
                    ),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: if uncertain {
                        crate::blockers::COMPUTE_RETURN_UNPROVEN.to_owned()
                    } else {
                        String::new()
                    },
                    fixes: vec![],
                    uncertain,
                });
            }
            // SC2004: resolve() in a tracked scope. Probed on
            // `@solidjs/signals@2.0.0-rc.0`: the dev bundle guards on the
            // *observer* (`dev.js:4738` — `if (getObserver()) throw new
            // Error("Cannot call resolve inside a reactive scope…")`), so a
            // resolve() inside a memo/effect compute, `createTrackedEffect`,
            // or tracked JSX throws in dev, while `untrack` callbacks,
            // component bodies, event handlers, and effect apply callbacks
            // all clear the observer and stay legal. The production bundle
            // has no guard — the call silently resolves a one-shot value —
            // so the dev throw is what this mirrors.
            if kind == Some(Primitive::Resolve)
                && let Some(scope) = resolve_tracked_scope(file, call, &allowed, self)
            {
                result.violations.push(StaticViolation {
                    id: "SC2004".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: format!(
                        "resolve() is called inside {scope}; resolve() reads the expression once and never tracks updates, and an active observer makes Solid throw \"Cannot call resolve inside a reactive scope\" here in dev"
                    ),
                    hint: "Call resolve() from imperative code, such as an event handler; on rc.17 an effect's apply and onSettled refuse it too. To depend on a pending value inside a computation, read the accessor directly: tracked reads suspend and re-run on their own. A deliberate one-shot read can be wrapped in untrack(), which clears the observer the runtime guards on.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
            // The same guard where nothing places the call: a helper that a
            // tracked scope may reach (`createMemo(() => waitReady())`).
            if kind == Some(Primitive::Resolve)
                && resolve_scope_unplaced(file, call, &allowed, self)
            {
                result.violations.push(StaticViolation {
                    id: "SC2004".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: "resolve() is called in code the analysis cannot place; if an observer is active when it runs (a memo or effect compute, or tracked JSX, that reaches it through a helper), Solid throws \"Cannot call resolve inside a reactive scope\" in dev, and whether one is cannot be proven either way".into(),
                    hint: "Call resolve() from imperative code, such as an event handler; on rc.17 an effect's apply and onSettled refuse it too. To depend on a pending value inside a computation, read the accessor directly: tracked reads suspend and re-run on their own. A deliberate one-shot read can be wrapped in untrack(), which clears the observer the runtime guards on.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
            // SC2005: until() in a tracked scope. `@solidjs/signals@2.0.0-rc.9`
            // opens `until` with the same observer guard `resolve` has
            // (`dist/dev.js:2718-2722`, `if (getObserver()) throw new
            // Error("Cannot call until inside a reactive scope; …")`), and the
            // production bundle drops it (`dist/prod/signals.js:530`, no
            // guard), so the scopes, the dev throw and the proof are
            // resolve's. `until` does not exist before rc.9, so nothing
            // compiled against rc.3's typings reaches this arm.
            if kind == Some(Primitive::Until)
                && let Some(scope) = resolve_tracked_scope(file, call, &allowed, self)
            {
                result.violations.push(StaticViolation {
                    id: "SC2005".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: format!(
                        "until() is called inside {scope}; an active observer makes Solid throw \"Cannot call until inside a reactive scope\" here in dev, and in production every run of the scope starts another root and effect that wait on the predicate"
                    ),
                    hint: "Await until() from imperative code — an action step (`yield until(...)`) or an event handler; on rc.17 an effect's apply and onSettled refuse it too. Inside a computation, read the condition directly: tracked reads re-run the computation when it changes.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
            // The same guard where nothing places the call: a helper that a
            // tracked scope may reach (`createMemo(() => waitReady())`).
            if kind == Some(Primitive::Until) && resolve_scope_unplaced(file, call, &allowed, self)
            {
                result.violations.push(StaticViolation {
                    id: "SC2005".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: "until() is called in code the analysis cannot place; if an observer is active when it runs (a memo or effect compute, or tracked JSX, that reaches it through a helper), Solid throws \"Cannot call until inside a reactive scope\" in dev, and whether one is cannot be proven either way".into(),
                    hint: "Await until() from imperative code — an action step (`yield until(...)`) or an event handler; on rc.17 an effect's apply and onSettled refuse it too. Inside a computation, read the condition directly: tracked reads re-run the computation when it changes.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
            // SC2007: a promise-valued source for rc.9's static `dynamic`
            // form, and rc.17's `dynamicComponent`, whose static form is the
            // same `staticDynamic` (`solid-js` `dist/solid.dev.js:1530`). `DynamicStatic` is only ever the dialect's answer where
            // the resolved `@solidjs/web` has the form (`call_form`), so no
            // other release reaches this arm. Every build calls the source
            // once, untracked, before `dynamic` returns; the dev builds then
            // throw on a thenable (`dist/web.dev.js:2249`,
            // `dist/server.dev.js:3979`) and the production builds fall
            // through to `() => undefined` (`dist/web.js:2080-2090`,
            // `dist/server.js:3729-3733`). `DynamicOptions.static` is a plain
            // `boolean` beside a source typed `() => T | Promise<T> | ...`,
            // so `tsc` accepts every source proven here.
            // ADR 0654: a source not proven synchronous is open, and so is a
            // form the options do not decide (`dynamic(source, options)`).
            if matches!(
                kind,
                Some(Primitive::DynamicStatic | Primitive::DynamicUnknownForm)
            ) && let Some(source) = call.arguments.first()
            {
                let state = static_source_state(file, source, self);
                let unknown_form = kind == Some(Primitive::DynamicUnknownForm);
                let reported = match state {
                    StaticSource::Sync => None,
                    StaticSource::Promise(proof) if !unknown_form => Some((proof, false)),
                    StaticSource::Promise(proof) => Some((proof, true)),
                    StaticSource::Open => Some(("is not proven to return synchronously", true)),
                };
                if let Some((proof, uncertain)) = reported {
                    result.violations.push(StaticViolation {
                        id: "SC2007".into(),
                        rule: "static-dynamic-async-source".into(),
                        message: format!(
                            "{primitive}() is called with {} and a source that {proof}; the static form calls the source once and renders what it returns synchronously, so the dev builds throw \"dynamic(): a static source must resolve synchronously, not to a promise\" here and the production builds render nothing",
                            if unknown_form { "options that may hold static: true" } else { "{ static: true }" }
                        ),
                        hint: format!(
                            "Drop static: true so {primitive}() settles the source in its memo (and render it under a <{}> boundary), or resolve the component before calling {primitive}() and pass a synchronous source.",
                            dialect.boundary_name(solid_dialect::Boundary::Async)
                        ),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: if uncertain {
                            crate::blockers::STATIC_SOURCE_UNPROVEN.to_owned()
                        } else {
                            String::new()
                        },
                        fixes: vec![],
                        uncertain,
                    });
                }
            }
            // SC2006: flush() in an action step. `@solidjs/signals` from
            // 2.0.0-rc.8 opens `flush` with `if (actionStepDepth > 0) throw
            // new Error("[FLUSH_IN_ACTION] …")` in dev (rc.9
            // `dist/dev-shared.js:2210-2219`), and `action`'s `step` raises
            // that depth only around the generator's own `next()`/`throw()`.
            // The production build takes the same branch and returns
            // `fn?.()` without draining. Which primitive throws, on which
            // release, and which callback is stepped are the dialect's; the
            // position proof below is the language's.
            if let Some(kind) = kind
                && dialect.throws_inside_action_step(kind)
                && let Some((body, stepped)) = action_step_body(file, call.span, self)
            {
                result.violations.push(StaticViolation {
                    id: "SC2006".into(),
                    rule: "flush-in-action".into(),
                    message: format!(
                        "{primitive}() is called in an action's {body} body, inside one of its steps; the installed @solidjs/signals throws FLUSH_IN_ACTION here in dev, and in production the call drains nothing, so the writes it was meant to reveal stay held until the action settles"
                    ),
                    hint: "Remove the flush(): an action's writes are held by its transaction and commit when the action settles, so no flush inside the body can reveal them. To observe the result, read after the action resolves (`await save(); …`).".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: stepped,
                });
            }
            // SC2008: a flush() that only ever runs on a user effect's apply
            // callback. The claim is universal: whenever the call runs, the
            // flush that runs effects is already in progress
            // (`globalQueue._running`), so the nested call returns without
            // draining in every client build, and a call that never runs is
            // as pointless, so reachability is no part of the proof. The dev
            // build of `@solidjs/signals` (from rc.1) usually also warns
            // `FLUSH_IN_EFFECT_CALLBACK`, but only while its global
            // `inEffectCallback` marker is still up: a nested synchronous
            // `runEffect` lowers it in its `finally` (rc.13 `dist/dev.js:1906`),
            // so the warning is not part of the claim. Which primitive, on
            // which release, and which callback is the effect phase are the
            // dialect's; the proof that no execution of the call is outside
            // that callback's stack, on the same package instance's queue, is
            // `effect_phase`'s. A server build selects neither.
            if let Some(kind) = kind
                && self.client_build
                && dialect.warns_inside_effect_phase(kind)
                && let Some(root) = self.lookup.callee_package_root(file, call.callee)
                && let Some(origin) = crate::effect_phase::effect_phase_origin(
                    file,
                    call.span,
                    &root,
                    self.package_instances,
                    self.lookup,
                )
            {
                let reached = match origin.helper_calls {
                    0 => format!("directly in {}", origin.callback_label()),
                    1 => format!(
                        "in a function that {} in {} calls",
                        origin.callback_label(),
                        origin.path
                    ),
                    calls => format!(
                        "in a function reached through {calls} calls from {} in {}",
                        origin.callback_label(),
                        origin.path
                    ),
                };
                result.violations.push(StaticViolation {
                    id: "SC2008".into(),
                    rule: "flush-in-effect-callback".into(),
                    message: format!(
                        "{primitive}() is called {reached}; whenever it runs, it runs inside the flush that drains effects, so the queue is already running and this flush drains nothing (the dev build usually also warns FLUSH_IN_EFFECT_CALLBACK), and the writes it was meant to reveal are processed in that flush's continuation, not at this call"
                    ),
                    hint: "Remove the flush(): writes made in an effect are processed by the flush that is already running. To force a drain afterwards, defer it (`queueMicrotask(() => flush())`) or call it from an event handler or other code that runs outside the effect.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: origin.mixed,
                });
            }
            // ADR 0639: `flush()`'s guard takes no event, so a handler dispatched
            // synchronously from an effect's apply warns there. A dispatch whose
            // caller is not placed outside every apply makes a flush in a handler
            // an obligation.
            if let Some(kind) = kind
                && self.client_build
                && dialect.warns_inside_effect_phase(kind)
                && (self.lookup.synchronous_event_flush_dispatch_present()
                    || crate::execution_role::in_denied_handler(file, call.span))
                && !result.violations.iter().any(|violation| {
                    violation.id == "SC2008"
                        && violation.location == location(file.path.shared(), call.callee)
                })
            {
                // A function a listener or a handler may hold: not a body, and
                // not code no delivery runs (a timer, past an await, a test).
                let delivered = self
                    .lookup
                    .file_by_path(file.path.as_str())
                    .is_none_or(|file| !self.lookup.flush_runs_off_any_delivery(file, call));
                if delivered
                    && crate::owners::containing_ast_function(&file.ast, call.callee).is_some_and(
                        |function| !self.lookup.function_may_be_component(file, function),
                    )
                {
                    result.violations.push(StaticViolation {
                        id: "SC2008".into(),
                        rule: "flush-in-effect-callback".into(),
                        message: format!(
                            "{primitive}() runs in a handler that may be delivered synchronously \
                             (a dispatch here, or a state change for an event no call names): \
                             from an effect's apply it would run inside the flush draining effects"
                        ),
                        hint: "Remove the flush(), or the synchronous dispatch from the apply."
                            .into(),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
            }
            // SC3002 (ADR 0625): `onFlush` under a children-capable owner. rc.21
            // warns on `getOwner()` with no `CONFIG_CHILDREN_FORBIDDEN`, the
            // owned-scope write guard's test, so the write role places it: an
            // effect's apply, a leaf and an event handler have no such owner.
            if kind == Some(Primitive::OnFlush) {
                let allowed = crate::execution_role::allowed_callback_spans(file, self.lookup);
                let role = crate::execution_role::semantic_write_execution_role(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    crate::execution_role::RootBodyGuard::Rejects,
                );
                if role.reports_disallowed_write() || role.reports_unplaced() {
                    result.violations.push(StaticViolation {
                        id: "SC3002".into(),
                        rule: "on-flush-in-owned-scope".into(),
                        message: "onFlush() is called in a component body or a computation; it is \
                                  meant for event handlers and effect callbacks"
                            .into(),
                        hint: "Call onFlush from an event handler or an effect's apply, where a \
                               write has an update to stay out of."
                            .into(),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain: role.reports_unplaced(),
                    });
                }
            }
            // SC6002 (ADR 0483): a computation or root created with no owner
            // on a createEffect apply's stack. rc.17's dev build throws
            // PRIMITIVE_IN_EFFECT_CALLBACK there (`assertNotInEffectCallback`,
            // `inEffectCallback && context === null`); which creating calls
            // reach it is the dialect's. The walk crosses no layer that sets an
            // owner. The marker is one global that a nested synchronous effect
            // run lowers, so the throw is proven only where no call runs in the
            // apply before this one; otherwise the claim is uncertifiable.
            if let Some(created) = kind
                && self.client_build
                && dialect.throws_creating_in_effect_callback(
                    created,
                    call.arguments.first().is_some_and(|argument| {
                        !argument.spread
                            && matches!(
                                argument.value,
                                solid_facts::ast::ArgumentValueKind::Function
                                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
                            )
                    }),
                )
                && let Some(root) = self.lookup.callee_package_root(file, call.callee)
                && let Some(origin) = crate::effect_phase::ownerless_effect_phase_origin(
                    file,
                    call.span,
                    &root,
                    self.package_instances,
                    self.lookup,
                )
            {
                // ADR 0495: each layer the walk crossed, and each helper's own
                // body before it calls its parameter (a tagged template runs
                // its tag function too). Within the apply's own text the
                // positional test stands as before.
                let earlier_call =
                    origin.earlier_call
                        || (origin.helper_calls == 0
                            && (file.ast.calls.iter().any(|other| {
                                origin.apply.contains(other.span)
                                    && other.span.end <= call.span.start
                                    && !other.span.contains(call.span)
                            }) || file.ast.tagged_template_tags.iter().any(|tag| {
                                origin.apply.contains(*tag) && tag.end <= call.span.start
                            })));
                let reached = match origin.helper_calls {
                    0 => format!("directly in {}", origin.callback_label()),
                    1 => format!(
                        "in a function that {} in {} calls",
                        origin.callback_label(),
                        origin.path
                    ),
                    calls => format!(
                        "in a function reached through {calls} calls from {} in {}",
                        origin.callback_label(),
                        origin.path
                    ),
                };
                result.violations.push(StaticViolation {
                    id: "SC6002".into(),
                    rule: "primitive-in-effect-callback".into(),
                    message: if earlier_call {
                        format!(
                            "{primitive}() is called {reached}, with no owner set in between; the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK here unless a call that runs earlier in the callback ran an effect synchronously (which lowers the marker the guard reads), and whether one does is not proven"
                        )
                    } else {
                        format!(
                            "{primitive}() is called {reached}, with no owner set in between; the effect phase runs with no owner, so the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK here, and a production build creates a computation nothing owns, never disposed"
                        )
                    },
                    hint: "Create the computation in the effect's compute function or the component body, where an owner holds it, or parent it explicitly: capture `const owner = getOwner()` outside and wrap the creation in `runWithOwner(owner, () => …)`.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    // ADR 0613: the effect phase is the queue's drain, which
                    // a synchronous `flush()` runs under its caller's owner.
                    uncertain: earlier_call
                        || origin.mixed
                        || self.lookup.flush_drains_under_owner(),
                });
            }
            let Some(kind @ (Primitive::Refresh | Primitive::Affects)) = kind else {
                continue;
            };
            let is_refresh = kind == Primitive::Refresh;
            // Zero-argument calls have no target to validate; `affects` also
            // rejects a second key argument's siblings. Extra `refresh`
            // arguments are NOT arity errors: the runtime reads only the
            // first argument and silently ignores the rest (probed, rc.0 —
            // `refresh(source, force)` is legal, if inert).
            let invalid_arity =
                call.arguments.is_empty() || !is_refresh && call.arguments.len() > 2;
            // Reported by `tsc`, so not reported here: both signatures are
            // fixed-arity (`refresh<T>(target: Refreshable<T>)`,
            // `affects(target)` / `affects(target, key)`), so a zero-argument
            // call and an over-long `affects` are TS2554. The call is still
            // skipped: a call with no target proves nothing about a write.
            if invalid_arity {
                continue;
            }
            let target = &call.arguments[0];
            let is_identifier = target.value == ArgumentValueKind::Identifier;
            // A function expression, a literal, `null`, or `undefined` is
            // proven to carry no source brand — flag it without any symbol
            // resolution. Everything else that is not a bare identifier gets
            // a chance to be a member/call chain rooted at a store binding:
            // every child record read through a store proxy carries the
            // brand ($TARGET trap; probed, rc.0), and the docs-canonical
            // `affects(state.user, "name")` / `affects(state.messages.at(-1)!,
            // "status")` are exactly such chains.
            let proven_non_source = matches!(
                target.value,
                ArgumentValueKind::Function
                    | ArgumentValueKind::AsyncFunction
                    | ArgumentValueKind::Null
                    | ArgumentValueKind::Undefined
            ) || target.runtime_value_kind.is_data_literal();
            // Also `tsc`'s. `Refreshable<T> = T & { readonly [$REFRESH]: any }`
            // is the brand *as a type*, so a thunk, a read value, a literal,
            // `null`, and `undefined` are all TS2345 against it; `affects`
            // takes `Accessor<unknown> | Store<object>` and rejects the same
            // set. The type draws exactly the line this rule drew, in both
            // directions — a valid target is accepted — which is what makes it
            // redundant rather than merely stricter.
            if !is_identifier && proven_non_source {
                continue;
            }
            let symbol = if is_identifier {
                self.entities
                    .get(&location(file.path.shared(), target.span))
            } else {
                // Behind TypeScript sugar (`!`, `as`, parentheses), walk the
                // member/call chain down to its root binding. A chain whose
                // base is not resolvable stays unresolved (SC9003) — fail
                // honest, not closed-wrong.
                member_chain_root(file, target.value_span.unwrap_or(target.span))
                    .and_then(|root| self.entities.at(file.path.as_str(), root))
            };
            let Some((symbol, kind)) =
                symbol.and_then(|symbol| Some((symbol, self.source_kinds.get(symbol).copied()?)))
            else {
                // The obligation this used to raise asked whether the target
                // carries the source brand — and the brand is a type
                // (`Refreshable<T>`), so TypeScript answers it completely: an
                // unbranded target is TS2345 and a branded one type-checks.
                // An unprovable target therefore needs no finding, only the
                // absence of a write claim below. ADR 0509: but a branded
                // target this cannot resolve (a prop through a `merge` view,
                // a context member) is still a write, where it runs: the
                // write is unplaced, an obligation, never silent.
                if is_refresh
                    // Only a call TypeScript accepts: an unbranded target is
                    // TS2345, which this never reports (invariant 1).
                    && self
                        .lookup
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
                    && let Some(multiplicity) = self
                        .reachable_calls
                        .get(&location(file.path.shared(), call.callee))
                        .copied()
                {
                    let role = crate::local_access::handed_write_execution(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                        RootBodyGuard::RejectsOutsideDisposal,
                        semantic_write_execution_role(
                            file,
                            call.callee,
                            &allowed,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                            RootBodyGuard::RejectsOutsideDisposal,
                        ),
                    );
                    if role.reports_disallowed_write() || role.reports_unplaced() {
                        // One record standing for every reachable path.
                        if multiplicity > 0 {
                            let write = ReactiveWrite {
                                setter: format!(
                                    "refresh({})",
                                    file.source_text(target.span).unwrap_or("…")
                                )
                                .into(),
                                operation: crate::ReactiveWriteOperation::Refresh,
                                source_kind: ReactiveSourceKind::Accessor,
                                location: location(file.path.as_str(), call.span),
                                declaration: location(file.path.as_str(), target.span),
                                execution: crate::ExecutionRole::Unknown,
                                owned_write: Default::default(),
                                context: analysis_context(
                                    file,
                                    call.span,
                                    self.entities,
                                    self.symbol_names,
                                    dialect,
                                    self.lookup,
                                )
                                .into(),
                                multiplicity,
                            };
                            result.writes.push(write);
                        }
                    }
                }
                continue;
            };
            // A member or call chain rooted at an accessor reads a plain
            // value off the accessor function — never a branded source
            // (probed: refresh(memo.name) and affects(memo.name) both throw
            // INVALID_*_TARGET in dev). Chains on store bases are accepted
            // above the brand: child proxies carry it.
            // `refresh(memo.name)` reads a plain property off the accessor
            // function, which carries no brand — and is therefore TS2345
            // against `Refreshable<T>` just like the wrapper forms above.
            if !is_identifier && kind == ReactiveSourceKind::Accessor {
                continue;
            }
            // Only the derived store forms own a compute node the runtime
            // can re-run. A value-form `createStore(obj)` store — or any
            // child record of one — is not refreshable, and refresh() on it
            // throws INVALID_REFRESH_TARGET in dev (probed, rc.0). A store
            // whose construction form is unknown (opaque packages, unproven
            // argument shapes) is never in this set, so acceptance is kept.
            // The value form's return type is not `Refreshable`: only
            // `createStore(fn, initial)`, `createProjection`, and the
            // function-form optimistic store brand their result, so
            // `refresh(valueFormStore)` is TS2345 (verified against
            // `@solidjs/signals@2.0.0-rc.0`). Still skipped, because a call
            // the runtime rejects records no write.
            if is_refresh
                && kind == ReactiveSourceKind::Store
                && self.value_form_stores.contains(symbol)
            {
                continue;
            }
            // A key on an accessor target selects the one-argument `affects`
            // overload, whose second parameter does not exist, so TypeScript
            // reports TS2345 on the key itself. Nothing left for SC7004.
            if !is_refresh {
                continue;
            }
            if file.ast.any_function_body_containing(call.span) {
                result.write_action_obligations.push((
                    "write",
                    file.path.to_string(),
                    u64::from(call.callee.start),
                    u64::from(call.callee.end),
                ));
            }
            let callee = location(file.path.shared(), call.callee);
            let Some(multiplicity) = self.reachable_calls.get(&callee).copied() else {
                continue;
            };
            let Some((name, declaration)) = self.accessors.get(symbol) else {
                continue;
            };
            if multiplicity == 0 {
                continue;
            }
            // One record standing for every reachable path: its role is
            // decided once.
            let write = ReactiveWrite {
                setter: format!("refresh({name})").into(),
                operation: crate::ReactiveWriteOperation::Refresh,
                source_kind: kind,
                location: location(
                    file.path.as_str(),
                    Span::new(
                        call.span.start,
                        call.arguments
                            .last()
                            .map_or(call.span.end, |argument| argument.span.end),
                    ),
                ),
                declaration: declaration.clone(),
                // `refresh`'s guard has no root exemption on any release
                // (probed rc.0-rc.9, dev).
                execution: crate::local_access::handed_write_execution(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    RootBodyGuard::RejectsOutsideDisposal,
                    semantic_write_execution_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                        RootBodyGuard::RejectsOutsideDisposal,
                    ),
                ),
                owned_write: self
                    .source_owned_write
                    .get(symbol)
                    .copied()
                    .unwrap_or_default(),
                context: analysis_context(
                    file,
                    call.span,
                    self.entities,
                    self.symbol_names,
                    dialect,
                    self.lookup,
                )
                .into(),
                multiplicity,
            };
            result.writes.push(write);
        }
        result
    }
}

/// The tracked scope a `resolve()` call provably runs in, named for the
/// message, or `None` where the call is legal or unprovable.
///
/// The decision mirrors the probed rc.0 guard, which keys on the *observer*:
///
/// - The innermost callback the call sits directly in (no intervening
///   function) decides first: a tracked, non-deferred callback — a memo or
///   effect compute, `createTrackedEffect`, a boundary body — reports;
///   `untrack`, `onSettled`, apply callbacks, and every other deferred or
///   inline-untracked scope is legal and wins even inside a memo. An
///   unresolvable container stays silent.
/// - With no direct callback container, a call inside a compiler-proven
///   tracked JSX region reports (JSX expressions run in render-effect
///   computes); component bodies, module scope, and helpers stay silent —
///   the component body runs with no observer (probed).
fn resolve_tracked_scope(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
) -> Option<String> {
    // ADR 0612: the guard reads the observer, which no task past an await has.
    // Past one on some path nothing is proven ([`resolve_scope_unplaced`]).
    if crate::execution_role::past_suspension(file, call.span, context.lookup).is_some() {
        return None;
    }
    // ADR 0494: a call in function layers that run in place inside the
    // callback (ADR 0469's climb, as ADR 0478 reads it) runs on the stack of
    // the outermost layer's call, with the callback's observer active; not in
    // a parameter list, not after an await.
    let at = match crate::execution_role::climb_in_place_layers(file, call.span, context.lookup) {
        // A library callback may run zero times (ADR 0493 review), so it is
        // not crossed.
        Some(climb)
            if !climb.exhausted
                && !climb.in_list
                && climb.suspended.is_none()
                && !crate::execution_role::climb_crosses_library_callback(
                    file,
                    call.span,
                    climb.at,
                    context.lookup,
                ) =>
        {
            climb.at
        }
        _ => call.span,
    };
    if let Some((container, index)) =
        file.ast
            .arguments_containing(at)
            .find(|(container, index)| {
                crate::execution_role::direct_callback_runs(
                    file,
                    container.arguments[*index].span,
                    at,
                )
            })
    {
        let primitive = context.lookup.primitive_at_call(file, container.span)?;
        let tracked = crate::owners::callback_execution_at_call(
            file,
            container,
            primitive,
            index,
            context.lookup,
        ) == Some(solid_dialect::Execution::Tracked)
            && !context.lookup.dialect.runs_callback_deferred(primitive);
        return tracked.then(|| {
            format!(
                "the tracked {} callback",
                context
                    .lookup
                    .dialect
                    .name_of(primitive)
                    .unwrap_or("compute")
            )
        });
    }
    // ADR 0517: the climbed call runs in the body of a function a tracked
    // position names by its binding (`createMemo(compute)`, ADR 0516's
    // `named_at_tracked_position`), which holds that function now: every run
    // of it there has the memo's observer active.
    if let Some(function) = crate::owners::containing_body_function(&file.ast, at)
        && context.lookup.function_component_status(file, function)
            == crate::indexes::ComponentStatus::No
        && crate::execution_role::named_at_tracked_position(file, at, context.lookup)
        && context.lookup.function_value_is_current(file, function)
    {
        return Some("the tracked compute callback named by its binding".to_owned());
    }
    let in_tracked_region = file
        .compiler
        .tracked_regions
        .iter()
        .any(|region| region.span.contains(call.span));
    (in_tracked_region
        && crate::execution_role::semantic_execution_role(
            file,
            call.span,
            allowed,
            context.entities,
            context.symbol_names,
            context.lookup,
        ) == crate::ExecutionRole::TrackedJsx)
        .then(|| "tracked JSX".to_owned())
}

/// Whether a `resolve`/`until` call that [`resolve_tracked_scope`] does not
/// report is still not proven free of an observer: an obligation unless a
/// fact proves no observer is active where it runs.
///
/// A callback of a primitive whose callback execution the dialect states is
/// that function's to judge. Otherwise the call is proven only in a role that
/// clears the observer (a component body, `untrack`, an effect apply, an
/// event, a directive application, module scope, a deferred callback, or
/// deleted code), and only when a body-written literal is not handed to a
/// consumer that may run it later. A tracked role [`resolve_tracked_scope`]
/// did not place (a named callback, `createMemo(read)`), `Unreported` (whose
/// mix may hold a tracked run) and `Unknown` are obligations.
fn resolve_scope_unplaced(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
) -> bool {
    use crate::ExecutionRole;
    if resolve_tracked_scope(file, call, allowed, context).is_some() {
        return false;
    }
    // ADR 0612: past an await on every path no observer is active; on some
    // path the call may still run on the compute's first stack.
    match crate::execution_role::past_suspension(file, call.span, context.lookup) {
        Some(true) => return false,
        Some(false) => return true,
        None => {}
    }
    // ADR 0517: a library callback on the way (`[1].forEach`) may run zero
    // times, so `resolve_tracked_scope` does not climb it; when the layers
    // beyond it reach a compute a tracked position names by its binding, the
    // call runs under that observer whenever it runs: an obligation.
    if let Some(climb) =
        crate::execution_role::climb_in_place_layers(file, call.span, context.lookup)
        && !climb.exhausted
        && !climb.in_list
        && climb.suspended.is_none()
        && crate::owners::containing_body_function(&file.ast, climb.at).is_some_and(|function| {
            context.lookup.function_component_status(file, function)
                == crate::indexes::ComponentStatus::No
                && crate::execution_role::named_at_tracked_position(file, climb.at, context.lookup)
        })
    {
        return true;
    }
    let mut judged_by_primitive = false;
    let mut steps_on_callers = false;
    for (container, index) in file.ast.arguments_containing(call.span) {
        if !crate::execution_role::direct_callback_runs(
            file,
            container.arguments[index].span,
            call.span,
        ) {
            continue;
        }
        let Some(primitive) = context.lookup.primitive_at_call(file, container.span) else {
            continue;
        };
        // ADR 0614: a `flush()` under a computation drains this callback with
        // that computation's observer active (ADR 0613).
        if context
            .lookup
            .dialect
            .callback_runs_drained_by_flush(primitive, index)
            && context.lookup.flush_drains_under_owner()
        {
            return true;
        }
        steps_on_callers |= primitive == solid_dialect::Primitive::Action;
        judged_by_primitive |= crate::owners::callback_execution_at_call(
            file,
            container,
            primitive,
            index,
            context.lookup,
        )
        .is_some();
    }
    // ADR 0614: an action's first step runs on its caller's stack, under the
    // caller's observer (ADR 0547), so its callers answer, not the primitive:
    // a read role cannot, since a tracked caller reports no read.
    if steps_on_callers {
        let resumed = crate::owners::action_step_resumption(file, call.span, context.lookup);
        return resumed != Some(crate::owners::StepResumption::Proven)
            && crate::owners::containing_body_function(&file.ast, call.span)
                .and_then(|step| {
                    crate::execution_role::action_step_call_sites(file, step, context.lookup)
                })
                .is_none_or(|calls| {
                    calls.iter().any(|caller| {
                        resolve_tracked_scope(file, caller, allowed, context).is_some()
                            || resolve_scope_unplaced(file, caller, allowed, context)
                    })
                });
    }
    if judged_by_primitive {
        return false;
    }
    let role = crate::execution_role::read_execution_role(
        file,
        call.span,
        allowed,
        context.entities,
        context.symbol_names,
        context.lookup,
    );
    let observer_cleared = matches!(
        role,
        ExecutionRole::UntrackedRendering
            | ExecutionRole::UntrackedCallback
            | ExecutionRole::UntrackedUnwindowed
            | ExecutionRole::EffectApply
            | ExecutionRole::EventCallback
            | ExecutionRole::DirectiveApply
            | ExecutionRole::ModuleInitialization
            | ExecutionRole::DeferredCallback
            | ExecutionRole::DiscardedRendering
    );
    // ADR 0614: a deferred role inside tracked JSX is no proof the observer is
    // clear: an IIFE in a flow child's expression runs under the render effect.
    let deferred_in_jsx = role == ExecutionRole::DeferredCallback
        && file
            .compiler
            .tracked_regions
            .iter()
            .any(|region| region.span.contains(call.span));
    !observer_cleared
        || deferred_in_jsx
        || crate::execution_role::callee_callback_timing(file, call.span, role, context.lookup)
}

/// Why a `dynamic` source provably returns a Promise, phrased for the
/// message, or `None` where that is not proven.
///
/// Two proofs, and nothing else:
///
/// - the source is an `async` function (not an async generator, whose
///   iterator has no `then`), which returns a native Promise on every call;
/// - the source is an expression-bodied, non-async arrow whose returned
///   expression is exactly a call the compiler resolved to the standard
///   library's `PromiseConstructor.resolve` or its construct signature
///   (`Promise.resolve(...)`, `new Promise(...)`). A same-named project
///   declaration, a shadowed `Promise`, or an unresolved call proves nothing.
///
/// The source may be written inline or named by an identifier that resolves,
/// through Type Facts, to a same-file `function` declaration or a `const`
/// initialized with a function literal. A block-bodied non-async function, a
/// parameter, an import, a reassignable binding, and every other source stay
/// silent: whether they return a Promise is not decided here.
fn static_source_state(
    file: &FileFacts,
    source: &solid_facts::ast::ArgumentFact,
    context: &StaticApiContext<'_>,
) -> StaticSource {
    let Some(function) = static_source_function(file, source, context) else {
        return StaticSource::Open;
    };
    // A generator's iterator has no `then`.
    if function.generator {
        return StaticSource::Sync;
    }
    if let Some(proof) = static_source_promise(file, function, context) {
        return StaticSource::Promise(proof);
    }
    let lookup = context.lookup;
    if crate::owners::computation_is_async(lookup, file, function.span) {
        return if crate::owners::computation_may_answer_sync(lookup, file, function.span) {
            StaticSource::Open
        } else {
            StaticSource::Promise("returns a Promise, by its type")
        };
    }
    // The producer's fact, or the absence of a proof: a closed return type
    // with no constituent that is a thenable.
    let closed = lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|fact| {
                !fact.return_open
                    && u64::from(function.span.start) == fact.expression.start_byte
                    && u64::from(function.span.end) == fact.expression.end_byte
            })
        });
    if closed {
        StaticSource::Sync
    } else {
        StaticSource::Open
    }
}

/// What a static `dynamic()` source does on its one call (ADR 0654).
enum StaticSource {
    Promise(&'static str),
    Sync,
    Open,
}

fn static_source_function<'a>(
    file: &'a FileFacts,
    source: &solid_facts::ast::ArgumentFact,
    context: &StaticApiContext<'_>,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    if source.spread {
        return None;
    }
    match source.value {
        ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction => {
            crate::cleanup::callback_argument_literal(file, source.span)
        }
        ArgumentValueKind::Identifier => {
            let declaration = source.binding_declaration?;
            let function = crate::static_rules::same_file_function(file, declaration)?;
            // The binder's declaration only chose what to ask; the proof is
            // that the compiler resolves the reference and the declared name
            // to one symbol.
            let path = file.path.as_str();
            let referenced = context.entities.at(path, source.span)?;
            let declared = context.entities.at(path, declaration)?;
            (referenced == declared).then_some(function)
        }
        _ => None,
    }
}

fn static_source_promise(
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
    context: &StaticApiContext<'_>,
) -> Option<&'static str> {
    if function.r#async {
        return Some("is an async function, which always returns a Promise");
    }
    if !function.expression_body {
        return None;
    }
    let returned = function.expression_return.as_ref()?;
    let callee = returned.callee?;
    let call = file
        .ast
        .calls
        .iter()
        .find(|call| call.callee == callee && call.span == returned.span)?;
    let resolved = context.lookup.resolved_callee_call(file, call.callee)?;
    if resolved.validity != typefacts::ResolvedCallValidity::Valid {
        return None;
    }
    let declaration = resolved.declaration.as_ref()?;
    if !declaration.standard_library {
        return None;
    }
    match (
        declaration.qualified_name.as_ref(),
        resolved.kind,
        call.construct,
    ) {
        ("PromiseConstructor.resolve", typefacts::CallKind::Call, false) => {
            Some("returns Promise.resolve(...)")
        }
        ("PromiseConstructor.construct", typefacts::CallKind::Construct, true) => {
            Some("returns new Promise(...)")
        }
        _ => None,
    }
}

/// The action body a call provably runs inside a step of, named for the
/// message, or `None` where that is not proven.
///
/// Probed on `@solidjs/signals` rc.0-rc.9, dev and prod (`step` brackets
/// `it.next(v)` with the action-step marker):
///
/// - The call must sit **directly** in the body of a generator function
///   written as the stepped argument ([`Dialect::callback_runs_as_action_steps`])
///   -- not in a nested function (a helper, an `untrack` callback, a
///   `setTimeout` or `.then` callback), and not in a parameter initializer,
///   which runs when `genFn(...args)` creates the generator, before the first
///   step. A helper called synchronously from the body does throw at runtime;
///   the proof is lexical and does not claim it.
/// - A **sync generator's** body runs entirely inside steps: after a `yield`,
///   a `yield promise` or a `yield*`, the next step resumes it synchronously.
/// - An **async generator's** body is inside a step only until its first
///   suspension; the continuation of an `await`, a `for await` or a `yield*`
///   runs from a microtask with no step on the stack. So the call is claimed
///   only when no suspension of this body can run before it: every `await` of
///   the body that starts before the call ends must contain it (it is the
///   awaited operand, evaluated first), no implicit suspension may start
///   before it, and no loop around it may contain a suspension that a later
///   iteration would pass first. A call after a suspension that a later
///   `yield` re-enters a step for is not claimed.
///
/// ADR 0516: a call in a sync generator called in place in that body
/// (`(function* () { flush(); })().next()`) runs inside the step where its
/// iterator is stepped there, which nothing follows: it is claimed as an
/// obligation (the `bool`), never a violation.
fn action_step_body(
    file: &FileFacts,
    site: Span,
    context: &StaticApiContext<'_>,
) -> Option<(&'static str, bool)> {
    action_step_body_bounded(file, site, context, 4)
}

/// [`action_step_body`] with `depth` unplaced layers left to cross
/// ([`action_step_body_unplaced`]).
fn action_step_body_bounded(
    file: &FileFacts,
    site: Span,
    context: &StaticApiContext<'_>,
    depth: usize,
) -> Option<(&'static str, bool)> {
    if let Some(body) = action_step_body_at(file, site, context) {
        return Some((body, false));
    }
    action_step_body_through(file, site, context, 4)
        .or_else(|| {
            // ADR 0537: a site inside an enclosing named function some call
            // of which runs in a step, where the layers between them are not
            // placed (a member's value, a climb past its bound): an
            // obligation.
            crate::execution_role::functions_around(file, site)
                .into_iter()
                .filter(|function| function.body.contains(site) && function.body != site)
                .find_map(|function| action_step_body_through(file, function.body, context, 4))
        })
        .or_else(|| action_step_body_unplaced(file, site, context, depth).map(|body| (body, true)))
}

/// ADR 0538: the site runs where no layer places it, and the code around
/// that layer is in a step: an obligation, never a violation. The layer is
/// one of: the in-place climb from the site reaching ADR 0532's bound (the
/// layers past it are unread, so the position is asked again from where the
/// climb stopped); the function around the climb written as a member's value
/// or an object slot, or handed whole by name to project code or an
/// unresolved call ([`crate::local_access::run_unplaced`]); a method of a
/// class or object literal written inside a function. A layer proven to run
/// elsewhere (after an await, a scheduled callback, a kept listener, a
/// generator) is not crossed: the enclosing proof refuses it as before.
fn action_step_body_unplaced(
    file: &FileFacts,
    site: Span,
    context: &StaticApiContext<'_>,
    depth: usize,
) -> Option<&'static str> {
    if depth == 0 {
        return None;
    }
    let at = match crate::execution_role::climb_in_place_layers(file, site, context.lookup) {
        Some(climb) if climb.in_list || climb.suspended.is_some() => return None,
        Some(climb) if climb.exhausted => {
            return action_step_body_bounded(file, climb.at, context, depth - 1)
                .map(|(body, _)| body);
        }
        Some(climb) => climb.at,
        None => site,
    };
    let function = crate::owners::owning_function(&file.ast, at)
        .filter(|function| function.body.contains(at) && !function.generator)?;
    let anchor = if function.method_name.is_some() {
        file.ast
            .classes
            .iter()
            .filter(|class| class.span.contains(function.span))
            .min_by_key(|class| class.span.end - class.span.start)
            .map_or(function.span, |class| class.span)
    } else if crate::local_access::run_unplaced(file, function, context.lookup) {
        function.span
    } else {
        return None;
    };
    // Module-level code runs in no step.
    crate::owners::containing_ast_function(&file.ast, anchor)?;
    action_step_body_bounded(file, anchor, context, depth - 1).map(|(body, _)| body)
}

/// ADR 0516: the layers [`action_step_body_at`] does not cross, as an
/// obligation: a sync generator called in place (its body runs where its
/// iterator is stepped), and a named function some exact call of which runs
/// in a step (other calls may run elsewhere, and the proof is lexical).
fn action_step_body_through(
    file: &FileFacts,
    site: Span,
    context: &StaticApiContext<'_>,
    depth: usize,
) -> Option<(&'static str, bool)> {
    if depth == 0 {
        return None;
    }
    let at = match crate::execution_role::climb_in_place_layers(file, site, context.lookup) {
        Some(climb) if !climb.exhausted && !climb.in_list && climb.suspended.is_none() => climb.at,
        Some(_) => return None,
        None => site,
    };
    let function = crate::owners::owning_function(&file.ast, at)
        .filter(|function| function.body.contains(at) && function.method_name.is_none())?;
    if file
        .ast
        .arguments_containing(function.span)
        .any(|(call, index)| {
            file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
        })
    {
        return None;
    }
    let mut calls = Vec::new();
    if function.generator {
        if function.r#async {
            return None;
        }
        calls.extend(
            crate::owners::generator_iterator_used(file, function).map(|call| (file, call.span)),
        );
    } else if !function.r#async
        && crate::owners::function_binding_name(file, function).is_some()
        && context.lookup.function_value_is_current(file, function)
    {
        for (caller, callee) in context
            .lookup
            .function_call_sites(file.path.as_str(), function.span)
        {
            let Some(call) = caller.ast.calls.iter().find(|call| {
                !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
            }) else {
                continue;
            };
            let exact =
                crate::execution_role::project_callee_at_parameter(caller, call, 0, context.lookup)
                    .is_some_and(|(callee_file, target, exact)| {
                        exact
                            && callee_file.path.as_str() == file.path.as_str()
                            && target.span == function.span
                    });
            if exact {
                calls.push((caller, call.span));
            }
        }
        // ADR 0537: a carrier it is handed to by name runs it in place there.
        calls.extend(
            crate::local_access::inline_carriers(file, function, context.lookup)
                .into_iter()
                .map(|carrier| (file, carrier)),
        );
    }
    calls.into_iter().find_map(|(caller, call)| {
        action_step_body_at(caller, call, context)
            .or_else(|| {
                action_step_body_through(caller, call, context, depth - 1).map(|(body, _)| body)
            })
            .map(|body| (body, true))
    })
}

fn action_step_body_at(
    file: &FileFacts,
    site: Span,
    context: &StaticApiContext<'_>,
) -> Option<&'static str> {
    // ADR 0478: a call in function layers that run in place inside the body
    // (an IIFE, an inline standard-library callback, a helper's callback it
    // calls in its own body; ADR 0469's climb) runs on the stack of the
    // outermost layer's call, so that call is the position the step proof
    // reads. A layer in a parameter list runs only when its argument is
    // omitted, and a site after an await in its layer runs on a later task:
    // neither is claimed. One in-place run proves the throw, so a helper that
    // also hands its callback on still counts.
    let at = match crate::execution_role::climb_in_place_layers(file, site, context.lookup) {
        Some(climb) if !climb.exhausted && !climb.in_list && climb.suspended.is_none() => climb.at,
        Some(_) => return None,
        None => site,
    };
    let (container, index) = file
        .ast
        .arguments_containing(at)
        .find(|(container, index)| {
            crate::execution_role::direct_callback_runs(file, container.arguments[*index].span, at)
        })?;
    let argument = &container.arguments[index];
    if argument.spread {
        return None;
    }
    let primitive = context.lookup.primitive_at_call(file, container.span)?;
    if !context
        .lookup
        .dialect
        .callback_runs_as_action_steps(primitive, index)
    {
        return None;
    }
    // The generator must *be* the argument (behind TypeScript sugar at
    // most): `action(wrap(function* () { … }))` steps whatever `wrap`
    // returns.
    let written = argument.value_span.unwrap_or(argument.span);
    // ADR 0454: a call in the generator's parameter list runs when the
    // generator is created, before its first step.
    let body = crate::owners::containing_body_function(&file.ast, at)
        .filter(|function| function.span == written && function.generator)?;
    if !body.r#async {
        return Some("sync generator");
    }
    crate::effect_phase::runs_before_first_suspension(file, body, at).then_some("async generator")
}

/// The root expression span of a member/call chain: `state.user` and
/// `state.messages.at(-1)` both resolve to `state`. Property accesses
/// (static and computed) and calls on member callees are followed because a
/// store proxy brands everything reached through it — `.at(...)` and index
/// access on a store base return branded child records. The returned span is
/// whatever the chain bottoms out on; the caller decides whether it resolves
/// to a source binding. Depth-bounded against pathological chains.
fn member_chain_root(file: &FileFacts, span: Span) -> Option<Span> {
    let mut current = span;
    for _ in 0..64 {
        if let Some(member) = file.ast.member_at(current) {
            current = member.object;
            continue;
        }
        if let Some(call) = file.ast.calls.iter().find(|call| call.span == current) {
            current = call.callee;
            continue;
        }
        return Some(current);
    }
    None
}
