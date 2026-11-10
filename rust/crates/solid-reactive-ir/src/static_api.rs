//! Static validation for Solid 2.0 API shapes and explicit refresh writes.

use crate::{HashMap, HashSet};
use std::cell::Cell;

use solid_dialect::{Primitive, SlotObserver, SlotOwner, SlotRun, SlotThrows};
use solid_facts::FileFacts;
use solid_facts::ast::ArgumentValueKind;
use solid_facts::core::Span;
use typefacts::Location;

use crate::effect_api::{ProofStatus, classify_effect_call};
use crate::execution_role::{RootBodyGuard, allowed_callback_spans, semantic_write_execution_role};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use crate::owners::{analysis_context, computation_is_async};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::stored_throw::{ThrowEnd, Thrower, throw_end};
use crate::{
    ReactiveSourceKind, ReactiveWrite, StaticDefect, StaticDefectKind, StaticViolation, location,
};

#[derive(PartialEq)]
pub(crate) struct StaticDirectiveFileResult {
    pub(super) defects: Vec<StaticDefect>,
    pub(super) violations: Vec<StaticViolation>,
    pub(super) writes: Vec<ReactiveWrite>,
    pub(super) write_action_obligations: Vec<(&'static str, String, u64, u64)>,
    /// ADR 0883: JSX SC6002 places on an apply's stack, and whether its proof
    /// answers for every run: `pipeline` reads that for the SC4001 row.
    pub(super) jsx_phase_sites: Vec<(Location, bool)>,
    /// ADR 0936: the silences the file's checks named for the sites they gave
    /// no row (an `onFlush` call the write role allows), reused with the chunk.
    pub(super) ends: crate::site_accounting::SiteEnds,
}

/// Runs the project-level static API stage and merges its independent file
/// results into the program draft. Each file's result is a late chunk
/// ([`crate::late_chunks`]): reused while the file and every read it recorded
/// (the lookup's answers and hand-outs, the source tables' entries it asked,
/// reachability's multiplicities, the client-build answer) answer as they did.
pub(crate) fn check_project(
    ctx: &AnalysisContext<'_>,
    chunks: Option<&mut crate::late_chunks::FileChunks<StaticDirectiveFileResult>>,
    draft: &mut ProgramDraft,
) {
    let checker = context_of(ctx);
    let checker = StaticApiContext {
        tables: checker.tables.with_client_build(checker.client_build),
        ..checker
    };
    let mut inputs = crate::late_chunks::ChunkInputs::of(ctx);
    inputs.discovery = checker.tables;
    for result in
        crate::late_chunks::FileChunks::run(chunks, &inputs, |file| checker.check_file(file))
    {
        draft.static_defects.extend(result.defects.iter().cloned());
        draft
            .static_violations
            .extend(result.violations.iter().cloned());
        draft.writes.extend(result.writes.iter().cloned());
        draft
            .write_action_obligations
            .extend(result.write_action_obligations.iter().cloned());
        draft
            .jsx_phase_sites
            .extend(result.jsx_phase_sites.iter().cloned());
        draft.site_ends.extend(&result.ends);
    }
}

pub(crate) fn context_of<'a>(ctx: &AnalysisContext<'a>) -> StaticApiContext<'a> {
    StaticApiContext {
        lookup: ctx.semantic_lookup,
        entities: ctx.entities,
        symbol_names: ctx.symbol_names,
        tables: crate::late_chunks::DiscoveryTables::of(ctx),
        client_build: !ctx.rule_options.runtime.selects_server_build()
            || crate::execution_paths::client_root_present(ctx.facts, ctx.dialect),
        package_instances: &ctx.rule_options.runtime.package_instances,
    }
}

/// ADR 0893: a primitive whose failure is a plain throw, and every entry read
/// ends in a stored error no read reports.
fn plain_throw_unobserved(
    file: &FileFacts,
    site: Span,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
    plain: bool,
    thrower: Thrower,
) -> bool {
    plain && throw_end(file, site, allowed, context, thrower) == ThrowEnd::Unobserved
}

fn unobserved_note(unobserved: bool) -> &'static str {
    if unobserved {
        "; but the throw is stored on a computation or taken by a `try` or an error boundary, and no read that reports it is proven"
    } else {
        ""
    }
}

pub(super) struct StaticApiContext<'a> {
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    /// The source tables, reachability and the client-build answer, read
    /// through the recorded accessors (a file's result is a late chunk).
    pub(super) tables: crate::late_chunks::DiscoveryTables<'a>,
    /// No server build of the Solid packages is selected, or a client root
    /// is present: read through [`Self::client_build`], recorded.
    client_build: bool,
    /// Which installed `solid-js` each file loads at runtime.
    pub(super) package_instances: &'a crate::PackageInstanceEvidence,
}

impl StaticApiContext<'_> {
    /// No server build of the Solid packages is selected, or a client root
    /// is present (a fact of every file): recorded as one read.
    fn client_build(&self) -> bool {
        self.tables.note_client_build(self.client_build);
        self.client_build
    }
}

/// ADR 0830: a function-literal argument of `call` returns a value that is
/// neither a function literal nor absent, on a line `tsc` may not check: the
/// published type (`(() => void) | void`) is what rejects a non-cleanup there.
fn cleanup_return_unchecked(file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
    use solid_facts::ast::ReturnValueKind;
    call.arguments.iter().any(|argument| {
        let span = file.ast.peel_ts_sugar_span(argument.span);
        file.ast
            .functions
            .iter()
            .find(|function| function.span == span)
            .is_some_and(|function| {
                function
                    .expression_return
                    .iter()
                    .chain(file.ast.returns.iter().filter(|returned| {
                        crate::owners::containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    }))
                    .filter(|returned| {
                        !matches!(
                            returned.value,
                            ReturnValueKind::Function | ReturnValueKind::Undefined
                        )
                    })
                    .any(|returned| {
                        file.tsc_may_be_silent_within(returned.argument.unwrap_or(returned.span))
                    })
            })
    })
}

impl StaticApiContext<'_> {
    /// ADR 0710: a role that runs off every effect apply's stack: an event
    /// handler (unless a synchronous dispatch may deliver it from an apply),
    /// a deferred callback, a directive apply, rendering, module code, or
    /// code the compiler discarded.
    fn runs_off_every_apply(&self, file: &FileFacts, site: Span, allowed: &[Span]) -> bool {
        use crate::ExecutionRole as Role;
        match crate::execution_role::semantic_execution_role(
            file,
            site,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        ) {
            Role::DeferredCallback
            | Role::DirectiveApply
            | Role::TrackedJsx
            | Role::UntrackedRendering
            | Role::DiscardedRendering
            | Role::ModuleInitialization => true,
            Role::EventCallback => !self.lookup.synchronous_event_dispatch_present(),
            _ => false,
        }
    }

    pub(super) fn check_file(&self, file: &FileFacts) -> StaticDirectiveFileResult {
        let mut result = StaticDirectiveFileResult {
            defects: Vec::new(),
            violations: Vec::new(),
            writes: Vec::new(),
            write_action_obligations: Vec::new(),
            jsx_phase_sites: Vec::new(),
            ends: crate::site_accounting::SiteEnds::default(),
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
                // ADR 0884: the compiler says this call is a primitive, and no
                // rule can read it as one: the operation must not go unseen.
                if let Some(name) = crate::owners::aliased_primitive(file, call, self.lookup) {
                    for (id, rule) in alias_claims(dialect, &name) {
                        result.violations.push(StaticViolation {
                            id: (*id).into(),
                            rule: (*rule).into(),
                            message: format!(
                                "This call resolves to Solid's {name} through a name solid-checker cannot follow, so whether {rule} applies here cannot be proven"
                            ),
                            hint: "Call the primitive directly by its imported name.".into(),
                            location: location(file.path.shared(), call.callee),
                            analysis_context: crate::blockers::PRIMITIVE_ALIAS_CALL.into(),
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                    result.defects.push(StaticDefect {
                        kind: StaticDefectKind::ReactiveDispatchUnresolved {
                            callee: name,
                            member: None,
                        },
                        location: location(file.path.shared(), call.callee),
                        analysis_context: crate::blockers::PRIMITIVE_ALIAS_CALL.into(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
                continue;
            };
            // `primitive` stays as the spelling to report; `kind` is what the
            // rules below branch on. A name this dialect does not export
            // resolves to `None` and matches nothing.
            let kind = primitive.primitive();
            // ADR 0701: every primitive the dialect gives an effect slot.
            let effect_semantics = kind
                .filter(|kind| dialect.effect_function_slot(*kind).is_some())
                .map(|kind| classify_effect_call(file, call, kind, self.lookup));
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
            } else if let Some(kind) = kind
                && file.tsc_may_be_silent_somewhere()
                && (self
                    .lookup
                    .resolved_callee_call(file, call.callee)
                    .is_some_and(|resolved| {
                        resolved.validity == typefacts::ResolvedCallValidity::Recovery
                    })
                    && file.tsc_may_be_silent_within(call.span)
                    || dialect.accepts_cleanup_return(kind) && cleanup_return_unchecked(file, call))
            {
                // ADR 0830: TypeScript rejects this primitive's call, or a
                // value a callback of it returns as its cleanup, and on that
                // line nothing may say so. The claims the type owns there (a
                // retired rule's: an apply's return, a refresh or affects
                // target, an `onSettled` return) are open.
                result.defects.push(StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file
                            .source_text(call.callee)
                            .unwrap_or("this call")
                            .to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), call.callee),
                    analysis_context: crate::blockers::SOLID_CALL_REJECTED_UNCHECKED.to_owned(),
                    fixes: vec![],
                    uncertain: true,
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
                // ADR 0865: `handleAsync` probes `Symbol.asyncIterator` as well
                // as `then`, unlike the static `dynamic()` check.
                Some(match static_source_state(file, argument, self) {
                    StaticSource::AsyncIterable => StaticSource::Promise(""),
                    state => state,
                })
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
                // ADR 0893: the throw may end stored on a compute no read reports.
                let unobserved = plain_throw_unobserved(
                    file,
                    call.span,
                    &allowed,
                    self,
                    dialect.plain_throw(Primitive::Resolve),
                    Thrower::UnderObserver,
                );
                result.violations.push(StaticViolation {
                    id: "SC2004".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: format!(
                        "resolve() is called inside {scope}; resolve() reads the expression once and never tracks updates, and an active observer makes Solid throw \"Cannot call resolve inside a reactive scope\" here in dev{}",
                        unobserved_note(unobserved)
                    ),
                    hint: "Call resolve() from imperative code, such as an event handler; on rc.17 an effect's apply and onSettled refuse it too. To depend on a pending value inside a computation, read the accessor directly: tracked reads suspend and re-run on their own. A deliberate one-shot read can be wrapped in untrack(), which clears the observer the runtime guards on.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: if unobserved {
                        crate::blockers::COMPUTATION_ERROR_UNOBSERVED.to_owned()
                    } else {
                        String::new()
                    },
                    fixes: vec![],
                    uncertain: unobserved,
                });
            }
            // The same guard where nothing places the call: a helper that a
            // tracked scope may reach (`createMemo(() => waitReady())`).
            let walk_limited = Cell::new(None);
            if kind == Some(Primitive::Resolve)
                && resolve_scope_unplaced(file, call, &allowed, self, &walk_limited)
            {
                result.violations.push(StaticViolation {
                    id: "SC2004".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: "resolve() is called in code the analysis cannot place; if an observer is active when it runs (a memo or effect compute, or tracked JSX, that reaches it through a helper), Solid throws \"Cannot call resolve inside a reactive scope\" in dev, and whether one is cannot be proven either way".into(),
                    hint: "Call resolve() from imperative code, such as an event handler; on rc.17 an effect's apply and onSettled refuse it too. To depend on a pending value inside a computation, read the accessor directly: tracked reads suspend and re-run on their own. A deliberate one-shot read can be wrapped in untrack(), which clears the observer the runtime guards on.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: walk_limit_context(&walk_limited),
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
                // ADR 0893: the throw may end stored on a compute no read reports.
                let unobserved = plain_throw_unobserved(
                    file,
                    call.span,
                    &allowed,
                    self,
                    dialect.plain_throw(Primitive::Until),
                    Thrower::UnderObserver,
                );
                result.violations.push(StaticViolation {
                    id: "SC2005".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: format!(
                        "until() is called inside {scope}; an active observer makes Solid throw \"Cannot call until inside a reactive scope\" here in dev, and in production every run of the scope starts another root and effect that wait on the predicate{}",
                        unobserved_note(unobserved)
                    ),
                    hint: "Await until() from imperative code — an action step (`yield until(...)`) or an event handler; on rc.17 an effect's apply and onSettled refuse it too. Inside a computation, read the condition directly: tracked reads re-run the computation when it changes.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: if unobserved {
                        crate::blockers::COMPUTATION_ERROR_UNOBSERVED.to_owned()
                    } else {
                        String::new()
                    },
                    fixes: vec![],
                    uncertain: unobserved,
                });
            }
            // The same guard where nothing places the call: a helper that a
            // tracked scope may reach (`createMemo(() => waitReady())`).
            if kind == Some(Primitive::Until)
                && resolve_scope_unplaced(file, call, &allowed, self, &walk_limited)
            {
                result.violations.push(StaticViolation {
                    id: "SC2005".into(),
                    rule: "no-await-primitive-in-tracked-scope".into(),
                    message: "until() is called in code the analysis cannot place; if an observer is active when it runs (a memo or effect compute, or tracked JSX, that reaches it through a helper), Solid throws \"Cannot call until inside a reactive scope\" in dev, and whether one is cannot be proven either way".into(),
                    hint: "Await until() from imperative code — an action step (`yield until(...)`) or an event handler; on rc.17 an effect's apply and onSettled refuse it too. Inside a computation, read the condition directly: tracked reads re-run the computation when it changes.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: walk_limit_context(&walk_limited),
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
                    // ADR 0865: `staticDynamic` throws on a thenable only.
                    StaticSource::Sync | StaticSource::AsyncIterable => None,
                    StaticSource::Promise(proof) if !unknown_form => Some((proof, false)),
                    StaticSource::Promise(proof) => Some((proof, true)),
                    StaticSource::Open => Some(("is not proven to return synchronously", true)),
                };
                if let Some((proof, uncertain)) = reported {
                    // ADR 0852: the source's thenable return proves the guard,
                    // but a throw in a generator its async caller steps rejects
                    // that caller's flight. Error observation is a separate,
                    // unproved step; do not claim a visible runtime error.
                    let flight_error_unobserved = crate::execution_role::generator_step_resumption(
                        file,
                        call.span,
                        self.lookup,
                    )
                    .is_some();
                    // ADR 0893: a throw stored on a compute no read reports.
                    let stored_unobserved = !flight_error_unobserved
                        && plain_throw_unobserved(
                            file,
                            call.span,
                            &allowed,
                            self,
                            dialect.plain_throw(Primitive::DynamicStatic),
                            Thrower::Always,
                        );
                    let uncertain = uncertain || flight_error_unobserved || stored_unobserved;
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
                        analysis_context: if flight_error_unobserved {
                            crate::blockers::ASYNC_FLIGHT_ERROR_UNOBSERVED.to_owned()
                        } else if stored_unobserved {
                            crate::blockers::COMPUTATION_ERROR_UNOBSERVED.to_owned()
                        } else if uncertain {
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
                    // ADRs 0516, 0537, 0538: a layer between the step and the
                    // call is not placed, so whether a step is on the stack
                    // when it runs is open.
                    analysis_context: if stepped {
                        crate::blockers::FUNCTION_RUN_UNPLACED.to_owned()
                    } else {
                        String::new()
                    },
                    fixes: vec![],
                    uncertain: stepped,
                });
            } else if let Some(kind) = kind
                && dialect.throws_inside_action_step(kind)
                && self.lookup.project_steps_action_callbacks()
                && let Some(blocker) = unplaced_blocker(file, call.span, self.lookup, false)
            {
                // ADR 0708: the call runs in a function no call edge places
                // (an object or class member, a slot, a tag, a function
                // handed where nothing follows it), and some callback in the
                // program runs as an action's steps: whether a step is on the
                // stack when it runs is unproven, an obligation (invariant 2).
                result.violations.push(StaticViolation {
                    id: "SC2006".into(),
                    rule: "flush-in-action".into(),
                    message: format!(
                        "{primitive}() is called {}, and this program steps an action's body; if a step is on the stack when it runs, the installed @solidjs/signals throws FLUSH_IN_ACTION in dev, and in production the call drains nothing",
                        unplaced_clause(blocker)
                    ),
                    hint: "Call flush() only from code proven outside every action body (an event handler that starts no action, a timer), or remove it: an action's writes commit when the action settles.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: blocker.into(),
                    fixes: vec![],
                    uncertain: true,
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
                && self.client_build()
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
                    message: if origin.undecided.is_some() {
                        format!(
                            "{primitive}() may run on an effect's apply: the walk from this call to the code that runs it stopped at its bound before it decided, so whether it runs inside the flush draining effects (and drains nothing) is not proven"
                        )
                    } else {
                        format!(
                            "{primitive}() is called {reached}; whenever it runs, it runs inside the flush that drains effects, so the queue is already running and this flush drains nothing (the dev build usually also warns FLUSH_IN_EFFECT_CALLBACK), and the writes it was meant to reveal are processed in that flush's continuation, not at this call"
                        )
                    },
                    hint: "Remove the flush(): writes made in an effect are processed by the flush that is already running. To force a drain afterwards, defer it (`queueMicrotask(() => flush())`) or call it from an event handler or other code that runs outside the effect.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: origin.undecided.unwrap_or_default().into(),
                    fixes: vec![],
                    uncertain: origin.mixed,
                });
            }
            // ADR 0639: `flush()`'s guard takes no event, so a handler dispatched
            // synchronously from an effect's apply warns there. A dispatch whose
            // caller is not placed outside every apply makes a flush in a handler
            // an obligation. ADR 0709: so is a flush in a function no call
            // edge places (an object or class member, a slot, a bundle's
            // `effect`, a function handed where nothing follows it): an apply
            // may be what runs it.
            let dispatched = || {
                self.lookup.synchronous_event_flush_dispatch_present()
                    || crate::execution_role::in_denied_handler(file, call.span)
            };
            let mut unplaced = None;
            if let Some(kind) = kind
                && self.client_build()
                && dialect.warns_inside_effect_phase(kind)
                && (dispatched() || {
                    unplaced = self
                        .lookup
                        .project_defers_callbacks()
                        .then(|| unplaced_blocker(file, call.span, self.lookup, false))
                        .flatten();
                    unplaced.is_some()
                })
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
                    let (message, hint, blocker) = if let Some(blocker) = unplaced {
                        (
                            if blocker == crate::blockers::FUNCTION_RUN_UNPLACED {
                                format!(
                                    "{primitive}() runs in a function no call edge places (a member, a slot, \
                                     a function handed on): if an effect's apply runs it, it runs inside the \
                                     flush draining effects and drains nothing"
                                )
                            } else {
                                format!(
                                    "{primitive}() runs {}: if an effect's apply runs it, it runs inside the \
                                     flush draining effects and drains nothing",
                                    unplaced_clause(blocker)
                                )
                            },
                            "Remove the flush(), or call it only from code proven outside every effect's apply (an event handler, a timer).",
                            blocker,
                        )
                    } else {
                        (
                            format!(
                                "{primitive}() runs in a handler that may be delivered synchronously \
                                 (a dispatch here, or a state change for an event no call names): \
                                 from an effect's apply it would run inside the flush draining effects"
                            ),
                            "Remove the flush(), or the synchronous dispatch from the apply.",
                            "",
                        )
                    };
                    result.violations.push(StaticViolation {
                        id: "SC2008".into(),
                        rule: "flush-in-effect-callback".into(),
                        message,
                        hint: hint.into(),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: blocker.into(),
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
                // `action`'s guard, so `action`'s walk: a role legal where the
                // call is written may be rejected at a carrier.
                let role = crate::local_access::handed_write_execution(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    crate::execution_role::RootBodyGuard::Rejects,
                    crate::execution_role::semantic_write_execution_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                        crate::execution_role::RootBodyGuard::Rejects,
                    ),
                );
                if !(role.reports_disallowed_write() || role.reports_unplaced()) {
                    // ADR 0936: the write role is a fact about the site, and no
                    // owner that rejects the call is there.
                    result.ends.end_at(
                        file,
                        call.callee,
                        crate::site_accounting::SiteSilence::OnFlushRoleAllows,
                        crate::proof_log::PremiseKind::RoleDoesNotReport,
                        || Some(call.callee),
                    );
                }
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
                && self.client_build()
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
                                    && crate::execution_role::call_may_run_code(
                                        file,
                                        other,
                                        self.lookup,
                                    )
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
                    message: if origin.undecided.is_some() {
                        format!(
                            "{primitive}() may run on an effect's apply with no owner set: the walk from this call to the code that runs it stopped at its bound before it decided, so whether the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK here is not proven"
                        )
                    } else if earlier_call {
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
                    analysis_context: origin.undecided.unwrap_or_default().into(),
                    fixes: vec![],
                    // ADR 0613: the effect phase is the queue's drain, which
                    // a synchronous `flush()` runs under its caller's owner.
                    uncertain: earlier_call
                        || origin.mixed
                        || self.lookup.flush_drains_under_owner(),
                });
            }
            // ADR 0710: a creating call in a function no call edge places (an
            // object or class member, a getter, a slot, a function handed
            // where nothing follows it) may run on an apply's stack with no
            // owner: an obligation, unless the walk above placed it.
            if let Some(created) = kind
                && self.client_build()
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
                && !result.violations.iter().any(|violation| {
                    violation.id == "SC6002"
                        && violation.location == location(file.path.shared(), call.callee)
                })
                && self.lookup.project_defers_callbacks()
                && let Some(blocker) = unplaced_blocker(file, call.span, self.lookup, true)
                && !self.runs_off_every_apply(file, call.span, &allowed)
            {
                result.violations.push(StaticViolation {
                    id: "SC6002".into(),
                    rule: "primitive-in-effect-callback".into(),
                    message: format!(
                        "{primitive}() is called {}; if an effect's apply runs it with no owner set in between, the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK, and whether one does is not proven",
                        if blocker == crate::blockers::FUNCTION_RUN_UNPLACED {
                            "in a function no call edge places (a member, a getter, a slot, a function handed on)"
                        } else {
                            unplaced_clause(blocker)
                        }
                    ),
                    hint: "Create the computation in the component body or an effect's compute function, where an owner holds it, or parent it explicitly with `runWithOwner(owner, () => …)`.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: blocker.into(),
                    fixes: vec![],
                    uncertain: true,
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
                symbol.and_then(|symbol| Some((symbol, self.tables.source_kind(symbol).copied()?)))
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
                        .tables
                        .reachable_call(&location(file.path.shared(), call.callee))
                        // ADR 0721: no edge reaches the call, yet it may run.
                        .or_else(|| {
                            crate::local_access::unreached_write_may_run(
                                file, call.span, self.lookup,
                            )
                            .then_some(1)
                        })
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
                && self.tables.value_form_store(symbol)
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
            let reached = self.tables.reachable_call(&callee);
            // ADR 0721: no edge reaches the call, yet it may run (a method, a
            // getter, a tag): one write whose place nothing proves.
            let unreached = reached.is_none()
                && crate::local_access::unreached_write_may_run(file, call.span, self.lookup);
            let Some(multiplicity) = reached.or(unreached.then_some(1)) else {
                continue;
            };
            let Some((name, declaration)) = self.tables.accessor(symbol) else {
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
                execution: if unreached {
                    crate::ExecutionRole::Unknown
                } else {
                    crate::local_access::handed_write_execution(
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
                    )
                },
                owned_write: self.tables.source_owned_write(symbol).unwrap_or_default(),
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
        self.check_jsx_in_effect_phase(file, &mut result);
        result
    }

    /// ADR 0883: JSX creating a computation or a root on an apply's stack, as
    /// a creating call does (SC6002): rc.21 reaches `createRenderEffect`,
    /// `createMemo` or `createRoot` with no owner set. Placed by the same walk.
    fn check_jsx_in_effect_phase(&self, file: &FileFacts, result: &mut StaticDirectiveFileResult) {
        for site in crate::owners::jsx_sites(file, self.lookup) {
            if site.kind == crate::owners::JsxSiteKind::MaybeRenderEffect || site.in_getter {
                continue;
            }
            let Some(origin) = crate::effect_phase::ownerless_jsx_effect_phase_origin(
                file,
                site.span,
                self.package_instances,
                self.lookup,
            ) else {
                continue;
            };
            let earlier_call = origin.earlier_call
                || (origin.helper_calls == 0
                    && file.ast.calls.iter().any(|other| {
                        origin.apply.contains(other.span)
                            && other.span.end <= site.span.start
                            && !other.span.contains(site.span)
                            && crate::execution_role::call_may_run_code(file, other, self.lookup)
                    }));
            // No client root in sight: which build runs the apply is open.
            let uncertain = earlier_call
                || origin.mixed
                || !self.client_build()
                || self.lookup.flush_drains_under_owner();
            // Written in the apply, a proved throw is every run of the site. In
            // a helper another entry may run it with no window (a timer), and
            // an uncertain throw may pass the guard: NO_OWNER_EFFECT stays open.
            result.jsx_phase_sites.push((
                location(file.path.shared(), site.span),
                !uncertain && origin.helper_calls == 0 && origin.undecided.is_none(),
            ));
            result.violations.push(StaticViolation {
                id: "SC6002".into(),
                rule: "primitive-in-effect-callback".into(),
                message: format!(
                    "JSX here creates a computation in {}, with no owner set in between; the dev build throws PRIMITIVE_IN_EFFECT_CALLBACK",
                    origin.callback_label()
                ),
                hint: "Build the JSX in the effect's compute function or the component body, where an owner is set; an apply should only touch what already exists.".into(),
                location: location(file.path.shared(), site.span),
                analysis_context: origin.undecided.unwrap_or_default().into(),
                fixes: vec![],
                uncertain,
            });
        }
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
    resolve_tracked_scope_within(file, call, allowed, context, ENTRY_DEPTH, &Cell::new(None))
}

/// ADR 0857 (reviews R13, R16): the row's blocker when the walk over entries
/// stopped before reading them all.
fn walk_limit_context(limited: &Cell<Option<&'static str>>) -> String {
    limited.get().unwrap_or_default().to_owned()
}

/// ADR 0857: how many calls [`resolve_tracked_scope_within`] follows out of
/// the function that holds a `resolve`/`until` call.
const ENTRY_DEPTH: usize = 4;

fn resolve_tracked_scope_within(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
    depth: usize,
    limited: &Cell<Option<&'static str>>,
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
        let Some(primitive) = context.lookup.primitive_at_call(file, container.span) else {
            // ADR 0857: a standard-library call that provably runs the
            // literal inline runs it under the call's own observer.
            return resolve_scope_of_entries(file, at, allowed, context, depth, true, limited);
        };
        match context
            .lookup
            .dialect
            .slot_run(primitive, index, container.arguments.len())
        {
            SlotRun::InPlace(run) => match (run.throws, run.observer) {
                // ADR 0941: a call that takes whatever its callback throws
                // (`isPending`) takes the observer guard's plain throw too, under any
                // observer: nothing there is thrown to the caller, so nothing is
                // reported (rc.21 probe: `isPending(() => until(...))` was quiet).
                (SlotThrows::Swallows, _) => return None,
                // ADR 0857: a callback proven to run during the call under the
                // caller's observer (`flush(fn)`, ADR 0941: `latest(fn)`) runs where
                // the call does.
                (SlotThrows::Escapes, SlotObserver::Keeps) => {
                    let Some(depth) = depth.checked_sub(1) else {
                        limited.set(Some(crate::blockers::SEARCH_DEPTH_LIMIT));
                        return None;
                    };
                    return resolve_tracked_scope_within(
                        file, container, allowed, context, depth, limited,
                    );
                }
                // An observer the call clears (`untrack`, `runWithOwner`) or one the dialect does not state
                // (`render`'s root): no active observer is proven.
                (SlotThrows::Escapes, SlotObserver::Clears | SlotObserver::Unstated) => {
                    return None;
                }
            },
            SlotRun::Computation => {}
            SlotRun::Deferred | SlotRun::Unknown => return None,
        }
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
    // ADR 0857: the climbed call runs where the layers land (`at`), so the
    // tracked region and the role are asked there.
    let in_tracked_region = file
        .compiler
        .tracked_regions
        .iter()
        .any(|region| region.span.contains(at));
    if in_tracked_region
        && crate::execution_role::semantic_execution_role(
            file,
            at,
            allowed,
            context.entities,
            context.symbol_names,
            context.lookup,
        ) == crate::ExecutionRole::TrackedJsx
    {
        return Some("tracked JSX".to_owned());
    }
    resolve_scope_of_entries(file, at, allowed, context, depth, true, limited)
}

/// ADR 0857: the tracked scope of a call that runs the function holding `at`
/// (no function between them): a direct call of its binding, or a
/// standard-library call proven to run it inline at least once
/// (`[1].map(Row)`, `[1].forEach(() => …)`), each asked as the call itself
/// is. One such entry under an observer is a run that throws, whatever the
/// function's other entries are, a component's render among them. A walk the
/// walk that stops sets `limited`: its answer proves nothing about the
/// entries it did not read.
fn resolve_scope_of_entries(
    file: &FileFacts,
    at: Span,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
    depth: usize,
    require_run: bool,
    limited: &Cell<Option<&'static str>>,
) -> Option<String> {
    use crate::local_access::HandOff;
    let Some(depth) = depth.checked_sub(1) else {
        limited.set(Some(crate::blockers::SEARCH_DEPTH_LIMIT));
        return None;
    };
    let lookup = context.lookup;
    let function = crate::owners::containing_body_function(&file.ast, at)?;
    // ADR 0857 (review R16): an `async` function's prefix runs on the call's
    // stack, and a site past its await never got here. Calling a bound
    // generator runs none of it, and a function its binding may no longer
    // hold is not what a call of the binding runs: no entry is read. A literal
    // with no binding (an `action` step, a handler) is its consumer's.
    if function.generator || !lookup.function_value_is_current(file, function) {
        if crate::owners::function_binding_name(file, function).is_some() {
            limited.set(Some(crate::blockers::FUNCTION_RUN_UNPLACED));
        }
        return None;
    }
    let hand_offs = match crate::local_access::function_reference_hand_offs(file, function, lookup)
    {
        Some(named) => named,
        None => vec![(
            function.span,
            crate::local_access::hand_off_at(file, function.span, lookup),
        )],
    };
    hand_offs.into_iter().find_map(|(reference, hand_off)| {
        let entry = match hand_off {
            HandOff::Call => file.ast.calls.iter().find(|call| {
                call.direct_callee
                    && !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == reference
            })?,
            HandOff::Inline(carrier)
                if !require_run
                    || crate::carrier_runs::carrier_provably_runs(file, carrier, lookup) =>
            {
                file.ast.call_at(carrier)?
            }
            _ => return None,
        };
        resolve_tracked_scope_within(file, entry, allowed, context, depth, limited)
            .map(|scope| format!("{scope}, through the call that runs this function"))
    })
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
    limited: &Cell<Option<&'static str>>,
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
    // ADR 0857: a call or a standard-library carrier that may run the function
    // holding the call (an empty or unknown receiver too) under an observer:
    // whether it runs is open.
    let at = crate::execution_role::climb_in_place_layers(file, call.span, context.lookup)
        .filter(|climb| !climb.exhausted && !climb.in_list && climb.suspended.is_none())
        .map_or(call.span, |climb| climb.at);
    if resolve_scope_of_entries(file, at, allowed, context, ENTRY_DEPTH, false, limited).is_some() {
        return true;
    }
    // Reviews R13, R16: the walk stopped, so the entries beyond are unread
    // and no role at the call itself can clear them.
    if limited.get().is_some() {
        return true;
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
        // ADR 0857: a callback that runs during the call with the caller's
        // listener (`flush(fn)`) is placed where the call is.
        if crate::execution_role::callback_inherits_listener(
            context.lookup.dialect,
            primitive,
            index,
            container.arguments.len(),
        ) {
            return resolve_scope_unplaced(file, container, allowed, context, limited);
        }
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
                            || resolve_scope_unplaced(file, caller, allowed, context, limited)
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
    // A generator's iterator has no `then`; an async generator's has
    // `Symbol.asyncIterator` (ADR 0865).
    if function.generator {
        return if function.r#async {
            StaticSource::AsyncIterable
        } else {
            StaticSource::Sync
        };
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

/// ADR 0884: the rules a primitive's call may answer to, each a U at an alias
/// call: a code with no site line is covered only by its own rule's row.
fn alias_claims(
    dialect: &dyn solid_dialect::Dialect,
    name: &str,
) -> &'static [(&'static str, &'static str)] {
    use solid_dialect::Primitive;
    const CREATES: (&str, &str) = ("SC6002", "primitive-in-effect-callback");
    const SYNC: (&str, &str) = ("SC7002", "sync-computation-received-async");
    match dialect.primitive(name) {
        Some(Primitive::Flush) => &[
            ("SC2006", "flush-in-action"),
            ("SC2008", "flush-in-effect-callback"),
        ],
        Some(Primitive::CreateMemo | Primitive::CreateEffect | Primitive::CreateRenderEffect) => {
            &[CREATES, SYNC]
        }
        Some(Primitive::CreateSignal | Primitive::CreateRoot) => &[CREATES],
        _ => &[],
    }
}

/// What a static `dynamic()` source does on its one call (ADR 0654).
enum StaticSource {
    Promise(&'static str),
    /// An async generator's iterator: no `then`, an async iterator (ADR 0865).
    AsyncIterable,
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
        // A closure handed to project code that no layer follows (a chain of
        // forwarding helpers, ADR 0828's `forward-2`): it may run on that
        // call's stack.
        crate::execution_role::handed_to_unreviewed_call_at(file, function, context.lookup)?
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

/// Sites one question may visit. The walk is linear in the helpers above the
/// site, so this ends only a pathological graph, and past it the answer is
/// "unplaced": a search that stops proves nothing (ADR 0674).
const UNPLACED_WALK_BUDGET: usize = 4096;

/// ADR 0708: [`crate::execution_role::unplaced_run`] at `site`, or at an
/// exact call of a function around it, through every helper above: a helper
/// whose callers include an object method or slot runs wherever that member
/// does, which no call edge places either.
///
/// ADR 0674: only reaching a placed end answers false. The walk visits each
/// function once, in any order, so a helper reached by a long path answers the
/// same as by a short one, and the budget fails closed.
pub(crate) fn runs_unplaced_exhaustive<'s, 'f: 's>(
    file: &'s FileFacts,
    site: Span,
    lookup: &SemanticLookup<'f>,
    ownerless: bool,
) -> bool {
    let mut visited = HashSet::default();
    search_fails_closed(
        (file, site),
        UNPLACED_WALK_BUDGET,
        |(file, site), pending| unplaced_step(file, site, lookup, ownerless, &mut visited, pending),
    )
}

/// A search that can only answer false by finishing: true when `step` answers
/// true at some node, and true when `budget` nodes were not enough.
fn search_fails_closed<T>(
    start: T,
    budget: usize,
    mut step: impl FnMut(T, &mut Vec<T>) -> bool,
) -> bool {
    let mut pending = vec![start];
    let mut budget = budget;
    while let Some(node) = pending.pop() {
        if budget == 0 || step(node, &mut pending) {
            return true;
        }
        budget -= 1;
    }
    false
}

/// One site of [`runs_unplaced_exhaustive`]: true when it is unplaced,
/// else the sites above it go on `pending`.
fn unplaced_step<'s, 'f: 's>(
    file: &'s FileFacts,
    site: Span,
    lookup: &SemanticLookup<'f>,
    ownerless: bool,
    visited: &mut HashSet<(String, Span)>,
    pending: &mut Vec<(&'s FileFacts, Span)>,
) -> bool {
    // ADR 0710: with `ownerless`, a site a primitive's synchronous callback
    // runs (other than `untrack`'s) has the owner that primitive sets, and a
    // body that may be a component's runs under the owner its render sets
    // (as ADR 0639's net assumes).
    if ownerless
        && (owner_set_around(file, site, lookup)
            || crate::owners::containing_ast_function(&file.ast, site)
                .is_some_and(|function| lookup.function_may_be_component(file, function)))
    {
        return false;
    }
    // ADR 0673: `unplaced_run` refuses a site `SiteReach` calls never entered,
    // and a method only a coercion runs (`${o}`) is one: no call edge reaches
    // `toString`. The hand-off census asks nothing of reach.
    if crate::execution_role::unplaced_run(file, site, lookup)
        || crate::local_access::function_escapes_unfollowed(file, site, lookup)
    {
        return true;
    }
    // After an await on every path the site is off every caller's stack.
    if crate::execution_role::follows_unconditional_await(file, site)
        || crate::execution_role::follows_dominating_await(file, site)
        || crate::execution_role::follows_try_head_await(file, site, lookup)
    {
        return false;
    }
    let at = match crate::execution_role::climb_in_place_layers(file, site, lookup) {
        // So after one in a layer in place.
        Some(climb) if climb.suspended == Some(true) => return false,
        // The layers that run in place end at a call in other code: ask
        // there. A layer holding the site in its parameter list (ADR 0454)
        // runs it when a call omits the argument, and an await on some paths
        // leaves the others on the stack.
        Some(climb) if climb.at != site => {
            pending.push((file, climb.at));
            return false;
        }
        Some(_) | None => site,
    };
    // The function a parameter default or a generator's body runs in: the
    // first runs at each call, the second where the iterator is drained,
    // which a caller at least holds.
    let Some(function) = crate::owners::owning_function(&file.ast, at) else {
        return false;
    };
    if !visited.insert((file.path.to_string(), function.span)) {
        return false;
    }
    // ADR 0673: a call in a function no edge enters (`toString`, which only a
    // coercion runs) has no call site here, so the binder names this file's.
    if let Some(name) = crate::owners::function_binding_name(file, function) {
        let name = name.span;
        pending.extend(
            file.ast
                .calls
                .iter()
                .filter(|call| {
                    !call.construct
                        && file
                            .ast
                            .reference_declaration(file.ast.peel_ts_sugar_span(call.callee))
                            == Some(name)
                })
                .map(|call| (file, call.span)),
        );
    }
    // ADR 0674: a reference in another file that hands the function on is no
    // call site, and no role reads a carrier in another file: unplaced.
    // A component's body is entered by its render, however its value travels.
    let foreign = if lookup.function_proven_component(file, function) {
        Vec::new()
    } else {
        crate::local_access::foreign_reference_hand_offs(file, function, lookup)
    };
    for (holder, at, hand_off) in foreign {
        use crate::local_access::HandOff;
        match hand_off {
            HandOff::Call | HandOff::FreshStack | HandOff::Value => {}
            HandOff::Inline(_) => return true,
            HandOff::Retained | HandOff::Unfollowed => {
                if !foreign_jsx_value(holder, at) && !foreign_quiet_primitive(holder, at, lookup) {
                    return true;
                }
            }
        }
    }
    for (caller, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        pending.extend(
            caller
                .ast
                .calls
                .iter()
                .filter(|call| {
                    !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
                })
                .map(|call| (caller, call.span)),
        );
    }
    false
}

/// Whether a rule's net answers at `site`, and with what blocker. Both walks
/// ask: [`runs_unplaced_through_callers`] names a limit it stopped at (ADR
/// 0750), [`runs_unplaced_exhaustive`] holds the hand-offs from hidden members
/// and other files (ADRs 0673, 0674), so a row either raises stays.
fn unplaced_blocker(
    file: &FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
    ownerless: bool,
) -> Option<&'static str> {
    let mut memo = rustc_hash::FxHashMap::default();
    runs_unplaced_through_callers(file, site, lookup, 4, ownerless, &mut memo).or_else(|| {
        runs_unplaced_exhaustive(file, site, lookup, ownerless)
            .then_some(crate::blockers::FUNCTION_RUN_UNPLACED)
    })
}

/// ADR 0708: [`crate::execution_role::unplaced_run`] at `site`, or at an
/// exact call of the named function around it, through `depth` helpers: a
/// helper whose callers include an object method or slot runs wherever that
/// member does, which no call edge places either. The blocker it answers with:
/// ADR 0750, a climb that had callers left to ask past `depth` (or whose
/// in-place climb was cut) is undecided, never placed.
///
/// Asks each site once per remaining depth: a helper many sites reach costs one
/// walk, not `sites^depth`, and the answer is the same (`walk_memo`). `ownerless`
/// is one value for a whole walk, so it needs no place in the key.
fn runs_unplaced_through_callers(
    file: &FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    ownerless: bool,
    memo: &mut UnplacedMemo,
) -> Option<&'static str> {
    crate::walk_memo::memoized(memo, (file.path.to_string(), site, depth), |memo| {
        runs_unplaced_once(file, site, lookup, depth, ownerless, memo)
    })
}

/// One site, asked at one remaining depth.
type UnplacedMemo = rustc_hash::FxHashMap<(String, Span, usize), Option<&'static str>>;

fn runs_unplaced_once(
    file: &FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    ownerless: bool,
    memo: &mut UnplacedMemo,
) -> Option<&'static str> {
    // ADR 0710: with `ownerless`, a site a primitive's synchronous callback
    // runs (other than `untrack`'s) has the owner that primitive sets, and a
    // body that may be a component's runs under the owner its render sets
    // (as ADR 0639's net assumes).
    if ownerless
        && (owner_set_around(file, site, lookup)
            || crate::owners::containing_ast_function(&file.ast, site)
                .is_some_and(|function| lookup.function_may_be_component(file, function)))
    {
        return None;
    }
    if crate::execution_role::unplaced_run(file, site, lookup) {
        return Some(crate::blockers::FUNCTION_RUN_UNPLACED);
    }
    let at = match crate::execution_role::climb_in_place_layers(file, site, lookup) {
        Some(climb) if climb.in_list || climb.suspended.is_some() => return None,
        Some(climb) if climb.exhausted => return Some(crate::blockers::SEARCH_DEPTH_LIMIT),
        // The layers that run in place end at a call in other code: ask
        // there.
        Some(climb) if climb.at != site => {
            let Some(depth) = depth.checked_sub(1) else {
                return Some(crate::blockers::SEARCH_DEPTH_LIMIT);
            };
            return runs_unplaced_through_callers(file, climb.at, lookup, depth, ownerless, memo);
        }
        Some(_) | None => site,
    };
    let function = crate::owners::owning_function(&file.ast, at)
        .filter(|function| function.body.contains(at) && !function.generator)?;
    crate::owners::function_binding_name(file, function)?;
    let calls = lookup
        .function_call_sites(file.path.as_str(), function.span)
        .into_iter()
        .flat_map(|(caller, callee)| {
            caller
                .ast
                .calls
                .iter()
                .filter(move |call| {
                    !call.construct && caller.ast.peel_ts_sugar_span(call.callee) == callee
                })
                .map(move |call| (caller, call))
        })
        .collect::<Vec<_>>();
    if calls.is_empty() {
        return None;
    }
    let Some(depth) = depth.checked_sub(1) else {
        return Some(crate::blockers::SEARCH_DEPTH_LIMIT);
    };
    let mut answer = None;
    for (caller, call) in calls {
        match runs_unplaced_through_callers(caller, call.span, lookup, depth, ownerless, memo) {
            Some(crate::blockers::FUNCTION_RUN_UNPLACED) => {
                return Some(crate::blockers::FUNCTION_RUN_UNPLACED);
            }
            Some(blocker) => answer = Some(blocker),
            None => {}
        }
    }
    answer
}

/// ADR 0674: whether `at` is the whole value of an attribute of an intrinsic
/// element: the roles of a handler, a `ref` or a tracked attribute place it.
/// A component's prop is the callee's code, so it stays unfollowed.
fn foreign_jsx_value(file: &FileFacts, at: Span) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        element.member_object.is_none()
            && file
                .source_text(element.name.span)
                .is_some_and(|name| name.starts_with(|first: char| first.is_ascii_lowercase()))
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|expression| file.ast.peel_ts_sugar_span(expression) == at)
            })
    })
}

/// ADR 0674: whether `at` is a whole argument of a primitive at a tracked
/// callback position: a computation's compute reads tracked.
fn foreign_quiet_primitive(file: &FileFacts, at: Span, lookup: &SemanticLookup<'_>) -> bool {
    file.ast.arguments_containing(at).any(|(call, index)| {
        file.ast.peel_ts_sugar_span(call.arguments[index].span) == at
            && !call.arguments[index].spread
            && lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    lookup
                        .dialect
                        .tracked_callback_timing(primitive, index, call.arguments.len())
                        .is_some()
                })
    })
}

/// ADR 0750: how a finding of [`runs_unplaced_through_callers`] names where
/// its call runs.
fn unplaced_clause(blocker: &str) -> &'static str {
    if blocker == crate::blockers::FUNCTION_RUN_UNPLACED {
        "in a function that runs where no call edge places it"
    } else {
        "in a function whose callers the walk stopped asking at its depth limit"
    }
}

/// ADR 0710: whether the functions directly around `site` are callbacks of
/// primitives that run them synchronously, the innermost one not `untrack`'s
/// (`runWithOwner`, `createRoot`, a computation's compute): that primitive
/// sets or may set an owner before its callback runs. A plain function on
/// the way out ends the question: its owner is its caller's.
fn owner_set_around(file: &FileFacts, site: Span, lookup: &SemanticLookup<'_>) -> bool {
    let mut at = site;
    for _ in 0..16 {
        let Some(function) = file.ast.innermost_function_owning(at) else {
            return false;
        };
        let Some((call, index)) =
            file.ast
                .arguments_containing(function.span)
                .find(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
                })
        else {
            return false;
        };
        let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
            return false;
        };
        // ADR 0944: asked of the dialect, not of the primitive's name. A callback that adds no owner
        // (`untrack`, `latest`, `isPending`, `flush(fn)`) is as owned as the call: ask around the call.
        match lookup
            .dialect
            .slot_run(primitive, index, call.arguments.len())
        {
            SlotRun::InPlace(run) => match run.owner {
                SlotOwner::Inherits => at = call.span,
                SlotOwner::Supplied { .. } | SlotOwner::Creates | SlotOwner::Detached => {
                    return true;
                }
            },
            SlotRun::Computation | SlotRun::Deferred | SlotRun::Unknown => return false,
        }
    }
    false
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
    // ADR 0944: a callback of a primitive that runs in place runs on the stack of its call, inside the step
    // (`untrack(() => flush())`: the action's depth is the runtime's, not the callback's). One that catches
    // what it throws (`isPending`) leaves a plain throw no report, and one whose own call throws first in a step
    // (`flush(fn)`) never runs the callback: neither is crossed.
    let at = match crate::execution_role::climb_through_slots(file, site, context.lookup, |slot| {
        match slot.run {
            SlotRun::InPlace(run) => match run.throws {
                SlotThrows::Escapes => !context
                    .lookup
                    .dialect
                    .throws_inside_action_step(slot.primitive),
                SlotThrows::Swallows => false,
            },
            SlotRun::Computation | SlotRun::Deferred | SlotRun::Unknown => false,
        }
    }) {
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

#[cfg(test)]
mod search_tests {
    use super::search_fails_closed;

    /// `edges[n]` are the nodes above `n`; `unplaced` is the only node that
    /// answers true.
    fn search(edges: &[&[usize]], unplaced: Option<usize>, budget: usize) -> bool {
        let mut visited = std::collections::HashSet::new();
        search_fails_closed(0, budget, |node, pending| {
            if Some(node) == unplaced {
                return true;
            }
            if visited.insert(node) {
                pending.extend(edges[node]);
            }
            false
        })
    }

    #[test]
    fn a_finished_search_answers_false() {
        assert!(!search(&[&[1, 2], &[3], &[3], &[]], None, 16));
    }

    #[test]
    fn past_the_budget_the_answer_is_not_clean() {
        let chain: Vec<Vec<usize>> = (0..10).map(|n| vec![n + 1]).chain([vec![]]).collect();
        let edges: Vec<&[usize]> = chain.iter().map(Vec::as_slice).collect();
        assert!(!search(&edges, None, 11));
        assert!(search(&edges, None, 10));
    }

    #[test]
    fn a_diamond_answers_the_same_by_either_path() {
        // 1 and 2 meet at 3, whose caller 4 is unplaced.
        assert!(search(&[&[1, 2], &[3], &[3], &[4], &[]], Some(4), 16));
        assert!(search(&[&[2, 1], &[3], &[3], &[4], &[]], Some(4), 16));
    }
}
