//! Invocation premises, independent of owner, tracking and lifetime proofs.
//!
//! Exact browser registrations have feasible host triggers (ADR 0270).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use solid_dialect::{BrowserCallbackTrigger, Dialect};
use solid_facts::ast::{BindingShape, CallFact, FunctionFact, FunctionKind, JsxElementFact};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts};

use crate::contract_semantics::{
    AcceptedContractIndex, CardinalityScope, Event, InvokeProtocol, OperationKind, Schedule,
    Trigger, UpperBound, ValueSource,
};
use crate::indexes::{CachedAstFileIndex, SemanticLookup};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InvocationSite {
    pub path: String,
    pub span: Span,
}

/// Feasibility is a separate premise from a callback's host *if* invoked.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CallbackDelivery {
    /// Exact invocation during the admitted caller/invoker execution.
    SameStack,
    /// A noncancelable queueMicrotask registration (standard runtime premise).
    NoncancelableMicrotask,
    /// Owner-approved existential premise: a reviewed host trigger CAN run.
    FeasibleHostTrigger(BrowserCallbackTrigger),
    /// The reviewed runtime returns the value without registering a trigger.
    NeverRegistered,
    /// The consumer invokes this callback only on a still-unproved trigger.
    Pending(BrowserCallbackTrigger),
}

impl CallbackDelivery {
    #[must_use]
    pub const fn is_feasible(self) -> bool {
        matches!(
            self,
            Self::SameStack | Self::NoncancelableMicrotask | Self::FeasibleHostTrigger(_)
        )
    }
}

/// The AND-premise: the supplying site AND the exact invoker execute in the
/// browser, and delivery is feasible. None of these establishes an owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrowserCallbackInvocation {
    pub supplied: InvocationSite,
    pub invoked: InvocationSite,
    pub target: InvocationSite,
    pub delivery: CallbackDelivery,
    /// Exact same-block cancellation; suppress only if this site is client-live.
    pub cancelled_by: Option<InvocationSite>,
    /// Dialect slot, accepted exact export/operation, local path or host API.
    pub premise: String,
}

fn site(file: &FileFacts, span: Span) -> InvocationSite {
    InvocationSite {
        path: file.path.to_string(),
        span,
    }
}

fn inert_literal(value: &solid_facts::ast::ArgumentLiteralFact) -> bool {
    use solid_facts::ast::ArgumentLiteralFact as Literal;
    match value {
        Literal::Null
        | Literal::Boolean(_)
        | Literal::Integer(_)
        | Literal::String(_)
        | Literal::Function
        | Literal::ArrayLength(0) => true,
        Literal::Object(properties) => properties
            .iter()
            .all(|property| inert_literal(&property.value)),
        _ => false,
    }
}

/// Entering an expression is not completion of its argument evaluation.
/// Unknown calls/getters/coercive inputs cannot prove a callback registration.
fn registration_arguments_complete(
    file: &FileFacts,
    call: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    call.arguments.iter().all(|argument| {
        let closed_objects =
            !matches!(
                &argument.literal_value,
                solid_facts::ast::ArgumentLiteralFact::Object(_)
            ) || (file.ast.object_get_shapes.iter().any(|shape| {
                shape.span == file.ast.peel_ts_sugar_span(argument.span) && shape.closed
            }) && file
                .ast
                .object_get_shapes
                .iter()
                .filter(|shape| {
                    argument.span.contains(shape.span)
                        && !file.ast.functions.iter().any(|function| {
                            argument.span.contains(function.span)
                                && function.span.contains(shape.span)
                        })
                })
                .all(|shape| shape.closed));
        !argument.spread
            && ((closed_objects && inert_literal(&argument.literal_value))
                || file
                    .ast
                    .host_undefined_arguments
                    .contains(&file.ast.peel_ts_sugar_span(argument.span))
                || target(file, argument.span, lookup).is_some_and(|target| {
                    file.ast.functions.iter().any(|function| {
                        function.span == target.span
                            && (function.kind == FunctionKind::Declaration
                                || function.span.end <= argument.span.start)
                    })
                }))
    })
}

fn identifier(file: &FileFacts, span: Span) -> bool {
    file.ast
        .identifiers
        .iter()
        .any(|id| id.span == span && id.role == solid_facts::ast::IdentifierRole::Reference)
}

/// Only an exact literal or an unchanged local lexical function binding.
fn target(file: &FileFacts, span: Span, lookup: &SemanticLookup<'_>) -> Option<InvocationSite> {
    let span = file.ast.peel_ts_sugar_span(span);
    let mut literals = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span == span);
    if let Some(function) = literals.next() {
        return (literals.next().is_none()
            && function.kind != FunctionKind::Declaration
            && function.method_name.is_none()
            && !function.generator
            && !function.rest_parameter
            && function.parameters.iter().all(|parameter| {
                parameter.initializer.is_none() && parameter.shape == BindingShape::Identifier
            }))
        .then(|| site(file, function.span));
    }
    if !identifier(file, span) {
        return None;
    }
    let (definition, function) = lookup.function_called_at(file.path.as_str(), span)?;
    (definition.path == file.path
        && function.method_name.is_none()
        && !function.generator
        && !function.rest_parameter
        && function.parameters.iter().all(|parameter| {
            parameter.initializer.is_none() && parameter.shape == BindingShape::Identifier
        })
        && (function.kind == FunctionKind::Declaration || function.span.end <= span.start)
        && lookup.function_value_is_current(definition, function))
    .then(|| site(definition, function.span))
}

/// Disposal registration needs an owner premise; a function boundary alone
/// supplies none. Reuse exact owner-creating slots and require the immediate
/// callable, so a timer/settlement/await nested inside a root cannot inherit it.
/// Helpers called from an owned context need a context-sensitive owner witness
/// and remain pending here. This does not establish that disposal will happen.
fn disposal_registration_owned(
    file: &FileFacts,
    call: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(function) = crate::owners::containing_ast_function(&file.ast, call.span) else {
        return false;
    };
    if file.ast.awaits.iter().any(|span| {
        function.span.contains(*span)
            && span.end <= call.span.start
            && crate::owners::written_directly_in(&file.ast, function, *span)
    }) {
        return false;
    }
    file.ast.calls.iter().any(|registration| {
        let Some(primitive) = lookup.primitive_at_call(file, registration.span) else {
            return false;
        };
        let Some(index) =
            crate::owners::owner_providing_argument(file, registration, Some(primitive), lookup)
        else {
            return false;
        };
        registration.arguments.get(index).is_some_and(|argument| {
            // Only a directly supplied literal. A reused named callback can
            // have an unrelated unowned browser invocation as well.
            file.ast.peel_ts_sugar_span(argument.span) == function.span
                && target(file, argument.span, lookup).is_some()
                && dialect_slot_runs_inline(lookup, primitive, index, registration.arguments.len())
        })
    })
}

fn dialect_slot_runs_inline(
    lookup: &SemanticLookup<'_>,
    primitive: solid_dialect::Primitive,
    index: usize,
    count: usize,
) -> bool {
    lookup
        .dialect
        .browser_callback_trigger(primitive, index, count)
        == Some(BrowserCallbackTrigger::DuringCall)
}

/// Deliberately tiny, syntax-proved no-demand/no-throw callbacks. A type such as
/// `() => number` is not this proof. Other bodies keep the feasible premise.
fn inert_callback(file: &FileFacts, value: Span, lookup: &SemanticLookup<'_>) -> bool {
    let Some(target) = target(file, value, lookup) else {
        return false;
    };
    file.ast
        .functions
        .iter()
        .find(|function| function.span == target.span)
        .is_some_and(|function| {
            !function.r#async
                && (function
                    .expression_return
                    .as_ref()
                    .and_then(|value| value.argument)
                    .is_some_and(|span| file.ast.host_zero_values.contains(&span))
                    || (!function.expression_body
                        && file
                            .source_text(function.body)
                            .is_some_and(|text| text.trim() == "{}")))
        })
}

fn dialect_trigger_dead(
    file: &FileFacts,
    call: &CallFact,
    primitive: solid_dialect::Primitive,
    index: usize,
    trigger: BrowserCallbackTrigger,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let inert_compute = call
        .arguments
        .first()
        .is_some_and(|argument| inert_callback(file, argument.span, lookup));
    if !inert_compute {
        return false;
    }
    let deferred = lookup
        .dialect
        .browser_callback_defer_options(primitive, index)
        .and_then(|slot| call.arguments.get(slot))
        .is_some_and(|options| match &options.literal_value {
            solid_facts::ast::ArgumentLiteralFact::Object(properties) => {
                properties.iter().any(|property| {
                    property.name == "defer"
                        && property.value == solid_facts::ast::ArgumentLiteralFact::Boolean(true)
                })
            }
            _ => false,
        });
    if deferred {
        return true;
    }
    trigger == BrowserCallbackTrigger::ComputationError
        && call
            .arguments
            .get(index)
            .and_then(|argument| {
                argument
                    .literal_members
                    .iter()
                    .find(|member| member.key == "effect")
            })
            .is_some_and(|member| inert_callback(file, member.value, lookup))
}

#[allow(clippy::too_many_arguments)]
fn emit(
    result: &mut Vec<BrowserCallbackInvocation>,
    file: &FileFacts,
    supplied: Span,
    value: Span,
    invoked: InvocationSite,
    delivery: CallbackDelivery,
    premise: String,
    lookup: &SemanticLookup<'_>,
) {
    if let Some(target) = target(file, value, lookup) {
        result.push(BrowserCallbackInvocation {
            supplied: site(file, supplied),
            invoked,
            target,
            delivery,
            cancelled_by: None,
            premise,
        });
    }
}

/// Direct parameter calls; no closures, rest, defaults, writes or member guessing.
/// Live-branch facts subsequently classify the exact invoker site; a guard is
/// never erased by this bounded summary.
fn parameter_calls<'a>(
    file: &'a FileFacts,
    function: &FunctionFact,
    index: usize,
    path: &[String],
    lookup: &SemanticLookup<'_>,
) -> Vec<&'a solid_facts::ast::CallFact> {
    if function.r#async
        || function.generator
        || function.rest_parameter
        || function.method_name.is_some()
    {
        return Vec::new();
    }
    let Some(parameter) = function.parameters.get(index) else {
        return Vec::new();
    };
    if parameter.shape != BindingShape::Identifier
        || parameter.initializer.is_some()
        || parameter.names.len() != 1
    {
        return Vec::new();
    }
    let name = parameter.names[0].span;
    let Some(symbol) = lookup.entities().at(file.path.as_str(), name) else {
        return Vec::new();
    };
    if crate::indexes::binding_written(file, name)
        || file.ast.assignments.iter().any(|assignment| {
            function.span.contains(assignment.target)
                && file.ast.members.iter().any(|member| {
                    member.span == file.ast.peel_ts_sugar_span(assignment.target)
                        && lookup.entities().at(file.path.as_str(), member.object) == Some(symbol)
                })
        })
    {
        return Vec::new();
    }
    // A props object that escapes can be mutated by opaque code before the
    // apparent member call. A callable parameter that escapes is likewise not
    // an invocation premise of this bounded summary.
    if file
        .ast
        .identifiers
        .iter()
        .filter(|id| {
            id.role == solid_facts::ast::IdentifierRole::Reference
                && function.span.contains(id.span)
                && lookup.entities().at(file.path.as_str(), id.span) == Some(symbol)
        })
        .any(|id| match path {
            [] => !file
                .ast
                .calls
                .iter()
                .any(|call| file.ast.peel_ts_sugar_span(call.callee) == id.span),
            [key] => !file.ast.members.iter().any(|member| {
                member.object == id.span
                    && file.source_text(member.property) == Some(key.as_str())
                    && file
                        .ast
                        .calls
                        .iter()
                        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
            }),
            _ => true,
        })
    {
        return Vec::new();
    }
    file.ast
        .calls
        .iter()
        .filter(|call| {
            if call.construct
                || !crate::owners::written_directly_in(&file.ast, function, call.span)
                || file.ast.any_jsx_containing(call.span)
            {
                return false;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            match path {
                [] => {
                    identifier(file, callee)
                        && lookup.entities().at(file.path.as_str(), callee) == Some(symbol)
                }
                [key] => file.ast.members.iter().any(|member| {
                    member.span == callee
                        && lookup.entities().at(file.path.as_str(), member.object) == Some(symbol)
                        && file.source_text(member.property) == Some(key.as_str())
                }),
                _ => false,
            }
        })
        .collect()
}

fn jsx_child_value(file: &FileFacts, child: Span) -> Option<Span> {
    // Only remove the syntax-proven JSX expression container, not arbitrary braces.
    let text = file.source_text(child)?;
    if text.starts_with('{') && text.ends_with('}') {
        let inside = &text[1..text.len() - 1];
        let left = inside.len() - inside.trim_start().len();
        let right = inside.len() - inside.trim_end().len();
        Some(Span::new(
            child.start + 1 + u32::try_from(left).ok()?,
            child.end - 1 - u32::try_from(right).ok()?,
        ))
    } else {
        None
    }
}

fn jsx_prop_value(file: &FileFacts, element: &JsxElementFact, prop: &str) -> Option<Span> {
    if !element.spreads.is_empty() {
        return None;
    }
    let mut attributes = element.attributes.iter().filter(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.name) == Some(prop)
    });
    if let Some(attribute) = attributes.next() {
        if attributes.next().is_some() || (prop == "children" && !element.children.is_empty()) {
            return None;
        }
        return attribute.expression;
    }
    if prop != "children" {
        return None;
    }
    let [child] = element.children.as_slice() else {
        return None;
    };
    jsx_child_value(file, *child)
}

fn dead_control_flow(
    file: &FileFacts,
    element: &JsxElementFact,
    primitive: solid_dialect::Primitive,
    dialect: &dyn Dialect,
) -> bool {
    dialect.browser_children_dead(
        primitive,
        jsx_prop_value(file, element, "when")
            .map(|span| file.ast.peel_ts_sugar_span(span))
            .and_then(|span| file.source_text(span))
            == Some("false"),
        jsx_prop_value(file, element, "each").is_some_and(|span| {
            file.ast
                .host_empty_arrays
                .contains(&file.ast.peel_ts_sugar_span(span))
        }),
        jsx_prop_value(file, element, "count").is_some_and(|span| {
            file.ast
                .host_zero_values
                .contains(&file.ast.peel_ts_sugar_span(span))
        }),
    )
}

fn host_event(event: Event) -> bool {
    matches!(
        event,
        Event::Call
            | Event::Render
            | Event::Flush
            | Event::Settle
            | Event::Cleanup
            | Event::External
            | Event::Transition
            | Event::AsyncEmission
    )
}

/// Exact binding selection stays in the existing contract resolver. This narrow
/// reader accepts unguarded rows only; it does not convert unknown guards to true.
fn package_rows(
    file: &FileFacts,
    symbol: &crate::SymbolId,
    contracts: &AcceptedContractIndex,
    lookup: &SemanticLookup<'_>,
) -> Vec<(usize, Vec<String>, CallbackDelivery, String)> {
    if !lookup.has_contract_binding(symbol) {
        return Vec::new();
    }
    let mut result = Vec::new();
    for import in file.ast.imports.iter().filter(|import| !import.type_only) {
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            if lookup.entities().at(file.path.as_str(), binding.local.span) != Some(symbol) {
                continue;
            }
            let Some(name) = binding.imported.as_deref().or_else(|| {
                (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
            }) else {
                continue;
            };
            let Ok(accepted) = contracts.resolve_name(file.path.as_str(), &import.module, name)
            else {
                continue;
            };
            let export = accepted.export();
            if !export.call.context_premises().is_empty()
                || !export.call.guards.cases.items().is_empty()
            {
                continue;
            }
            for callback in export.callbacks().items() {
                let ValueSource::Parameter { index, path } = &callback.from else {
                    continue;
                };
                let Some(operation) = export.operation(&callback.operation.0) else {
                    continue;
                };
                if operation.kind != OperationKind::Invoke
                    || operation.invoke_protocol() != InvokeProtocol::Call
                    || operation.guard.is_some()
                    || operation.cardinality.max == Some(UpperBound::Finite(0))
                {
                    continue;
                }
                let guaranteed_inline = operation.trigger == Some(Trigger::Event(Event::Call))
                    && operation.at == Some(Event::Call)
                    && operation.schedule == Some(Schedule::SameStack)
                    && operation.cardinality.scope == Some(CardinalityScope::Call)
                    && operation.cardinality.min.is_some_and(|min| min > 0);
                result.push((
                    usize::from(*index),
                    path.clone(),
                    if guaranteed_inline {
                        CallbackDelivery::SameStack
                    } else {
                        let trigger_event = match &operation.trigger {
                            Some(Trigger::Event(event) | Trigger::Resource { event, .. }) => {
                                Some(*event)
                            }
                            _ => None,
                        };
                        if trigger_event.is_some_and(host_event)
                            && operation.at.is_some_and(host_event)
                            && operation.schedule.is_some()
                        {
                            CallbackDelivery::FeasibleHostTrigger(
                                BrowserCallbackTrigger::ContractTrigger,
                            )
                        } else {
                            CallbackDelivery::Pending(BrowserCallbackTrigger::ContractTrigger)
                        }
                    },
                    format!(
                        "{}:{}:{}:{}",
                        import.module,
                        name,
                        accepted.contract().receipt().semantic_digest.as_str(),
                        callback.operation.0
                    ),
                ));
            }
        }
    }
    result
}

fn standard_name<'a>(
    file: &FileFacts,
    call: &CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    let resolved = lookup.resolved_callee_call(file, call.callee)?;
    let declaration = resolved.declaration.as_ref()?;
    (resolved.validity == typefacts::ResolvedCallValidity::Valid && declaration.standard_library)
        .then_some(declaration.qualified_name.as_ref())
}

fn same_receiver(
    file: &FileFacts,
    first: &CallFact,
    second: &CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let receiver = |call: &CallFact| {
        file.ast
            .members
            .iter()
            .find(|member| member.span == file.ast.peel_ts_sugar_span(call.callee))
            .map(|member| member.object)
    };
    match (receiver(first), receiver(second)) {
        (None, None) => {
            identifier(file, file.ast.peel_ts_sugar_span(first.callee))
                && identifier(file, file.ast.peel_ts_sugar_span(second.callee))
        }
        (Some(first), Some(second)) if identifier(file, first) && identifier(file, second) => {
            let Some(symbol) = lookup.entities().at(file.path.as_str(), first) else {
                return false;
            };
            lookup.entities().at(file.path.as_str(), second) == Some(symbol)
                && !file
                    .ast
                    .reference_declarations
                    .iter()
                    .any(|(reference, declaration)| {
                        *reference == first && crate::indexes::binding_written(file, *declaration)
                    })
        }
        _ => false,
    }
}

/// Only exact same-block cancellation with inert intervening statements proves
/// non-execution. Neither
/// absence of cancellation nor cancellation in an opaque helper is negative proof.
fn synchronous_cancel(
    file: &FileFacts,
    registration: Span,
    trigger: BrowserCallbackTrigger,
    lookup: &SemanticLookup<'_>,
) -> Option<InvocationSite> {
    let registered = file
        .ast
        .calls
        .iter()
        .find(|call| call.span == registration)?;
    for pair in file
        .ast
        .host_call_sequences
        .iter()
        .filter(|pair| pair.first == registration)
    {
        let cancelled = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == pair.second)?;
        if cancelled.arguments.iter().any(|argument| argument.spread)
            || !same_receiver(file, registered, cancelled, lookup)
        {
            continue;
        }
        let Some(name) = standard_name(file, cancelled, lookup) else {
            continue;
        };
        match trigger {
            BrowserCallbackTrigger::TimerTask
            | BrowserCallbackTrigger::AnimationFrame
            | BrowserCallbackTrigger::IdleTask => {
                let matched = match trigger {
                    BrowserCallbackTrigger::TimerTask => matches!(
                        name,
                        "clearTimeout"
                            | "WindowOrWorkerGlobalScope.clearTimeout"
                            | "clearInterval"
                            | "WindowOrWorkerGlobalScope.clearInterval"
                    ),
                    BrowserCallbackTrigger::IdleTask => {
                        matches!(name, "cancelIdleCallback" | "Window.cancelIdleCallback")
                    }
                    _ => matches!(
                        name,
                        "cancelAnimationFrame" | "AnimationFrameProvider.cancelAnimationFrame"
                    ),
                };
                if !matched || cancelled.arguments.len() != 1 {
                    continue;
                }
                let argument = file.ast.peel_ts_sugar_span(cancelled.arguments[0].span);
                if argument == registration {
                    return Some(site(file, cancelled.span));
                }
                let Some(binding) = file.ast.bindings.iter().find(|binding| {
                    binding.immutable
                        && binding.shape == BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding
                            .initializer
                            .is_some_and(|span| file.ast.peel_ts_sugar_span(span) == registration)
                }) else {
                    continue;
                };
                let declaration = binding.names[0].span;
                if !crate::indexes::binding_written(file, declaration)
                    && file
                        .ast
                        .reference_declarations
                        .contains(&(argument, declaration))
                    && lookup
                        .entities()
                        .at(file.path.as_str(), declaration)
                        .is_some()
                    && lookup.entities().at(file.path.as_str(), argument)
                        == lookup.entities().at(file.path.as_str(), declaration)
                {
                    return Some(site(file, cancelled.span));
                }
            }
            BrowserCallbackTrigger::DomEvent => {
                if !matches!(
                    name,
                    "removeEventListener"
                        | "EventTarget.removeEventListener"
                        | "Window.removeEventListener"
                ) || !(2..=3).contains(&registered.arguments.len())
                    || !(2..=3).contains(&cancelled.arguments.len())
                {
                    continue;
                }
                let capture = |call: &CallFact| match call
                    .arguments
                    .get(2)
                    .map(|argument| &argument.literal_value)
                {
                    None => Some(false),
                    Some(solid_facts::ast::ArgumentLiteralFact::Boolean(value)) => Some(*value),
                    Some(solid_facts::ast::ArgumentLiteralFact::Object(properties)) => properties
                        .iter()
                        .find(|property| property.name == "capture")
                        .and_then(|property| match &property.value {
                            solid_facts::ast::ArgumentLiteralFact::Boolean(value) => Some(*value),
                            _ => None,
                        }),
                    _ => None,
                };
                if capture(registered).is_none() || capture(registered) != capture(cancelled) {
                    continue;
                }
                let first = &registered.arguments[0].literal_value;
                let second = &cancelled.arguments[0].literal_value;
                if !matches!(first, solid_facts::ast::ArgumentLiteralFact::String(_))
                    || first != second
                {
                    continue;
                }
                if target(file, registered.arguments[1].span, lookup).is_some()
                    && target(file, registered.arguments[1].span, lookup)
                        == target(file, cancelled.arguments[1].span, lookup)
                {
                    return Some(site(file, cancelled.span));
                }
            }
            _ => {}
        }
    }
    None
}

/// Collect invocation premises; reachability is applied afterwards as an AND
/// over supplying and invoking sites, never from mere containment.
#[must_use]
pub fn browser_callback_invocations(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    contracts: &AcceptedContractIndex,
) -> Vec<BrowserCallbackInvocation> {
    let (indexes, _, _) = crate::cache::build_typescript_indexes(
        &facts.typescript,
        dialect,
        facts.files.len(),
        &facts.runtime_symbol_redirects,
    );
    let mut names = indexes.symbol_names.clone();
    crate::symbols::add_solid_import_names(facts, &indexes.entities, dialect, &mut names);
    let ast = facts
        .files
        .iter()
        .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
        .collect::<HashMap<_, _>>();
    let runtime = crate::runtime_configuration::scan(facts, dialect);
    if !runtime.permits_proof() {
        return Vec::new();
    }
    let resolved = crate::contracts::resolve_accepted_contract_imports(
        facts,
        contracts,
        &indexes.entities,
        &names,
        dialect,
        &runtime,
    );
    let lookup = SemanticLookup::new(
        facts,
        &ast,
        &indexes.entities,
        &names,
        dialect,
        &resolved,
        false,
    );
    let mut result = Vec::new();
    for file in &facts.files {
        let dead_children = file
            .ast
            .jsx_elements
            .iter()
            .filter_map(|element| {
                let primitive = crate::exact_jsx_primitive_name(
                    file,
                    element,
                    &indexes.entities,
                    &names,
                    dialect,
                )?
                .primitive()?;
                dead_control_flow(file, element, primitive, dialect).then_some(element.span)
            })
            .collect::<Vec<_>>();
        for call in file.ast.calls.iter().filter(|call| {
            !call.construct
                && registration_arguments_complete(file, call, &lookup)
                && !dead_children.iter().any(|span| span.contains(call.span))
        }) {
            if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
                for (index, argument) in call.arguments.iter().enumerate() {
                    if let Some(trigger) =
                        dialect.browser_callback_trigger(primitive, index, call.arguments.len())
                    {
                        emit(
                            &mut result,
                            file,
                            call.span,
                            argument.span,
                            site(file, call.span),
                            if trigger == BrowserCallbackTrigger::DuringCall {
                                CallbackDelivery::SameStack
                            } else if dialect_trigger_dead(
                                file, call, primitive, index, trigger, &lookup,
                            ) {
                                CallbackDelivery::Pending(trigger)
                            } else if trigger == BrowserCallbackTrigger::ComputationDemand {
                                // Options may request a lazy memo whose returned accessor
                                // is never demanded. Registration alone is not that demand.
                                CallbackDelivery::Pending(trigger)
                            } else if trigger == BrowserCallbackTrigger::OwnerDisposal
                                && !disposal_registration_owned(file, call, &lookup)
                            {
                                // No positive registration premise. This covers
                                // known unowned helpers/fresh-stack callbacks too.
                                CallbackDelivery::Pending(trigger)
                            } else {
                                CallbackDelivery::FeasibleHostTrigger(trigger)
                            },
                            format!("dialect:{primitive:?}:{index}"),
                            &lookup,
                        );
                    }
                    if argument.exact_object_literal {
                        for member in &argument.literal_members {
                            if let Some(trigger) = dialect.browser_callback_member_trigger(
                                primitive,
                                index,
                                call.arguments.len(),
                                member.key.as_str(),
                            ) {
                                emit(
                                    &mut result,
                                    file,
                                    call.span,
                                    member.value,
                                    site(file, call.span),
                                    if dialect_trigger_dead(
                                        file, call, primitive, index, trigger, &lookup,
                                    ) {
                                        CallbackDelivery::Pending(trigger)
                                    } else {
                                        CallbackDelivery::FeasibleHostTrigger(trigger)
                                    },
                                    format!("dialect-member:{primitive:?}:{index}:{}", member.key),
                                    &lookup,
                                );
                            }
                        }
                    }
                }
            }
            if let Some(symbol) = lookup.callee_symbol(file, call.callee) {
                for (index, path, delivery, premise) in
                    package_rows(file, &crate::SymbolId::from(symbol), contracts, &lookup)
                {
                    let Some(argument) = call.arguments.get(index) else {
                        continue;
                    };
                    let value = match path.as_slice() {
                        [] => Some(argument.span),
                        [key] => argument
                            .literal_members
                            .iter()
                            .find(|member| member.key.as_str() == key)
                            .map(|member| member.value),
                        _ => None,
                    };
                    if let Some(value) = value {
                        emit(
                            &mut result,
                            file,
                            call.span,
                            value,
                            site(file, call.span),
                            delivery,
                            premise,
                            &lookup,
                        );
                    }
                }
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if identifier(file, callee)
                && let Some((definition, function)) =
                    lookup.function_called_at(file.path.as_str(), callee)
                && definition.path == file.path
                && lookup.function_value_is_current(definition, function)
            {
                for (index, argument) in call.arguments.iter().enumerate() {
                    for invocation in parameter_calls(definition, function, index, &[], &lookup) {
                        emit(
                            &mut result,
                            file,
                            call.span,
                            argument.span,
                            site(definition, invocation.span),
                            CallbackDelivery::SameStack,
                            format!("local-parameter:{index}"),
                            &lookup,
                        );
                    }
                }
            }
            if let Some(resolved) = lookup.resolved_callee_call(file, call.callee)
                && let Some(declaration) = resolved.declaration.as_ref()
                && declaration.standard_library
            {
                let name = declaration.qualified_name.as_ref();
                // Exact producer signature and argument mapping, not source names.
                for (index, argument) in call.arguments.iter().enumerate() {
                    let callability = lookup
                        .entity_at(file.path.as_str(), argument.span)
                        .and_then(|entity| entity.callability);
                    if crate::runtime_semantics::argument_behavior(resolved, callability, index)
                        .is_none()
                    {
                        continue;
                    }
                    let delivery = match (name, index) {
                        ("queueMicrotask" | "WindowOrWorkerGlobalScope.queueMicrotask", 0) => {
                            CallbackDelivery::NoncancelableMicrotask
                        }
                        (
                            "setTimeout"
                            | "WindowOrWorkerGlobalScope.setTimeout"
                            | "setInterval"
                            | "WindowOrWorkerGlobalScope.setInterval",
                            0,
                        ) => {
                            CallbackDelivery::FeasibleHostTrigger(BrowserCallbackTrigger::TimerTask)
                        }
                        ("requestIdleCallback" | "Window.requestIdleCallback", 0) => {
                            CallbackDelivery::FeasibleHostTrigger(BrowserCallbackTrigger::IdleTask)
                        }
                        (
                            "requestAnimationFrame"
                            | "AnimationFrameProvider.requestAnimationFrame",
                            0,
                        ) => CallbackDelivery::FeasibleHostTrigger(
                            BrowserCallbackTrigger::AnimationFrame,
                        ),
                        (
                            "addEventListener"
                            | "EventTarget.addEventListener"
                            | "Window.addEventListener",
                            1,
                        ) => {
                            CallbackDelivery::FeasibleHostTrigger(BrowserCallbackTrigger::DomEvent)
                        }
                        _ => continue,
                    };
                    emit(
                        &mut result,
                        file,
                        call.span,
                        argument.span,
                        site(file, call.span),
                        delivery,
                        format!("host:{name}:{index}"),
                        &lookup,
                    );
                }
            }
        }
        for element in &file.ast.jsx_elements {
            if dead_children.iter().any(|span| span.contains(element.span)) {
                continue;
            }
            let intrinsic = element.member_object.is_none()
                && file
                    .source_text(element.name.span)
                    .and_then(|name| name.chars().next())
                    .is_some_and(|first| first.is_ascii_lowercase());
            if intrinsic && element.spreads.is_empty() {
                for attribute in &element.attributes {
                    let Some(value) = attribute.expression else {
                        continue;
                    };
                    if element
                        .attributes
                        .iter()
                        .filter(|other| {
                            other.namespace == attribute.namespace
                                && file.source_text(other.name) == file.source_text(attribute.name)
                        })
                        .count()
                        != 1
                    {
                        continue;
                    }
                    if file.compiler.callback_roles.iter().any(|role| {
                        role.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                            && (role.span == value || role.span == attribute.span)
                    }) {
                        emit(
                            &mut result,
                            file,
                            element.span,
                            value,
                            site(file, element.span),
                            CallbackDelivery::FeasibleHostTrigger(BrowserCallbackTrigger::DomEvent),
                            "compiler:intrinsic-event".into(),
                            &lookup,
                        );
                    }
                }
            }
            if let Some(primitive) =
                crate::exact_jsx_primitive_name(file, element, &indexes.entities, &names, dialect)
                    .and_then(|name| name.primitive())
                && let Some(trigger) = dialect.browser_children_trigger(primitive)
                && !dialect.browser_children_dead(
                    primitive,
                    jsx_prop_value(file, element, "when")
                        .map(|span| file.ast.peel_ts_sugar_span(span))
                        .and_then(|span| file.source_text(span))
                        == Some("false"),
                    jsx_prop_value(file, element, "each").is_some_and(|span| {
                        file.ast
                            .host_empty_arrays
                            .contains(&file.ast.peel_ts_sugar_span(span))
                    }),
                    jsx_prop_value(file, element, "count").is_some_and(|span| {
                        file.ast
                            .host_zero_values
                            .contains(&file.ast.peel_ts_sugar_span(span))
                    }),
                )
                && let Some(value) = jsx_prop_value(file, element, "children")
            {
                emit(
                    &mut result,
                    file,
                    element.span,
                    value,
                    site(file, element.span),
                    CallbackDelivery::FeasibleHostTrigger(trigger),
                    format!("dialect-children:{primitive:?}"),
                    &lookup,
                );
            }
            if element.member_object.is_some() || !element.spreads.is_empty() {
                continue;
            }
            let Some(symbol) = lookup.entities().at(file.path.as_str(), element.name.span) else {
                continue;
            };
            for (index, path, delivery, premise) in package_rows(file, symbol, contracts, &lookup) {
                if index != 0 {
                    continue;
                }
                let [prop] = path.as_slice() else {
                    continue;
                };
                if let Some(value) = jsx_prop_value(file, element, prop) {
                    emit(
                        &mut result,
                        file,
                        element.span,
                        value,
                        site(file, element.span),
                        delivery,
                        premise,
                        &lookup,
                    );
                }
            }
            if let Some((definition, component)) =
                lookup.function_called_at(file.path.as_str(), element.name.span)
                && definition.path == file.path
                && lookup.function_value_is_current(definition, component)
            {
                let props = element
                    .attributes
                    .iter()
                    .filter(|attribute| attribute.namespace.is_none())
                    .filter_map(|attribute| file.source_text(attribute.name))
                    .chain(std::iter::once("children"));
                for prop in props {
                    let Some(value) = jsx_prop_value(file, element, prop) else {
                        continue;
                    };
                    for invocation in
                        parameter_calls(definition, component, 0, &[prop.into()], &lookup)
                    {
                        emit(
                            &mut result,
                            file,
                            element.span,
                            value,
                            site(definition, invocation.span),
                            CallbackDelivery::SameStack,
                            format!("local-prop:{prop}"),
                            &lookup,
                        );
                    }
                }
            }
        }
    }
    for fact in &mut result {
        if let CallbackDelivery::FeasibleHostTrigger(trigger) = fact.delivery
            && let Some(file) = facts
                .files
                .iter()
                .find(|file| file.path.as_str() == fact.supplied.path)
        {
            fact.cancelled_by = synchronous_cancel(file, fact.supplied.span, trigger, &lookup);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_invocations(source: &str, path: &[String]) -> usize {
        use solid_facts::core::Generation;
        use std::collections::HashSet;
        let ast = solid_facts::ast::extract("case.tsx", source).unwrap();
        let compiler = solid_facts::compiler::ExecutionMap {
            compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        let generation = Generation::new(1).unwrap();
        let file = FileFacts::new(generation, source, ast, compiler).unwrap();
        let function = &file.ast.functions[0];
        let name = function.parameters[0].names[0].span;
        let locations = std::iter::once(name).chain(
            file.ast
                .reference_declarations
                .iter()
                .filter(|(_, declaration)| *declaration == name)
                .map(|(reference, _)| *reference),
        );
        let entities = crate::indexes::EntitySymbols {
            by_path: HashMap::from([(
                "case.tsx".into(),
                locations
                    .map(|span| {
                        (
                            (u64::from(span.start), u64::from(span.end)),
                            crate::SymbolId::from("parameter"),
                        )
                    })
                    .collect(),
            )]),
        };
        let facts = ProjectFacts {
            generation,
            project_id: "callback-host".into(),
            files: vec![file],
            typescript: solid_facts::TypeScriptTable::from_parts(
                3,
                1,
                "callback-host",
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: HashMap::new(),
        };
        let contracts = crate::contracts::ResolvedContracts {
            bindings: vec![],
            by_symbol: HashMap::new(),
            direct_returns: HashMap::new(),
            callee_bindings: HashMap::new(),
            returned_callable_bindings: HashSet::new(),
            missing_exports: vec![],
            counts: crate::ContractBindingCounts::default(),
        };
        let ast = HashMap::new();
        let names = HashMap::new();
        let lookup = SemanticLookup::new(
            &facts,
            &ast,
            &entities,
            &names,
            &solid_dialect::Solid2,
            &contracts,
            false,
        );
        parameter_calls(
            &facts.files[0],
            &facts.files[0].ast.functions[0],
            0,
            path,
            &lookup,
        )
        .len()
    }

    #[test]
    fn exact_local_prop_paths_record_invokers_without_guessing_delivery() {
        let path = ["content".into()];
        assert_eq!(
            local_invocations("function C(props) { return props.content(); }", &path),
            1
        );
        for source in [
            "function C(props) { return null; }",
            "function C(props) { opaque(props); return props.content(); }",
            "function C(props) { props.content = other; return props.content(); }",
            "function C(props) { return () => props.content(); }",
            "function C(props) { return props[key](); }",
        ] {
            assert_eq!(local_invocations(source, &path), 0, "{source}");
        }
        assert_eq!(
            local_invocations("function run(callback) { callback(); }", &[]),
            1
        );
        // Candidates retain their exact invoking site. The host graph rejects
        // client-dead/unknown predicates; collecting a candidate grants no edge.
        for source in [
            "function C(props) { if (import.meta.env.SSR) props.content(); }",
            "function C(props) { return props.content?.(); }",
        ] {
            assert_eq!(local_invocations(source, &path), 1, "{source}");
        }
        assert_eq!(
            local_invocations(
                "function run(callback) { callback = other; callback(); }",
                &[]
            ),
            0
        );
    }

    #[test]
    fn retention_and_optional_invocation_are_not_feasibility() {
        assert!(CallbackDelivery::SameStack.is_feasible());
        assert!(CallbackDelivery::NoncancelableMicrotask.is_feasible());
        assert!(
            CallbackDelivery::FeasibleHostTrigger(BrowserCallbackTrigger::TimerTask).is_feasible()
        );
        for trigger in [
            BrowserCallbackTrigger::TimerTask,
            BrowserCallbackTrigger::DomEvent,
            BrowserCallbackTrigger::OwnerDisposal,
            BrowserCallbackTrigger::RenderSelection,
            BrowserCallbackTrigger::ContractTrigger,
        ] {
            assert!(!CallbackDelivery::Pending(trigger).is_feasible());
        }
    }
}
