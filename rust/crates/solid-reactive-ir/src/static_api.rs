//! Static validation for Solid 2.0 API shapes and explicit refresh writes.

use std::collections::{HashMap, HashSet};

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, RuntimeValueKind};
use solid_facts::core::Span;
use typefacts::Location;

use crate::execution_role::{allowed_callback_spans, semantic_write_execution_role};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use crate::owners::{analysis_context, computation_is_async_with_contracts};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_file_results};
use crate::{
    ReactiveSourceKind, ReactiveWrite, StaticDefect, StaticDefectKind, StaticViolation, location,
    primitive_name,
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
        contracted: ctx.contracted,
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
    pub(super) source_owned_write: &'a HashMap<SymbolId, bool>,
    pub(super) value_form_stores: &'a HashSet<SymbolId>,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) reachable_calls: &'a HashMap<Location, usize>,
    pub(super) contracted: &'a HashMap<SymbolId, crate::contracts::ResolvedContractBinding>,
}

impl StaticApiContext<'_> {
    pub(super) fn check_file(&self, file: &FileFacts) -> StaticDirectiveFileResult {
        let mut result = StaticDirectiveFileResult {
            defects: Vec::new(),
            violations: Vec::new(),
            writes: Vec::new(),
            write_action_obligations: Vec::new(),
        };
        let allowed = allowed_callback_spans(file, self.lookup);
        let dialect = self.lookup.dialect;
        for call in &file.ast.calls {
            let Some(primitive) = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
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
            // Where the effect function has to be is a dialect question, and
            // the same one `callback_positions` already answers: 2.0 puts it
            // at index 1 of `createEffect(compute, apply)`, 1.x at index 0 of
            // `createEffect(fn, value?)`. Hardcoding index 1 would fire this
            // rule on every correct 1.x effect.
            // An absent/`undefined` effect slot is the removed 1.x
            // single-callback form (dev throws MISSING_EFFECT_FN). A proven
            // non-function value in the slot — `null`, a string/number/
            // boolean literal — is the same defect with a worse failure
            // mode: the runtime reads `.effect` off it or calls it, crashing
            // the effect queue (`null.effect` / `5.effect is not a
            // function`; probed, rc.0). An `{ effect, error }` object is the
            // documented error-handling form and stays legal; identifiers
            // and unresolved expressions stay silent.
            if kind == Some(Primitive::CreateEffect)
                && let Some(&index) = dialect.callback_positions(Primitive::CreateEffect).last()
                && call.arguments.get(index).is_none_or(|argument| {
                    matches!(
                        argument.value,
                        ArgumentValueKind::Undefined | ArgumentValueKind::Null
                    ) || argument.runtime_value_kind == RuntimeValueKind::Primitive
                })
            {
                result.defects.push(StaticDefect {
                    kind: StaticDefectKind::MissingEffectFunction,
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                });
            }
            // Where each primitive takes its options object -- a second index
            // vocabulary, and the dialect's to answer: 2.0's numbers read
            // 1.x's `createMemo(fn, seed)` seed as an options object.
            let options_index = kind
                .filter(|kind| dialect.supports_sync_option(*kind))
                .and_then(|kind| dialect.options_argument(kind));
            if let Some(options_index) = options_index
                && call.arguments.get(options_index).is_some_and(|argument| {
                    argument.boolean_properties.iter().any(|property| {
                        file.source_text(property.name) == Some("sync") && property.value
                    })
                })
                && call.arguments.first().is_some_and(|argument| {
                    computation_is_async_with_contracts(
                        self.lookup,
                        file,
                        argument.span,
                        self.contracted,
                    )
                })
            {
                result.violations.push(StaticViolation {
                    id: "SC7002".into(),
                    rule: "sync-node-received-async".into(),
                    message: format!(
                        "{primitive} is marked sync: true but its computation can return a Promise or AsyncIterable; sync: true asserts a synchronous result so the runtime can skip the async-shape probe — dev still probes and throws SYNC_NODE_RECEIVED_ASYNC, and production stores the unawaited value as-is"
                    ),
                    hint: format!(
                        "Drop sync: true and let the read suspend to a <{}> boundary, or make the computation synchronous by moving the async work into its own computation and reading the settled accessor here.",
                        dialect.boundary_name(solid_dialect::Boundary::Async)
                    ),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
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
                    rule: "resolve-in-reactive-scope".into(),
                    message: format!(
                        "resolve() is called inside {scope}; resolve() reads the expression once and never tracks updates, and an active observer makes Solid throw \"Cannot call resolve inside a reactive scope\" here in dev"
                    ),
                    hint: "Call resolve() from imperative code — an event handler, onSettled, or an effect's apply function. To depend on a pending value inside a computation, read the accessor directly: tracked reads suspend and re-run on their own. A deliberate one-shot read can be wrapped in untrack(), which clears the observer the runtime guards on.".into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
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
                // absence of a write claim below.
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
            // whose construction form is unknown (contracts, unproven
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
            for _ in 0..multiplicity {
                result.writes.push(ReactiveWrite {
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
                    execution: semantic_write_execution_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    ),
                    allowed_by_option: self
                        .source_owned_write
                        .get(symbol)
                        .copied()
                        .unwrap_or(false),
                    context: analysis_context(
                        file,
                        call.span,
                        self.entities,
                        self.symbol_names,
                        dialect,
                        self.lookup,
                    )
                    .into(),
                });
            }
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
    if let Some((container, index)) =
        file.ast
            .arguments_containing(call.span)
            .find(|(container, index)| {
                crate::execution_role::direct_callback_contains(
                    file,
                    container.arguments[*index].span,
                    call.span,
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
        if let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.span == current)
        {
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
