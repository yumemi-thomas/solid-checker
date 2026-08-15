//! Shared fine-grained rules decomposed out of upstream's `reactivity`.
//!
//! Upstream ships one rule reporting eight unrelated defects behind eight
//! message ids, so a project that disagrees with one of them has to silence
//! the other seven too. Here each is its own rule; the mapping is documented
//! in `docs/rules/README.md`.
//!
//! Two of the eight — the untracked read and the read after `await` — are the
//! engine's own analysis and are reported from the reactive IR as SC1001 and
//! SC1002, not from here. This module owns the rest.
//!
//! Unlike the sibling modules, which port a 1.x-era ESLint surface, the
//! defects here are defects in both language versions — an accessor used as
//! a value, a proxy written through, a listener bound to a call's result —
//! so both dialects' catalogs carry them (1.x as `v1/<rule>`, 2.0 under the
//! checker's plain names, same SC codes so suppressions survive a
//! migration). The one exception is `no-async-tracked-scope`; see its doc.
//!
//! # Proven sources, not named ones
//!
//! Upstream decides what is reactive from syntax and convention: a variable
//! destructured from a call spelled `createSignal`, a parameter whose name
//! looks like `props`, a function whose name matches `/^(?:use|create)[A-Z]/`.
//! That is why it reports a memo passed to a helper but not the structurally
//! identical signal (upstream issue #182), and why a signal reached through a
//! wrapper like `makePersisted(createSignal(...))` defeats it entirely (#190).
//!
//! These rules ask the engine instead: [`UpstreamCompatContext::accessors`]
//! and `setters` are the sources it *proved*, through TypeScript symbol
//! resolution, package contracts and cross-file propagation. A local named
//! `createSignal` that is not Solid's is not a source here, and a signal that
//! arrives through three wrappers still is.

use solid_facts::FileFacts;
use solid_facts::ast::{FunctionFact, IdentifierRole};
use solid_facts::core::Span;
use typefacts::Callability;

use super::{UpstreamCompatContext, is_lowercase_led, text};
use crate::owners::containing_ast_function;
use crate::{
    DirectMutationTarget, ReactiveSourceKind, StaticDefect, StaticDefectKind, StaticViolation,
    known_primitive, location,
};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
    defects: &mut Vec<StaticDefect>,
) {
    uncalled_accessor(file, context, defects);
    no_direct_mutation(file, context, defects);
    // The one rule in this module whose defect is version-specific: Solid
    // 2.0 models async computations as a feature (see the rule's doc), so
    // only the 1.x catalog carries it.
    if context.dialect.reports_async_tracked_scope() {
        no_async_tracked_scope(file, context, violations);
    }
    expected_function_got_expression(file, context, defects);
    reactive_source_uncaptured(file, context, defects);
    untracked_derived_function(file, context, defects);
}

/// `v1/untracked-derived-function` — upstream's `untrackedReactive`.
///
/// A function that derives from reactive state and is only ever called where
/// nothing tracks. Wrapping reads in a function defers them, but deferral only
/// pays off if something eventually calls it in a tracking scope; otherwise
/// the derivation reads its inputs once per call and subscribes to nothing.
///
/// # Why this one is narrow
///
/// Every other rule here proves something local and positive. This one has to
/// prove a *negative* — that no tracking scope anywhere ever calls the
/// function — and a wrong negative reports the single most common shape in
/// Solid code. So it fires only when every use is enumerable and every one of
/// them is untracked:
///
/// - the function is bound inside another function, so no reference to it can
///   exist outside this file;
/// - every reference is the callee of a direct call, never an argument, never
///   a return value, never a JSX child — anything else could hand it to a
///   tracking scope this cannot see;
/// - every call sits directly in the declaring function's own body, not in a
///   nested one, since a nested function may itself be a callback somewhere
///   tracked;
/// - and none is inside JSX, which tracks.
///
/// A single reference that fails any of those abandons the function rather
/// than guessing. That leaves the textbook case and little else, which is the
/// right trade for a rule whose false positives would land on correct code.
///
/// # Why the abandonments are not classified further
///
/// Two stronger fact domains were weighed here and deliberately not consulted:
///
/// - **Dialect callback slots / compiler tracked regions.** A call inside
///   `createMemo(() => derived())` could be *proven* tracked instead of
///   abandoned — but a tracked call already means the derivation works, so
///   "proven tracked" and "abandoned" both end in silence. Classifying
///   changes no outcome.
/// - **Counting deferred positions as untracked evidence.** A call inside an
///   event handler, `onMount`, or `untrack` is provably outside tracking,
///   but reading current values there is idiomatic — upstream exempts those
///   positions too — so counting them as defect evidence would flag correct
///   code.
fn untracked_derived_function(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    let candidates = file
        .ast
        .bindings
        .iter()
        .filter_map(|binding| {
            if !binding.initializer_function {
                return None;
            }
            let (Some(initializer), [declared]) = (binding.initializer, binding.names.as_slice())
            else {
                return None;
            };
            let function = function_at(file, initializer)?;
            // Bound inside another function: references cannot escape the
            // file except by being passed or returned, both refused below.
            let enclosing = containing_ast_function(&file.ast, binding.declaration)?;
            let symbol = context
                .entities
                .at(file.path.as_str(), declared.span)?
                .clone();
            Some((binding, declared, function, enclosing, symbol))
        })
        .collect::<Vec<_>>();

    // Derivation is transitive: `bar = () => foo()` derives whenever `foo`
    // does. Only reads/calls directly owned by the candidate function count;
    // a dormant nested function is code the outer function never executes.
    let mut derived = std::collections::HashSet::new();
    loop {
        let mut changed = false;
        for (_, _, function, _, symbol) in &candidates {
            if derived.contains(symbol) {
                continue;
            }
            let direct_source = file.ast.identifiers.iter().any(|identifier| {
                identifier.role == IdentifierRole::Reference
                    && function.body.contains(identifier.span)
                    && containing_ast_function(&file.ast, identifier.span)
                        .is_some_and(|owner| owner.span == function.span)
                    && context
                        .entities
                        .at(file.path.as_str(), identifier.span)
                        .is_some_and(|read| {
                            read != symbol
                                && (context.accessors.contains_key(read)
                                    || context.prop_sources.get(read).is_some_and(
                                        |(_, declaration)| {
                                            // Proven-static props are not
                                            // reactive state to derive from.
                                            context.props_reactivity.object_use(declaration)
                                                != crate::source_discovery::PropUse::Static
                                        },
                                    ))
                        })
            });
            let derived_call = file.ast.calls.iter().any(|call| {
                function.body.contains(call.span)
                    && containing_ast_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == function.span)
                    && context
                        .entities
                        .at(file.path.as_str(), call.callee)
                        .is_some_and(|callee| derived.contains(callee))
            });
            if direct_source || derived_call {
                changed |= derived.insert(symbol.clone());
            }
        }
        if !changed {
            break;
        }
    }

    // Execution-role gate (2.0 catalog): a call in a tracked compute tracks
    // its reads, and a call in an event handler, deferred/leaf callback,
    // effect apply, untrack, or directive application reads legitimately
    // fresh values at call time. Neither is untracked-read evidence.
    let role_exemptions = context.dialect.derived_function_role_exemptions();
    let allowed = if role_exemptions {
        crate::execution_role::allowed_callback_spans(file, context.lookup)
    } else {
        Vec::new()
    };
    for (_, declared, _, enclosing, symbol) in candidates {
        if !derived.contains(&symbol) {
            continue;
        }
        let mut calls = 0usize;
        let enumerable = file
            .ast
            .identifiers
            .iter()
            .filter(|identifier| {
                identifier.role == IdentifierRole::Reference
                    && context.entities.at(file.path.as_str(), identifier.span) == Some(&symbol)
            })
            .all(|identifier| {
                let Some(call) = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| call.callee == identifier.span)
                else {
                    return false; // referenced without being called
                };
                let directly_enclosed = containing_ast_function(&file.ast, call.span)
                    .is_some_and(|owner| owner.span == enclosing.span);
                if !directly_enclosed || within_jsx(file, call.span) {
                    return false;
                }
                if role_exemptions
                    && matches!(
                        crate::execution_role::semantic_execution_role(
                            file,
                            call.span,
                            &allowed,
                            context.entities,
                            context.lookup.symbol_names(),
                            context.lookup,
                        ),
                        crate::ExecutionRole::TrackedJsx
                            | crate::ExecutionRole::EventCallback
                            | crate::ExecutionRole::DeferredCallback
                            | crate::ExecutionRole::UntrackedCallback
                            | crate::ExecutionRole::EffectApply
                            | crate::ExecutionRole::DirectiveApply
                    )
                {
                    // Tracked or fresh-at-call-time: this call misbehaves in
                    // no way and contributes no untracked evidence.
                    return true;
                }
                calls += 1;
                true
            });
        if !enumerable || calls == 0 {
            continue;
        }
        let name = text(file, declared.span);
        defects.push(StaticDefect {
            kind: StaticDefectKind::UntrackedDerivedFunction {
                name: name.to_owned(),
            },
            location: location(file.path.shared(), declared.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: false,
        });
    }
}

/// Whether a span sits anywhere inside JSX, which is a tracking scope.
///
/// Fragments track their children exactly as elements do and live in their
/// own table, so both are consulted; checking only elements reported a
/// derived function rendered through `<>{…}</>` as never tracked.
fn within_jsx(file: &FileFacts, span: Span) -> bool {
    file.ast
        .jsx_elements
        .iter()
        .any(|element| element.span.contains(span))
        || file
            .ast
            .jsx_fragments
            .iter()
            .any(|fragment| fragment.contains(span))
}

/// `v1/reactive-source-uncaptured` — the uncertifiable half of upstream's
/// `reactivity`.
///
/// A proven reactive source handed, uncalled, to a callee nothing in the
/// analysis can describe: not one of the dialect's primitives, no body in the
/// project, no package-contract summary. The callee may call it in a tracking
/// scope, read it once and sever the reactivity, or store it for later — and
/// which of those it does decides whether the surrounding code is correct.
///
/// This reports the *gap*, not a defect, which is why it is uncertifiable
/// rather than a violation. It is the call-site companion to
/// `v1/package-contract-export-missing`: that rule fires once at an
/// undescribed import, this one fires where the missing description actually
/// costs certification.
///
/// Passing a source on is idiomatic — it is how accessors travel — so the
/// unknown callee is the entire premise. A helper defined in the project is
/// read directly and never reported here.
///
/// Scoped to callees imported from a package, because those are the callees
/// the fix applies to: the hint says "describe this export in the package's
/// contract", which is only actionable for a package export. An ambient
/// global (`setTimeout`, `console.log`, an array method) comes from no
/// package, so reporting it demanded a contract nobody can write — the rule
/// stays silent there and the reads flowing through remain uncertified
/// rather than reported.
fn reactive_source_uncaptured(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    // Symbols bound by value imports from bare specifiers — the only callees
    // whose reactive behaviour a package contract could ever describe. A
    // bare specifier is necessary but not sufficient: a tsconfig path alias
    // (`@/utils/x`) is spelled bare too, so anything TypeScript resolved
    // into the project's own sources is excluded — no contract can describe
    // project code, and the engine walks it directly.
    let package_imported: std::collections::HashSet<&crate::SymbolId> = file
        .ast
        .imports
        .iter()
        .filter(|import| !import.module.starts_with('.') && !import.module.starts_with('/'))
        .flat_map(|import| &import.bindings)
        .filter(|binding| !binding.type_only)
        .filter_map(|binding| context.entities.at(file.path.as_str(), binding.local.span))
        .filter(|symbol| !context.lookup.symbol_is_project_code(symbol.as_str()))
        .collect();
    if package_imported.is_empty() {
        return;
    }
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        // A dialect primitive is described by definition, and a call the
        // engine resolved into the project has a body it already walked.
        if known_primitive(&primitives.calls[index]).is_some()
            || context
                .lookup
                .function_called_at(file.path.as_str(), call.callee)
                .is_some()
        {
            continue;
        }
        let callee_symbol = context.entities.at(file.path.as_str(), call.callee);
        let Some(callee_symbol) = callee_symbol else {
            continue;
        };
        if !package_imported.contains(callee_symbol)
            || context.contracted.contains_key(callee_symbol)
        {
            continue;
        }
        let callee_text = text(file, call.callee);
        for argument in &call.arguments {
            // Only a bare reference to the source itself. A call, a member
            // read, or an expression built from one passes a *value*, whose
            // reactivity was already resolved before the callee saw it.
            let Some(symbol) = context.entities.at(file.path.as_str(), argument.span) else {
                continue;
            };
            let Some((name, _)) = context.accessors.get(symbol) else {
                continue;
            };
            if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                continue;
            }
            let name = name.as_str();
            defects.push(StaticDefect {
                kind: StaticDefectKind::ReactiveSourceUncaptured {
                    source: name.to_owned(),
                    callee: callee_text.to_owned(),
                },
                location: location(file.path.shared(), argument.span),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
}

/// `v1/no-async-tracked-scope` — upstream's `noAsyncTrackedScope`.
///
/// An `async` function handed to a position the dialect tracks. A computation
/// collects dependencies only until its first suspension point, so every read
/// after an `await` subscribes to nothing and the scope silently stops
/// responding.
///
/// Which slot tracks is asked of the dialect rather than assumed, and that is
/// the whole reason this is a per-dialect rule: `createEffect`'s callback is
/// argument 0 in 1.x and argument 1 in 2.0, so a table baked for one version
/// reports the other's seed value as a tracked scope.
///
/// Scoped to functions written at the call. An identifier naming an `async`
/// function declared elsewhere is the same defect, but proving the binding
/// reaches this slot is interprocedural work the engine already does for
/// reads — [`crate::StaticViolation`]s for those surface as
/// `v1/reactive-read-after-await` instead, per-read and with the offending
/// read located.
///
/// # Why this asks only the dialect, not contracts or async type facts
///
/// - **Package contracts.** A contract's `ContractCallback` carries an
///   `execution` of `"tracked"`, but that means "the graph schedules it" —
///   an `onSettled`-style callback is contract-tracked while its reads do
///   not subscribe (see `Dialect::callback_semantics_at`). Flagging every
///   async literal handed to a contract-tracked slot would therefore report
///   correct code. The engine already threads contract callbacks through its
///   interprocedural graph, so an actual read-after-await inside one still
///   surfaces, per-read, as SC1001/SC1002.
/// - **`can_return_async` type facts.** A non-`async` function that returns
///   a Promise has no `await`, so every read in it runs before any
///   suspension and subscribes normally — the defect this rule reports
///   cannot occur in it.
///
/// # Why the 2.0 catalog does not carry this rule
///
/// Solid 2.0 models async computations as a feature: an async compute
/// produces an async accessor the engine tracks through `async_reads` and
/// the `Loading`-boundary rules (SC5001–SC5003). A blanket "no async in a
/// tracked slot" would contradict the dialect's own model there; 2.0's
/// after-await reads are still covered per-read by SC1002.
fn no_async_tracked_scope(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let primitives = context.lookup.primitives(file);
    for (index, call) in file.ast.calls.iter().enumerate() {
        let Some(primitive) = known_primitive(&primitives.calls[index]) else {
            continue;
        };
        let name = primitives.calls[index]
            .as_ref()
            .map_or("", crate::PrimitiveName::as_str);
        for slot in 0..call.arguments.len() {
            if !context
                .dialect
                .callback_semantics_at(primitive, slot, call.arguments.len())
                .tracks_reads
            {
                continue;
            }
            let argument = &call.arguments[slot];
            let Some(function) = function_at(file, argument.span) else {
                continue;
            };
            if !function.r#async {
                continue;
            }
            violations.push(StaticViolation {
                id: "SC5004".into(),
                rule: "no-async-tracked-scope".into(),
                message: format!(
                    "this {name} scope is an async function; Solid tracks dependencies only up to the first await, so every reactive read after it subscribes to nothing"
                ),
                hint: format!(
                    "Keep the {name} scope synchronous and move the async work into createResource, whose source function stays tracked; read the resulting accessor from here."
                ),
                location: location(file.path.shared(), argument.span),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
}

/// `v1/expected-function-got-expression` — upstream's
/// `expectedFunctionGotExpression`.
///
/// A native element's event-handler binding receiving an already-evaluated
/// expression: `onClick={count()}` binds whatever `count()` returned during
/// setup as the listener.
///
/// Narrower than upstream, deliberately. `onClick={makeHandler()}` is a
/// factory call and correct, and upstream cannot tell the two apart because it
/// works from syntax. Two things are proof here, and nothing else is reported:
/// the callee is a *proven* reactive accessor, so the call yields that
/// accessor's value; or TypeScript resolved the whole expression to a type
/// that is not callable. A factory whose return type is a function satisfies
/// neither and stays silent.
fn expected_function_got_expression(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for element in &file.ast.jsx_elements {
        let element_name = text(file, element.name.span);
        // Components receive props, not listeners: `onFoo` on a component is
        // an ordinary prop and its value may legitimately be anything.
        if element_name.contains('.') || !is_lowercase_led(element_name) {
            continue;
        }
        for attribute in element
            .attributes
            .iter()
            .filter(|attribute| attribute.namespace.is_none())
        {
            let name = text(file, attribute.name);
            if !name.starts_with("on")
                || !name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic)
            {
                continue;
            }
            let Some(expression) = attribute.expression else {
                continue;
            };
            // A native listener receives its function value once during DOM
            // setup. Reading that function through reactive props/store state
            // here freezes the initial handler. The member root is a proven
            // source; a plain object member is left alone. Props follow the
            // caller classification: a handler prop every call site passes
            // statically is a plain property whose value never changes —
            // installing it once is exactly right and stays silent.
            let member_symbol = file
                .ast
                .members
                .iter()
                .find(|member| member.span == expression)
                .and_then(|_| member_root(file, expression))
                .and_then(|root| {
                    Some((root, context.entities.at(file.path.as_str(), root)?))
                });
            if let Some((root, symbol)) = member_symbol {
                let uncertain = if context.accessors.contains_key(symbol) {
                    Some(false)
                } else if let Some((_, declaration)) = context.prop_sources.get(symbol) {
                    let property = file
                        .ast
                        .members
                        .iter()
                        .find(|member| member.object == root && expression.contains(member.span))
                        .map(|member| text(file, member.property))
                        .unwrap_or_default();
                    match context.props_reactivity.prop_use(declaration, property) {
                        crate::source_discovery::PropUse::Static => None,
                        crate::source_discovery::PropUse::Reactive => Some(false),
                        crate::source_discovery::PropUse::Unknown => Some(true),
                    }
                } else {
                    None
                };
                if let Some(uncertain) = uncertain {
                    defects.push(StaticDefect {
                        kind: StaticDefectKind::ReactiveHandlerRead {
                            attribute: name.to_owned(),
                            expression: text(file, expression).to_owned(),
                        },
                        location: location(file.path.shared(), expression),
                        analysis_context: String::new(),
                        fixes: vec![],
                        uncertain,
                    });
                    continue;
                }
                if context.accessors.contains_key(symbol)
                    || context.prop_sources.contains_key(symbol)
                {
                    // A proven-static prop member: not a defect, and not the
                    // call-result shape below either.
                    continue;
                }
            }
            // The binding must be a call spanning the whole expression. A bare
            // reference is the correct form, and a call merely *inside* the
            // expression — `onClick={() => save(id())}` — is the fix, not the
            // defect, so the call's span must be the expression minus its
            // surrounding whitespace.
            let call_span = trimmed_span(file, expression);
            let Some(call) = file.ast.calls.iter().find(|call| call.span == call_span) else {
                continue;
            };
            let proven_accessor = context
                .entities
                .at(file.path.as_str(), call.callee)
                .is_some_and(|symbol| {
                    context.accessors.contains_key(symbol)
                        && context.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                });
            if !proven_accessor && !proven_not_callable(context, file, expression) {
                continue;
            }
            defects.push(StaticDefect {
                kind: StaticDefectKind::HandlerCallResult {
                    attribute: name.to_owned(),
                    callee: text(file, call.callee).to_owned(),
                    call: text(file, call.span).to_owned(),
                },
                location: location(file.path.shared(), expression),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
}

/// Whether the expression's resolved type provably cannot be a listener.
///
/// The primary proof is the checker's own [`typefacts::Callability`] verdict,
/// which TypeScript derives from the actual call signatures of every union
/// constituent — never from rendered type text. The descriptor-text screen
/// below is only the fallback for spans whose callability was not demanded,
/// and only for the primitive types that can never hold a function.
///
/// An array- or tuple-shaped type is exempt from the callability proof even
/// though it has no call signatures: Solid's handler props accept a bound
/// `[handler, data]` pair, so a call returning one is a factory for a valid
/// listener, not a mistake.
fn proven_not_callable(
    context: &UpstreamCompatContext<'_>,
    file: &FileFacts,
    expression: Span,
) -> bool {
    // Exact-span facts first: the smallest *contained* entity of a call
    // expression is its callee, whose callability describes the accessor
    // being called, not the value the call produced.
    let descriptor = super::expression_descriptor(context, file, expression);
    let callability = super::expression_callability(context, file, expression);
    if descriptor
        .is_some_and(|descriptor| super::array_like_type(descriptor.text.as_ref(), callability))
    {
        return false;
    }
    match callability {
        Some(Callability::NonCallable) => true,
        Some(Callability::Callable | Callability::Mixed) => false,
        Some(Callability::Unknown) | None => descriptor
            .map(|descriptor| descriptor.text.as_ref())
            .or_else(|| {
                // No fact at the exact span: fall back to the contained
                // descriptor the rule consulted before exact demands
                // existed. Text-screened, so a callee's function type never
                // reads as "not callable".
                context
                    .lookup
                    .smallest_contained_descriptor(file.path.as_str(), expression)
                    .map(|descriptor| descriptor.text.as_ref())
            })
            .is_some_and(is_not_callable_text),
    }
}

/// The descriptor-text fallback: only the primitive types that can never
/// hold a function count. Anything structural, generic, aliased, or
/// unresolved is left alone, because a type this cannot read is not proof of
/// anything.
fn is_not_callable_text(descriptor: &str) -> bool {
    matches!(
        descriptor,
        "string" | "number" | "boolean" | "bigint" | "symbol" | "void" | "null" | "undefined"
    ) || descriptor.starts_with('"')
        || descriptor.parse::<f64>().is_ok()
}

/// The sub-span of `span` with surrounding whitespace stripped, so a span
/// can be compared against an exact AST node span without comparing text.
fn trimmed_span(file: &FileFacts, span: Span) -> Span {
    let source = text(file, span);
    let leading = source.len() - source.trim_start().len();
    let trailing = source.len() - source.trim_end().len();
    Span::new(span.start + leading as u32, span.end - trailing as u32)
}

/// The function written at exactly this span, if the argument is a literal
/// function rather than a reference to one.
fn function_at(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .functions
        .iter()
        .find(|function| function.span == span)
}

/// `v1/uncalled-accessor` — upstream's `badSignal`.
///
/// An accessor referenced where a *value* was meant: interpolated into a
/// template literal, used as a coercive operand, used as a computed member
/// key, or assigned to a native JSX value attribute. The accessor is a
/// function, so the expression sees `function count() {...}` rather than the
/// current value, and it never updates.
///
/// The positions are the ones upstream enumerates; what differs is the
/// premise. A reference only counts when the engine proved the symbol is an
/// accessor, so a plain function of the same shape is not reported, and an
/// accessor that reached this file through a helper or a re-export is.
fn uncalled_accessor(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for identifier in &file.ast.identifiers {
        if identifier.role != IdentifierRole::Reference {
            continue;
        }
        let Some(symbol) = source_symbol_at(context, file, identifier.span) else {
            continue;
        };
        // Proven accessor, and specifically an accessor: a store is read by
        // path, not by call, so `store.items` is correct and not this rule's
        // business.
        let Some((name, _)) = context.accessors.get(symbol) else {
            continue;
        };
        let name = name.as_str();
        if context.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            continue;
        }
        // A call of the accessor is the correct use; so is passing it on,
        // which hands the callee something it can call later. The exemption
        // is deliberately unconditional: TypeFacts' resolved-call parameter
        // callability could in principle prove a callee's slot non-callable,
        // but a non-callable parameter receiving an accessor is already a
        // type error TypeScript reports itself, and recovery-signature
        // resolutions would make the "proof" wrong exactly where it fired.
        if is_called(file, identifier.span) || is_argument(file, identifier.span) {
            continue;
        }
        let Some(position) = value_position(file, identifier.span, context.dialect) else {
            continue;
        };
        defects.push(StaticDefect {
            kind: StaticDefectKind::UncalledAccessor {
                name: name.to_owned(),
                position: position.to_owned(),
            },
            location: location(file.path.shared(), identifier.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: false,
        });
    }
}

fn source_symbol_at<'a>(
    context: &'a UpstreamCompatContext<'_>,
    file: &FileFacts,
    span: Span,
) -> Option<&'a crate::SymbolId> {
    context.entities.at(file.path.as_str(), span).or_else(|| {
        context
            .source_reference_index
            .get(file.path.as_str())
            .and_then(|by_range| by_range.get(&(u64::from(span.start), u64::from(span.end))))
    })
}

/// `v1/no-direct-mutation` — upstream's `noWrite`.
///
/// A reactive value written through instead of through its setter: `count =
/// 2` on a signal accessor, or `props.value = x` / `store.a.b = x` on props
/// and stores, which are readonly proxies in Solid.
fn no_direct_mutation(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    defects: &mut Vec<StaticDefect>,
) {
    for assignment in &file.ast.assignments {
        // The assignment target is either the reactive binding itself or the
        // object of a member chain rooted at one. The root is resolved with
        // the same reference-index fallback `uncalled-accessor` uses: an
        // assignment target is an ordinary operator operand, exactly the
        // expression shape entity facts may skip (see the field doc on
        // `source_reference_index`).
        let root = member_root(file, assignment.target).unwrap_or(assignment.target);
        let Some(symbol) = source_symbol_at(context, file, root) else {
            continue;
        };
        // Proven accessors and proven props roots are both readonly through
        // members; `prop_sources` covers the props objects the accessor map
        // does not.
        let (name, _) = match context.accessors.get(symbol) {
            Some(source) => source,
            None => {
                let Some(source) = context.prop_sources.get(symbol) else {
                    continue;
                };
                source
            }
        };
        let props = !context.accessors.contains_key(symbol);
        let name = name.as_str();
        let through_member = root != assignment.target;
        let kind = context.source_kinds.get(symbol).copied();
        // `createMutable` is the one Solid 1.x source whose proxy is designed
        // to be written through directly. It still behaves like a store for
        // reads, but treating that shared read shape as readonly recreates
        // eslint-plugin-solid issue #99.
        if through_member
            && context
                .source_primitives
                .get(symbol)
                .is_some_and(|primitive| primitive.as_str() == "createMutable")
        {
            continue;
        }
        // A props root only misbehaves when written *through* — rebinding a
        // local `props`-like identifier is shadowing, not a dropped write.
        if props && !through_member {
            continue;
        }
        let target = if through_member {
            if props {
                DirectMutationTarget::Props
            } else if kind == Some(ReactiveSourceKind::Store) {
                DirectMutationTarget::Store
            } else {
                DirectMutationTarget::ReactiveValue
            }
        } else {
            DirectMutationTarget::AccessorBinding
        };
        // 2.0 write-enables the original store proxy for the duration of its
        // own setter's draft callback (probed on rc.0: the write commits;
        // through another store's setter it is silently dropped). A write
        // through the store rooted lexically inside that store's own setter
        // callback is therefore correct code, not a dropped write.
        if target == DirectMutationTarget::Store
            && context.dialect.store_setter_callback_enables_proxy_writes()
            && inside_own_setter_callback(file, context, assignment.target, symbol)
        {
            continue;
        }
        defects.push(StaticDefect {
            kind: StaticDefectKind::DirectMutation {
                name: name.to_owned(),
                target,
            },
            location: location(file.path.shared(), assignment.target),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: false,
        });
    }
}

/// Whether `target` (an assignment through a store proxy rooted at
/// `store_symbol`) sits lexically inside a callback passed to that store's
/// *own* setter.
///
/// The pairing is proven, not guessed: the callee must resolve to the second
/// slot of the exact array destructuring whose first slot is `store_symbol` —
/// `const [store, setStore] = createStore(...)`. Another store's setter, a
/// same-spelled local, or an eagerly evaluated argument (no function between
/// the assignment and the argument) all fail the proof and keep the finding.
fn inside_own_setter_callback(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    target: Span,
    store_symbol: &crate::SymbolId,
) -> bool {
    file.ast.arguments_containing(target).any(|(call, index)| {
        let argument = &call.arguments[index];
        // The write must run when the setter invokes the callback, not while
        // building the argument list.
        if !file
            .ast
            .functions_within(argument.span)
            .any(|function| function.body.contains(target))
        {
            return false;
        }
        let Some((binding_file, binding, callee_symbol)) = context
            .lookup
            .binding_at_reference(file.path.as_str(), call.callee)
        else {
            return false;
        };
        if binding.shape != solid_facts::ast::BindingShape::Array {
            return false;
        }
        let slot_symbol = |slot: usize| {
            binding
                .array_slots
                .get(slot)
                .and_then(Option::as_ref)
                .and_then(|name| context.entities.at(binding_file.path.as_str(), name.span))
        };
        slot_symbol(1) == Some(&callee_symbol) && slot_symbol(0) == Some(store_symbol)
    })
}

/// Whether this identifier is the callee of a call — the correct use.
fn is_called(file: &FileFacts, span: Span) -> bool {
    file.ast.calls.iter().any(|call| call.callee == span)
}

/// Whether this identifier is being passed to a call, which hands the
/// receiver something it can call later and is how accessors travel.
fn is_argument(file: &FileFacts, span: Span) -> bool {
    file.ast
        .calls
        .iter()
        .any(|call| call.arguments.iter().any(|argument| argument.span == span))
}

/// The value position this reference sits in, phrased for the message, or
/// `None` when the position does not demand a value.
///
/// Deliberately narrow. Every arm is a position where a function object is
/// provably not what the author meant: string interpolation renders
/// `function () {...}`, a computed key stringifies it, and the rest is
/// upstream's own enumeration. Anywhere else — a bare reference, a return, a
/// property value — passing the accessor on is idiomatic, and reporting it
/// would be the false positive upstream's issue #193 describes.
fn value_position(
    file: &FileFacts,
    span: Span,
    dialect: &dyn solid_dialect::Dialect,
) -> Option<&'static str> {
    if let Some(operand) = file
        .ast
        .coercive_operands
        .iter()
        .find(|operand| operand.span == span)
    {
        return Some(match operand.kind {
            solid_facts::ast::CoerciveOperandKind::Binary => "a binary operator",
            solid_facts::ast::CoerciveOperandKind::Unary => "a unary operator",
        });
    }

    // Component props are lazy getters and may intentionally receive an
    // accessor. Native attributes are different: the compiler hands the
    // expression's value to the DOM operation, so a bare accessor is
    // stringified/assigned as the function object. Callback-like attributes
    // remain excluded because their contract expects a function — and, where
    // the dialect's runtime routes a `children` attribute through child
    // insertion (which calls zero-argument functions; code-read on
    // `@solidjs/web@2.0.0-rc.0` `insert`/`flatten`), `children` is a live
    // callback position too, not a value position.
    if file.ast.jsx_elements.iter().any(|element| {
        let element_name = text(file, element.name.span);
        is_lowercase_led(element_name)
            && element.attributes.iter().any(|attribute| {
                let name = text(file, attribute.name);
                attribute.namespace.is_none()
                    && attribute.expression == Some(span)
                    && name != "ref"
                    && !(name == "children" && dialect.native_children_attribute_invokes_functions())
                    && !(name.starts_with("on")
                        && name.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic))
            })
    }) {
        return Some("a native JSX attribute");
    }

    // The object form of `class` coerces each property value by truthiness
    // (probed on `@solidjs/web@2.0.0-rc.0`: `ssrClassName({ active: () =>
    // false })` renders "active"; the client's `className` applies the same
    // `!!value[key]`). An accessor object is always truthy, so the class is
    // permanently on and never updates — in the object form directly or
    // nested in the array form.
    if dialect.class_object_values_are_truthiness_coerced()
        && file.ast.object_properties.iter().any(|property| {
            property.value == span
                && !property.computed
                && file.ast.jsx_elements.iter().any(|element| {
                    is_lowercase_led(text(file, element.name.span))
                        && element.attributes.iter().any(|attribute| {
                            attribute.namespace.is_none()
                                && text(file, attribute.name) == "class"
                                && attribute
                                    .expression
                                    .is_some_and(|expression| expression.contains(property.span))
                        })
                })
        })
    {
        return Some("a class object value, which is coerced by truthiness");
    }

    // An untagged template literal stringifies each interpolation, so an
    // accessor there renders its own source text. A *tagged* one hands the
    // interpolations to the tag as values, which may legitimately call
    // them. The fact table records only untagged literals — a tag's quasi
    // is filtered out at collection — so `css`color: ${theme}`` stays out
    // of this rule while an untagged template nested inside a tagged
    // interpolation still reports.
    if file
        .ast
        .template_literals
        .iter()
        .any(|template| template.expressions.contains(&span))
    {
        return Some("a template literal");
    }
    if file
        .ast
        .members
        .iter()
        .any(|member| member.property == span && file.ast.computed_members.contains(&member.span))
    {
        return Some("a computed property access");
    }
    None
}

/// The object a member chain is rooted at: `store.a.b` → `store`.
fn member_root(file: &FileFacts, span: Span) -> Option<Span> {
    let mut current = file.ast.members.iter().find(|member| member.span == span)?;
    loop {
        match file
            .ast
            .members
            .iter()
            .find(|member| member.span == current.object)
        {
            Some(outer) => current = outer,
            None => return Some(current.object),
        }
    }
}
