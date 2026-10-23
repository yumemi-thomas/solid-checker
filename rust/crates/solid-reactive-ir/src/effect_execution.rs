//! A split effect's apply phase needs an invocation premise (ADR 0302).

use crate::{HashMap, HashSet};

use solid_dialect::{OptionLiteral, Primitive, ReactiveRole, ResultSlot};
use solid_facts::FileFacts;
use solid_facts::ast::{BindingShape, CallFact, FunctionFact};
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplyExecution {
    Proven,
    Absent,
    Unknown,
}

fn deferred_execution(options: OptionLiteral, dependency: ApplyExecution) -> ApplyExecution {
    match options {
        OptionLiteral::Absent | OptionLiteral::False => ApplyExecution::Proven,
        OptionLiteral::True => dependency,
        // Even a constant compute can apply once if this value disables defer.
        OptionLiteral::Unknown => ApplyExecution::Unknown,
    }
}

pub(crate) fn at_call(
    file: &FileFacts,
    call: &CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> ApplyExecution {
    if lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
        .is_none()
    {
        return ApplyExecution::Proven;
    }
    lookup
        .effect_apply_calls()
        .get(&(file.path.as_str(), call.span, argument))
        .copied()
        .unwrap_or(ApplyExecution::Unknown)
}

pub(crate) type ApplyCalls<'a> = HashMap<(&'a str, Span, usize), ApplyExecution>;

pub(crate) fn calls<'a>(lookup: &SemanticLookup<'a>) -> ApplyCalls<'a> {
    let mut result = ApplyCalls::default();
    for file in lookup.files() {
        for call in &file.ast.calls {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            for argument in 0..call.arguments.len() {
                if lookup
                    .dialect
                    .effect_apply_defer_options(primitive, argument)
                    .is_some()
                {
                    result.insert(
                        (file.path.as_str(), call.span, argument),
                        prove_at_call(file, call, primitive, argument, lookup),
                    );
                }
            }
        }
    }
    result
}

fn prove_at_call(
    file: &FileFacts,
    call: &CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> ApplyExecution {
    let Some(options) = lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
    else {
        return ApplyExecution::Proven;
    };
    let option = crate::call_option_literal(file, call, options, "defer");
    let dependency = if option == OptionLiteral::True {
        call.arguments
            .iter()
            .enumerate()
            .find(|(index, _)| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, *index, call.arguments.len())
                    .tracks_reads
            })
            .map(|(_, argument)| argument)
            .map_or(ApplyExecution::Unknown, |compute| {
                compute_dependency(file, compute.span, lookup, &mut HashSet::default())
            })
    } else {
        ApplyExecution::Unknown
    };
    deferred_execution(option, dependency)
}

fn callback<'a, 'facts: 'a>(
    file: &'a FileFacts,
    value: Span,
    lookup: &SemanticLookup<'facts>,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    let value = file.ast.peel_ts_sugar_span(value);
    file.ast
        .function_at(value)
        .map(|function| (file, function))
        .or_else(|| {
            let identifier = file
                .ast
                .identifiers
                .binary_search_by_key(&value, |identifier| identifier.span)
                .ok()
                .is_some_and(|index| {
                    file.ast.identifiers[index].role == solid_facts::ast::IdentifierRole::Reference
                });
            identifier
                .then(|| {
                    lookup.function_called_at(file.path.as_str(), value).filter(
                        |(file, function)| {
                            function.method_name.is_none()
                                && lookup.function_value_is_current(file, function)
                        },
                    )
                })
                .flatten()
        })
}

/// A bounded dependency proof, independent of execution-role/owner indexes.
/// Unknown calls, property Gets, async work and conditional reads never supply
/// a positive dependency. In particular, a callable type is not a source.
fn compute_dependency(
    file: &FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span)>,
) -> ApplyExecution {
    let Some((file, function)) = callback(file, value, lookup) else {
        return ApplyExecution::Unknown;
    };
    let key = (file.path.to_string(), function.span);
    if function.r#async || function.generator || !visiting.insert(key.clone()) {
        return ApplyExecution::Unknown;
    }
    let direct = |span| crate::owners::written_directly_in(&file.ast, function, span);
    let mut unknown = function
        .parameters
        .iter()
        .any(|parameter| parameter.initializer.is_some())
        || file.ast.members.iter().any(|member| direct(member.span))
        || file.ast.spreads.iter().any(|spread| direct(spread.span))
        || file.ast.coercing_operands.iter().any(|span| direct(*span))
        || file.ast.iterated_operands.iter().any(|span| direct(*span));
    let mut proven = false;
    for call in file.ast.calls.iter().filter(|call| direct(call.span)) {
        let dependency = if mutable_accessor(file, call.callee, lookup) {
            ApplyExecution::Proven
        } else if lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| lookup.dialect.runs_callback_synchronously(primitive))
        {
            // The dialect's synchronous clearing primitives do not subscribe
            // this compute to their callback reads.
            ApplyExecution::Absent
        } else if callback(file, call.callee, lookup).is_some() && call.arguments.is_empty() {
            compute_dependency(file, call.callee, lookup, visiting)
        } else {
            ApplyExecution::Unknown
        };
        match dependency {
            ApplyExecution::Proven
                if !call.construct
                    && crate::interproc::site_on_every_completion(
                        lookup,
                        file,
                        function.span,
                        call.span,
                    ) =>
            {
                proven = true;
            }
            ApplyExecution::Absent => {}
            _ => unknown = true,
        }
    }
    visiting.remove(&key);
    if proven {
        ApplyExecution::Proven
    } else if unknown {
        ApplyExecution::Unknown
    } else {
        ApplyExecution::Absent
    }
}

/// Exact primitive result identity, with a retained setter witnessing a source
/// that can change. A derived/readonly accessor needs another proof and stays
/// unknown. Follow only immutable identifier aliases, never guessed dispatch.
fn mutable_accessor(file: &FileFacts, callee: Span, lookup: &SemanticLookup<'_>) -> bool {
    let mut path = file.path.as_str();
    let mut span = callee;
    let mut seen = HashSet::default();
    while let Some((definition, binding, symbol)) = lookup.binding_at_reference(path, span) {
        if !binding.immutable || !seen.insert(symbol.clone()) {
            return false;
        }
        if let Some(initializer) = binding.call_initializer {
            let Some(primitive) = lookup.primitive_at_call(definition, initializer) else {
                return false;
            };
            let Some(source_call) = definition
                .ast
                .calls
                .iter()
                .find(|call| call.span == initializer)
            else {
                return false;
            };
            // A custom equality function can prevent every notification. Its
            // presence is not a witness that this source can change.
            let Some(options) = lookup.dialect.options_argument(primitive) else {
                return false;
            };
            if !matches!(
                crate::call_option_literal(definition, source_call, options, "equals"),
                OptionLiteral::Absent | OptionLiteral::False
            ) {
                return false;
            }
            if binding.shape != BindingShape::Array {
                return false;
            }
            let slot = binding.array_slots.iter().position(|name| {
                name.as_ref().is_some_and(|name| {
                    lookup.entities().at(definition.path.as_str(), name.span) == Some(&symbol)
                })
            });
            return slot.is_some_and(|slot| {
                lookup
                    .dialect
                    .reactive_result_slot(primitive, ResultSlot::TupleItem(slot))
                    == Some(ReactiveRole::Accessor)
            }) && binding.array_slots.iter().enumerate().any(|(slot, name)| {
                name.is_some()
                    && lookup
                        .dialect
                        .reactive_result_slot(primitive, ResultSlot::TupleItem(slot))
                        == Some(ReactiveRole::Setter)
            });
        }
        let Some(alias) = &binding.initializer_identifier else {
            return false;
        };
        path = definition.path.as_str();
        span = alias.span;
    }
    false
}

/// Applies also to operations nested inside an inert apply. For a named
/// callback, independent invocations keep their own execution claim; combine
/// all supplying sites instead of letting a dead site cancel a live one.
pub(crate) fn at_span(file: &FileFacts, span: Span, lookup: &SemanticLookup<'_>) -> ApplyExecution {
    let regions = lookup.effect_apply_regions();
    let mut answer = ApplyExecution::Proven;
    for (_, execution) in regions
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .filter(|(region, _)| region.contains(span))
    {
        match execution {
            ApplyExecution::Absent => return ApplyExecution::Absent,
            ApplyExecution::Unknown => answer = ApplyExecution::Unknown,
            ApplyExecution::Proven => {}
        }
    }
    answer
}

pub(crate) type ApplyRegions = HashMap<String, Vec<(Span, ApplyExecution)>>;

pub(crate) fn regions(lookup: &SemanticLookup<'_>) -> ApplyRegions {
    let mut targets: HashMap<(String, Span), Vec<ApplyExecution>> = HashMap::default();
    let mut uses = HashSet::default();
    for file in lookup.files() {
        for call in &file.ast.calls {
            if let Some((definition, function)) = callback(file, call.callee, lookup) {
                targets
                    .entry((definition.path.to_string(), function.span))
                    .or_default()
                    .push(ApplyExecution::Proven);
                uses.insert((file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee)));
            }
            for (index, argument) in call.arguments.iter().enumerate() {
                let values = std::iter::once(argument.span)
                    .chain(argument.literal_members.iter().map(|member| member.value));
                for value in values {
                    let Some((definition, function)) = callback(file, value, lookup) else {
                        continue;
                    };
                    let execution = lookup.primitive_at_call(file, call.span).map_or(
                        ApplyExecution::Proven,
                        |primitive| {
                            let execution = at_call(file, call, primitive, index, lookup);
                            // The error arm can be queued by a settled compute
                            // error even when initial success apply is skipped.
                            // Do not turn absence of a dependency into absence
                            // of that separate trigger.
                            let error = argument.literal_members.iter().any(|member| {
                                member.value == value
                                    && lookup.dialect.browser_callback_member_trigger(
                                        primitive,
                                        index,
                                        call.arguments.len(),
                                        member.key.as_str(),
                                    ) == Some(
                                        solid_dialect::BrowserCallbackTrigger::ComputationError,
                                    )
                            });
                            if error && execution != ApplyExecution::Proven {
                                let inert = call.arguments.first().is_some_and(|compute| {
                                    crate::callback_host::inert_callback(file, compute.span, lookup)
                                });
                                if inert {
                                    ApplyExecution::Absent
                                } else {
                                    ApplyExecution::Unknown
                                }
                            } else {
                                execution
                            }
                        },
                    );
                    targets
                        .entry((definition.path.to_string(), function.span))
                        .or_default()
                        .push(execution);
                    uses.insert((file.path.as_str(), file.ast.peel_ts_sugar_span(value)));
                }
            }
        }
    }
    // An escape/export is another possible invocation. A dead supplying site
    // cannot suppress the body of a reusable named callback globally.
    for file in lookup.files() {
        for reference in &file.ast.identifiers {
            if reference.role != solid_facts::ast::IdentifierRole::Reference
                || uses.contains(&(file.path.as_str(), reference.span))
            {
                continue;
            }
            if let Some((definition, function)) = callback(file, reference.span, lookup)
                && let Some(sites) = targets.get_mut(&(definition.path.to_string(), function.span))
            {
                sites.push(ApplyExecution::Unknown);
            }
        }
        for export in &file.ast.exports {
            for specifier in export.specifiers.iter().chain(&export.declarations) {
                if let Some((definition, function)) = callback(file, specifier.local.span, lookup)
                    && let Some(sites) =
                        targets.get_mut(&(definition.path.to_string(), function.span))
                {
                    sites.push(ApplyExecution::Unknown);
                }
            }
        }
    }
    let mut result = ApplyRegions::default();
    for ((path, span), sites) in targets {
        let execution = if sites.contains(&ApplyExecution::Proven) {
            ApplyExecution::Proven
        } else if sites.contains(&ApplyExecution::Unknown) {
            ApplyExecution::Unknown
        } else {
            ApplyExecution::Absent
        };
        if execution != ApplyExecution::Proven {
            result.entry(path).or_default().push((span, execution));
        }
    }
    result
}

/// Every diagnostic producer shares the same invocation premise, including
/// owner/leaf rules and reads attributed from a helper. Run after retained
/// stage contributions merge, so a cache cannot preserve an obsolete premise.
pub(crate) fn gate_diagnostics(
    lookup: &SemanticLookup<'_>,
    draft: &mut crate::pipeline::ProgramDraft,
) {
    let execution = |location: &crate::Location| {
        let Some(file) = lookup.file_by_path(location.path.as_ref()) else {
            return ApplyExecution::Proven;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            return ApplyExecution::Unknown;
        };
        at_span(file, Span::new(start, end), lookup)
    };
    draft
        .reads
        .retain_mut(|read| match execution(&read.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                read.callee_callback_timing = true;
                // Withholding an invocation is no universal non-strict-use
                // proof. The callback may still run in its strict apply phase.
                read.project_consumer_non_strict = false;
                true
            }
            ApplyExecution::Proven => true,
        });
    macro_rules! gate {
        ($table:ident, $uncertain:ident) => {
            draft
                .$table
                .retain_mut(|row| match execution(&row.location) {
                    ApplyExecution::Absent => false,
                    ApplyExecution::Unknown => {
                        row.$uncertain = true;
                        true
                    }
                    ApplyExecution::Proven => true,
                });
        };
    }
    draft
        .missing_owners
        .retain_mut(|row| match execution(&row.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                row.uncertain = true;
                row.runtime_uncertain = true;
                true
            }
            ApplyExecution::Proven => true,
        });
    gate!(leaf_operations, uncertain);
    gate!(static_defects, uncertain);
    gate!(static_violations, uncertain);
    gate!(async_reads, invocation_context_unproven);
    draft
        .writes
        .retain_mut(|row| match execution(&row.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                row.execution = crate::ExecutionRole::Unknown;
                row.blocker = Some(crate::PlacementBlocker::DeferredApply);
                true
            }
            ApplyExecution::Proven => true,
        });
    draft
        .action_invocations
        .retain_mut(|row| match execution(&row.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                row.execution = crate::ExecutionRole::Unknown;
                row.blocker = Some(crate::PlacementBlocker::DeferredApply);
                true
            }
            ApplyExecution::Proven => true,
        });
    draft
        .directive_creations
        .retain(|row| execution(&row.location) == ApplyExecution::Proven);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_defer_options_decide_initial_execution() {
        let source = "f(() => 0, () => {}, { defer: true }); f(() => 0, () => {}, { defer: false }); f(() => 0, () => {}, {}); f(() => 0, () => {}, options); f(() => 0, () => {}, { ...options, defer: true }); f(() => 0, () => {}, { get defer() { return true; } }); f(() => 0, () => {}, { ['defer']: true });";
        let ast = solid_facts::ast::extract("options.ts", source).unwrap();
        let file = FileFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            path: solid_facts::core::SourcePath::new("options.ts").unwrap(),
            source_hash: ast.source.hash.clone(),
            source: source.into(),
            compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: ast.source.hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
            ast: std::sync::Arc::new(ast),
        };
        let actual = file
            .ast
            .calls
            .iter()
            .map(|call| crate::call_option_literal(&file, call, 2, "defer"))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                OptionLiteral::True,
                OptionLiteral::False,
                OptionLiteral::Absent,
                OptionLiteral::Unknown,
                OptionLiteral::Unknown,
                OptionLiteral::Unknown,
                OptionLiteral::Unknown
            ]
        );
    }

    #[test]
    fn defer_requires_dependency_but_unknown_options_never_prove_delivery() {
        for dependency in [
            ApplyExecution::Proven,
            ApplyExecution::Absent,
            ApplyExecution::Unknown,
        ] {
            assert_eq!(
                deferred_execution(OptionLiteral::True, dependency),
                dependency
            );
            assert_eq!(
                deferred_execution(OptionLiteral::Unknown, dependency),
                ApplyExecution::Unknown
            );
            for options in [OptionLiteral::Absent, OptionLiteral::False] {
                assert_eq!(
                    deferred_execution(options, dependency),
                    ApplyExecution::Proven
                );
            }
        }
    }
}
