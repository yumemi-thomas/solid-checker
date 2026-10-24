//! Callback owners: the owner and execution at a callback call, returned callbacks, and their invocation sites.

use super::*;

/// The callback execution fact for one concrete primitive call, after proving
/// any lazy returned adapter is actually consumed.
///
/// Solid 1.x `mapArray` and `indexArray` merely allocate and return a function;
/// their list and mapper arguments run only when that returned function runs.
/// The dialect owns that runtime distinction, while this helper proves the
/// use from canonical TypeScript symbols and the enclosing AST call shape.
pub(crate) fn callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    if crate::effect_execution::at_call(file, call, primitive, argument, lookup)
        != crate::effect_execution::ApplyExecution::Proven
    {
        return None;
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    semantics.execution
}

/// The callback-owner fact for one concrete primitive call, after proving
/// that callbacks implemented by a returned lazy adapter can run.
///
/// Execution, reachability, cleanup, and both owner-graph passes must ask the
/// same call-site question. Otherwise a discarded adapter can disappear from
/// read analysis while still manufacturing an impossible owner diagnostic.
pub(crate) fn callback_owner_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    use crate::effect_execution::ApplyExecution;
    match crate::effect_execution::at_call(file, call, primitive, argument, lookup) {
        ApplyExecution::Absent => return None,
        ApplyExecution::Unknown => return Some(solid_dialect::CallbackOwner::Conditional),
        ApplyExecution::Proven => {}
    }
    if lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
        .is_some_and(|options| {
            crate::call_option_literal(file, call, options, "defer")
                == solid_dialect::OptionLiteral::True
        })
    {
        // A deferred apply has no inherited first run, including render effects.
        return Some(solid_dialect::CallbackOwner::None);
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    if primitive == Primitive::RunWithOwner && argument == 1 {
        return run_with_owner_callback_owner(file, call, lookup);
    }
    semantics.owner
}

/// `runWithOwner` is the one ownership primitive whose owner is data. Both
/// dialects accept `Owner | null`; 2.0 makes the null spelling the documented
/// way to detach a root. Preserve a definite answer where the call site proves
/// one and keep a conditional edge everywhere else so certification cannot
/// silently assume a nullable owner exists.
pub(crate) fn run_with_owner_callback_owner(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    let owner = call.arguments.first()?;
    if owner.value == solid_facts::ast::ArgumentValueKind::Undefined
        || owner.value == solid_facts::ast::ArgumentValueKind::Null
    {
        return Some(solid_dialect::CallbackOwner::None);
    }
    // `void expression` has no dedicated AST argument kind yet. This text
    // check is conservative (it can only detach an owner) and remains local
    // to the one syntax shape not represented by the fact protocol.
    let source = file.source_text(owner.span)?.trim();
    if source.starts_with("void ") {
        return Some(solid_dialect::CallbackOwner::None);
    }

    // 2.0's createOwner() is non-null by construction. Resolve the nested
    // call semantically so a local function with the same spelling proves
    // nothing.
    if lookup.primitive_at_call(file, owner.span) == Some(Primitive::CreateOwner) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }
    if component_owner_binding(file, owner.span, lookup) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }

    if let Some(descriptor) = lookup
        .entity_at(file.path.as_str(), owner.span)
        .and_then(|entity| entity.type_descriptor.as_deref())
    {
        // TypeScript may preserve a user alias's name rather than render its
        // nullable expansion, so absence of the word `null` is not proof.
        // The compiler-resolved alias declaration and origin module must be
        // the dialect's own Owner export; a user-local type with the same
        // spelling has no such role. Nullable/union types remain Creates here
        // only when the selected type itself is the Solid Owner branch, and
        // the existing call-site analysis keeps unresolved values conditional.
        if descriptor.alias_declarations.iter().any(|declaration| {
            lookup
                .dialect
                .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                == Some(solid_dialect::TypeRole::Owner)
        }) {
            return Some(solid_dialect::CallbackOwner::Creates);
        }
    }

    Some(solid_dialect::CallbackOwner::Conditional)
}

/// ADR 0206: whether `value` names a `const` bound to `getOwner()` written
/// directly in a proven component's body.
///
/// A proven component's body runs under an owner: the owner graph seeds it
/// owned (`owner_node`), as its render does. `getOwner()` called directly
/// there, not in a nested function or a default, therefore returns that
/// owner, never `null`, and a `const` keeps it. The owner may have been
/// disposed by the time a later callback hands it to `runWithOwner`; that is
/// not the null owner this question is about.
pub(super) fn component_owner_binding(
    file: &solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), value)
    else {
        return false;
    };
    let Some(initializer) = binding.call_initializer else {
        return false;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || lookup.primitive_at_call(binding_file, initializer) != Some(Primitive::GetOwner)
    {
        return false;
    }
    let Some(component) = containing_ast_function(&binding_file.ast, initializer) else {
        return false;
    };
    !component.r#async
        && !component.generator
        && written_directly_in(&binding_file.ast, component, initializer)
        && lookup.function_component_status(binding_file, component)
            == crate::indexes::ComponentStatus::Proven
}

/// Resolve the primitive whose returned function is the callee of `call`.
///
/// Canonical TypeScript symbols prove identity, AST binding facts prove the
/// initializer/alias chain, and the dialect's resolved primitive index proves
/// that the initializer is the actual Solid export rather than a same-spelled
/// local function. The walk is a memo per file
/// ([`crate::indexes::SemanticLookup::returned_primitive_at`], ADR 0392).
pub(crate) fn returned_primitive_invocation(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Primitive, Option<usize>)> {
    lookup.returned_primitive_at(file, call)
}

/// The execution semantics of one callback argument accepted by a proven
/// returned function (including a specific destructured tuple slot).
pub(crate) fn returned_callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    // Solid 2.0 answers `None` for every returned-callback question, so the
    // binding-chain resolution below cannot produce an answer there. Skip it
    // rather than walk it once per call and discard the result.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
    lookup
        .dialect
        .returned_callback_semantics_at(primitive, result_slot, argument, call.arguments.len())
        .execution
}

pub(crate) fn returned_callback_result_is_invoked(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !returned_callback_invocation_sites_memo(file, factory_call, lookup).is_empty()
}

/// Proven project invocations of the function returned by `factory_call`.
///
/// TypeScript symbols establish that calls, JSX tags, and `.preload()` uses
/// refer to the exact factory result. AST facts establish how the value is
/// consumed. For values passed to another known callback-taking primitive,
/// the invocation inherits that callback's owner; direct calls and
/// JSX uses inherit their containing function's owner.
pub(crate) fn returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    returned_callback_invocation_sites_memo(file, factory_call, lookup)
        .as_ref()
        .clone()
}

/// [`returned_callback_invocation_sites`], assembled once per factory call and
/// build: the owner of an inherited callback is not cheap, and execution,
/// owner and standing passes each ask about the same call.
fn returned_callback_invocation_sites_memo(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> std::sync::Arc<Vec<ReturnedCallbackInvocationSite>> {
    lookup.returned_invocation_sites_memo(file.path.as_str(), factory_call.span, || {
        assemble_returned_callback_invocation_sites(file, factory_call, lookup)
    })
}

/// The uses ([`SemanticLookup::returned_uses`]) with what each inherits.
fn assemble_returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    use crate::indexes::UseKind;
    let uses = lookup.returned_uses(file.path.as_str(), factory_call.span);
    let mut sites = Vec::with_capacity(uses.len());
    for used in uses.iter() {
        let Some(use_file) = lookup.returned_use_file(used) else {
            continue;
        };
        let UseKind::Argument(index) = used.kind else {
            sites.push(ReturnedCallbackInvocationSite {
                path: use_file.path.to_string(),
                span: used.span,
                inherited_execution: None,
                inherited_owner: OwnerEdgeKind::Preserve,
            });
            continue;
        };
        // Handed to another callback-taking primitive: the use inherits that
        // callback's execution and owner.
        let index = index as usize;
        let Some(outer) = use_file.ast.call_at(used.span) else {
            continue;
        };
        let Some(primitive) = lookup.primitive_at_call(use_file, outer.span) else {
            continue;
        };
        let execution = lookup
            .dialect
            .callback_semantics_at(primitive, index, outer.arguments.len())
            .execution;
        if execution.is_none() {
            continue;
        }
        let inherited_owner = callback_owner_at_call(use_file, outer, primitive, index, lookup)
            .map(|owner| callback_owner_edge_kind_at(lookup, primitive, owner))
            .unwrap_or(OwnerEdgeKind::Preserve);
        sites.push(ReturnedCallbackInvocationSite {
            path: use_file.path.to_string(),
            span: outer.span,
            inherited_execution: execution,
            inherited_owner,
        });
    }
    sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    sites.dedup();
    sites
}
