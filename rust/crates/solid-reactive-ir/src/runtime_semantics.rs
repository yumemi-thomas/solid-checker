//! Audited ECMAScript and Web-runtime argument behavior.
//!
//! Runtime behavior is selected from the compiler-resolved signature and its
//! argument-to-parameter mapping. Source spelling, rendered types, and member
//! lookup are deliberately not inputs: a shadowed or structurally similar API
//! must remain unknown.

use typefacts::{
    ArgumentMappingStatus, CallKind, Callability, ParameterFact, ResolvedCall, ResolvedCallValidity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeArgumentBehavior {
    /// The argument is invoked before the runtime call returns.
    InlineCallback,
    /// The argument may be retained and invoked after the runtime call returns,
    /// on a stack this table does not establish.
    ///
    /// "After the call returns" is not "on a fresh stack", and the members
    /// that stay here are the ones where the difference is observable:
    /// `addEventListener` (a synchronous `dispatchEvent` or `el.click()` runs
    /// the listener on the dispatcher's stack, inside whatever computation made
    /// that call), `Function.prototype.bind`'s bound arguments (the bound
    /// function is called by whoever holds it), `PromiseLike.then` (any
    /// thenable, which may call back synchronously) and the Geolocation pair
    /// (both methods "call back with error" synchronously when the document is
    /// not fully active). A caller's listener may still be current when such a
    /// callback runs, so no clearing is claimed for it.
    DeferredCallback,
    /// The argument is handed to a host task or microtask queue and invoked
    /// from it: after the runtime call returns, and on an otherwise empty
    /// JavaScript execution-context stack, so no listener of any caller can be
    /// current when it runs. See [`FRESH_STACK_SCHEDULERS`].
    FreshStackCallback,
    /// Storage exposes the value to later code without proving invocation.
    RetainedValue,
    /// The argument value may be read, copied, or retained, but is not invoked.
    ValueOnly,
}

/// The reviewed host schedulers that invoke a callback argument only from a
/// task or microtask queue, never on the scheduling call's own stack or on a
/// stack another caller can enter synchronously. Each is a standard-library
/// declaration the table below matches by its compiler-selected qualified
/// name, never by spelling:
///
/// - `queueMicrotask`, `Promise.then`/`catch`/`finally` -- microtasks (HTML
///   "queue a microtask"; ECMA-262 `HostEnqueuePromiseJob`), which run only
///   when the execution-context stack is empty;
/// - `setTimeout`, `setInterval` -- HTML timer initialization steps, which
///   queue a global task;
/// - `requestAnimationFrame`, `requestIdleCallback` -- run from the event
///   loop's "update the rendering" and idle-period steps;
/// - the same four and `queueMicrotask` read as members
///   (`window.setTimeout`), whose compiler-selected declarations are the
///   `WindowOrWorkerGlobalScope`, `AnimationFrameProvider` and `Window`
///   members the global functions are bound to;
/// - `Scheduler.postTask` -- queues a scheduler task;
/// - the `IntersectionObserver`, `ResizeObserver`, `MutationObserver`,
///   `PerformanceObserver` and `ReportingObserver` constructors -- their
///   callbacks are delivered by a queued task or microtask; `takeRecords()`
///   returns records without invoking the callback.
///
/// This is the list [`RuntimeArgumentBehavior::FreshStackCallback`] answers
/// for, and the only host evidence that lets a package contract say a
/// `deferred` callback runs `untracked`. `PromiseLike.then`, `addEventListener`,
/// `Function.prototype.bind` and Geolocation are deliberately absent; see
/// [`RuntimeArgumentBehavior::DeferredCallback`].
pub(super) const FRESH_STACK_SCHEDULERS: &[&str] = &[
    "queueMicrotask",
    "setTimeout",
    "setInterval",
    "requestAnimationFrame",
    "requestIdleCallback",
    "WindowOrWorkerGlobalScope.queueMicrotask",
    "WindowOrWorkerGlobalScope.setTimeout",
    "WindowOrWorkerGlobalScope.setInterval",
    "AnimationFrameProvider.requestAnimationFrame",
    "Window.requestIdleCallback",
    "Promise.then",
    "Promise.catch",
    "Promise.finally",
    "Scheduler.postTask",
    "IntersectionObserver.construct",
    "ResizeObserver.construct",
    "MutationObserver.construct",
    "PerformanceObserver.construct",
    "ReportingObserver.construct",
];

impl RuntimeArgumentBehavior {
    /// Whether the callback, when it runs, is proven to run with no caller's
    /// listener current -- the one fact a `deferred` contract row needs before
    /// it may say `untracked`.
    pub(super) const fn runs_on_fresh_stack(self) -> bool {
        matches!(self, Self::FreshStackCallback)
    }
}

/// The audited behavior of `argument` at `call`, with the fresh-stack subset of
/// the deferring schedulers split out.
///
/// The table in [`timing_behavior`] answers *when* the argument runs; whether a
/// deferred one runs on a fresh stack is a second, separately reviewed fact,
/// and [`FRESH_STACK_SCHEDULERS`] is its only source. Keeping the list apart
/// from the timing arms means a scheduler added to the table is `deferred` and
/// `ambient-at-execution` until someone reviews its stack, rather than
/// inheriting a clearing claim from the arm it was pasted beside.
pub(super) fn argument_behavior(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
    argument: usize,
) -> Option<RuntimeArgumentBehavior> {
    let behavior = timing_behavior(call, actual_callability, argument)?;
    let fresh_stack = behavior == RuntimeArgumentBehavior::DeferredCallback
        && call.declaration.as_ref().is_some_and(|declaration| {
            declaration.standard_library
                && FRESH_STACK_SCHEDULERS.contains(&declaration.qualified_name.as_ref())
        });
    Some(if fresh_stack {
        RuntimeArgumentBehavior::FreshStackCallback
    } else {
        behavior
    })
}

/// Whether the host may invoke `argument` on its *invoker's* stack: after the
/// runtime call returns, but synchronously inside whatever code later hands
/// control to the host, which may be the very computation that registered it.
///
/// This is the non-fresh-stack remainder of
/// [`RuntimeArgumentBehavior::DeferredCallback`] -- `addEventListener`'s
/// listener (a synchronous `dispatchEvent` or `el.click()`), a
/// `Function.prototype.bind` bound argument (the bound function is called by
/// whoever holds it), a `PromiseLike.then` callback (a thenable may call back
/// synchronously) and the Geolocation callbacks (the specification's
/// "call back with error" for a document that is not fully active runs inside
/// `getCurrentPosition`/`watchPosition`) -- plus the listener slot of every
/// other default-library `addEventListener` declaration.
///
/// The timing table matches only the `EventTarget` and `Window`
/// declarations, but `EventTarget` declares the method and every DOM subtype
/// redeclares it with a narrower event map (`HTMLElement.addEventListener`,
/// `Document.addEventListener`, ...). The producer's
/// `defaultLibraryMemberInvokers` table already relies on the fact audited at
/// the pinned typescript-go revision: every declaration of that name in the
/// bundled default library is the same `EventTarget` registration whose
/// argument 1 is the listener. That fact is used here for one purpose only,
/// to withhold a claim about *when* the listener runs; it does not widen the
/// timing table, whose rows also feed package contracts.
pub(super) fn runs_on_invoker_stack(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
    argument: usize,
) -> bool {
    match argument_behavior(call, actual_callability, argument) {
        Some(RuntimeArgumentBehavior::DeferredCallback) => true,
        Some(_) => false,
        None => default_library_listener_slot(call, actual_callability, argument),
    }
}

/// Argument 1 of a default-library `addEventListener` declaration, whatever
/// DOM interface redeclares it. See [`runs_on_invoker_stack`].
fn default_library_listener_slot(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
    argument: usize,
) -> bool {
    call.validity == ResolvedCallValidity::Valid
        && call.kind == CallKind::Call
        && argument == 1
        && potentially_callable(actual_callability)
        && resolved_parameter(call, argument).is_some()
        && call.declaration.as_ref().is_some_and(|declaration| {
            declaration.standard_library
                && declaration
                    .qualified_name
                    .rsplit_once('.')
                    .is_some_and(|(owner, member)| {
                        !owner.is_empty() && member == "addEventListener"
                    })
        })
}

fn timing_behavior(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
    argument: usize,
) -> Option<RuntimeArgumentBehavior> {
    if call.validity != ResolvedCallValidity::Valid {
        return None;
    }
    let parameter = resolved_parameter(call, argument)?;
    let declaration = call.declaration.as_ref()?;
    let argument_callable = potentially_callable(actual_callability);
    if declaration.standard_library {
        let known_callback = match declaration.qualified_name.as_ref() {
            "queueMicrotask" | "WindowOrWorkerGlobalScope.queueMicrotask"
                if argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "setTimeout"
            | "setInterval"
            | "requestAnimationFrame"
            | "requestIdleCallback"
            | "WindowOrWorkerGlobalScope.setTimeout"
            | "WindowOrWorkerGlobalScope.setInterval"
            | "AnimationFrameProvider.requestAnimationFrame"
            | "Window.requestIdleCallback"
                if argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "Window.addEventListener" | "EventTarget.addEventListener"
                if call.kind == CallKind::Call && argument == 1 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "Promise.then" | "PromiseLike.then"
                if call.kind == CallKind::Call && argument <= 1 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "Promise.catch" | "Promise.finally"
                if call.kind == CallKind::Call && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "Array.forEach"
            | "ReadonlyArray.forEach"
            | "Set.forEach"
            | "Map.forEach"
            | "Array.map"
            | "ReadonlyArray.map"
            | "Array.flatMap"
            | "ReadonlyArray.flatMap"
            | "Array.filter"
            | "ReadonlyArray.filter"
            | "Array.some"
            | "ReadonlyArray.some"
            | "Array.every"
            | "ReadonlyArray.every"
            | "Array.find"
            | "ReadonlyArray.find"
            | "Array.findIndex"
            | "ReadonlyArray.findIndex"
            | "Array.findLast"
            | "ReadonlyArray.findLast"
            | "Array.findLastIndex"
            | "ReadonlyArray.findLastIndex"
                if call.kind == CallKind::Call && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::InlineCallback)
            }
            "Array.reduce"
            | "ReadonlyArray.reduce"
            | "Array.reduceRight"
            | "ReadonlyArray.reduceRight"
            | "Array.sort"
                if call.kind == CallKind::Call && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::InlineCallback)
            }
            "String.replace" | "String.replaceAll"
                if call.kind == CallKind::Call && argument == 1 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::InlineCallback)
            }
            _ => None,
        };
        if known_callback.is_some() {
            return known_callback;
        }
    }
    if parameter.callability == Callability::NonCallable {
        return Some(RuntimeArgumentBehavior::ValueOnly);
    }
    if !declaration.standard_library {
        return None;
    }
    let callable = parameter_may_be_callable(parameter) && potentially_callable(actual_callability);

    match declaration.qualified_name.as_ref() {
        // This is an audited behavior table over exact compiler-selected
        // standard-library declarations. The selected declaration carries its
        // canonical symbol and complete owner chain; custom same-name methods
        // never enter this table.
        "Window.removeEventListener" | "EventTarget.removeEventListener"
            if call.kind == CallKind::Call =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "Function.call" | "CallableFunction.call" | "NewableFunction.call"
            if call.kind == CallKind::Call && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "Function.bind" | "CallableFunction.bind" | "NewableFunction.bind"
            if call.kind == CallKind::Call && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "Function.bind" | "CallableFunction.bind" | "NewableFunction.bind"
            if call.kind == CallKind::Call && argument > 0 && callable =>
        {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "StringConstructor.call"
        | "NumberConstructor.call"
        | "BooleanConstructor.call"
        | "BigIntConstructor.call"
        | "SymbolConstructor.call"
        | "ObjectConstructor.call"
            if call.kind == CallKind::Call && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "ObjectConstructor.entries" | "ObjectConstructor.keys" | "ObjectConstructor.values"
            if call.kind == CallKind::Call && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "IntersectionObserver.construct"
        | "ResizeObserver.construct"
        | "MutationObserver.construct"
        | "PerformanceObserver.construct"
            if call.kind == CallKind::Construct && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "ReportingObserver.construct" if call.kind == CallKind::Construct && argument == 0 => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "ArrayConstructor.construct" if call.kind == CallKind::Construct && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "SetConstructor.construct"
        | "MapConstructor.construct"
        | "WeakSetConstructor.construct"
        | "WeakMapConstructor.construct"
            if call.kind == CallKind::Construct && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "ArrayConstructor.isArray" if call.kind == CallKind::Call && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "NumberConstructor.isFinite"
        | "NumberConstructor.isInteger"
        | "NumberConstructor.isNaN"
        | "NumberConstructor.isSafeInteger"
            if call.kind == CallKind::Call && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "parseFloat" | "parseInt" if call.kind == CallKind::Call && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "JSON.stringify" if call.kind == CallKind::Call && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "JSON.stringify" if call.kind == CallKind::Call && argument == 1 && callable => {
            Some(RuntimeArgumentBehavior::InlineCallback)
        }
        "Reflect.apply" if call.kind == CallKind::Call && argument == 0 => {
            Some(RuntimeArgumentBehavior::InlineCallback)
        }
        "Reflect.apply" if call.kind == CallKind::Call && matches!(argument, 1 | 2) => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "Reflect.set" | "Reflect.get" | "Reflect.has" | "Reflect.deleteProperty"
            if call.kind == CallKind::Call =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }

        // Collection insertion retains a callable value without invoking it.
        "Array.push" | "Array.unshift" if call.kind == CallKind::Call && callable => {
            Some(RuntimeArgumentBehavior::RetainedValue)
        }
        "Set.add" | "WeakSet.add" if call.kind == CallKind::Call && callable => {
            Some(RuntimeArgumentBehavior::RetainedValue)
        }
        "Map.set" | "WeakMap.set" if call.kind == CallKind::Call && argument == 1 && callable => {
            Some(RuntimeArgumentBehavior::RetainedValue)
        }

        // Object.assign reads/copies properties but does not invoke a source
        // object merely because that object is callable.
        "ObjectConstructor.assign" if call.kind == CallKind::Call => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "Geolocation.getCurrentPosition" | "Geolocation.watchPosition"
            if call.kind == CallKind::Call && matches!(argument, 0 | 1) && argument_callable =>
        {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "Scheduler.postTask"
            if call.kind == CallKind::Call && argument == 0 && argument_callable =>
        {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "ArrayConstructor.from"
        | "Int8ArrayConstructor.from"
        | "Uint8ArrayConstructor.from"
        | "Uint8ClampedArrayConstructor.from"
        | "Int16ArrayConstructor.from"
        | "Uint16ArrayConstructor.from"
        | "Int32ArrayConstructor.from"
        | "Uint32ArrayConstructor.from"
        | "Float32ArrayConstructor.from"
        | "Float64ArrayConstructor.from"
        | "BigInt64ArrayConstructor.from"
        | "BigUint64ArrayConstructor.from"
            if call.kind == CallKind::Call && argument == 1 && callable =>
        {
            Some(RuntimeArgumentBehavior::InlineCallback)
        }
        _ => None,
    }
}

/// Whether the compiler-resolved call is the synchronous callback position of
/// the built-in Array/ReadonlyArray `filter`. The standard-library bit and
/// owner chain are both required; a project-defined or unresolved `.filter`
/// must not inherit Array runtime behavior from its spelling.
pub(super) fn is_proven_array_filter(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
) -> bool {
    call.declaration.as_ref().is_some_and(|declaration| {
        declaration.standard_library
            && matches!(
                declaration.qualified_name.as_ref(),
                "Array.filter" | "ReadonlyArray.filter"
            )
            && argument_behavior(call, actual_callability, 0)
                == Some(RuntimeArgumentBehavior::InlineCallback)
    })
}

pub(super) fn resolved_parameter(call: &ResolvedCall, argument: usize) -> Option<&ParameterFact> {
    call.arguments
        .iter()
        .find(|mapping| mapping.argument_index == argument as u64)
        .filter(|mapping| mapping.status == ArgumentMappingStatus::Resolved)
        .and_then(|mapping| mapping.parameter.as_ref())
}

pub(super) fn retains_argument_value(call: &ResolvedCall, argument: usize) -> bool {
    call.validity == ResolvedCallValidity::Valid
        && call.kind == CallKind::Construct
        && call.declaration.as_ref().is_some_and(|declaration| {
            declaration.standard_library
                && declaration.qualified_name.as_ref() == "ProxyConstructor.construct"
        })
        && argument == 1
}

fn parameter_may_be_callable(parameter: &ParameterFact) -> bool {
    !matches!(parameter.callability, Callability::NonCallable)
}

pub(super) fn proven_array_method_argument_behavior(
    method: &str,
    callability: Option<Callability>,
) -> Option<RuntimeArgumentBehavior> {
    match method {
        "push" | "unshift" if potentially_callable(callability) => {
            Some(RuntimeArgumentBehavior::RetainedValue)
        }
        _ => None,
    }
}

pub(super) fn potentially_callable(callability: Option<Callability>) -> bool {
    !matches!(callability, Some(Callability::NonCallable))
}

/// Whether an argument's own syntax already proves it is not a function.
///
/// A literal is its own proof: `0`, `"a"`, `null`, `[1, 2]`, and `{ a: 1 }`
/// evaluate to values that cannot be invoked, whatever the callee does with
/// them. This is deliberately independent of [`potentially_callable`], which
/// answers from the type system and reports "potentially callable" whenever
/// it has no type at all -- exactly the case for every argument of an
/// untyped JavaScript runtime artifact. Only `Function` and `Unknown` leave
/// the question open.
pub(super) fn literal_argument_is_not_callable(kind: solid_facts::ast::RuntimeValueKind) -> bool {
    use solid_facts::ast::RuntimeValueKind;
    matches!(
        kind,
        RuntimeValueKind::Primitive
            | RuntimeValueKind::Nullish
            | RuntimeValueKind::Array
            | RuntimeValueKind::Object
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use typefacts::{
        ArgumentMapping, DeclarationOwner, Location, ParameterFact, ResolvedDeclaration,
    };

    use super::*;

    fn resolved_call(
        name: &str,
        owner: Option<&str>,
        standard_library: bool,
        parameter_callability: Callability,
    ) -> ResolvedCall {
        let location = Location {
            path: Arc::from(if standard_library {
                if name == "call" {
                    "bundled:/libs/lib.es5.d.ts"
                } else {
                    "bundled:/libs/lib.dom.d.ts"
                }
            } else {
                "/project/runtime.ts"
            }),
            start_byte: 0,
            end_byte: 1,
        };
        ResolvedCall {
            target: Arc::from(name),
            return_type_text: Arc::from("unknown"),
            targets: None,
            validity: ResolvedCallValidity::Valid,
            kind: if name == "construct" {
                CallKind::Construct
            } else {
                CallKind::Call
            },
            declaration: Some(ResolvedDeclaration {
                symbol: Arc::from(if standard_library {
                    format!("stdlib::{name}")
                } else {
                    format!("project::{name}")
                }),
                name: Arc::from(name),
                kind: Arc::from("method"),
                location: location.clone(),
                owners: owner.map_or_else(
                    || Arc::from([]),
                    |owner| {
                        Arc::from([DeclarationOwner {
                            symbol: Arc::from(format!("owner::{owner}")),
                            name: Arc::from(owner),
                            kind: Arc::from("interface"),
                            location: location.clone(),
                        }])
                    },
                ),
                qualified_name: Arc::from(
                    owner.map_or_else(|| name.to_owned(), |owner| format!("{owner}.{name}")),
                ),
                origin_module: Arc::from(""),
                source_file: location.path.clone(),
                standard_library,
            }),
            arguments: Arc::from([argument_mapping(0, parameter_callability)]),
        }
    }

    fn argument_mapping(index: u64, callability: Callability) -> ArgumentMapping {
        ArgumentMapping {
            argument_index: index,
            status: ArgumentMappingStatus::Resolved,
            unresolved: None,
            parameter: Some(ParameterFact {
                index,
                symbol: Arc::from(format!("parameter::{index}")),
                declaration: None,
                rest: false,
                optional: false,
                callability,
                type_descriptor: None,
                object_shape: None,
            }),
        }
    }

    fn callability(value: Callability) -> Option<Callability> {
        Some(value)
    }

    #[test]
    fn recognizes_resolved_queue_microtask_as_deferred() {
        assert_eq!(
            argument_behavior(
                &resolved_call("queueMicrotask", None, true, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            Some(RuntimeArgumentBehavior::FreshStackCallback)
        );
    }

    #[test]
    fn recognizes_window_timer_and_idle_callbacks_as_deferred() {
        for name in ["setTimeout", "requestIdleCallback"] {
            assert_eq!(
                argument_behavior(
                    // lib.dom's TimerHandler union is not definitely callable,
                    // but the actual arrow argument is.
                    &resolved_call(name, None, true, Callability::NonCallable),
                    callability(Callability::Callable),
                    0,
                ),
                Some(RuntimeArgumentBehavior::FreshStackCallback)
            );
            assert_eq!(
                argument_behavior(
                    &resolved_call(name, None, true, Callability::NonCallable),
                    callability(Callability::NonCallable),
                    0,
                ),
                Some(RuntimeArgumentBehavior::ValueOnly)
            );
        }
    }

    #[test]
    fn refuses_custom_same_name_declarations() {
        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "queueMicrotask",
                    Some("CustomScheduler"),
                    false,
                    Callability::Callable,
                ),
                callability(Callability::Callable),
                0,
            ),
            None
        );
    }

    #[test]
    fn collection_retention_requires_the_selected_standard_owner() {
        assert_eq!(
            argument_behavior(
                &resolved_call("add", Some("Set"), true, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            Some(RuntimeArgumentBehavior::RetainedValue)
        );
        assert_eq!(
            argument_behavior(
                &resolved_call("add", Some("CustomCollection"), true, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            None
        );
    }

    #[test]
    fn resolved_non_callable_parameters_are_value_only_without_an_api_allowlist() {
        assert_eq!(
            argument_behavior(
                &resolved_call("getItem", Some("Storage"), true, Callability::NonCallable,),
                callability(Callability::Unknown),
                0,
            ),
            Some(RuntimeArgumentBehavior::ValueOnly)
        );
    }

    #[test]
    fn reflect_apply_requires_the_selected_owner() {
        assert_eq!(
            argument_behavior(
                &resolved_call("apply", Some("Reflect"), true, Callability::Callable,),
                callability(Callability::Callable),
                0,
            ),
            Some(RuntimeArgumentBehavior::InlineCallback)
        );
        assert_eq!(
            argument_behavior(
                &resolved_call("apply", Some("CustomApply"), true, Callability::Callable,),
                callability(Callability::Callable),
                0,
            ),
            None
        );
    }

    #[test]
    fn observer_constructors_require_exact_standard_construct_signatures() {
        for owner in [
            "IntersectionObserver",
            "ResizeObserver",
            "MutationObserver",
            "PerformanceObserver",
        ] {
            assert_eq!(
                argument_behavior(
                    &resolved_call("construct", Some(owner), true, Callability::Callable),
                    callability(Callability::Callable),
                    0,
                ),
                Some(RuntimeArgumentBehavior::FreshStackCallback),
                "{owner}"
            );
        }

        let mut wrong_call_kind = resolved_call(
            "construct",
            Some("ResizeObserver"),
            true,
            Callability::Callable,
        );
        wrong_call_kind.kind = CallKind::Call;
        assert_eq!(
            argument_behavior(&wrong_call_kind, callability(Callability::Callable), 0),
            None
        );

        let mut wrong_argument = resolved_call(
            "construct",
            Some("ResizeObserver"),
            true,
            Callability::Callable,
        );
        wrong_argument.arguments = Arc::from([
            argument_mapping(0, Callability::Callable),
            argument_mapping(1, Callability::Callable),
        ]);
        assert_eq!(
            argument_behavior(&wrong_argument, callability(Callability::Callable), 1),
            None
        );

        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "construct",
                    Some("ResizeObserver"),
                    false,
                    Callability::Callable,
                ),
                callability(Callability::Callable),
                0,
            ),
            None
        );
        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "construct",
                    Some("ReportingObserver"),
                    true,
                    Callability::Callable,
                ),
                callability(Callability::Callable),
                0,
            ),
            Some(RuntimeArgumentBehavior::FreshStackCallback)
        );
    }

    #[test]
    fn string_value_only_requires_the_selected_call_signature() {
        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "call",
                    Some("StringConstructor"),
                    true,
                    Callability::Unknown,
                ),
                callability(Callability::Unknown),
                0,
            ),
            Some(RuntimeArgumentBehavior::ValueOnly)
        );
        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "String",
                    Some("StringConstructor"),
                    true,
                    Callability::Unknown,
                ),
                callability(Callability::Unknown),
                0,
            ),
            None
        );
        assert_eq!(
            argument_behavior(
                &resolved_call("String", None, false, Callability::Unknown),
                callability(Callability::Unknown),
                0,
            ),
            None
        );
        assert_eq!(
            argument_behavior(
                &resolved_call("call", Some("OtherConstructor"), true, Callability::Unknown),
                callability(Callability::Unknown),
                0,
            ),
            None
        );
        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "call",
                    Some("StringConstructor"),
                    false,
                    Callability::Unknown,
                ),
                callability(Callability::Unknown),
                0,
            ),
            None
        );

        let mut wrong_argument = resolved_call(
            "call",
            Some("StringConstructor"),
            true,
            Callability::Unknown,
        );
        wrong_argument.arguments = Arc::from([
            argument_mapping(0, Callability::Unknown),
            argument_mapping(1, Callability::Unknown),
        ]);
        assert_eq!(
            argument_behavior(&wrong_argument, callability(Callability::Unknown), 1),
            None
        );
    }

    #[test]
    fn exact_standard_library_identities_cover_new_runtime_behaviors() {
        for (owner, name) in [
            ("NumberConstructor", "call"),
            ("BooleanConstructor", "call"),
            ("ObjectConstructor", "call"),
        ] {
            assert_eq!(
                argument_behavior(
                    &resolved_call(name, Some(owner), true, Callability::Unknown),
                    callability(Callability::Unknown),
                    0,
                ),
                Some(RuntimeArgumentBehavior::ValueOnly),
                "{owner}.{name}"
            );
        }

        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "construct",
                    Some("ArrayConstructor"),
                    true,
                    Callability::Unknown
                ),
                callability(Callability::Unknown),
                0,
            ),
            Some(RuntimeArgumentBehavior::ValueOnly)
        );

        assert_eq!(
            argument_behavior(
                &resolved_call(
                    "from",
                    Some("ArrayConstructor"),
                    true,
                    Callability::Callable
                ),
                callability(Callability::Unknown),
                1,
            ),
            None
        );

        let mut array_from = resolved_call(
            "from",
            Some("ArrayConstructor"),
            true,
            Callability::NonCallable,
        );
        array_from.arguments = Arc::from([
            argument_mapping(0, Callability::NonCallable),
            argument_mapping(1, Callability::Callable),
        ]);
        assert_eq!(
            argument_behavior(&array_from, callability(Callability::Unknown), 1),
            Some(RuntimeArgumentBehavior::InlineCallback)
        );

        let mut replacement =
            resolved_call("replace", Some("String"), true, Callability::NonCallable);
        replacement.arguments = Arc::from([
            argument_mapping(0, Callability::NonCallable),
            argument_mapping(1, Callability::NonCallable),
        ]);
        assert_eq!(
            argument_behavior(&replacement, callability(Callability::Unknown), 1),
            Some(RuntimeArgumentBehavior::InlineCallback)
        );

        // Geolocation defers, and does not promise a fresh stack: "call back
        // with error" runs synchronously when the document is not fully
        // active. `postTask` only ever runs its callback from a queued task.
        for (owner, name, argument, expected) in [
            (
                "Geolocation",
                "getCurrentPosition",
                0,
                RuntimeArgumentBehavior::DeferredCallback,
            ),
            (
                "Geolocation",
                "getCurrentPosition",
                1,
                RuntimeArgumentBehavior::DeferredCallback,
            ),
            (
                "Geolocation",
                "watchPosition",
                0,
                RuntimeArgumentBehavior::DeferredCallback,
            ),
            (
                "Scheduler",
                "postTask",
                0,
                RuntimeArgumentBehavior::FreshStackCallback,
            ),
        ] {
            let mut call = resolved_call(name, Some(owner), true, Callability::Callable);
            if argument == 1 {
                call.arguments = Arc::from([
                    argument_mapping(0, Callability::Callable),
                    argument_mapping(1, Callability::Mixed),
                ]);
            }
            assert_eq!(
                argument_behavior(&call, callability(Callability::Unknown), argument),
                Some(expected),
                "{owner}.{name} argument {argument}"
            );
        }

        assert_eq!(
            argument_behavior(
                &resolved_call("push", Some("Uint8Array"), true, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            None
        );
        assert_eq!(
            argument_behavior(
                &resolved_call("replace", Some("CustomString"), true, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            None
        );
    }

    #[test]
    fn unresolved_argument_mappings_fail_closed() {
        let mut call = resolved_call("getItem", Some("Storage"), true, Callability::NonCallable);
        Arc::make_mut(&mut call.arguments)[0].status = ArgumentMappingStatus::Unresolved;
        Arc::make_mut(&mut call.arguments)[0].parameter = None;
        assert_eq!(
            argument_behavior(&call, callability(Callability::Unknown), 0),
            None
        );
    }

    /// The deferring schedulers that are *not* a fresh stack keep their timing
    /// and gain no clearing: a synchronous `dispatchEvent`/`click()` runs an
    /// event listener on the dispatcher's stack, a bound function is called by
    /// whoever holds it, and a thenable's `then` may call back at once.
    #[test]
    fn deferrals_a_caller_can_enter_synchronously_are_not_fresh_stacks() {
        for owner in ["EventTarget", "Window"] {
            let mut listen =
                resolved_call("addEventListener", Some(owner), true, Callability::Callable);
            listen.arguments = Arc::from([
                argument_mapping(0, Callability::NonCallable),
                argument_mapping(1, Callability::Callable),
            ]);
            let behavior = argument_behavior(&listen, callability(Callability::Callable), 1);
            assert_eq!(
                behavior,
                Some(RuntimeArgumentBehavior::DeferredCallback),
                "{owner}"
            );
            assert!(!behavior.is_some_and(RuntimeArgumentBehavior::runs_on_fresh_stack));
        }

        let mut bind = resolved_call(
            "bind",
            Some("CallableFunction"),
            true,
            Callability::Callable,
        );
        bind.arguments = Arc::from([
            argument_mapping(0, Callability::Unknown),
            argument_mapping(1, Callability::Callable),
        ]);
        assert_eq!(
            argument_behavior(&bind, callability(Callability::Callable), 1),
            Some(RuntimeArgumentBehavior::DeferredCallback)
        );

        let mut thenable = resolved_call("then", Some("PromiseLike"), true, Callability::Callable);
        thenable.arguments = Arc::from([argument_mapping(0, Callability::Callable)]);
        assert_eq!(
            argument_behavior(&thenable, callability(Callability::Callable), 0),
            Some(RuntimeArgumentBehavior::DeferredCallback)
        );
        let mut promise = resolved_call("then", Some("Promise"), true, Callability::Callable);
        promise.arguments = Arc::from([argument_mapping(0, Callability::Callable)]);
        assert_eq!(
            argument_behavior(&promise, callability(Callability::Callable), 0),
            Some(RuntimeArgumentBehavior::FreshStackCallback)
        );
    }

    /// The fresh-stack list is a refinement of the timing table, never a
    /// source of timing on its own: a same-named project declaration, or a
    /// listed name at a position the table does not defer, gains nothing.
    #[test]
    fn the_fresh_stack_list_only_refines_a_standard_deferral() {
        assert_eq!(
            argument_behavior(
                &resolved_call("setTimeout", None, false, Callability::Callable),
                callability(Callability::Callable),
                0,
            ),
            None
        );
        let mut timer = resolved_call("setTimeout", None, true, Callability::Callable);
        timer.arguments = Arc::from([
            argument_mapping(0, Callability::Callable),
            argument_mapping(1, Callability::NonCallable),
        ]);
        assert_eq!(
            argument_behavior(&timer, callability(Callability::NonCallable), 1),
            Some(RuntimeArgumentBehavior::ValueOnly)
        );
        assert!(RuntimeArgumentBehavior::FreshStackCallback.runs_on_fresh_stack());
        for behavior in [
            RuntimeArgumentBehavior::InlineCallback,
            RuntimeArgumentBehavior::DeferredCallback,
            RuntimeArgumentBehavior::RetainedValue,
            RuntimeArgumentBehavior::ValueOnly,
        ] {
            assert!(!behavior.runs_on_fresh_stack(), "{behavior:?}");
        }
    }
}
