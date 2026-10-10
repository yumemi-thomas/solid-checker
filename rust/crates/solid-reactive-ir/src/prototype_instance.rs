//! Exact fresh-constructor instances, with no receiver Type Facts dependency.
//! All runtime work here is the authored getObserver-gated TriggerCache recipe.

use super::LocalAccessContext;
use crate::cache::LocalAccessResult;
use crate::contract_semantics::{
    PrototypeInstanceRecipe, PrototypeMember, PrototypeMemberKind, PrototypePopulation,
};
use crate::execution_role::{
    ObserverPresence, allowed_callback_spans, direct_callback_contains, observer_presence_at,
    read_analysis_context, semantic_execution_role,
};
use crate::{ExecutionRole, LeafOwnerOperation, LeafOwnerOperationKind, ReactiveRead};
use solid_facts::ast::{BindingShape, IdentifierRole};
use solid_facts::core::Span;
use std::sync::Arc;

fn obligation(
    file: &solid_facts::FileFacts,
    span: Span,
    export: &str,
    reason: &str,
) -> crate::StaticDefect {
    crate::StaticDefect {
        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
            callee: export.to_owned(),
            member: None,
        },
        location: crate::location(file.path.shared(), span),
        analysis_context: reason.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

fn member_at<'a>(
    file: &solid_facts::FileFacts,
    span: Span,
    members: &'a [PrototypeMember],
) -> Option<&'a PrototypeMember> {
    let member = file.ast.members.iter().find(|member| member.span == span)?;
    if file.ast.computed_members.contains(&span) || file.ast.optional_members.contains(&span) {
        return None;
    }
    let key = file.source_text(member.property)?;
    members.iter().find(|member| member.name == key)
}

/// No aliases, wrappers, instance writes, extracted methods or unknown calls.
/// Native iteration is recognized by its normalized operand fact, never text.
fn constructor_input_is_static(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    population: PrototypePopulation,
    facts: &solid_facts::ProjectFacts,
) -> bool {
    use solid_facts::ast::{ArgumentLiteralFact, ArgumentValueKind};
    if call.arguments.is_empty() {
        return true;
    }
    if population == PrototypePopulation::Opaque || call.arguments.len() != 1 {
        return false;
    }
    let argument = &call.arguments[0];
    if argument.spread {
        return false;
    }
    if argument.value == ArgumentValueKind::Null {
        return true;
    }
    // Literal arrays use native iteration. Any project access through a
    // prototype may replace that protocol, so withhold this input proof.
    if facts.files.iter().any(|file| {
        !file.ast.computed_members.is_empty()
            || file.ast.members.iter().any(|member| {
                matches!(
                    file.source_text(member.property),
                    Some(
                        "prototype"
                            | "getPrototypeOf"
                            | "setPrototypeOf"
                            | "defineProperty"
                            | "defineProperties"
                    )
                )
            })
    }) {
        return false;
    }
    let ArgumentLiteralFact::ArrayLength(length) = &argument.literal_value else {
        return false;
    };
    if !file.ast.array_literals.contains(&argument.span)
        || usize::try_from(*length).ok() != Some(argument.literal_members.len())
        || file
            .ast
            .spreads
            .iter()
            .any(|spread| argument.span.contains(spread.span))
        || file
            .ast
            .calls
            .iter()
            .any(|nested| argument.span.contains(nested.span))
        || file
            .ast
            .members
            .iter()
            .any(|member| argument.span.contains(member.span))
        || file
            .ast
            .assignments
            .iter()
            .any(|assignment| argument.span.contains(assignment.target))
        || file
            .ast
            .deleted_targets
            .iter()
            .any(|target| argument.span.contains(*target))
    {
        return false;
    }
    population == PrototypePopulation::Values
        || argument.literal_members.iter().all(|entry| {
            // Native entry-array spreading runs no caller iterator/getter. A
            // dynamic yielded tuple would require callback-result provenance.
            file.ast.array_literals.contains(&entry.value)
        })
}

fn receiver_is_stable(
    file: &solid_facts::FileFacts,
    root: Span,
    members: &[PrototypeMember],
) -> bool {
    let refers = |span| file.ast.reference_declaration(span) == Some(root);
    if crate::indexes::binding_written(file, root)
        || file
            .ast
            .exports
            .iter()
            .filter(|export| !export.type_only)
            .any(|export| {
                export
                    .declarations
                    .iter()
                    .chain(&export.specifiers)
                    .any(|export| {
                        !export.type_only
                            && (export.local.span == root || refers(export.local.span))
                    })
            })
    {
        return false;
    }
    for id in &file.ast.identifiers {
        if id.role != IdentifierRole::Reference {
            continue;
        }
        // An erased type position is not an instance escape.
        if file
            .ast
            .transparent_wrappers
            .iter()
            .any(|wrapper| wrapper.span.contains(id.span) && !wrapper.inner.contains(id.span))
        {
            continue;
        }
        if file.ast.reference_declaration(id.span).is_none()
            && file.source_text(id.span) == file.source_text(root)
        {
            return false;
        }
        if !refers(id.span) {
            continue;
        }
        if file
            .ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(file.ast.deleted_targets.iter().copied())
            .chain(file.ast.iteration_targets.iter().copied())
            .any(|target| target.contains(id.span))
        {
            return false;
        }
        if file.ast.iterated_operands.contains(&id.span)
            && members.iter().any(|member| member.name == "@@iterator")
        {
            continue;
        }
        let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.object == id.span)
        else {
            return false;
        };
        let Some(recipe) = member_at(file, member.span, members) else {
            return false;
        };
        if recipe.kind != PrototypeMemberKind::Getter
            && !file
                .ast
                .calls
                .iter()
                .any(|call| call.callee == member.span && !call.construct)
        {
            return false;
        }
    }
    true
}

impl LocalAccessContext<'_, '_> {
    /// A project use of the class value other than exact `new` could replace
    /// its prototype. Check all imports projecting this recipe conservatively;
    /// equal recipes may withhold an unrelated constructor but never trust one.
    fn prototype_constructor_is_stable(&self, recipe: &PrototypeInstanceRecipe) -> bool {
        // Namespace/default module objects and dynamic loads can expose the
        // class through a path this initial binder-only census cannot close.
        // Re-exports and opaque evaluation likewise leave the prototype open.
        if self.facts.files.iter().any(|file| {
            !file.ast.module_loads.is_empty()
                || !file.ast.module_hazards.is_empty()
                || file
                    .ast
                    .exports
                    .iter()
                    .any(|export| !export.type_only && export.module.is_some())
                || file.ast.imports.iter().any(|import| {
                    !import.type_only
                        && import.bindings.iter().any(|binding| {
                            !binding.type_only
                                && matches!(
                                    binding.kind,
                                    solid_facts::ast::ImportKind::Namespace
                                        | solid_facts::ast::ImportKind::Default
                                )
                        })
                })
        }) {
            return false;
        }
        for file in &self.facts.files {
            for import in &file.ast.imports {
                for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
                    let root = binding.local.span;
                    let related = self
                        .lookup
                        .callee_symbol(file, root)
                        .and_then(|symbol| self.contract_returns.get(symbol))
                        .is_some_and(|(returned, _)| returned.prototype.as_ref() == Some(recipe))
                        || file.ast.identifiers.iter().any(|id| {
                            id.role == IdentifierRole::Reference
                                && file.ast.reference_declaration(id.span) == Some(root)
                                && self
                                    .lookup
                                    .callee_symbol(file, id.span)
                                    .and_then(|symbol| self.contract_returns.get(symbol))
                                    .is_some_and(|(returned, _)| {
                                        returned.prototype.as_ref() == Some(recipe)
                                    })
                        });
                    if !related {
                        continue;
                    }
                    if file
                        .ast
                        .exports
                        .iter()
                        .filter(|export| !export.type_only)
                        .any(|export| {
                            export
                                .declarations
                                .iter()
                                .chain(&export.specifiers)
                                .any(|export| {
                                    !export.type_only
                                        && (export.local.span == root
                                            || file.ast.reference_declaration(export.local.span)
                                                == Some(root))
                                })
                        })
                    {
                        return false;
                    }
                    for id in file
                        .ast
                        .identifiers
                        .iter()
                        .filter(|id| id.role == IdentifierRole::Reference)
                    {
                        if file.ast.transparent_wrappers.iter().any(|wrapper| {
                            wrapper.span.contains(id.span) && !wrapper.inner.contains(id.span)
                        }) {
                            continue;
                        }
                        if file.ast.reference_declaration(id.span) == Some(root)
                            && !file.ast.calls.iter().any(|call| {
                                call.construct
                                    && file.ast.peel_ts_sugar_span(call.callee) == id.span
                            })
                        {
                            return false;
                        }
                        if file.ast.reference_declaration(id.span).is_none()
                            && file.source_text(id.span) == file.source_text(root)
                        {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub(super) fn prototype_instance_accesses(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        let allowed = allowed_callback_spans(file, self.lookup);
        for constructor in &file.ast.calls {
            let Some((returned, declaration)) = self
                .lookup
                .callee_symbol(file, constructor.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
            else {
                continue;
            };
            let Some(recipe) = &returned.prototype else {
                continue;
            };
            result.prototype_recipes_observed = true;
            let members = &recipe.members;
            if constructor.result_discarded && constructor.construct && constructor.direct_callee {
                continue;
            }
            let export = file.source_text(constructor.callee).unwrap_or_default();
            let imported = file
                .ast
                .reference_declaration(constructor.callee)
                .is_some_and(|root| {
                    file.ast
                        .imports
                        .iter()
                        .filter(|import| !import.type_only)
                        .any(|import| {
                            import
                                .bindings
                                .iter()
                                .any(|binding| !binding.type_only && binding.local.span == root)
                        })
                });
            let root = file
                .ast
                .bindings
                .iter()
                .find(|binding| {
                    binding.immutable
                        && binding.shape == BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding.initializer == Some(constructor.span)
                })
                .map(|binding| binding.names[0].span);
            let Some(root) =
                root.filter(|_| constructor.construct && constructor.direct_callee && imported)
            else {
                result.dispatch_obligations.push(obligation(file, constructor.span, export,
                    "prototype instance lacks an exact direct imported constructor and immutable binding; wrappers, aliases and inline escape remain open"));
                continue;
            };
            if !constructor_input_is_static(file, constructor, recipe.population, self.facts) {
                result.dispatch_obligations.push(obligation(file, constructor.span, export,
                    "constructor population can execute an unproven input iterator or entry spread before the instance is bound"));
                continue;
            }
            if !receiver_is_stable(file, root, members)
                || !self.prototype_constructor_is_stable(recipe)
            {
                result.dispatch_obligations.push(obligation(file, constructor.span, export,
                    "prototype receiver or constructor escapes, is mutated, or reaches an unstated member; exact instance dispatch is not established"));
                continue;
            }
            for member in &file.ast.members {
                if file
                    .ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
                    != Some(root)
                {
                    continue;
                }
                let Some(recipe) = member_at(file, member.span, members) else {
                    continue;
                };
                let invocation = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| call.callee == member.span && !call.construct);
                let site = match recipe.kind {
                    PrototypeMemberKind::Getter => Some(member.span),
                    PrototypeMemberKind::Method => invocation.map(|call| call.span),
                    PrototypeMemberKind::Iterator => invocation.and_then(|call| {
                        if call.result_discarded {
                            // Creating and discarding a generator never resumes it.
                            return None;
                        }
                        if file.ast.iterated_operands.contains(&call.span) {
                            return Some(call.span);
                        }
                        file.ast.calls.iter().find_map(|next| {
                            let next_member = file.ast.members.iter().find(|member| member.span == next.callee)?;
                            (next_member.object == call.span && !next.construct && next.arguments.is_empty()
                                && !file.ast.computed_members.contains(&next_member.span)
                                && !file.ast.optional_members.contains(&next_member.span)
                                && file.source_text(next_member.property) == Some("next"))
                                .then_some(next.span)
                        }).or_else(|| {
                            result.dispatch_obligations.push(obligation(file, call.span, export,
                                "iterator result escapes or its first synchronous resumption is not established"));
                            None
                        })
                    }),
                };
                if let Some(site) = site {
                    self.prototype_track_at(
                        file,
                        root,
                        site,
                        recipe,
                        declaration,
                        &allowed,
                        result,
                    );
                }
            }
            if let Some(recipe) = members.iter().find(|member| member.name == "@@iterator") {
                for operand in &file.ast.iterated_operands {
                    if file.ast.reference_declaration(*operand) == Some(root) {
                        self.prototype_track_at(
                            file,
                            root,
                            *operand,
                            recipe,
                            declaration,
                            &allowed,
                            result,
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn prototype_track_at(
        &self,
        file: &solid_facts::FileFacts,
        root: Span,
        site: Span,
        recipe: &PrototypeMember,
        declaration: &typefacts::Location,
        allowed: &[Span],
        result: &mut LocalAccessResult,
    ) {
        let execution = semantic_execution_role(
            file,
            site,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        if execution == ExecutionRole::DiscardedRendering {
            return;
        }
        let observer = observer_presence_at(
            file,
            site,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        // for-await/yield* may resume after suspension. The synchronous recipe
        // cannot transfer the observer or cleanup owner across that boundary.
        let suspended = file
            .ast
            .implicit_suspensions
            .iter()
            .any(|span| span.contains(site));
        if observer == ObserverPresence::Unknown || suspended {
            result.dispatch_obligations.push(obligation(file, site, &recipe.name,
                "prototype tracking needs an exact observer and synchronous first resumption; observer/cache state and helper invocation remain open"));
            return;
        }
        if observer == ObserverPresence::Absent {
            // The getObserver guard exits before even inspecting a retained
            // cache. This is stronger than ADR 0255's static-store Get recipe.
            return;
        }
        // Select only the nearest direct primitive callback: an outer leaf
        // never forbids cleanup registered under a nested memo's created owner.
        let leaf = file
            .ast
            .arguments_containing(site)
            .filter(|(call, index)| {
                direct_callback_contains(file, call.arguments[*index].span, site)
            })
            .min_by_key(|(call, index)| {
                let span = call.arguments[*index].span;
                span.end - span.start
            })
            .and_then(|(call, index)| {
                let primitive = crate::call_primitive_name(
                    file,
                    call,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                )?;
                let is_leaf = primitive.primitive().is_some_and(|primitive| {
                    crate::owners::callback_owner_at_call(file, call, primitive, index, self.lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                });
                is_leaf.then(|| primitive.to_string())
            });
        if let Some(owner) = leaf {
            result.prototype_leaf_operations.push(LeafOwnerOperation {
                kind: LeafOwnerOperationKind::ObserverCacheTracking,
                owner,
                through_contract: true,
                location: crate::location(file.path.shared(), site),
                fix: None,
                call_site_gate: None,
                uncertain: false,
                possible: false,
                via: Some(recipe.name.clone()),
            });
            // A cold cache can throw while creating the signal; a hot cache
            // throws at cleanup registration. Either path forbids this call.
            return;
        }
        for track in &recipe.tracks {
            // The backing identity includes this exact `new` binding and its
            // cache. Keys remain argument/shared selectors, never name guesses.
            let cache = format!(
                "instance:{}:{}:{}:{}:{:?}:{:?}:{:?}",
                file.path,
                root.start,
                root.end,
                track.cache,
                track.argument,
                track.shared,
                track.argument.map(|_| site.start)
            );
            result.reads.push(Arc::new(ReactiveRead {
                kind: "accessor".into(),
                accessor: cache.into(),
                location: crate::location(file.path.shared(), site),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, site, execution, self.lookup).into(),
                via: recipe.name.clone().into(),
                origin: Some(declaration.clone()),
                origin_context: recipe.name.clone().into(),
                uncertain: false,
                missing_jsx_census: false,
                host_callback_timing: false,
                callee_callback_timing: false,
                project_consumer_non_strict: false,
                callback_invocation_unproven: false,
                package_internal: false,
                summary_attributed: false,
            }));
            if crate::owners::counts_as_strict_read_root(file, site, execution, self.lookup) {
                result.strict_read_obligations += 1;
            }
        }
    }
}
