use std::sync::Arc;

use typefacts::{ArgumentMapping, DeclarationOwner, Location, ParameterFact, ResolvedDeclaration};

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
        default_library_binding: None,
        return_type_text: Arc::from("unknown"),
        assertion_signature: false,
        never_returns: false,
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
fn additional_host_callbacks_keep_their_audited_stack() {
    for (name, owner, index, expected) in [
        (
            "construct",
            Some("PromiseConstructor"),
            0,
            RuntimeArgumentBehavior::InlineCallback,
        ),
        (
            "forEach",
            Some("NodeList"),
            0,
            RuntimeArgumentBehavior::InlineCallback,
        ),
        (
            "forEach",
            Some("NodeListOf"),
            0,
            RuntimeArgumentBehavior::InlineCallback,
        ),
        (
            "addListener",
            Some("MediaQueryList"),
            0,
            RuntimeArgumentBehavior::DeferredCallback,
        ),
        (
            "addEventListener",
            None,
            1,
            RuntimeArgumentBehavior::DeferredCallback,
        ),
        (
            "setActionHandler",
            Some("MediaSession"),
            1,
            RuntimeArgumentBehavior::FreshStackCallback,
        ),
    ] {
        let mut call = resolved_call(name, owner, true, Callability::Callable);
        call.arguments = Arc::from([argument_mapping(index, Callability::Callable)]);
        assert_eq!(
            argument_behavior(&call, Some(Callability::Callable), index as usize),
            Some(expected),
            "{owner:?}.{name}"
        );
        call.kind = if call.kind == CallKind::Call {
            CallKind::Construct
        } else {
            CallKind::Call
        };
        assert_eq!(
            argument_behavior(&call, Some(Callability::Callable), index as usize),
            None,
            "wrong invocation kind: {owner:?}.{name}"
        );
        let mut custom = resolved_call(name, owner, false, Callability::Callable);
        custom.arguments = Arc::from([argument_mapping(index, Callability::Callable)]);
        assert_eq!(
            argument_behavior(&custom, Some(Callability::Callable), index as usize),
            None,
            "same spelling outside the default library"
        );
    }
}

#[test]
fn augmented_scheduler_keeps_the_selected_signature_and_requires_binding_evidence() {
    let native = resolved_call("setInterval", None, true, Callability::Callable);
    let mut call = resolved_call("setInterval", Some("ambient"), false, Callability::Callable);
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        None
    );
    call.default_library_binding = native.declaration.clone().map(Arc::new);
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        Some(RuntimeArgumentBehavior::FreshStackCallback)
    );
    assert!(!call.declaration.as_ref().unwrap().standard_library);
    call.arguments = Arc::from([argument_mapping(0, Callability::NonCallable)]);
    assert_eq!(
        argument_behavior(&call, Some(Callability::NonCallable), 0),
        Some(RuntimeArgumentBehavior::ValueOnly)
    );
    call.arguments = Arc::from([]);
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        None
    );
    call.arguments = native.arguments.clone();
    call.validity = ResolvedCallValidity::Recovery;
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        None
    );
    call.validity = ResolvedCallValidity::Valid;
    Arc::make_mut(call.default_library_binding.as_mut().unwrap()).qualified_name =
        Arc::from("unreviewed");
    assert!(!augmented_global_runtime_call(&call));
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        None
    );
}

#[test]
fn augmented_fetch_keeps_value_parameters_without_inventing_callback_timing() {
    let native = resolved_call("fetch", None, true, Callability::NonCallable);
    let mut call = resolved_call("fetch", None, false, Callability::NonCallable);
    assert!(!augmented_global_runtime_call(&call));
    call.default_library_binding = native.declaration.map(Arc::new);
    assert!(augmented_global_runtime_call(&call));
    assert_eq!(
        argument_behavior(&call, Some(Callability::NonCallable), 0),
        Some(RuntimeArgumentBehavior::ValueOnly)
    );
    assert!(!call.declaration.as_ref().unwrap().standard_library);
    call.arguments = Arc::from([argument_mapping(0, Callability::Callable)]);
    assert_eq!(
        argument_behavior(&call, Some(Callability::Callable), 0),
        None
    );
    call.kind = CallKind::Construct;
    assert!(!augmented_global_runtime_call(&call));
    call.kind = CallKind::Call;
    call.validity = ResolvedCallValidity::Recovery;
    assert!(!augmented_global_runtime_call(&call));
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

    let mut replacement = resolved_call("replace", Some("String"), true, Callability::NonCallable);
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

/// ADR 0396: a registration function counts only when its declaration lies in
/// an installed reviewed runner package under a registration function's name;
/// a project path, a runner-looking directory without `node_modules`, and a
/// helper declared in a runner's file are not one.
#[test]
fn a_test_runner_registration_is_a_reviewed_package_declaration() {
    for path in [
        "/app/node_modules/vitest/dist/index.d.ts",
        "/app/node_modules/.pnpm/vitest@5.0.0_x/node_modules/vitest/dist/chunks/task-utils.d.ts",
        "/app/node_modules/bun-types/test.d.ts",
        "/app/node_modules/@types/node/test.d.ts",
        "/app/node_modules/@playwright/test/index.d.ts",
        "/app/node_modules/playwright/types/test.d.ts",
        "/app/node_modules/@jest/globals/build/index.d.ts",
        "/app/node_modules/@types/jest/index.d.ts",
        "/app/node_modules/@types/mocha/index.d.ts",
    ] {
        for name in ["test", "it", "describe", "beforeEach", "afterAll", "suite"] {
            assert!(
                test_runner_function_declaration(path, name).is_some(),
                "{path} {name}"
            );
        }
        assert!(
            test_runner_function_declaration(path, "expect").is_none(),
            "{path}"
        );
        assert!(
            test_runner_function_declaration(path, "vi").is_none(),
            "{path}"
        );
    }
    for path in [
        "/app/src/it.ts",
        "/app/vitest/index.d.ts",
        "/app/node_modules/other/vitest.d.ts",
        "/app/node_modules/@types/node/fs.d.ts",
        "/app/node_modules/@types/react/index.d.ts",
    ] {
        assert!(
            test_runner_function_declaration(path, "it").is_none(),
            "{path}"
        );
    }
    assert!(test_runner_modifier("only") && test_runner_modifier("each"));
    assert!(!test_runner_modifier("step") && !test_runner_modifier("call"));
    assert!(test_runner_curried_modifier("each") && !test_runner_curried_modifier("only"));
    assert!(test_runner_suite_function("describe") && test_runner_suite_function("suite"));
    assert!(!test_runner_suite_function("it") && !test_runner_suite_function("test"));
    assert!(test_runner_suite_modifier("describe") && !test_runner_suite_modifier("serial"));
}

/// ADR 0396: each runner's suite-body answer is its own, read on its bytes:
/// vitest collects the body after the file loads, jest-circus, mocha,
/// `node:test` and playwright call it inside the registration, and `bun:test`
/// shows no readable bytes.
#[test]
fn a_suite_body_is_placed_per_runner() {
    for (path, expected) in [
        (
            "/app/node_modules/vitest/dist/index.d.ts",
            SuiteBodyRun::Scheduled,
        ),
        (
            "/app/node_modules/@vitest/runner/dist/index.d.ts",
            SuiteBodyRun::Scheduled,
        ),
        (
            "/app/node_modules/bun-types/test.d.ts",
            SuiteBodyRun::Unproven,
        ),
        (
            "/app/node_modules/@types/node/test.d.ts",
            SuiteBodyRun::Inline,
        ),
        (
            "/app/node_modules/@playwright/test/index.d.ts",
            SuiteBodyRun::Inline,
        ),
        (
            "/app/node_modules/playwright/types/test.d.ts",
            SuiteBodyRun::Inline,
        ),
        (
            "/app/node_modules/@jest/globals/build/index.d.ts",
            SuiteBodyRun::Inline,
        ),
        (
            "/app/node_modules/@types/jest/index.d.ts",
            SuiteBodyRun::Inline,
        ),
        (
            "/app/node_modules/@types/mocha/index.d.ts",
            SuiteBodyRun::Inline,
        ),
    ] {
        assert_eq!(
            test_runner_function_declaration(path, "describe"),
            Some(expected),
            "{path}"
        );
    }
}
