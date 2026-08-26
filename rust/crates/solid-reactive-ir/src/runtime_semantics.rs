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
    /// The argument may be retained and invoked after the runtime call returns.
    DeferredCallback,
    /// The argument value may be read, copied, or retained, but is not invoked.
    ValueOnly,
}

pub(super) fn argument_behavior(
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
            "queueMicrotask" if argument == 0 && argument_callable => {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "setTimeout" | "setInterval" | "requestAnimationFrame" | "requestIdleCallback"
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
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "Set.add" | "WeakSet.add" if call.kind == CallKind::Call && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "Map.set" | "WeakMap.set" if call.kind == CallKind::Call && argument == 1 && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
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
            Some(RuntimeArgumentBehavior::DeferredCallback)
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
            Some(RuntimeArgumentBehavior::DeferredCallback)
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
                Some(RuntimeArgumentBehavior::DeferredCallback)
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
            Some(RuntimeArgumentBehavior::DeferredCallback)
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
                Some(RuntimeArgumentBehavior::DeferredCallback),
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
            Some(RuntimeArgumentBehavior::DeferredCallback)
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

        for (owner, name, argument) in [
            ("Geolocation", "getCurrentPosition", 0),
            ("Geolocation", "getCurrentPosition", 1),
            ("Geolocation", "watchPosition", 0),
            ("Scheduler", "postTask", 0),
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
                Some(RuntimeArgumentBehavior::DeferredCallback),
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
}
