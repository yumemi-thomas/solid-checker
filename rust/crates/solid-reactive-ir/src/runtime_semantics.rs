//! Audited ECMAScript and Web-runtime argument behavior.
//!
//! Runtime behavior is selected from the compiler-resolved signature and its
//! argument-to-parameter mapping. Source spelling, rendered types, and member
//! lookup are deliberately not inputs: a shadowed or structurally similar API
//! must remain unknown.

use typefacts::{
    ArgumentMappingStatus, CallKind, Callability, ParameterFact, ResolvedCall,
    ResolvedCallValidity, ResolvedDeclaration,
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
        let known_callback = match declaration.name.as_ref() {
            "queueMicrotask" if argument == 0 && argument_callable => {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "setTimeout" | "setInterval" | "requestAnimationFrame" | "requestIdleCallback"
                if argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "addEventListener" if argument == 1 && argument_callable => {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "then" if promise_member(declaration) && argument <= 1 && argument_callable => {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "catch" | "finally"
                if promise_member(declaration) && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            "forEach" | "map" | "flatMap" | "filter" | "some" | "every" | "find" | "findIndex"
            | "findLast" | "findLastIndex"
                if argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::InlineCallback)
            }
            "reduce" | "reduceRight" | "sort" if argument == 0 && argument_callable => {
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
    let name = declaration.name.as_ref();
    let callable = parameter_may_be_callable(parameter) && potentially_callable(actual_callability);

    match name {
        // This is an audited behavior table over exact compiler-selected
        // standard-library declarations. The selected declaration carries its
        // canonical symbol and complete owner chain; custom same-name methods
        // never enter this table.
        "removeEventListener" => Some(RuntimeArgumentBehavior::ValueOnly),
        "call" | "bind" if function_member(declaration) && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "bind" if function_member(declaration) && argument > 0 && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "String" if argument == 0 => Some(RuntimeArgumentBehavior::ValueOnly),
        "entries" | "keys" | "values"
            if declaration_owner(declaration, "ObjectConstructor") && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "construct"
            if call.kind == CallKind::Construct
                && declaration_owner(declaration, "IntersectionObserver")
                && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "isArray" if declaration_owner(declaration, "ArrayConstructor") && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "isFinite" | "isInteger" | "isNaN" | "isSafeInteger"
            if declaration_owner(declaration, "NumberConstructor") && argument == 0 =>
        {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "parseFloat" | "parseInt" if declaration.owners.is_empty() && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "stringify" if declaration_owner(declaration, "JSON") && argument == 0 => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "stringify" if declaration_owner(declaration, "JSON") && argument == 1 && callable => {
            Some(RuntimeArgumentBehavior::InlineCallback)
        }
        "apply" if declaration_owner(declaration, "Reflect") && argument == 0 => {
            Some(RuntimeArgumentBehavior::InlineCallback)
        }
        "apply" if declaration_owner(declaration, "Reflect") && matches!(argument, 1 | 2) => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }
        "set" | "get" | "has" | "deleteProperty" if declaration_owner(declaration, "Reflect") => {
            Some(RuntimeArgumentBehavior::ValueOnly)
        }

        // Collection insertion retains a callable value without invoking it.
        "push" | "unshift" if array_member(declaration) && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "add" if set_member(declaration) && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }
        "set" if map_member(declaration) && argument == 1 && callable => {
            Some(RuntimeArgumentBehavior::DeferredCallback)
        }

        // Object.assign reads/copies properties but does not invoke a source
        // object merely because that object is callable.
        "assign" if declaration_owner(declaration, "ObjectConstructor") => {
            Some(RuntimeArgumentBehavior::ValueOnly)
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
            && declaration.name.as_ref() == "filter"
            && declaration_owner_in(declaration, &["Array", "ReadonlyArray"])
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
                && declaration.name.as_ref() == "construct"
                && declaration_owner(declaration, "ProxyConstructor")
        })
        && argument == 1
}

fn parameter_may_be_callable(parameter: &ParameterFact) -> bool {
    !matches!(parameter.callability, Callability::NonCallable)
}

fn declaration_owner(declaration: &ResolvedDeclaration, expected: &str) -> bool {
    declaration
        .owners
        .iter()
        .any(|owner| owner.name.as_ref() == expected)
}

fn declaration_owner_in(declaration: &ResolvedDeclaration, expected: &[&str]) -> bool {
    declaration
        .owners
        .iter()
        .any(|owner| expected.contains(&owner.name.as_ref()))
}

fn promise_member(declaration: &ResolvedDeclaration) -> bool {
    declaration_owner_in(declaration, &["Promise", "PromiseLike"])
}

fn function_member(declaration: &ResolvedDeclaration) -> bool {
    declaration_owner_in(
        declaration,
        &["Function", "CallableFunction", "NewableFunction"],
    )
}

fn array_member(declaration: &ResolvedDeclaration) -> bool {
    declaration_owner_in(
        declaration,
        &[
            "Array",
            "ReadonlyArray",
            "Int8Array",
            "Uint8Array",
            "Uint8ClampedArray",
            "Int16Array",
            "Uint16Array",
            "Int32Array",
            "Uint32Array",
            "Float32Array",
            "Float64Array",
            "BigInt64Array",
            "BigUint64Array",
        ],
    )
}

fn set_member(declaration: &ResolvedDeclaration) -> bool {
    declaration_owner_in(declaration, &["Set", "WeakSet"])
}

fn map_member(declaration: &ResolvedDeclaration) -> bool {
    declaration_owner_in(declaration, &["Map", "WeakMap"])
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
                "/typescript/lib/lib.dom.d.ts"
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
                qualified_name: Arc::from(name),
                origin_module: Arc::from(""),
                source_file: location.path.clone(),
                standard_library,
            }),
            arguments: Arc::from([ArgumentMapping {
                argument_index: 0,
                status: ArgumentMappingStatus::Resolved,
                unresolved: None,
                parameter: Some(ParameterFact {
                    index: 0,
                    symbol: Arc::from("parameter::0"),
                    declaration: None,
                    rest: false,
                    optional: false,
                    callability: parameter_callability,
                    type_descriptor: None,
                }),
            }]),
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
                    &resolved_call(name, Some("Window"), true, Callability::NonCallable),
                    callability(Callability::Callable),
                    0,
                ),
                Some(RuntimeArgumentBehavior::DeferredCallback)
            );
            assert_eq!(
                argument_behavior(
                    &resolved_call(name, Some("Window"), true, Callability::NonCallable),
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
    fn intersection_observer_requires_a_construct_signature() {
        let mut call = resolved_call(
            "construct",
            Some("IntersectionObserver"),
            true,
            Callability::Callable,
        );
        assert_eq!(
            argument_behavior(&call, callability(Callability::Callable), 0),
            Some(RuntimeArgumentBehavior::DeferredCallback)
        );
        call.kind = CallKind::Call;
        assert_eq!(
            argument_behavior(&call, callability(Callability::Callable), 0),
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
