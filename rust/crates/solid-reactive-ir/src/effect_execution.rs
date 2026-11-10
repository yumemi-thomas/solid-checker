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

/// Every apply argument with its execution, `(path, call, argument,
/// execution)` sorted: the [`EffectApplyCalls`](crate::indexes::EffectApplyCalls)
/// memo's answer.
pub(crate) type ApplyCallRows = Vec<(String, Span, usize, ApplyExecution)>;

impl crate::base::memo::Canonical for ApplyExecution {}

/// One file's answer for one of the tables below, with what it read of the
/// program, the file's own facts included (S7g): reused while the file is
/// unchanged and every read answers as recorded ([`crate::base::reads`]).
struct Chunk<T> {
    source_hash: solid_facts::core::SourceHash,
    compiler: std::sync::Arc<solid_facts::compiler::ExecutionMap>,
    proof: crate::base::reads::ReadProof,
    value: T,
}

type Chunks<T> = HashMap<std::sync::Arc<str>, Chunk<T>>;

/// The calls of a file whose primitive has an apply with a defer option.
type DeferCalls = Vec<(usize, Primitive)>;

#[derive(Default)]
struct DeferScans(Chunks<DeferCalls>);

/// What one file contributes to the apply regions: the callbacks its calls
/// supply (a call's callee, its arguments and their literal members) with
/// the supplying call and that site's execution, and the callbacks its other
/// references and exports can escape to.
#[derive(Clone, Default)]
struct RegionSites {
    supplied: Vec<((String, Span), Span, ApplyExecution)>,
    escaped: Vec<(String, Span)>,
}

#[derive(Default)]
struct RegionChunks(Chunks<RegionSites>);

const DEFER_SCANS: crate::base::memo::derived::SolverId = crate::base::memo::derived::SolverId(448);
const REGION_CHUNKS: crate::base::memo::derived::SolverId =
    crate::base::memo::derived::SolverId(449);

/// `file`'s chunk in `chunks`, computed when absent or no longer valid. Kept
/// only for a retained build whose paths are unique (a path held by two slots
/// is answered fresh).
fn chunked<T: Clone>(
    chunks: &mut Chunks<T>,
    file: &FileFacts,
    lookup: &SemanticLookup<'_>,
    kept: bool,
    compute: impl FnOnce() -> T,
) -> T {
    if !kept {
        return compute();
    }
    if let Some(chunk) = chunks.get(file.path.as_str())
        && chunk.source_hash == file.source_hash
        && (std::sync::Arc::ptr_eq(&chunk.compiler, &file.compiler)
            || crate::cache::same_compiler_semantics(&chunk.compiler, &file.compiler))
        && chunk.proof.holds(lookup)
    {
        return chunk.value.clone();
    }
    // The chunk reads the file's own entity rows (which primitive a call
    // names, which function a reference resolves to) as well as its source:
    // an edit elsewhere can re-point them with this file's text unchanged
    // (an import alias that becomes or stops being a primitive). Its reads
    // name the file itself, as they name every file the lookup hands out.
    let (value, reads) = crate::base::reads::record(|| {
        lookup.note_file_read(file);
        compute()
    });
    chunks.insert(
        file.path.shared(),
        Chunk {
            source_hash: file.source_hash.clone(),
            compiler: std::sync::Arc::clone(&file.compiler),
            proof: crate::base::reads::ReadProof::new(reads, lookup),
            value: value.clone(),
        },
    );
    value
}

/// Whether the build's files have distinct paths.
fn unique_paths(lookup: &SemanticLookup<'_>) -> bool {
    let mut seen = HashSet::default();
    lookup
        .file_slots()
        .iter()
        .all(|file| seen.insert(file.path.as_str()))
}

pub(crate) fn calls(lookup: &SemanticLookup<'_>) -> ApplyCallRows {
    let unique = unique_paths(lookup);
    lookup.with_kept_state::<DeferScans, _>(DEFER_SCANS, |scans, kept| {
        let kept = kept && unique;
        let mut result = ApplyCallRows::default();
        for file in lookup.file_slots() {
            let candidates = chunked(&mut scans.0, file, lookup, kept, || {
                file.ast
                    .calls
                    .iter()
                    .enumerate()
                    .filter_map(|(index, call)| {
                        let primitive = lookup.primitive_at_call(file, call.span)?;
                        (0..call.arguments.len())
                            .any(|argument| {
                                lookup
                                    .dialect
                                    .effect_apply_defer_options(primitive, argument)
                                    .is_some()
                            })
                            .then_some((index, primitive))
                    })
                    .collect()
            });
            for (index, primitive) in candidates {
                let Some(call) = file.ast.calls.get(index) else {
                    continue;
                };
                for argument in 0..call.arguments.len() {
                    if lookup
                        .dialect
                        .effect_apply_defer_options(primitive, argument)
                        .is_some()
                    {
                        result.push((
                            file.path.to_string(),
                            call.span,
                            argument,
                            prove_at_call(file, call, primitive, argument, lookup),
                        ));
                    }
                }
            }
        }
        if kept {
            let current = lookup
                .file_slots()
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            scans.0.retain(|path, _| current.contains(&**path));
        }
        finish_calls(result)
    })
}

fn finish_calls(result: ApplyCallRows) -> ApplyCallRows {
    // A path held by two slots keeps the later slot's proof, as the map did.
    let mut last = HashMap::<(String, Span, usize), usize>::default();
    for (index, (path, span, argument, _)) in result.iter().enumerate() {
        last.insert((path.clone(), *span, *argument), index);
    }
    let mut kept = result
        .iter()
        .enumerate()
        .filter(|(index, (path, span, argument, _))| {
            last.get(&(path.clone(), *span, *argument)) == Some(index)
        })
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    kept.sort_unstable_by(|left, right| {
        (&left.0, left.1, left.2).cmp(&(&right.0, right.1, right.2))
    });
    kept
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
    // A path's regions are sorted by span ([`regions`]): one that starts
    // after `span` does not contain it.
    let regions = regions.get(file.path.as_str()).map_or(&[][..], |regions| {
        &regions[..regions.partition_point(|(region, _)| region.start <= span.start)]
    });
    for (_, execution) in regions.iter().filter(|(region, _)| region.contains(span)) {
        match execution {
            ApplyExecution::Absent => return ApplyExecution::Absent,
            ApplyExecution::Unknown => answer = ApplyExecution::Unknown,
            ApplyExecution::Proven => {}
        }
    }
    answer
}

pub(crate) type ApplyRegions = HashMap<String, Vec<(Span, ApplyExecution)>>;

/// Each path's not-proven regions, sorted by path then span: the
/// [`EffectApplyRegions`](crate::indexes::EffectApplyRegions) memo's answer.
pub(crate) type ApplyRegionRows = Vec<(String, Vec<(Span, ApplyExecution)>)>;

pub(crate) fn regions(lookup: &SemanticLookup<'_>) -> ApplyRegionRows {
    let unique = unique_paths(lookup);
    lookup.with_kept_state::<RegionChunks, _>(REGION_CHUNKS, |chunks, kept| {
        let kept = kept && unique;
        let mut targets: HashMap<(String, Span), Vec<Site>> = HashMap::default();
        let mut escaped = Vec::new();
        for file in lookup.file_slots() {
            let sites = chunked(&mut chunks.0, file, lookup, kept, || {
                file_region_sites(file, lookup)
            });
            for (target, call, execution) in sites.supplied {
                targets.entry(target).or_default().push(Site {
                    at: Some((file.path.to_string(), call)),
                    execution,
                });
            }
            escaped.extend(sites.escaped);
        }
        // An escape/export is another possible invocation. A dead supplying
        // site cannot suppress the body of a reusable named callback
        // globally; and a callback no site supplies has no region.
        for target in escaped {
            if let Some(sites) = targets.get_mut(&target) {
                sites.push(Site {
                    at: None,
                    execution: ApplyExecution::Unknown,
                });
            }
        }
        if kept {
            let current = lookup
                .file_slots()
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            chunks.0.retain(|path, _| current.contains(&**path));
        }
        fold_regions(targets)
    })
}

/// What `file` supplies and may escape to, from its own calls, references and
/// exports (the callbacks resolve through the lookup, which records its reads).
fn file_region_sites(file: &FileFacts, lookup: &SemanticLookup<'_>) -> RegionSites {
    let mut sites = RegionSites::default();
    let mut uses = HashSet::default();
    for call in &file.ast.calls {
        if let Some((definition, function)) = callback(file, call.callee, lookup) {
            sites.supplied.push((
                (definition.path.to_string(), function.span),
                call.span,
                ApplyExecution::Proven,
            ));
            uses.insert(file.ast.peel_ts_sugar_span(call.callee));
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
                sites.supplied.push((
                    (definition.path.to_string(), function.span),
                    call.span,
                    execution,
                ));
                uses.insert(file.ast.peel_ts_sugar_span(value));
            }
        }
    }
    for reference in &file.ast.identifiers {
        if reference.role != solid_facts::ast::IdentifierRole::Reference
            || uses.contains(&reference.span)
        {
            continue;
        }
        if let Some((definition, function)) = callback(file, reference.span, lookup) {
            sites
                .escaped
                .push((definition.path.to_string(), function.span));
        }
    }
    for export in &file.ast.exports {
        for specifier in export.specifiers.iter().chain(&export.declarations) {
            if let Some((definition, function)) = callback(file, specifier.local.span, lookup) {
                sites
                    .escaped
                    .push((definition.path.to_string(), function.span));
            }
        }
    }
    sites
}

/// One invocation of a region's callback: the supplying call (none for an
/// escape or export, which is unconditional) and its own execution.
struct Site {
    at: Option<(String, Span)>,
    execution: ApplyExecution,
}

const fn rank(execution: ApplyExecution) -> u8 {
    match execution {
        ApplyExecution::Absent => 0,
        ApplyExecution::Unknown => 1,
        ApplyExecution::Proven => 2,
    }
}

/// A supplying call runs only when every callback lexically around it runs
/// (ADR 0590): a call written in an apply that provably never runs supplies
/// nothing, one in an apply that may not run supplies at most an unknown run.
/// Each callback's execution is the best of its sites, each site's the worst
/// of its own and of the callbacks around it: the least fixed point, so
/// callbacks only dead code calls (recursion included) never run. A callback
/// no site supplies is not a region and runs as its context does.
fn fold_regions(targets: HashMap<(String, Span), Vec<Site>>) -> ApplyRegionRows {
    let mut keys = targets.keys().cloned().collect::<Vec<_>>();
    keys.sort_unstable();
    // Each path's callbacks by span, for the enclosing lookup below.
    let mut by_path = HashMap::<&str, Vec<(Span, usize)>>::default();
    for (position, (path, span)) in keys.iter().enumerate() {
        by_path
            .entry(path.as_str())
            .or_default()
            .push((*span, position));
    }
    // sites[t]: (own execution, the callbacks around the supplying call).
    let sites = keys
        .iter()
        .map(|key| {
            targets[key]
                .iter()
                .map(|site| {
                    let around = site.at.as_ref().map_or_else(Vec::new, |(path, call)| {
                        by_path
                            .get(path.as_str())
                            .into_iter()
                            .flatten()
                            .filter(|(span, _)| span.contains(*call))
                            .map(|(_, position)| *position)
                            .collect::<Vec<_>>()
                    });
                    (site.execution, around)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut dependents = vec![Vec::new(); keys.len()];
    for (target, target_sites) in sites.iter().enumerate() {
        for (_, around) in target_sites {
            for enclosing in around {
                dependents[*enclosing].push(target);
            }
        }
    }
    let mut value = vec![ApplyExecution::Absent; keys.len()];
    let mut queued = vec![true; keys.len()];
    let mut work = (0..keys.len()).collect::<std::collections::VecDeque<_>>();
    while let Some(target) = work.pop_front() {
        queued[target] = false;
        let next = sites[target]
            .iter()
            .map(|(execution, around)| {
                around
                    .iter()
                    .map(|enclosing| value[*enclosing])
                    .chain(std::iter::once(*execution))
                    .min_by_key(|execution| rank(*execution))
                    .unwrap_or(*execution)
            })
            .max_by_key(|execution| rank(*execution))
            .unwrap_or(ApplyExecution::Absent);
        if rank(next) > rank(value[target]) {
            value[target] = next;
            for dependent in &dependents[target] {
                if !queued[*dependent] {
                    queued[*dependent] = true;
                    work.push_back(*dependent);
                }
            }
        }
    }
    let mut result = ApplyRegions::default();
    for ((path, span), execution) in keys.into_iter().zip(value) {
        if execution != ApplyExecution::Proven {
            result.entry(path).or_default().push((span, execution));
        }
    }
    let mut rows = result
        .into_iter()
        .map(|(path, mut regions)| {
            regions.sort_unstable_by_key(|(span, _)| *span);
            (path, regions)
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    rows
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
    // ADR 0933: a row an apply proven absent removes leaves a typed silence,
    // ADR 0936 recorded in the draft for the sites the universe lists.
    macro_rules! absent {
        ($table:ident, $name:expr, $closure:expr) => {{
            let stage =
                crate::proof_log::RowStage::begin($name, &draft.$table, |row| &row.location);
            retain_recording(
                &mut draft.$table,
                |row| &row.location,
                &mut draft.site_ends,
                matches!(
                    $name,
                    "reads"
                        | "writes"
                        | "action_invocations"
                        | "async_reads"
                        | "static_defects"
                        | "static_violations"
                ),
                $closure,
            );
            if let Some(stage) = stage {
                stage.end(
                    "ApplyExecution::Absent",
                    &[crate::proof_log::PremiseKind::ApplyNeverRuns],
                    &draft.$table,
                    |row| &row.location,
                );
            }
        }};
    }
    absent!(reads, "reads", |read| match execution(&read.location) {
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
            absent!(
                $table,
                stringify!($table),
                |row| match execution(&row.location) {
                    ApplyExecution::Absent => false,
                    ApplyExecution::Unknown => {
                        row.$uncertain = true;
                        true
                    }
                    ApplyExecution::Proven => true,
                }
            );
        };
    }
    absent!(
        missing_owners,
        "missing_owners",
        |row| match execution(&row.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                row.uncertain = true;
                row.runtime_uncertain = true;
                true
            }
            ApplyExecution::Proven => true,
        }
    );
    gate!(leaf_operations, uncertain);
    // ADR 0590: a row an apply of unknown execution supplies; as a read's,
    // its blocker is that the callee's invocation is unresolved.
    macro_rules! gate_named {
        ($table:ident) => {
            absent!(
                $table,
                stringify!($table),
                |row| match execution(&row.location) {
                    ApplyExecution::Absent => false,
                    ApplyExecution::Unknown => {
                        row.uncertain = true;
                        if !crate::blockers::named(&row.analysis_context) {
                            row.analysis_context = crate::blockers::with_detail(
                                crate::blockers::CALLEE_CALLBACK_TIMING,
                                &row.analysis_context,
                            );
                        }
                        true
                    }
                    ApplyExecution::Proven => true,
                }
            );
        };
    }
    gate_named!(static_defects);
    gate_named!(static_violations);
    gate!(async_reads, invocation_context_unproven);
    absent!(writes, "writes", |row| match execution(&row.location) {
        ApplyExecution::Absent => false,
        ApplyExecution::Unknown => {
            row.execution = crate::ExecutionRole::Unknown;
            true
        }
        ApplyExecution::Proven => true,
    });
    absent!(
        action_invocations,
        "action_invocations",
        |row| match execution(&row.location) {
            ApplyExecution::Absent => false,
            ApplyExecution::Unknown => {
                row.execution = crate::ExecutionRole::Unknown;
                true
            }
            ApplyExecution::Proven => true,
        }
    );
    let stage = crate::proof_log::RowStage::begin(
        "directive_creations",
        &draft.directive_creations,
        |row| &row.location,
    );
    draft
        .directive_creations
        .retain(|row| execution(&row.location) == ApplyExecution::Proven);
    if let Some(stage) = stage {
        // Dropped unless the apply is proven, an unknown one included.
        stage.end(
            "ApplyExecution::NotProven",
            &[],
            &draft.directive_creations,
            |row| &row.location,
        );
    }
}

/// `rows.retain_mut(keep)`, naming the rows `keep` removes as sites an apply
/// proven absent ends with no row (ADR 0936), when `record`.
fn retain_recording<T>(
    rows: &mut Vec<T>,
    location: impl Fn(&T) -> &crate::Location,
    ends: &mut crate::site_accounting::SiteEnds,
    record: bool,
    mut keep: impl FnMut(&mut T) -> bool,
) {
    rows.retain_mut(|row| {
        let kept = keep(row);
        if !kept && record {
            ends.removed(
                location(row),
                crate::site_accounting::SiteSilence::ApplyNeverRuns,
                crate::proof_log::PremiseKind::ApplyNeverRuns,
            );
        }
        kept
    });
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
    fn a_call_in_a_dead_region_supplies_nothing() {
        // a.ts: apply [10, 20) is dead; it calls f [30, 40) at 12; f calls
        // itself at 32 and g [50, 60) at 34; h [70, 80) is called at 14 in
        // the apply and at 100, outside every region; u [110, 120) is called
        // in an unknown apply [130, 140) at 132.
        let site = |at: Option<(u32, u32)>, execution| Site {
            at: at.map(|(start, end)| ("a.ts".to_string(), Span::new(start, end))),
            execution,
        };
        let key = |start, end| ("a.ts".to_string(), Span::new(start, end));
        let mut targets = HashMap::default();
        targets.insert(
            key(10, 20),
            vec![site(Some((0, 25)), ApplyExecution::Absent)],
        );
        targets.insert(
            key(30, 40),
            vec![
                site(Some((12, 13)), ApplyExecution::Proven),
                site(Some((32, 33)), ApplyExecution::Proven),
            ],
        );
        targets.insert(
            key(50, 60),
            vec![site(Some((34, 35)), ApplyExecution::Proven)],
        );
        targets.insert(
            key(70, 80),
            vec![
                site(Some((14, 15)), ApplyExecution::Proven),
                site(Some((100, 101)), ApplyExecution::Proven),
            ],
        );
        targets.insert(
            key(130, 140),
            vec![site(Some((125, 145)), ApplyExecution::Unknown)],
        );
        targets.insert(
            key(110, 120),
            vec![site(Some((132, 133)), ApplyExecution::Proven)],
        );
        let rows = fold_regions(targets);
        assert_eq!(
            rows,
            vec![(
                "a.ts".to_string(),
                vec![
                    (Span::new(10, 20), ApplyExecution::Absent),
                    (Span::new(30, 40), ApplyExecution::Absent),
                    (Span::new(50, 60), ApplyExecution::Absent),
                    (Span::new(110, 120), ApplyExecution::Unknown),
                    (Span::new(130, 140), ApplyExecution::Unknown),
                ]
            )]
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
