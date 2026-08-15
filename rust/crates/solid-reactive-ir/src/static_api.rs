//! Static validation for Solid 2.0 API shapes and explicit refresh writes.

use std::collections::{HashMap, HashSet};

use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::ArgumentValueKind;
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
                    ) || argument.primitive_literal
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
            let Some(kind @ (Primitive::Refresh | Primitive::Affects)) = kind else {
                continue;
            };
            let is_refresh = kind == Primitive::Refresh;
            // Zero-argument calls have no target to validate; `affects` also
            // rejects a second key argument's siblings. Extra `refresh`
            // arguments are NOT arity errors: the runtime reads only the
            // first argument and silently ignores the rest (probed, rc.0 —
            // `refresh(source, force)` is legal, if inert).
            let invalid_arity = call.arguments.is_empty() || !is_refresh && call.arguments.len() > 2;
            if invalid_arity {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: format!("invalid-{primitive}-target"),
                    message: if is_refresh {
                        "refresh() requires the original derived signal, store, or projection binding to recompute as its argument".into()
                    } else {
                        "affects() takes a source target and at most one optional store property key; extra keys are not a path".into()
                    },
                    hint: if is_refresh {
                        "Pass the original refreshable binding directly: refresh(source). Wrapper thunks and already-read values are not refresh targets.".into()
                    } else {
                        "Call affects(source) for signals, affects(store) for a store record, or affects(store, \"key\") for one property. Mark multiple properties with separate calls or target the nested record directly.".into()
                    },
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
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
            ) || target.primitive_literal
                || target.container_literal;
            if !is_identifier && proven_non_source {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: format!("invalid-{primitive}-target"),
                    message: format!(
                        "{primitive}() received a wrapper, read value, or literal instead of the original Solid source binding; the brand on the binding created by createSignal, createMemo, or createStore is how Solid identifies what to recompute"
                    ),
                    hint: if is_refresh {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: refresh(user), not refresh(user()) or refresh(() => user()).".into()
                    } else {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: affects(user), not affects(user()).".into()
                    },
                    location: location(file.path.shared(), target.span),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
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
            let target_location = location(file.path.shared(), target.span);
            let Some((symbol, kind)) = symbol
                .and_then(|symbol| Some((symbol, self.source_kinds.get(symbol).copied()?)))
            else {
                result.violations.push(StaticViolation {
                    id: "SC9003".into(),
                    rule: format!("{primitive}-target-unresolved"),
                    message: format!(
                        "cannot trace the target of {primitive}() back to a Solid source; solid-checker cannot prove it is a branded accessor, store, or projection, so this call may throw at runtime"
                    ),
                    hint: "Pass the binding created by createSignal, createMemo, createStore, or createProjection directly. If the source is re-exported or wrapped by a package, declare that export's return kind in the package's reactivity contract so the brand survives the import.".into(),
                    location: target_location,
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            };
            // A member or call chain rooted at an accessor reads a plain
            // value off the accessor function — never a branded source
            // (probed: refresh(memo.name) and affects(memo.name) both throw
            // INVALID_*_TARGET in dev). Chains on store bases are accepted
            // above the brand: child proxies carry it.
            if !is_identifier && kind == ReactiveSourceKind::Accessor {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: format!("invalid-{primitive}-target"),
                    message: format!(
                        "{primitive}() received a wrapper, read value, or literal instead of the original Solid source binding; the brand on the binding created by createSignal, createMemo, or createStore is how Solid identifies what to recompute"
                    ),
                    hint: if is_refresh {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: refresh(user), not refresh(user()) or refresh(() => user()).".into()
                    } else {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: affects(user), not affects(user()).".into()
                    },
                    location: target_location,
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            }
            // Only the derived store forms own a compute node the runtime
            // can re-run. A value-form `createStore(obj)` store — or any
            // child record of one — is not refreshable, and refresh() on it
            // throws INVALID_REFRESH_TARGET in dev (probed, rc.0). A store
            // whose construction form is unknown (contracts, unproven
            // argument shapes) is never in this set, so acceptance is kept.
            if is_refresh
                && kind == ReactiveSourceKind::Store
                && self.value_form_stores.contains(symbol)
            {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: "invalid-refresh-target".into(),
                    message: "refresh() targets a store created with the value form createStore(value); such a store has no computation to re-run, and Solid throws INVALID_REFRESH_TARGET here in dev".into(),
                    hint: "Refreshable stores are the derived forms: createStore(fn, initial), createProjection, and function-form createOptimisticStore. Update a value-form store through its setter, or move the derivation into the function form and refresh that binding.".into(),
                    location: target_location,
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            }
            if !is_refresh {
                if is_identifier
                    && kind == ReactiveSourceKind::Accessor
                    && call.arguments.len() == 2
                {
                    result.violations.push(StaticViolation {
                        id: "SC7004".into(),
                        rule: "affects-keys-on-accessor".into(),
                        message: "affects() received a property key but its target is a signal accessor; a key narrows a store record to one slot, and an accessor is already a single slot".into(),
                        hint: "Drop the key for signal targets (affects(source)), or pass the owning store record if you meant to mark one property (affects(store, \"todos\")).".into(),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: String::new(),
                        fixes: vec![],
                    });
                }
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
