//! Execution-role classification.
//!
//! Given a source span, classify the reactive execution context it runs in
//! (tracked JSX, deferred callback, effect apply, event handler, …). This is
//! the compiler-fact classifier plus the semantic (AST-driven) classifier and
//! the two role-keyed read helpers that consume its result.

use std::collections::{HashMap, HashSet};

use solid_dialect::{Dialect, Execution, Primitive};
use solid_facts::core::Span;

use super::{
    EntitySymbols, ExecutionRole, PrimitiveName, SemanticLookup, SymbolId, jsx_primitive_name,
    known_primitive, location, primitive_name,
};
use crate::owners::{
    callback_execution_at_call, containing_ast_function, enclosing_function_label,
    function_binding_name, returned_callback_execution_at_call, returned_callback_invocation_sites,
    returned_primitive_invocation,
};

/// The effect primitives: the ones 2.0 spells `(compute, apply)`.
fn is_effect(primitive: Primitive) -> bool {
    matches!(
        primitive,
        Primitive::CreateEffect | Primitive::CreateRenderEffect
    )
}

/// The argument an effect runs *after* its compute, if this dialect has one.
///
/// 2.0's `createEffect(compute, apply)` has it at index 1. 1.x's second
/// argument is a seed value threaded to the next run as `prev`, so 1.x has
/// none — and this used to be the literal `1` for both. A read in a 1.x seed
/// would be classified `EffectApply`, reporting it as running in a phase that
/// version does not have.
fn effect_apply_argument(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Option<usize> {
    if !is_effect(primitive) {
        return None;
    }
    (0..argument_count).find(|index| {
        dialect
            .callback_semantics_at(primitive, *index, argument_count)
            .execution
            == Some(Execution::Deferred)
    })
}

fn callback_runs_outside_tracking(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument: usize,
    argument_count: usize,
) -> bool {
    let semantics = dialect.callback_semantics_at(primitive, argument, argument_count);
    match semantics.execution {
        None => false,
        // A tracked callback creates its own observer unless the primitive's
        // exact runtime contract explicitly overrides that classification.
        Some(Execution::Tracked) => !semantics.tracks_reads,
        // Deferred callbacks execute after/outside the current tracking pass.
        Some(Execution::Deferred) => true,
        // Inline means the callback inherits the caller's Listener. Only
        // primitives such as untrack/createRoot/runWithOwner that explicitly
        // clear Listener belong to the outside-tracking set.
        Some(Execution::Inline) => dialect.runs_callback_deferred(primitive),
    }
}

/// The argument positions holding a callback that runs outside the
/// surrounding tracking scope — an effect's apply argument, or a deferred
/// executor's whole callback.
///
/// A strict subset of [`Dialect::callback_positions`], which also answers for
/// `createMemo`, `createSignal` and the rest of the tracked index-0 set. The
/// two questions are independent: position says *where* a callback sits,
/// [`Dialect::runs_callback_deferred`] says *how it executes*.
fn deferred_callback_positions(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Vec<usize> {
    (0..argument_count)
        .filter(|index| callback_runs_outside_tracking(dialect, primitive, *index, argument_count))
        .filter(|index| !dialect.reports_untracked_reads_at(primitive, *index, argument_count))
        .collect()
}

pub(super) fn execution_role(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
) -> ExecutionRole {
    execution_role_where(facts, span, allowed, |_| true)
}

fn execution_role_where(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
    callback_applies: impl Fn(&solid_facts::compiler::CallbackRole) -> bool,
) -> ExecutionRole {
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    let tracked = facts.tracked_regions.iter().filter_map(|region| {
        region
            .span
            .contains(span)
            .then_some((region.span, ExecutionRole::TrackedJsx, 2_u8))
    });
    let untracked = facts.untracked_regions.iter().filter_map(|region| {
        region
            .span
            .contains(span)
            .then_some((region.span, ExecutionRole::UntrackedRendering, 1_u8))
    });
    let callbacks = facts.callback_roles.iter().filter_map(|callback| {
        (callback.span.contains(span) && callback_applies(callback)).then_some((
            callback.span,
            match callback.role {
                solid_facts::compiler::CallbackRoleKind::EventHandler => {
                    ExecutionRole::EventCallback
                }
                solid_facts::compiler::CallbackRoleKind::Deferred => {
                    ExecutionRole::DeferredCallback
                }
                solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                    ExecutionRole::DirectiveApply
                }
                solid_facts::compiler::CallbackRoleKind::Render => {
                    ExecutionRole::UntrackedRendering
                }
            },
            0_u8,
        ))
    });
    if let Some((_, role, _)) = tracked
        .chain(untracked)
        .chain(callbacks)
        .min_by_key(|(region, _, tie_break)| (region.end - region.start, *tie_break))
    {
        return role;
    }
    ExecutionRole::Unknown
}

/// Compiler execution role for code that actually runs at `span`.
///
/// A callback-role span covers the complete JSX value expression handed to
/// the compiler. Only a function value inside that expression executes in the
/// callback phase; eager subexpressions such as `onClick={makeHandler()}` or
/// `ref={[makeDirective()]}` execute while rendering and must fall through to
/// their tracked/untracked region or enclosing component.
fn source_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
) -> ExecutionRole {
    execution_role_where(&file.compiler, span, allowed, |callback| {
        matches!(
            callback.role,
            solid_facts::compiler::CallbackRoleKind::Deferred
                | solid_facts::compiler::CallbackRoleKind::Render
        ) || file
            .ast
            .functions_within(callback.span)
            .max_by_key(|function| function.span.end - function.span.start)
            .is_some_and(|function| function.body.contains(span))
    })
}

pub(super) fn semantic_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    semantic_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::new(),
    )
}

/// Classifies a write or action through resolved project call sites when its
/// own source span has no compiler execution fact.
///
/// Any proven tracking-phase invocation makes the operation unsafe. Otherwise
/// a proven imperative role is retained, while cycles with no independently
/// classified call site remain `Unknown`.
pub(super) fn semantic_write_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    semantic_write_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::new(),
    )
}

fn semantic_write_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    visiting: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    let direct = semantic_execution_role(file, span, allowed, entities, symbol_names, lookup);
    if direct != ExecutionRole::Unknown {
        return direct;
    }
    let Some(function) = crate::owners::containing_ast_function(&file.ast, span) else {
        return ExecutionRole::Unknown;
    };
    let key = (file.path.to_string(), function.span);
    if !visiting.insert(key.clone()) {
        return ExecutionRole::Unknown;
    }
    let mut imperative = None;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        let caller_allowed = allowed_callback_spans(caller_file, lookup);
        let role = semantic_write_execution_role_within(
            caller_file,
            callee,
            &caller_allowed,
            entities,
            symbol_names,
            lookup,
            visiting,
        );
        if role.reports_disallowed_write() {
            visiting.remove(&key);
            return role;
        }
        if role != ExecutionRole::Unknown {
            imperative.get_or_insert(role);
        }
    }
    visiting.remove(&key);
    imperative.unwrap_or(ExecutionRole::Unknown)
}

/// `classifying` is the stack of spans whose role is currently being derived
/// from their own invocation sites. A returned adapter invoked inside its own
/// factory callback — `const a = on(() => a(), fn)`, or two adapters invoked
/// in each other's callbacks — makes that derivation cyclic; a site already on
/// the stack supplies no independent execution context and is skipped instead
/// of re-entered.
fn semantic_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    if let Some(role) = context_provider_value_role(file, span, lookup) {
        return role;
    }
    if assigned_member_function_contains(file, span, entities) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = named_callback_execution_role(file, span, lookup) {
        return role;
    }
    if let Some(role) = returned_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = returned_factory_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = inline_callback_execution_role(file, span, allowed, lookup, classifying) {
        return role;
    }
    let dialect = lookup.dialect;
    if file.ast.arguments_containing(span).any(|(call, index)| {
        primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
        .as_ref()
        .and_then(PrimitiveName::primitive)
        .and_then(|primitive| effect_apply_argument(dialect, primitive, call.arguments.len()))
            == Some(index)
            && direct_callback_contains(file, call.arguments[index].span, span)
    }) {
        return ExecutionRole::EffectApply;
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
        primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
        .as_ref()
        .and_then(PrimitiveName::primitive)
        .is_some_and(|primitive| {
            callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
                && direct_callback_contains(file, call.arguments[index].span, span)
        })
    }) {
        return ExecutionRole::UntrackedCallback;
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = control_flow_execution_role(file, span, entities, symbol_names, dialect) {
        return role;
    }
    if file
        .compiler
        .tracked_regions
        .iter()
        .any(|region| region.span.contains(span))
    {
        return ExecutionRole::TrackedJsx;
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
        matches!(
            call.arguments[index].value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
        .as_ref()
        .and_then(PrimitiveName::primitive)
        .is_some_and(|primitive| {
            callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .tracks_reads
        })
    }) {
        return ExecutionRole::TrackedJsx;
    }
    let compiler_role = source_execution_role(file, span, allowed);
    if compiler_role != ExecutionRole::Unknown {
        return compiler_role;
    }
    if lookup.inside_component(file, span) {
        return ExecutionRole::UntrackedRendering;
    }
    // Module initialization is an AST-proven one-shot execution context. It
    // is not a compiler-fact gap: no reactive owner or subscriber can be
    // active before a containing function is invoked.
    if !file.ast.any_function_body_containing(span) {
        return ExecutionRole::ModuleInitialization;
    }
    ExecutionRole::Unknown
}

/// Inline callbacks that do not clear the reactive listener inherit the
/// caller's execution role. This covers wrappers such as `batch`,
/// `catchError`'s protected body, and `modifyMutable`; `untrack` and other
/// explicit untracked callbacks are classified by the later dialect branch.
fn inline_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, call.span)?;
            if callback_execution_at_call(file, call, primitive, index, lookup)?
                != Execution::Inline
                || callback_runs_outside_tracking(
                    lookup.dialect,
                    primitive,
                    index,
                    call.arguments.len(),
                )
            {
                return None;
            }
            let key = (file.path.to_string(), call.span);
            if !classifying.insert(key.clone()) {
                return None;
            }
            let role = semantic_execution_role_within(
                file,
                call.span,
                allowed,
                lookup.entities(),
                lookup.symbol_names(),
                lookup,
                classifying,
            );
            classifying.remove(&key);
            Some(role)
        })
}

/// Compose an inline callback of a higher-order factory with the proven use of
/// the returned function. For example, `on` reads its dependency inline when
/// the returned adapter runs: `createEffect(on(...))` therefore tracks that
/// read, while a direct top-level adapter call does not.
fn returned_factory_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    // Every proof this composition needs starts from
    // `callback_requires_return_invocation`, which Solid 2.0 leaves at its
    // `false` default for every primitive.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    file.ast
        .arguments_containing(span)
        .find_map(|(factory_call, index)| {
            if !direct_callback_contains(file, factory_call.arguments[index].span, span) {
                return None;
            }
            let primitive = lookup.primitive_at_call(file, factory_call.span)?;
            let semantics = lookup.dialect.callback_semantics_at(
                primitive,
                index,
                factory_call.arguments.len(),
            );
            if !semantics.requires_return_invocation
                || semantics.execution != Some(Execution::Inline)
            {
                return None;
            }

            let mut roles = Vec::new();
            for site in returned_callback_invocation_sites(file, factory_call, lookup) {
                let role = match site.inherited_execution {
                    Some(Execution::Tracked) => Some(ExecutionRole::TrackedJsx),
                    Some(Execution::Deferred) => Some(ExecutionRole::DeferredCallback),
                    Some(Execution::Inline) | None => {
                        let key = (site.path.clone(), site.span);
                        if classifying.contains(&key) {
                            // A cyclic invocation site — the adapter calling
                            // itself through its own callback — has no context
                            // of its own to contribute.
                            None
                        } else {
                            lookup
                                .files()
                                .iter()
                                .find(|candidate| candidate.path.as_str() == site.path)
                                .map(|use_file| {
                                    classifying.insert(key.clone());
                                    let role = semantic_execution_role_within(
                                        use_file,
                                        site.span,
                                        &[],
                                        lookup.entities(),
                                        lookup.symbol_names(),
                                        lookup,
                                        classifying,
                                    );
                                    classifying.remove(&key);
                                    role
                                })
                        }
                    }
                };
                roles.extend(role);
            }
            roles.sort_by_key(|role| *role as u8);
            roles.dedup();
            match roles.as_slice() {
                [] => None,
                [role] => Some(*role),
                // The same returned adapter is used in incompatible execution
                // contexts. A single diagnostic site cannot truthfully claim one
                // dominates, so preserve uncertainty instead of manufacturing a
                // false positive in either direction.
                _ => Some(ExecutionRole::DeferredCallback),
            }
        })
}

fn returned_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
            match lookup
                .dialect
                .returned_callback_semantics_at(primitive, result_slot, index, call.arguments.len())
                .execution?
            {
                Execution::Tracked => Some(ExecutionRole::TrackedJsx),
                Execution::Deferred => Some(ExecutionRole::DeferredCallback),
                // The returned function restores/inherits its caller's execution
                // context. Classify the proven invocation span, outside the
                // callback body itself, so a top-level transition remains
                // untracked while one started from an effect remains tracked.
                Execution::Inline => {
                    let key = (file.path.to_string(), call.span);
                    if !classifying.insert(key.clone()) {
                        return None;
                    }
                    let role = semantic_execution_role_within(
                        file,
                        call.span,
                        &[],
                        lookup.entities(),
                        lookup.symbol_names(),
                        lookup,
                        classifying,
                    );
                    classifying.remove(&key);
                    Some(role)
                }
            }
        })
}

/// Whether `span` is evaluated as the `value` getter of a resolved Solid 1.x
/// `createContext().Provider`.
///
/// Both halves are semantic proof: the JSX member object resolves to the
/// binding initialized by the exact Solid `createContext` primitive, and the
/// final member is `Provider`. An arbitrary component named `SomeProvider` or
/// object with a same-spelled property satisfies neither condition.
fn context_provider_value_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let provider_member = lookup.dialect.context_provider_member()?;
    let element = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.attributes.iter().any(|attribute| {
                file.source_text(attribute.local_name) == Some("value")
                    && attribute
                        .expression
                        .is_some_and(|expression| expression.contains(span))
            })
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let (Some(object), Some(property)) = (element.member_object, element.member_property) else {
        return None;
    };
    if file.source_text(property) != Some(provider_member) {
        return None;
    }
    if !lookup.is_context_reference(file.path.as_str(), object) {
        return None;
    }
    // Solid 1.x's createProvider implementation eagerly reads props.value
    // inside untrack. A function expression is the one exception: reading
    // the getter only creates/stores the function; its body is not run.
    let stores_function = file.ast.functions_body_containing(span).any(|function| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .is_some_and(|expression| expression.contains(function.span))
        })
    });
    Some(if stores_function {
        ExecutionRole::DeferredCallback
    } else {
        ExecutionRole::UntrackedRendering
    })
}

pub(super) fn assigned_member_function_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        file.ast.assignments.iter().any(|assignment| {
            if assignment.value != solid_facts::ast::AssignmentValueKind::Function
                || !assignment.value_span.contains(function.span)
            {
                return false;
            }
            let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.span == assignment.target)
            else {
                return false;
            };
            let Some(object_symbol) = entities.at(file.path.as_str(), member.object) else {
                return false;
            };
            let Some(owner) = containing_ast_function(&file.ast, assignment.target) else {
                return false;
            };
            let caller_owned = owner.parameters.iter().any(|parameter| {
                parameter
                    .names
                    .iter()
                    .any(|name| entities.at(file.path.as_str(), name.span) == Some(object_symbol))
            });
            let returned = file.ast.returns.iter().any(|returned| {
                returned.value == solid_facts::ast::ReturnValueKind::Identifier
                    && containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|candidate| candidate.span == owner.span)
                    && entities.at(file.path.as_str(), returned.span) == Some(object_symbol)
            });
            caller_owned || returned
        })
    })
}

pub(super) fn control_flow_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let callback = file
        .ast
        .functions_body_containing(span)
        .filter(|function| element.span.contains(function.span))
        .max_by_key(|function| function.body.end - function.body.start)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|nested| callback.body.contains(nested.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

/// How this file's primitive calls and control-flow JSX name each of its own
/// functions as a callback.
///
/// Every flag is a property of the *function*, not of the read being
/// classified: which argument positions name it, and how the dialect executes
/// those positions. Deriving them once per file replaces the whole-file call and
/// JSX scans [`named_callback_execution_role`] used to run for every read it was
/// asked about -- five of them per read in the worst case.
#[derive(Default)]
pub(super) struct NamedCallbackRoles {
    by_function: HashMap<Span, NamedCallbackRole>,
}

/// The classification a named callback's positions support, in the order
/// [`named_callback_execution_role`] consults them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NamedCallbackRole {
    /// Some position naming this function is one an arm below can truthfully
    /// classify. An effect's tracked compute and an inline adapter argument
    /// (1.x `on`'s deps) satisfy none of them -- admitting those would fall
    /// through to the rendering tail and misreport `createEffect(namedCompute)`
    /// as untracked rendering. Answering None instead defers such reads to
    /// compiler facts and the returned-adapter classifiers.
    admitted: bool,
    untracked_callback: bool,
    effect_apply: bool,
    tracked: bool,
    deferred: bool,
}

impl NamedCallbackRoles {
    fn entry(&mut self, function: Span) -> &mut NamedCallbackRole {
        self.by_function.entry(function).or_default()
    }

    fn get(&self, function: Span) -> NamedCallbackRole {
        self.by_function.get(&function).copied().unwrap_or_default()
    }
}

/// Every way this file's callback positions can name one of its own functions.
///
/// The three maps are the *semantic* alternatives to comparing source text: an
/// exact demanded entity, a proven TypeScript reference to the function's
/// symbol, and -- for a function whose symbol carries a canonical Solid name --
/// that name. Admitting a function because some identifier in a callback
/// position happens to be spelled the same, which is what these replace,
/// accepts a same-spelled binding from any other scope or module.
struct NamedCallbackIndex<'a> {
    /// This file's functions that resolve to a symbol, in AST order.
    functions: Vec<Span>,
    by_symbol: HashMap<&'a str, Vec<usize>>,
    by_reference: HashMap<Span, Vec<usize>>,
    by_canonical_name: HashMap<&'a str, Vec<usize>>,
}

impl<'a> NamedCallbackIndex<'a> {
    fn new(
        file: &solid_facts::FileFacts,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolId>,
        lookup: &SemanticLookup<'_>,
    ) -> Self {
        let candidates = file
            .ast
            .functions
            .iter()
            .filter_map(|function| {
                Some((function.span, function_symbol(file, function, entities)?))
            })
            .collect::<Vec<_>>();
        let mut by_symbol = HashMap::<&str, Vec<usize>>::new();
        let mut by_reference = HashMap::<Span, Vec<usize>>::new();
        let mut by_canonical_name = HashMap::<&str, Vec<usize>>::new();
        for (index, (_, symbol)) in candidates.iter().enumerate() {
            by_symbol.entry(symbol.as_str()).or_default().push(index);
            if let Some(name) = symbol_names.get(*symbol) {
                by_canonical_name
                    .entry(name.as_str())
                    .or_default()
                    .push(index);
            }
            for reference in lookup.symbol_references(symbol.as_str()) {
                if reference.path.as_ref() != file.path.as_str() {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    continue;
                };
                by_reference
                    .entry(Span::new(start, end))
                    .or_default()
                    .push(index);
            }
        }
        Self {
            functions: candidates.iter().map(|(span, _)| *span).collect(),
            by_symbol,
            by_canonical_name,
            by_reference,
        }
    }

    /// The functions an exact demanded entity at `span` names -- the whole of a
    /// bare `createEffect(compute, applyValue)` argument, say.
    fn identity_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
    ) -> &[usize] {
        entities
            .at(file.path.as_str(), span)
            .and_then(|symbol| self.by_symbol.get(symbol.as_str()))
            .map_or(&[], Vec::as_slice)
    }

    /// The functions a use at `span` names, by any of the three proofs.
    fn named_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
        matched: &mut Vec<usize>,
    ) {
        matched.extend(self.identity_at(file, entities, span));
        if let Some(references) = self.by_reference.get(&span) {
            matched.extend(references.iter().copied());
        }
        if let Some(text) = file.source_text(span)
            && let Some(named) = self.by_canonical_name.get(text)
        {
            matched.extend(named.iter().copied());
        }
    }
}

/// Derive [`NamedCallbackRoles`] for one file. Memoized by
/// [`SemanticLookup::named_callback_roles`]; nothing else should call it.
pub(super) fn named_callback_roles(
    file: &solid_facts::FileFacts,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> NamedCallbackRoles {
    let dialect = lookup.dialect;
    let index = NamedCallbackIndex::new(file, entities, symbol_names, lookup);
    let mut roles = NamedCallbackRoles::default();
    if index.functions.is_empty() {
        return roles;
    }
    let primitives = lookup.primitives(file);
    let mut named = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let direct_primitive = known_primitive(&primitives.calls[call_index]);
        let returned_primitive = direct_primitive
            .is_none()
            .then(|| returned_primitive_invocation(file, call, lookup))
            .flatten();
        if direct_primitive.is_none() && returned_primitive.is_none() {
            continue;
        }
        let count = call.arguments.len();
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            // The argument itself, plus the callbacks an options object names
            // through its `effect`/`error` properties.
            named.clear();
            index.named_at(file, entities, argument.span, &mut named);
            for property in &argument.identifier_properties {
                index.named_at(file, entities, property.span, &mut named);
            }
            named.sort_unstable();
            named.dedup();
            if named.is_empty() {
                continue;
            }
            if let Some((_primitive, _result_slot)) = returned_primitive {
                let execution =
                    returned_callback_execution_at_call(file, call, argument_index, lookup);
                let tracked = execution == Some(Execution::Tracked);
                let deferred = execution == Some(Execution::Deferred);
                // Returned-function contracts describe the argument itself,
                // never a callback named inside an options object. Require
                // exact TypeScript identity for the admitted function. Inline
                // returned callbacks inherit the individual call site's role
                // and therefore cannot be collapsed into this per-function
                // index.
                for candidate in index.identity_at(file, entities, argument.span) {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted |= tracked || deferred;
                    entry.tracked |= tracked;
                    entry.deferred |= deferred;
                }
                continue;
            }
            let primitive = direct_primitive.expect("one primitive kind is proven above");
            let proven =
                callback_execution_at_call(file, call, primitive, argument_index, lookup).is_some();
            let untracked = dialect.reports_untracked_reads_at(primitive, argument_index, count);
            // Deliberately not gated on `proven`: an effect's apply position is
            // read straight off the dialect signature, as it always was.
            let effect_apply =
                effect_apply_argument(dialect, primitive, count) == Some(argument_index);
            let tracked = !is_effect(primitive)
                && dialect
                    .callback_semantics_at(primitive, argument_index, count)
                    .tracks_reads;
            let deferred =
                callback_runs_outside_tracking(dialect, primitive, argument_index, count);
            for candidate in &named {
                let entry = roles.entry(index.functions[*candidate]);
                entry.admitted |= proven && (untracked || effect_apply || tracked || deferred);
                entry.untracked_callback |= proven && untracked;
                entry.effect_apply |= effect_apply;
            }
            // The tracked and deferred arms have always demanded that the whole
            // argument *be* the function, never that an options object mention
            // it.
            for candidate in index.identity_at(file, entities, argument.span) {
                let entry = roles.entry(index.functions[*candidate]);
                entry.tracked |= proven && tracked;
                entry.deferred |= proven && deferred;
            }
        }
    }
    for (element_index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !known_primitive(&primitives.jsx[element_index])
            .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        {
            continue;
        }
        for identifier in file.ast.identifiers_within(element.span) {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || file
                    .ast
                    .jsx_containing(identifier.span)
                    .any(|nested| nested.span != element.span && element.span.contains(nested.span))
            {
                continue;
            }
            named.clear();
            index.named_at(file, entities, identifier.span, &mut named);
            for candidate in &named {
                roles.entry(index.functions[*candidate]).admitted = true;
            }
        }
    }
    roles
}

pub(super) fn named_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let roles = lookup.named_callback_roles(file);
    let callback = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    let role = roles.get(callback.span);
    if role.untracked_callback {
        return Some(ExecutionRole::UntrackedCallback);
    }
    if role.effect_apply {
        return Some(ExecutionRole::EffectApply);
    }
    if role.tracked {
        return Some(ExecutionRole::TrackedJsx);
    }
    if role.deferred {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|element| callback.body.contains(element.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

pub(super) fn function_symbol<'a>(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &'a EntitySymbols,
) -> Option<&'a SymbolId> {
    let name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))?;
    entities.get(&location(file.path.shared(), name.span))
}

pub(super) fn argument_references_callback_symbol(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    symbol: &str,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    entities
        .get(&location(file.path.shared(), argument.span))
        .map(SymbolId::as_str)
        == Some(symbol)
        || argument.identifier_properties.iter().any(|property| {
            entities
                .get(&location(file.path.shared(), property.span))
                .map(SymbolId::as_str)
                == Some(symbol)
                || symbol_names.get(symbol).map(SymbolId::as_str) == file.source_text(property.span)
        })
}

pub(super) fn direct_callback_contains(
    file: &solid_facts::FileFacts,
    argument: Span,
    span: Span,
) -> bool {
    if !argument.contains(span) {
        return false;
    }
    let callback = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start);
    let owner = containing_ast_function(&file.ast, span);
    match (callback, owner) {
        (Some(callback), Some(owner)) => callback.span == owner.span,
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn read_analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> String {
    if execution == ExecutionRole::EffectApply {
        "createEffect apply callback".into()
    } else {
        let context = enclosing_function_label(file, span);
        format_read_context(&context, file.ast.any_conditional_test_containing(span))
    }
}

fn format_read_context(context: &str, in_conditional_test: bool) -> String {
    if in_conditional_test {
        if context.is_empty() {
            "while evaluating a condition".into()
        } else {
            format!("{context} while evaluating a condition")
        }
    } else {
        context.into()
    }
}

pub(super) fn async_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    if execution == ExecutionRole::DeferredCallback && file.ast.any_jsx_containing(span) {
        ExecutionRole::TrackedJsx
    } else {
        execution
    }
}

pub(super) fn allowed_callback_spans(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let mut spans = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let mut indices =
            known_primitive(&primitives.calls[call_index]).map_or_else(Vec::new, |primitive| {
                deferred_callback_positions(dialect, primitive, call.arguments.len())
                    .into_iter()
                    .filter(|index| {
                        callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
                    })
                    .collect()
            });
        if let Some(symbol) = lookup.callee_symbol(file, call.callee)
            && let Some(callbacks) = lookup.contract_callbacks(symbol)
        {
            for callback in callbacks {
                let exclusively_deferred = callbacks.iter().all(|candidate| {
                    candidate.parameter != callback.parameter || candidate.execution == "deferred"
                });
                if callback.execution == "deferred"
                    && exclusively_deferred
                    && !indices.contains(&callback.parameter)
                {
                    indices.push(callback.parameter);
                }
            }
        }
        for index in indices {
            if let Some(argument) = call.arguments.get(index) {
                spans.push(argument.span);
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use solid_facts::{
        compiler::{
            COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind, ExecutionMap, ExecutionRegion,
            RegionReason,
        },
        core::{SourceHash, Span},
    };

    use super::{execution_role, format_read_context};
    use crate::ExecutionRole;

    fn execution_map() -> ExecutionMap {
        ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of("value"),
            tracked_regions: vec![],
            untracked_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        }
    }

    #[test]
    fn compiler_execution_distinguishes_explicit_untracked_from_unknown() {
        let mut facts = execution_map();
        assert_eq!(
            execution_role(&facts, Span::new(0, 5), &[]),
            ExecutionRole::Unknown
        );
        facts.untracked_regions.push(ExecutionRegion {
            span: Span::new(0, 5),
            reason: RegionReason::JsxChild,
        });
        assert_eq!(
            execution_role(&facts, Span::new(0, 5), &[]),
            ExecutionRole::UntrackedRendering
        );
    }

    #[test]
    fn smallest_compiler_region_wins_across_execution_fact_categories() {
        let mut facts = execution_map();
        facts.untracked_regions.push(ExecutionRegion {
            span: Span::new(0, 100),
            reason: RegionReason::JsxChild,
        });
        facts.callback_roles.push(CallbackRole {
            span: Span::new(40, 60),
            role: CallbackRoleKind::EventHandler,
        });

        assert_eq!(
            execution_role(&facts, Span::new(50, 51), &[]),
            ExecutionRole::EventCallback
        );
    }

    #[test]
    fn conditional_context_does_not_repeat_the_function_name_as_a_return_kind() {
        assert_eq!(
            format_read_context("ConditionalReturn", true),
            "ConditionalReturn while evaluating a condition"
        );
        assert_eq!(
            format_read_context("", true),
            "while evaluating a condition"
        );
    }
}
