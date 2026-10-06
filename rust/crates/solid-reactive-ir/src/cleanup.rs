//! Leaf-owner diagnostics, and the cleanup-return *classification* the
//! ownership rules consume.
//!
//! Detects `onCleanup`/leaf-owner misuse, and answers the owner-analysis
//! subsystem's question "does this callback hand the owner a cleanup"
//! (`function_cleanup_return_proof`), which SC4001 and SC4001 depend on.
//!
//! It deliberately reports nothing about a returned value's *legality*. Solid
//! 2.0 types the effect callback's return as `(() => void) | void`
//! (`EffectFunction` in `@solidjs/signals`), so every illegal return is
//! already a TypeScript error and AGENTS.md's absolute rule puts it out of
//! scope; the classification below survives because ownership and disposal are
//! not expressible as a type. See `docs/precision-backlog.md` for the ledger
//! entry and the `tsc` evidence.

use std::collections::{HashMap, HashSet};

use solid_dialect::{CleanupRule, Primitive};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::{ResolvedCallValidity, RuntimeValueDomain};

use super::{
    Fix, LeafOwnerOperation, PrimitiveName, SemanticLookup, SymbolId, TextEdit,
    call_primitive_name, location,
};
use crate::execution_role::direct_callback_contains;
use crate::owners::{callback_owner_at_call, containing_ast_function};
use crate::pipeline::{AnalysisContext, ProgramDraft, parallel_file_results};

/// Runs the project-level leaf-owner stage.
pub(crate) fn collect_project(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let safe_call_symbols = ctx
        .accessors
        .keys()
        .chain(ctx.setters.keys())
        .chain(ctx.actions.keys())
        .cloned()
        .collect::<HashSet<_>>();
    let setter_symbols = ctx.setters.keys().cloned().collect::<HashSet<_>>();
    draft.leaf_operations.extend(
        parallel_file_results(&ctx.facts.files, |file| {
            leaf_owner_operations_for_file(
                file,
                ctx.symbol_names,
                &safe_call_symbols,
                &setter_symbols,
                ctx.semantic_lookup,
            )
        })
        .into_iter()
        .flatten(),
    );
}

pub(super) fn leaf_owner_operations_for_file(
    file: &FileFacts,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    safe_call_symbols: &HashSet<SymbolId>,
    setter_symbols: &HashSet<SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Vec<LeafOwnerOperation> {
    let Some(file) = lookup.file_by_path(file.path.as_str()) else {
        return Vec::new();
    };
    let entities = lookup.entities();
    let dialect = lookup.dialect;
    let resolution = LeafScopeResolution {
        lookup,
        symbol_names,
        safe_call_symbols,
        setter_symbols,
    };
    let mut operations = Vec::new();
    for owner_call in &file.ast.calls {
        // The compiler deleted this call. Gated at the owner call, which is
        // this pass's single entry: every operation below -- a forbidden
        // primitive, an unresolved callback, a cross-file helper's operations
        // -- exists only because a leaf scope opens here, and a call the
        // emitter removed opens none. "These nested primitives are never
        // disposed" describes a disposal that never comes due.
        //
        // A positive lookup and an early return, the shape
        // `push_owner_requirement` uses, because this pass consults no
        // execution facts at all and so has no role to give a
        // `DiscardedRendering` arm to. `uncertain` would be wrong for the same
        // reason it is wrong there: nothing here is unproven, the two
        // compilers agree the code is gone.
        //
        // The gate is on the owner call rather than on each push site because
        // deletion travels down, not up: a producer `Elided` span is a single
        // attribute or child value expression, so any nested call it contains
        // is contained with it, while a leaf callback resolved in *another*
        // file is only reachable from this deleted call site.
        if crate::execution_role::discarded_region_contains(file, owner_call.span) {
            continue;
        }
        let owner = call_primitive_name(file, owner_call, entities, symbol_names, dialect);
        let Some(owner) = owner.as_ref() else {
            continue;
        };
        // The leaf owners: an owner whose callback is the end of the ownership
        // chain, so anything created inside it never gets disposed.
        //
        // Asked of the dialect rather than matched here. The pair this used to
        // hardcode -- `onSettled` and `createTrackedEffect` -- is 2.0-only, so
        // under 1.x these rules could not fire at all. Taking the argument
        // index from the same answer also drops the assumption that the
        // callback is always first.
        let Some(callback_argument) = owner
            .primitive()
            .into_iter()
            .flat_map(|primitive| {
                (0..owner_call.arguments.len()).filter(move |index| {
                    callback_owner_at_call(file, owner_call, primitive, *index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
            })
            .next()
            .and_then(|index| owner_call.arguments.get(index))
        else {
            continue;
        };
        let region = callback_argument.span;
        // 2.0's `onSettled` is a leaf owner only when the call runs under a
        // live children-capable owner; out-of-band it enqueues a plain
        // callback where none of these operations throw. This pass is
        // lexical, so record the call for the owner fixed point to resolve
        // against the propagated owner graph.
        let call_site_gate = owner
            .primitive()
            .filter(|primitive| dialect.leaf_owner_requires_owned_call_site(*primitive))
            .map(|_| location(file.path.shared(), owner_call.span));
        // Both leaf-scope paths need the leaf callback itself, not just the
        // argument text it was written in, and the answer is the same for
        // every call in the region — compute it once per owner call.
        //
        // The callback must be an exact function value. Direct literals and
        // identifiers are resolved as before. A call is followed only through
        // a closed local return: one unconditional return of a function
        // literal, or one unconditional return of the exact callback
        // parameter. This proves the value received by the owner without
        // trusting a name, package, conditional, or wrapper with unknown
        // behavior. Every other valid opaque shape becomes SC9012.
        let callback_literal = callback_argument_literal(file, region);
        if callback_literal.is_none() && file.ast.call_at(region).is_none() {
            let callback_span = file.ast.peel_ts_sugar_span(region);
            // Preserve the identifier path's existing diagnostic anchor: the
            // owner receives the identifier itself, while the forbidden body
            // is only evidence for that operation.
            let exact_callback = entities
                .at(file.path.as_str(), callback_span)
                .and_then(|symbol| lookup.function_for_symbol(symbol));
            if let Some((callback_file, callback)) = exact_callback {
                let mut kinds = Vec::new();
                let mut visited = Vec::new();
                let complete = function_forbidden_operations(
                    resolution,
                    callback_file,
                    callback,
                    &mut kinds,
                    &mut visited,
                    8,
                    None,
                );
                let via = file
                    .source_text(callback_span)
                    .unwrap_or_default()
                    .to_owned();
                for kind in kinds {
                    operations.push(LeafOwnerOperation {
                        through_contract: false,
                        kind,
                        owner: owner.to_string(),
                        location: location(file.path.shared(), callback_span),
                        fix: None,
                        call_site_gate: call_site_gate.clone(),
                        uncertain: false,
                        via: Some(via.clone()),
                    });
                }
                if complete {
                    continue;
                }
                operations.push(LeafOwnerOperation {
                    through_contract: false,
                    kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                    owner: owner.to_string(),
                    location: location(file.path.shared(), callback_span),
                    fix: None,
                    call_site_gate,
                    uncertain: true,
                    via: None,
                });
                continue;
            }
        }
        let Some((callback_file, leaf_callback)) = callback_literal
            .map(|callback| (file, callback))
            .or_else(|| callback_argument_function(lookup, file, region, 8))
        else {
            let valid_call = lookup
                .resolved_callee_call(file, owner_call.callee)
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            if !valid_call {
                // Invalid callback shapes belong to TypeScript. A missing
                // validity fact cannot safely distinguish those from a valid
                // opaque callback, so it cannot produce this obligation.
                continue;
            }
            let callback_span = file.ast.peel_ts_sugar_span(region);
            operations.push(LeafOwnerOperation {
                through_contract: false,
                kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                owner: owner.to_string(),
                location: location(file.path.shared(), callback_span),
                fix: None,
                call_site_gate,
                uncertain: true,
                via: None,
            });
            continue;
        };
        let callback_region = leaf_callback.span;
        for call in &callback_file.ast.calls {
            if call.span == owner_call.span || !callback_region.contains(call.span) {
                continue;
            }
            // And the call must sit in that callback's own synchronous
            // extent: a call inside a nested function (an event handler built
            // in the callback) runs later, in that function's scope, where
            // the leaf scope is no longer live.
            if !direct_callback_contains(callback_file, leaf_callback.span, call.span) {
                continue;
            }
            let primitive =
                call_primitive_name(callback_file, call, entities, symbol_names, dialect);
            let Some(primitive) = primitive else {
                // ADR 0179: a package export whose accepted contract states a
                // registration on its caller's owner, at the call and on every
                // call, performs it here -- inside the leaf scope.
                if let Some(registrations) = lookup
                    .callee_symbol(callback_file, call.callee)
                    .and_then(|symbol| lookup.contract_leaf_forbidden_operations(symbol))
                    .filter(|registrations| !registrations.is_empty())
                {
                    let via = callback_file
                        .source_text(call.callee)
                        .unwrap_or_default()
                        .to_owned();
                    for registration in registrations {
                        operations.push(LeafOwnerOperation {
                            through_contract: true,
                            kind: match registration {
                                crate::OwnerRequirementOperation::Cleanup => {
                                    crate::LeafOwnerOperationKind::Cleanup
                                }
                                _ => crate::LeafOwnerOperationKind::Primitive(via.clone()),
                            },
                            owner: owner.to_string(),
                            location: location(callback_file.path.shared(), call.callee),
                            fix: None,
                            call_site_gate: call_site_gate.clone(),
                            uncertain: false,
                            via: Some(via.clone()),
                        });
                    }
                    continue;
                }
                // Not a primitive: an exactly-resolved in-project helper
                // called here runs its synchronous extent in this leaf
                // scope, so a forbidden operation inside it executes here.
                let mut kinds = Vec::new();
                let mut visited = Vec::new();
                let complete = helper_forbidden_operations(
                    resolution,
                    callback_file,
                    call,
                    &mut kinds,
                    &mut visited,
                    8,
                    None,
                );
                if kinds.is_empty() && complete {
                    continue;
                }
                let via = callback_file
                    .source_text(call.callee)
                    .unwrap_or_default()
                    .to_owned();
                for kind in kinds {
                    operations.push(LeafOwnerOperation {
                        through_contract: false,
                        kind,
                        owner: owner.to_string(),
                        location: location(callback_file.path.shared(), call.callee),
                        fix: None,
                        call_site_gate: call_site_gate.clone(),
                        uncertain: false,
                        via: Some(via.clone()),
                    });
                }
                if !complete {
                    operations.push(LeafOwnerOperation {
                        through_contract: false,
                        kind: crate::LeafOwnerOperationKind::UnresolvedCallback,
                        owner: owner.to_string(),
                        location: location(callback_file.path.shared(), call.callee),
                        fix: None,
                        call_site_gate: call_site_gate.clone(),
                        uncertain: true,
                        via: Some(via),
                    });
                }
                continue;
            };
            let Some(kind) = forbidden_operation_kind(dialect, callback_file, call, &primitive)
            else {
                continue;
            };
            // Only `onCleanup` has a rewrite -- return the cleanup
            // instead of registering it -- and only where the owner reads
            // a returned function as cleanup. That is a 2.0 idea: 1.x's
            // leaf owner threads return values elsewhere, so offering the
            // rewrite there would introduce a bug, not fix one.
            let fix = (primitive.primitive() == Some(Primitive::OnCleanup)
                && owner
                    .primitive()
                    .is_some_and(|owner| dialect.accepts_cleanup_return(owner)))
            .then(|| terminal_cleanup_fix(callback_file, leaf_callback.span, call))
            .flatten();
            operations.push(LeafOwnerOperation {
                through_contract: false,
                kind,
                owner: owner.to_string(),
                location: location(callback_file.path.shared(), call.callee),
                fix,
                call_site_gate: call_site_gate.clone(),
                uncertain: false,
                via: None,
            });
        }
    }
    operations
}

/// Resolve the function value an owner actually receives at an exact callback
/// argument. In addition to literals and identifiers, this follows a very
/// small closed-world return shape for local functions: the callee must be an
/// exact local function, and any compiler-resolved call fact present must be
/// valid. It must have exactly one unconditional return, and that return must
/// be a function literal or the exact callback parameter. A conditional
/// return, local alias, member, package function, or invalid fact remains
/// unresolved.
fn callback_argument_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a FileFacts,
    argument: Span,
    depth: usize,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
    if depth == 0 {
        return None;
    }
    if let Some(function) = callback_argument_literal(file, argument) {
        return Some((file, function));
    }
    let raw_argument = argument;
    let argument = file.ast.peel_ts_sugar_span(raw_argument);
    let Some(call) = file
        .ast
        .call_at(raw_argument)
        .or_else(|| file.ast.call_at(argument))
    else {
        let symbol = lookup.entities().at(file.path.as_str(), argument)?;
        return lookup.function_for_symbol(symbol);
    };
    let symbol = lookup.entities().at(file.path.as_str(), call.callee)?;
    let (factory_file, factory) = lookup.function_for_symbol(symbol)?;
    if lookup
        .resolved_callee_call(file, call.callee)
        .is_some_and(|resolved| resolved.validity != ResolvedCallValidity::Valid)
    {
        return None;
    }
    let returned = exact_function_return(factory_file, factory)?;
    let returned_value = returned.argument?;
    match returned.value {
        solid_facts::ast::ReturnValueKind::Function => factory_file
            .ast
            .functions
            .iter()
            .find(|function| function.span == returned_value)
            .map(|function| (factory_file, function)),
        solid_facts::ast::ReturnValueKind::Identifier => {
            let returned_symbol = lookup
                .entities()
                .at(factory_file.path.as_str(), returned_value)?;
            let parameter_index = factory.parameters.iter().position(|parameter| {
                parameter.names.iter().any(|name| {
                    lookup.entities().at(factory_file.path.as_str(), name.span)
                        == Some(returned_symbol)
                })
            })?;
            let argument = call.arguments.get(parameter_index)?;
            if argument.spread {
                return None;
            }
            callback_argument_function(lookup, file, argument.span, depth - 1)
        }
        solid_facts::ast::ReturnValueKind::Undefined
        | solid_facts::ast::ReturnValueKind::Call
        | solid_facts::ast::ReturnValueKind::Member
        | solid_facts::ast::ReturnValueKind::Other => None,
    }
}

/// The sole unconditional return of a local callback adapter.
fn exact_function_return<'a>(
    file: &'a FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::ReturnFact> {
    if let Some(returned) = function.expression_return.as_ref() {
        return Some(returned);
    }
    let returned = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .collect::<Vec<_>>();
    match returned.as_slice() {
        [only] if !only.conditional => Some(only),
        _ => None,
    }
}

/// The function literal written *directly* in a callback argument, or `None`
/// when the argument is any other expression.
///
/// Every rule that reasons about what runs inside the callback a callee
/// receives needs this, and only a literal in argument position makes the
/// enclosing argument text and that callback the same region.
/// `owner(makeCb())` and `owner(wrap(() => …))` both contain a call — the
/// first evaluated under the enclosing owner before the leaf scope exists, the
/// second handed to an opaque wrapper that decides whether and when it runs —
/// so neither is proof and both fail closed here.
///
/// Parentheses and whitespace are the only fillers a literal tolerates
/// between the argument's bounds and its own; anything else means the
/// function is an operand rather than the argument.
pub(crate) fn callback_argument_literal(
    file: &FileFacts,
    argument: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    let function = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start)?;
    let start = usize::try_from(argument.start).ok()?;
    let end = usize::try_from(argument.end).ok()?;
    let inner_start = usize::try_from(function.span.start).ok()?;
    let inner_end = usize::try_from(function.span.end).ok()?;
    let filler = |text: &str| {
        text.bytes()
            .all(|byte| byte.is_ascii_whitespace() || byte == b'(' || byte == b')')
    };
    (filler(file.source.get(start..inner_start)?) && filler(file.source.get(inner_end..end)?))
        .then_some(function)
}

/// The leaf-owner operation kind a call performs under `dialect`, or `None`
/// when the call is not a forbidden operation.
///
/// Shared by the lexical path (a primitive written inside the leaf callback)
/// and the dynamic-extent path (the same primitive reached through an exactly
/// resolved helper), so the two cannot answer differently for one call.
fn forbidden_operation_kind(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &PrimitiveName,
) -> Option<crate::LeafOwnerOperationKind> {
    let kind = primitive.primitive()?;
    let forbidden = match dialect.cleanup_rule(kind) {
        CleanupRule::Always => true,
        // `createSignal(fn)` registers work; `createSignal(0)` does not.
        // Flattening this arm into the unconditional one would turn every
        // plainly seeded signal under a leaf owner into a false positive.
        CleanupRule::WhenFirstArgumentIsFunction => call
            .arguments
            .first()
            .is_some_and(|argument| file.ast.functions_within(argument.span).next().is_some()),
        CleanupRule::Never => false,
    };
    forbidden.then(|| match kind {
        Primitive::OnCleanup => crate::LeafOwnerOperationKind::Cleanup,
        Primitive::Flush => crate::LeafOwnerOperationKind::Flush,
        _ => crate::LeafOwnerOperationKind::Primitive(primitive.to_string()),
    })
}

/// Collects the forbidden-operation kinds an exactly-resolved helper performs
/// in its own *synchronous extent* — its body minus nested function bodies,
/// which calling the helper does not execute — following further exact helper
/// calls transitively up to `depth`.
///
/// This is the dynamic-extent half of the leaf-owner rules: `onCleanup` or
/// `flush` in a helper called synchronously from a leaf callback throws at
/// runtime exactly as the inline spelling does. Only the exact TypeScript
/// entity join resolves a callee (see `SemanticLookup::function_for_symbol`);
/// an unresolved, ambiguous, or package callee contributes nothing here and
/// stays owned by the package-contract obligation surface.
/// The project-wide lookups both leaf-scope walkers thread through every hop.
#[derive(Clone, Copy)]
struct LeafScopeResolution<'a, 'lookup> {
    lookup: &'a SemanticLookup<'lookup>,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    /// Calls that cannot open a leaf scope of their own: accessors, setters,
    /// and actions.
    safe_call_symbols: &'a HashSet<SymbolId>,
    /// The setters among them, whose function argument runs (ADR 0210).
    setter_symbols: &'a HashSet<SymbolId>,
}

/// ADR 0209: the class a call's `this` is exactly an instance of, when the
/// function the call is written in is a method declared in that class and was
/// entered through such an instance. Inside an inherited method `this` may be
/// a subclass instance that overrides what `this.m` names, so no class is
/// carried there.
type ExactThis = Option<(String, Span)>;

/// ADR 0209: the method a member call runs, when its receiver is exactly an
/// instance of one project class.
///
/// The receiver is either `this` under [`ExactThis`], or a `const` bound
/// directly to `new C(…)` whose callee resolves to a project class. Then the
/// object's class is exactly `C`. The method TypeScript resolves runs when it
/// is declared in `C` itself, or, through `this` in `C`'s own method, when `C`
/// inherits it. Not when
/// some assignment writes a member of that name, or anything writes through a
/// `prototype` (`member_name_may_be_reassigned`). The method itself is
/// returned with the class its `this` is exact for, when it is declared in
/// that class.
fn exact_instance_method<'a>(
    lookup: &SemanticLookup<'a>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    exact_this: &ExactThis,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact, ExactThis)> {
    if call.construct {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .find(|member| member.span == callee)?;
    let name = file.source_text(member.property)?;
    if lookup.member_name_may_be_reassigned(name) {
        return None;
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let through_this = file.source_text(receiver) == Some("this");
    let class = if through_this {
        exact_this
            .clone()
            .filter(|(path, _)| path == file.path.as_str())?
    } else {
        let (binding_file, binding, _) =
            lookup.binding_at_reference(file.path.as_str(), receiver)?;
        if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
            return None;
        }
        let construction = binding_file
            .ast
            .call_at(binding_file.ast.peel_ts_sugar_span(binding.initializer?))
            .filter(|construction| construction.construct)?;
        let class_symbol = lookup.callee_symbol(binding_file, construction.callee)?;
        let (class_file, class) = lookup.class_for_symbol(class_symbol)?;
        (class_file.path.to_string(), class.span)
    };
    let symbol = lookup.callee_symbol(file, call.callee)?;
    let (method_file, method) = lookup.function_for_symbol(symbol)?;
    method.method_name.as_ref()?;
    let own = method_file.path.as_str() == class.0 && class.1.contains(method.span);
    // Through a binding, TypeScript resolves the member on the binding's type,
    // which an annotation can widen to a superclass (`const d: Base = new
    // Derived()` names `Base.run` while `Derived.run` runs). Only a method
    // declared in the constructed class itself is the one that runs. Inside
    // that class's own method, `this` is typed by that class, so an inherited
    // resolution is exact too.
    if !own && !through_this {
        return None;
    }
    Some((method_file, method, own.then_some(class)))
}

fn helper_forbidden_operations(
    resolution: LeafScopeResolution<'_, '_>,
    call_file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
    exact_this: ExactThis,
) -> bool {
    let LeafScopeResolution {
        lookup,
        safe_call_symbols,
        ..
    } = resolution;
    if depth == 0 {
        return false;
    }
    if let Some((method_file, method, method_this)) =
        exact_instance_method(lookup, call_file, call, &exact_this)
    {
        return function_forbidden_operations(
            resolution,
            method_file,
            method,
            kinds,
            visited,
            depth,
            method_this,
        );
    }
    let callee = call_file.ast.peel_ts_sugar_span(call.callee);
    if lookup.is_member_span(call_file, callee)
        || call_file
            .ast
            .computed_members
            .binary_search(&callee)
            .is_ok()
    {
        return member_call_operations(resolution, call_file, call, callee, kinds, visited, depth);
    }
    let Some(symbol) = lookup.entities().at(call_file.path.as_str(), call.callee) else {
        return false;
    };
    if resolution.setter_symbols.contains(symbol) {
        // A setter runs a function argument -- the updater -- before it
        // returns (ADR 0210).
        return setter_updater_operations(resolution, call_file, call, kinds, visited, depth);
    }
    if safe_call_symbols.contains(symbol) {
        return true;
    }
    let Some((helper_file, helper)) = lookup.function_for_symbol(symbol) else {
        let Some(resolved) =
            lookup
                .resolved_callee_call(call_file, call.callee)
                .filter(|resolved| {
                    resolved
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.standard_library)
                })
        else {
            return false;
        };
        return standard_library_argument_operations(
            resolution, call_file, call, resolved, kinds, visited, depth,
        );
    };
    function_forbidden_operations(resolution, helper_file, helper, kinds, visited, depth, None)
}

/// A member call that is not an exact instance's method (ADR 0210).
///
/// The compiler's entity at a member callee's complete span names the
/// receiver's root binding, not the member: `items().forEach` answers `items`
/// and `register.bind` answers `register`. Read as the callee, that made an
/// accessor's array method a safe accessor call and `register.bind(null)` a
/// call of `register`. Only the resolved call names the member, and only a
/// standard-library member is followed: its arguments by
/// [`standard_library_argument_operations`], and the receiver itself when the
/// member is `call` or `apply`, which run it before they return.
fn member_call_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    callee: Span,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
) -> bool {
    let Some((resolved, declaration)) = resolution
        .lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| Some((resolved, resolved.declaration.as_ref()?)))
        .filter(|(_, declaration)| declaration.standard_library)
    else {
        return false;
    };
    let mut complete = standard_library_argument_operations(
        resolution, file, call, resolved, kinds, visited, depth,
    );
    let invokes_receiver = matches!(
        declaration.qualified_name.as_ref(),
        "Function.call"
            | "Function.apply"
            | "CallableFunction.call"
            | "CallableFunction.apply"
            | "NewableFunction.call"
            | "NewableFunction.apply"
    );
    if invokes_receiver {
        complete &= file
            .ast
            .members
            .iter()
            .find(|member| member.span == callee)
            .is_some_and(|member| {
                argument_body_operations(resolution, file, member.object, kinds, visited, depth)
            });
    }
    complete
}

/// Whether the function arguments a standard-library call may run in the
/// leaf scope are proven free of forbidden operations (ADR 0210).
///
/// The host runs no Solid code of its own, but it runs the functions it is
/// handed, and the audited timing table
/// ([`crate::runtime_semantics::argument_behavior`]) says when:
///
/// - an inline callback (`list.forEach(register)`) runs before the call
///   returns, so its body is walked like a helper's and a forbidden operation
///   there is a violation of this call;
/// - a fresh-stack callback (`setTimeout`, `queueMicrotask`) runs from a host
///   queue, after the leaf scope is gone;
/// - a deferred callback (`addEventListener`'s listener, `bind`'s bound
///   arguments) runs after the call returns. A synchronous dispatch in the
///   leaf scope (`el.click()`, `el.focus()`) would run a listener there, but
///   dispatch is not modeled: the same `focus()` runs listeners registered
///   anywhere, and the walk has always read it as a host call that runs only
///   what it is handed;
/// - a `PromiseLike.then` callback may run before the call returns, because
///   the thenable is any object with a `then`. A forbidden operation there,
///   or a body the walk cannot follow, leaves the obligation open rather than
///   proving a violation;
/// - a value the host only reads or keeps is not run;
/// - any other argument the host may call -- a callable parameter with no
///   audited timing (`new Promise(executor)`) -- leaves the obligation open.
///
/// A parameter typed `any` or `unknown` (`console.log(...data)`) is read as a
/// value. Implicit invocation through getters, `toString`, `valueOf`, an
/// iterator or a thenable is not modeled, as it is not for the rest of the
/// standard-library trust here.
fn standard_library_argument_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    resolved: &typefacts::ResolvedCall,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
) -> bool {
    use crate::runtime_semantics::RuntimeArgumentBehavior;
    use typefacts::Callability;
    let lookup = resolution.lookup;
    // A `PromiseLike` may be any object with a `then`: its implementation is
    // not the host's and may call back before it returns.
    let thenable = resolved
        .declaration
        .as_ref()
        .is_some_and(|declaration| declaration.qualified_name.as_ref() == "PromiseLike.then");
    let mut complete = true;
    for (index, argument) in call.arguments.iter().enumerate() {
        if crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind) {
            continue;
        }
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if callability == Some(Callability::NonCallable) {
            continue;
        }
        if argument.spread {
            // A spread hands over values at indices the mapping does not name.
            complete = false;
            continue;
        }
        // Every default-library listener slot, whichever DOM interface
        // redeclares `addEventListener`, is a deferred callback.
        let listener_slot =
            crate::runtime_semantics::runs_on_invoker_stack(resolved, callability, index);
        match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
            Some(RuntimeArgumentBehavior::DeferredCallback) if thenable => {
                let mut possible = Vec::new();
                let mut scratch = visited.clone();
                complete &= argument_body_operations(
                    resolution,
                    file,
                    argument.span,
                    &mut possible,
                    &mut scratch,
                    depth,
                ) && possible.is_empty();
            }
            Some(RuntimeArgumentBehavior::InlineCallback) => {
                complete &= argument_body_operations(
                    resolution,
                    file,
                    argument.span,
                    kinds,
                    visited,
                    depth,
                );
            }
            Some(
                RuntimeArgumentBehavior::DeferredCallback
                | RuntimeArgumentBehavior::FreshStackCallback
                | RuntimeArgumentBehavior::RetainedValue
                | RuntimeArgumentBehavior::ValueOnly,
            ) => {}
            None if listener_slot => {}
            None => {
                let parameter_callable = crate::runtime_semantics::resolved_parameter(
                    resolved, index,
                )
                .is_none_or(|parameter| {
                    !matches!(
                        parameter.callability,
                        Callability::NonCallable | Callability::Unknown
                    )
                });
                complete &= !parameter_callable;
            }
        }
    }
    complete
}

/// A setter's function argument is its updater, which the setter runs before
/// it returns (ADR 0210). Any other argument is the new value.
fn setter_updater_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
) -> bool {
    let mut complete = true;
    for argument in &call.arguments {
        if crate::runtime_semantics::literal_argument_is_not_callable(argument.runtime_value_kind) {
            continue;
        }
        let callability = resolution
            .lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        if callability == Some(typefacts::Callability::NonCallable) {
            continue;
        }
        complete &=
            argument_body_operations(resolution, file, argument.span, kinds, visited, depth);
    }
    complete
}

/// The forbidden operations of the function an argument evaluates to, when
/// that function is exactly known: a function literal written as the
/// argument, or an identifier bound to a project function. Anything else --
/// a call's result, a member, a parameter -- is not followed.
fn argument_body_operations(
    resolution: LeafScopeResolution<'_, '_>,
    file: &FileFacts,
    argument: Span,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
) -> bool {
    if let Some(literal) = callback_argument_literal(file, argument) {
        return function_forbidden_operations(
            resolution,
            file,
            literal,
            kinds,
            visited,
            depth - 1,
            None,
        );
    }
    let lookup = resolution.lookup;
    let Some((function_file, function)) = lookup
        .entities()
        .at(file.path.as_str(), file.ast.peel_ts_sugar_span(argument))
        .and_then(|symbol| lookup.function_for_symbol(symbol))
    else {
        return false;
    };
    function_forbidden_operations(
        resolution,
        function_file,
        function,
        kinds,
        visited,
        depth - 1,
        None,
    )
}

fn function_forbidden_operations(
    resolution: LeafScopeResolution<'_, '_>,
    helper_file: &FileFacts,
    helper: &solid_facts::ast::FunctionFact,
    kinds: &mut Vec<crate::LeafOwnerOperationKind>,
    visited: &mut Vec<(String, Span)>,
    depth: usize,
    exact_this: ExactThis,
) -> bool {
    let LeafScopeResolution {
        lookup,
        symbol_names,
        ..
    } = resolution;
    if depth == 0 {
        return false;
    }
    let key = (helper_file.path.as_str().to_owned(), helper.span);
    if visited.contains(&key) {
        return true;
    }
    visited.push(key);
    let dialect = lookup.dialect;
    let entities = lookup.entities();
    let mut complete = true;
    for inner in helper_file.ast.calls_within(helper.body) {
        // A call inside a nested function -- its body or its parameter list
        // (ADR 0204) -- is not executed by calling the helper; it belongs to
        // whatever later invokes that function.
        if !crate::owners::written_directly_in(&helper_file.ast, helper, inner.span) {
            continue;
        }
        // After an `await` that every run of the helper reaches first, the
        // call executes from a promise continuation: the leaf scope that
        // invoked the helper is long gone and no owner exists at all. That is
        // a different claim (an ownerless operation, `missing-owner`'s), not a
        // forbidden call in the leaf scope.
        if helper_file
            .ast
            .unconditional_awaits
            .iter()
            .any(|await_span| {
                await_span.end <= inner.span.start
                    && containing_ast_function(&helper_file.ast, *await_span)
                        .is_some_and(|function| function.span == helper.span)
            })
        {
            continue;
        }
        let primitive = call_primitive_name(helper_file, inner, entities, symbol_names, dialect);
        let Some(primitive) = primitive else {
            complete &= helper_forbidden_operations(
                resolution,
                helper_file,
                inner,
                kinds,
                visited,
                depth - 1,
                exact_this.clone(),
            );
            continue;
        };
        // One kind per call site, however many helper calls or transitive
        // hops reach it: the finding names the operation, not each way of
        // arriving at it. Kinds arrive interleaved across hops, so an
        // adjacent-only `dedup` would leave byte-identical operations in the
        // serialized IR — reject at the push instead.
        if let Some(kind) = forbidden_operation_kind(dialect, helper_file, inner, &primitive)
            && !kinds.contains(&kind)
        {
            kinds.push(kind);
        }
    }
    complete
}

/// What one `return` in a cleanup-accepting callback proves about the value
/// the owner receives.
///
/// Only `ValidFunction` is load-bearing now: it is the ownership fact
/// `function_returns_cleanup` (SC4001) asks for — "does this hand the
/// owner an actual cleanup function". The other three outcomes are the ways
/// that proof can fail, and they are kept apart because they fail for
/// materially different reasons; collapsing them would hide that
/// `return nothing` where `nothing: undefined` is *legal* and merely hands the
/// owner nothing, which is not the same as a value the owner cannot use.
///
/// None is a cleanup-legality finding. Legality is TypeScript's:
/// `EffectFunction` returns `(() => void) | void`, so an unusable value is a
/// type error and reporting it again would duplicate `tsc`. The owner pass may
/// still turn `Unresolved` into an uncertifiable SC4001 when a possibly
/// returned cleanup would need an owner.
enum CleanupReturnStatus {
    /// Proven to be a function: an owner that reads returned cleanups
    /// registers it.
    ValidFunction,
    /// Proven legal and unable to contain a function — `undefined` or `void`.
    ValidNonFunction,
    /// Proven legal and may be either a cleanup function or `undefined`.
    /// Legality is settled, but owner registration is conditional.
    OptionalFunction,
    /// Proven to be a value an owner cannot use as cleanup — which is exactly
    /// the domain `tsc` rejects, so it only means "no cleanup here".
    Invalid,
    /// Neither proven; no cleanup may be assumed.
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CleanupReturnProof {
    Function,
    OptionalFunction,
    NoFunction,
    Unresolved,
}

fn terminal_cleanup_fix(
    file: &solid_facts::FileFacts,
    owner_region: Span,
    call: &solid_facts::ast::CallFact,
) -> Option<Fix> {
    let callback = file
        .ast
        .functions
        .iter()
        .filter(|function| owner_region.contains(function.span))
        .max_by_key(|function| function.span.end - function.span.start)?;
    let body_end = usize::try_from(callback.body.end).ok()?.checked_sub(1)?;
    let call_end = usize::try_from(call.span.end).ok()?;
    if call_end > body_end || body_end > file.source.len() {
        return None;
    }
    if !file.source.as_bytes()[call_end..body_end]
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == b';')
    {
        return None;
    }
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    let start = usize::try_from(argument.span.start).ok()?;
    let end = usize::try_from(argument.span.end).ok()?;
    let argument = file.source.get(start..end)?.trim();
    if argument.is_empty() {
        return None;
    }
    Some(Fix {
        message: "Return the cleanup function instead of calling onCleanup".into(),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), call.span),
            new_text: format!("return {argument}"),
        }],
    })
}

fn cleanup_return_status(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    returned: &solid_facts::ast::ReturnFact,
) -> CleanupReturnStatus {
    let entities = lookup.entities();
    match returned.value {
        solid_facts::ast::ReturnValueKind::Undefined => CleanupReturnStatus::ValidNonFunction,
        solid_facts::ast::ReturnValueKind::Function => CleanupReturnStatus::ValidFunction,
        solid_facts::ast::ReturnValueKind::Member => {
            // Computed dispatch is not an exact property proof: the key may
            // select any member at runtime. Static member expressions have a
            // complete-expression value-domain fact at this exact return
            // span, so classify those from the same evidence as identifiers.
            if file
                .ast
                .computed_members
                .binary_search(&returned.span)
                .is_ok()
            {
                CleanupReturnStatus::Unresolved
            } else {
                domain_cleanup_return_status(
                    lookup
                        .entity_at(file.path.as_str(), returned.span)
                        .and_then(|entity| entity.runtime_value_domain.as_ref()),
                )
            }
        }
        solid_facts::ast::ReturnValueKind::Other => domain_cleanup_return_status(
            lookup
                .entity_at(file.path.as_str(), returned.span)
                .and_then(|entity| entity.runtime_value_domain.as_ref()),
        ),
        solid_facts::ast::ReturnValueKind::Call => {
            let Some(callee) = returned.callee else {
                return CleanupReturnStatus::Unresolved;
            };
            let resolved = lookup
                .entity_at(file.path.as_str(), callee)
                .and_then(|entity| entity.resolved_call.as_ref())
                .is_some_and(|call| call.validity == ResolvedCallValidity::Valid);
            if !resolved {
                return CleanupReturnStatus::Unresolved;
            }
            // The *result* of the call, never its callee: `callResultDomain` is
            // matched by the producer against a call-like node occupying exactly
            // the demanded span, so `makeCount()` where `makeCount(): number`
            // classifies as the number it produces rather than the callable
            // `makeCount`. An absent field (no exact call-like node) and an
            // `unknown` domain (a checker error or recovery type) both stay
            // fail-closed in `domain_cleanup_return_status`.
            domain_cleanup_return_status(returned_call_domain(lookup, file, callee))
        }
        solid_facts::ast::ReturnValueKind::Identifier => {
            let Some(symbol) = entities.get(&location(file.path.shared(), returned.span)) else {
                return CleanupReturnStatus::Unresolved;
            };
            if identifier_return_is_exact_function(lookup, file, symbol) {
                CleanupReturnStatus::ValidFunction
            } else {
                domain_cleanup_return_status(
                    lookup
                        .entity_at(file.path.as_str(), returned.span)
                        .and_then(|entity| entity.runtime_value_domain.as_ref()),
                )
            }
        }
    }
}

fn identifier_return_is_exact_function(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    symbol: &SymbolId,
) -> bool {
    let entities = lookup.entities();
    file.ast.functions.iter().any(|function| {
        function.name.as_ref().is_some_and(|name| {
            entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
        })
    }) || file.ast.bindings.iter().any(|binding| {
        binding.initializer_function
            && binding
                .names
                .iter()
                .any(|name| entities.get(&location(file.path.shared(), name.span)) == Some(symbol))
    })
}

/// Classifies a cleanup return from the compiler's runtime value domain.
///
/// The producer derives this from checker types, flags, constraints,
/// assignability, union constituents, and call signatures — never from
/// rendered type text — so aliases, `any`, `unknown`, and recovery types
/// arrive as `unknown` and stay fail-closed here instead of being guessed from
/// spelling. A missing fact (the demand was not planned, or the compiler had
/// no answer) is likewise unresolved.
fn domain_cleanup_return_status(domain: Option<&RuntimeValueDomain>) -> CleanupReturnStatus {
    let Some(domain) = domain.filter(|domain| !domain.unknown()) else {
        return CleanupReturnStatus::Unresolved;
    };
    match (
        domain.may_be_callable(),
        domain.may_be_other(),
        domain.may_be_undefined(),
    ) {
        // Only ever a function, or only ever a function or `undefined`: legal
        // either way, but the optional form keeps owner registration open.
        (true, false, false) => CleanupReturnStatus::ValidFunction,
        (true, false, true) => CleanupReturnStatus::OptionalFunction,
        // Never a function and never voidish: the owner is handed a value it
        // cannot use, on every execution that reaches this return.
        (false, true, false) => CleanupReturnStatus::Invalid,
        // Only `undefined`/`void`.
        (false, false, true) => CleanupReturnStatus::ValidNonFunction,
        // A domain that admits both a legal and an illegal value proves
        // neither, and `never` (a known empty domain) describes a value this
        // return never produces.
        (true, true, _) | (false, true, true) => CleanupReturnStatus::Invalid,
        (false, false, false) => CleanupReturnStatus::ValidNonFunction,
    }
}

pub(super) fn function_cleanup_return_proof(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> CleanupReturnProof {
    let status = |returned: &solid_facts::ast::ReturnFact| {
        match cleanup_return_status(lookup, file, returned) {
            CleanupReturnStatus::ValidFunction => match returned.value {
                solid_facts::ast::ReturnValueKind::Function => CleanupReturnProof::Function,
                solid_facts::ast::ReturnValueKind::Identifier => lookup
                    .entities()
                    .get(&location(file.path.shared(), returned.span))
                    .filter(|symbol| identifier_return_is_exact_function(lookup, file, symbol))
                    .map_or(CleanupReturnProof::OptionalFunction, |_| {
                        CleanupReturnProof::Function
                    }),
                // A callable checker type alone does not prove the runtime
                // value is non-nullish when strictNullChecks is disabled.
                solid_facts::ast::ReturnValueKind::Call
                | solid_facts::ast::ReturnValueKind::Member
                | solid_facts::ast::ReturnValueKind::Other
                | solid_facts::ast::ReturnValueKind::Undefined => {
                    CleanupReturnProof::OptionalFunction
                }
            },
            CleanupReturnStatus::ValidNonFunction | CleanupReturnStatus::Invalid => {
                CleanupReturnProof::NoFunction
            }
            CleanupReturnStatus::OptionalFunction => CleanupReturnProof::OptionalFunction,
            CleanupReturnStatus::Unresolved => CleanupReturnProof::Unresolved,
        }
    };
    if let Some(returned) = function.expression_return.as_ref() {
        return status(returned);
    }
    let returned = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .collect::<Vec<_>>();
    if returned.is_empty() {
        return CleanupReturnProof::NoFunction;
    }
    if let [only] = returned.as_slice()
        && !only.conditional
    {
        return status(only);
    }
    let statuses = returned.into_iter().map(status).collect::<Vec<_>>();
    if statuses.contains(&CleanupReturnProof::Unresolved) {
        return CleanupReturnProof::Unresolved;
    }
    if statuses
        .iter()
        .all(|status| *status == CleanupReturnProof::NoFunction)
    {
        CleanupReturnProof::NoFunction
    } else {
        // Multiple/conditional exits do not prove that a cleanup is returned
        // on every execution, even when one arm visibly returns a function.
        CleanupReturnProof::OptionalFunction
    }
}

/// The runtime value domain of what a returned call *produces*.
///
/// Resolved at the call's own span, where the producer answers only for a
/// call-like node matching that span exactly. A callee-shaped fact can
/// therefore never be substituted, which is what made the older callability
/// probe classify `makeCount()` from `makeCount`.
fn returned_call_domain<'a>(
    lookup: &'a SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    callee: Span,
) -> Option<&'a RuntimeValueDomain> {
    let call = lookup.call_by_callee(file, callee)?;
    lookup
        .entity_at(file.path.as_str(), call.span)
        .and_then(|entity| entity.call_result_domain.as_ref())
}
