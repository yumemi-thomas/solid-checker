//! ADR 0456: a disposal callback the code can also call through the handle its
//! registration returns.
//!
//! rc.17's `onCleanup(fn)` returns `fn`
//! ([`solid_dialect::Dialect::disposal_callback_returned`]), so `const h =
//! onCleanup(fn); h();` runs `fn` at `h()`, on that call's stack and under its
//! owner, with no disposal around it. A write the callback runs is judged
//! there too, not only in the disposal.

use super::*;

/// Who runs a disposal callback besides the disposal itself
/// ([`cleanup_handle`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CleanupHandle {
    /// No code: the call's result is discarded, the call returns no callback,
    /// or the result is bound to a `const` nothing reads.
    DisposalOnly,
    /// Each of these calls of the handle (a `const` binding's calls, or the
    /// call written on the result, `onCleanup(fn)()`) runs the callback where
    /// it is written.
    Called(Vec<Span>),
    /// Code this does not read may call it: the result is stored, passed,
    /// returned, reassigned or exported, or the release does not say what it
    /// returns.
    Unplaced,
}

/// The handle census of the disposal callback at `index` of `call`. A census
/// of one file-local `const` binding, by its declaration: every reference must
/// be the callee of a call in this file, the binding must not be exported, and
/// no module may call direct `eval`.
pub(super) fn cleanup_handle(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> CleanupHandle {
    use solid_dialect::DisposalCallbackReturn as Return;
    if call.result_discarded {
        return CleanupHandle::DisposalOnly;
    }
    match lookup.dialect.disposal_callback_returned(primitive, index) {
        Return::NotReturned => return CleanupHandle::DisposalOnly,
        Return::Unknown => return CleanupHandle::Unplaced,
        Return::Returned => {}
    }
    let ast = &file.ast;
    let call_of = |callee: Span| {
        ast.calls
            .iter()
            .find(|outer| !outer.construct && ast.peel_ts_sugar_span(outer.callee) == callee)
    };
    if let Some(outer) = call_of(call.span) {
        return CleanupHandle::Called(vec![outer.span]);
    }
    let Some(binding) = ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .call_initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == call.span)
    }) else {
        return CleanupHandle::Unplaced;
    };
    let [name] = binding.names.as_slice() else {
        return CleanupHandle::Unplaced;
    };
    let declaration = name.span;
    if flow_binding_exported(file, declaration) || lookup.program_has_eval() {
        return CleanupHandle::Unplaced;
    }
    let Some(symbol) = lookup.entities().at(file.path.as_str(), declaration) else {
        return CleanupHandle::Unplaced;
    };
    if lookup
        .symbol_references(symbol.as_str())
        .iter()
        .any(|reference| reference.path.as_ref() != file.path.as_str())
    {
        return CleanupHandle::Unplaced;
    }
    let mut sites = Vec::new();
    for reference in ast.identifiers.iter().filter(|identifier| {
        identifier.role == solid_facts::ast::IdentifierRole::Reference
            && ast.reference_declaration(identifier.span) == Some(declaration)
    }) {
        match call_of(reference.span) {
            Some(site) => sites.push(site.span),
            None => return CleanupHandle::Unplaced,
        }
    }
    if sites.is_empty() {
        CleanupHandle::DisposalOnly
    } else {
        CleanupHandle::Called(sites)
    }
}

/// Where a climb from a site to the disposal callback that runs it ends
/// ([`disposal_handle_climb`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum HandleClimb {
    /// The site runs on the disposal callback's stack, through layers that run
    /// in place (an IIFE, an inline standard-library callback, a callback that
    /// keeps its call's owner and runs during the call), and the callback's
    /// call has this handle.
    Reached(CleanupHandle),
    /// The site is in a function the callback stores, names or hands on, whose
    /// calls this climb does not follow, or under an owner a call in the
    /// callback gives it, and the callback's handle is not
    /// [`CleanupHandle::DisposalOnly`]: its lexical role is no proof.
    Unrun,
    /// The site, or a call on the way to the callback, follows an await (or a
    /// generator step) that may suspend the run before it: it runs on a later
    /// task with no owner, whoever calls the callback, so no call of the
    /// handle places it, and the callback's handle is not
    /// [`CleanupHandle::DisposalOnly`]. The same questions the write walk and
    /// [`runs_in_disposal`] ask (`may_follow_suspension`).
    Suspended,
}

/// How a function is registered as a disposal callback by name
/// ([`named_disposal_registration`]).
enum NamedRegistration<'f> {
    /// No reference of the function's name is handed to a disposal callback
    /// position.
    None,
    /// Exactly one reference is, as the whole argument, of a binding that
    /// always holds this function (a `function` declaration nothing writes, a
    /// `const` initialized with it).
    One(&'f solid_facts::ast::CallFact, usize, Primitive),
    /// A reference is handed there but the binding is not exact (a `let`, a
    /// written declaration), the argument is more than the name (`a ? fn : g`),
    /// or several references register it: which calls the handle runs is not
    /// read.
    Unplaced,
}

/// The disposal registration of `function` by the name it is bound to
/// (`const fn = () => …; const h = onCleanup(fn)`), found by exact
/// resolution of each reference in the file. A function nothing names, or
/// whose name no disposal callback position receives, is [`NamedRegistration::None`].
fn named_disposal_registration<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> NamedRegistration<'f> {
    let ast = &file.ast;
    let declaration = if function.kind == solid_facts::ast::FunctionKind::Declaration {
        function.name.as_ref().map(|name| name.span)
    } else {
        ast.bindings
            .iter()
            .find(|binding| {
                binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.initializer.is_some_and(|initializer| {
                        ast.peel_ts_sugar_span(initializer) == function.span
                    })
            })
            .and_then(|binding| binding.names.first().map(|name| name.span))
    };
    let Some(declaration) = declaration else {
        return NamedRegistration::None;
    };
    let mut found = None;
    let mut unplaced = false;
    for reference in ast.identifiers.iter().filter(|identifier| {
        identifier.role == solid_facts::ast::IdentifierRole::Reference
            && ast.reference_declaration(identifier.span) == Some(declaration)
    }) {
        for (call, index) in ast.arguments_containing(reference.span) {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            if !lookup
                .dialect
                .callback_runs_only_in_disposal(primitive, index)
            {
                continue;
            }
            let argument = ast.peel_ts_sugar_span(call.arguments[index].span);
            if argument == reference.span {
                if found.replace((call, index, primitive)).is_some() {
                    unplaced = true;
                }
            } else if !ast
                .functions
                .iter()
                .any(|inner| argument.contains(inner.span) && inner.span.contains(reference.span))
            {
                unplaced = true;
            }
        }
    }
    match found {
        None if !unplaced => NamedRegistration::None,
        Some((call, index, primitive))
            if !unplaced && lookup.function_value_is_current(file, function) =>
        {
            NamedRegistration::One(call, index, primitive)
        }
        _ => NamedRegistration::Unplaced,
    }
}

/// The disposal callback around `span` whose handle may run it outside the
/// disposal. `None` when the climb meets no disposal callback, a fresh stack
/// (`setTimeout`), a callback the engine runs on its own schedule, an await
/// that suspends the run, or a component: the role those give the site does not
/// depend on who calls the callback.
pub(super) fn disposal_handle_climb(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<HandleClimb> {
    let mut at = span;
    let mut unrun = false;
    let mut suspended = false;
    // Bounded: each step moves to the call or function that holds the
    // previous one.
    for _ in 0..16 {
        let function = crate::owners::owning_function(&file.ast, at)?;
        if !unrun {
            // Past an await the run is on a later task, with no owner, whoever
            // called the callback. One that may be skipped places nothing.
            if follows_dominating_await(file, at) || follows_try_head_await(file, at, lookup) {
                return None;
            }
            if may_follow_suspension(file, at) || function.generator {
                suspended = true;
            }
        }
        if lookup.function_component_status(file, function) != ComponentStatus::No {
            return None;
        }
        let primitive_call = file
            .ast
            .arguments_containing(function.span)
            .filter(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .find_map(|(call, index)| {
                lookup
                    .primitive_at_call(file, call.span)
                    .map(|primitive| (call, index, primitive))
            });
        // A function registered by the name it is bound to runs in the
        // disposal as the inline callback does, and at its handle's calls.
        let primitive_call = match primitive_call {
            Some(found) => Some(found),
            None => match named_disposal_registration(file, function, lookup) {
                NamedRegistration::None => None,
                NamedRegistration::One(call, index, primitive) => Some((call, index, primitive)),
                NamedRegistration::Unplaced => {
                    return Some(if unrun {
                        HandleClimb::Unrun
                    } else {
                        HandleClimb::Reached(CleanupHandle::Unplaced)
                    });
                }
            },
        };
        if let Some((call, index, primitive)) = primitive_call {
            if lookup
                .dialect
                .callback_runs_only_in_disposal(primitive, index)
            {
                let handle = cleanup_handle(file, call, primitive, index, lookup);
                return match handle {
                    CleanupHandle::DisposalOnly => None,
                    _ if unrun => Some(HandleClimb::Unrun),
                    _ if suspended => Some(HandleClimb::Suspended),
                    handle => Some(HandleClimb::Reached(handle)),
                };
            }
            if callback_execution_at_call(file, call, primitive, index, lookup)
                != Some(Execution::Inline)
            {
                return None;
            }
            // A callback that runs during its call but under an owner the
            // call gives it (`runWithOwner`, `createRoot`) is still on the
            // cleanup's stack, but the role at a handle call is no longer the
            // write's. A write directly in it, or in layers it runs in place,
            // is the owner question's (`runs_in_disposal`), never this climb's.
            if !lookup
                .dialect
                .callback_preserves_owner_write_context(primitive)
            {
                unrun = true;
            }
            at = call.span;
            continue;
        }
        match layer_run(file, function, lookup) {
            LayerRun::InPlace(call) => at = call,
            LayerRun::FreshStack => return None,
            // A helper that also hands the callback on or keeps it runs it in
            // place once among other runs this climb does not follow (ADR
            // 0457): the role at a handle call is no proof for the write, so
            // it is a layer the callback does not run in place.
            LayerRun::Elsewhere | LayerRun::InPlaceAmongOthers(_) => {
                unrun = true;
                at = function.span;
            }
        }
    }
    None
}

/// Whether a handle call at `site` is unplaced: it sits where the backend
/// withdraws a violation to an obligation, whatever rule claims it
/// (`solid-facts-backend` `diagnostics.rs`): a file the compiler refused, an
/// instance field initializer ([`inside_instance_field`]), a function that may
/// be a component without being proven one ([`SemanticLookup::inside_possible_component`])
/// or that is analyzed as one without its identity being proven
/// ([`SemanticLookup::site_in_undecided_component`]). The finding is reported
/// at the write, inside the callback, so the backend never sees this site; a
/// rejecting role here must not become a proof there.
pub(super) fn handle_site_unplaced(
    file: &solid_facts::FileFacts,
    site: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    file.compiler.refusal().is_some()
        || inside_instance_field(file, site)
        || lookup.inside_possible_component(file, site)
        || lookup.site_in_undecided_component(file, site)
}

/// The marker under which [`cleanup_handle_write_role`] is deriving the role
/// of a handle's calls: a call of the handle inside its own callback is then
/// a re-entry this cannot place.
fn handle_marker(file: &solid_facts::FileFacts, sites: &[Span]) -> (String, Span) {
    (
        format!("\0cleanup-handle:{}", file.path.as_str()),
        sites.first().copied().unwrap_or(Span::new(0, 0)),
    )
}

/// What a disposal callback's handle makes of a write's role
/// ([`cleanup_handle_write_role`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HandleRole {
    /// Final: a handle call rejects the write (the violation), or something
    /// leaves it unplaced, whatever the function's other entries say.
    Decided(ExecutionRole),
    /// The write is in a layer the callback does not run in place: the lexical
    /// role is no proof, and the layer's own entries decide.
    Unrun,
}

/// The write role of `span` when a disposal callback around it can also run
/// through its handle ([`disposal_handle_climb`]), or `None` when the lexical
/// role stands: no such callback, or every handle call is legal, so the
/// disposal run decides as before. A handle call whose own role rejects the
/// write proves the violation (rc.17 runs the callback there, outside the
/// disposal, under the call's owner); an unplaced call or a census that is not
/// complete leaves the write unplaced.
#[allow(clippy::too_many_arguments)]
pub(super) fn cleanup_handle_write_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    visiting: &mut HashSet<(String, Span)>,
) -> Option<HandleRole> {
    let sites = match disposal_handle_climb(file, span, lookup)? {
        HandleClimb::Reached(CleanupHandle::Called(sites)) => sites,
        HandleClimb::Reached(CleanupHandle::DisposalOnly) => return None,
        HandleClimb::Reached(CleanupHandle::Unplaced) => {
            return Some(HandleRole::Decided(ExecutionRole::Unknown));
        }
        HandleClimb::Unrun => return Some(HandleRole::Unrun),
        HandleClimb::Suspended => return Some(HandleRole::Decided(ExecutionRole::Unknown)),
    };
    let marker = handle_marker(file, &sites);
    if !visiting.insert(marker.clone()) {
        return Some(HandleRole::Decided(ExecutionRole::Unknown));
    }
    let mut unplaced = false;
    let mut rejected = None;
    for site in sites {
        let role = semantic_write_execution_role_within(
            file,
            site,
            allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
            visiting,
        );
        if role.reports_disallowed_write() && handle_site_unplaced(file, site, lookup) {
            unplaced = true;
        } else if role.reports_disallowed_write() {
            rejected = Some(role);
            break;
        } else if role == ExecutionRole::Unknown {
            unplaced = true;
        }
    }
    visiting.remove(&marker);
    match rejected {
        Some(role) => Some(HandleRole::Decided(role)),
        None if unplaced => Some(HandleRole::Decided(ExecutionRole::Unknown)),
        None => None,
    }
}
