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
/// - `MediaSession.setActionHandler` -- Media Session's action handling
///   queues a user-interaction task before invoking the handler;
/// - the `IntersectionObserver`, `ResizeObserver`, `MutationObserver`,
///   `PerformanceObserver` and `ReportingObserver` constructors -- their
///   callbacks are delivered by a queued task, microtask or rendering update;
///   `takeRecords()` returns records without invoking the callback.
///
/// This is the list [`RuntimeArgumentBehavior::FreshStackCallback`] answers
/// for, and the only host evidence that lets a summary row say a
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
    "MediaSession.setActionHandler",
    "IntersectionObserver.construct",
    "ResizeObserver.construct",
    "MutationObserver.construct",
    "PerformanceObserver.construct",
    "ReportingObserver.construct",
];

/// ADR 0396: the test runners whose registered callbacks are reviewed to run
/// with no Solid owner and outside every strict-read window. Each entry is a
/// run of consecutive path components of an installed declaration file (so a
/// project directory named `vitest` is not one: `node_modules` leads each),
/// matched on the declaration the callee's root identifier resolves to, never
/// on a spelling. The premise reads each runner's published source: the runner
/// invokes every registered test, suite body and hook from its own scheduler
/// (an awaited call from its collection or run loop), with no Solid API on
/// the stack. A runner configured to wrap tests in a Solid root, or patched,
/// is outside it (ADR 0396).
///
/// - `vitest` and `@vitest/runner`: the declarations of `test`, `it`,
///   `describe`, `suite` and the hooks (vitest 5 declares them in its own
///   package, vitest 4 re-exports `@vitest/runner`'s, the package that holds
///   the runner loop; its `globals` typing lives in the same package);
/// - `bun:test` (`bun-types/test.d.ts`) and `node:test`
///   (`@types/node/test.d.ts`): the module declarations, whose `describe` is
///   `suite` and `it` is `test`;
/// - `@playwright/test` and `playwright/types/test.d.ts`, where `test` is
///   declared;
/// - `@jest/globals`, `@types/jest` and `@types/mocha`.
const TEST_RUNNER_DECLARATIONS: &[&[&str]] = &[
    &["node_modules", "vitest"],
    &["node_modules", "@vitest", "runner"],
    &["node_modules", "bun-types", "test.d.ts"],
    &["node_modules", "@types", "node", "test.d.ts"],
    &["node_modules", "@playwright", "test"],
    &["node_modules", "playwright", "types", "test.d.ts"],
    &["node_modules", "@jest", "globals"],
    &["node_modules", "@types", "jest"],
    &["node_modules", "@types", "mocha"],
];

/// The declared names of the runners' registration functions: tests and
/// suites, and the hooks (`before` and `after` are `node:test`'s and mocha's
/// spelling of `beforeAll` and `afterAll`).
const TEST_RUNNER_FUNCTIONS: &[&str] = &[
    "test",
    "it",
    "describe",
    "suite",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "before",
    "after",
];

/// The members a registration function carries that register the same kind of
/// callback (`it.only`, `test.describe.serial`, `describe.skip`), and the ones
/// of them that are called once before the registration (`test.each(rows)(…)`,
/// `it.skipIf(cond)(…)`).
const TEST_RUNNER_MODIFIERS: &[&str] = &[
    "only",
    "skip",
    "todo",
    "concurrent",
    "sequential",
    "parallel",
    "serial",
    "fails",
    "fail",
    "fixme",
    "slow",
    "describe",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "each",
    "for",
    "skipIf",
    "runIf",
];
const TEST_RUNNER_CURRIED_MODIFIERS: &[&str] = &["each", "for", "skipIf", "runIf"];

/// Whether a declaration of the name `name` in the file `path` is one of the
/// reviewed runners' registration functions ([`TEST_RUNNER_DECLARATIONS`]).
pub(super) fn test_runner_function_declaration(path: &str, name: &str) -> bool {
    if !TEST_RUNNER_FUNCTIONS.contains(&name) {
        return false;
    }
    let components = path.split(['/', '\\']).collect::<Vec<_>>();
    TEST_RUNNER_DECLARATIONS.iter().any(|runner| {
        components
            .windows(runner.len())
            .any(|window| window == *runner)
    })
}

/// Whether `member` may follow a registration function in a callee chain.
pub(super) fn test_runner_modifier(member: &str) -> bool {
    TEST_RUNNER_MODIFIERS.contains(&member)
}

/// Whether the chain's call before the final registration may be `member(…)`.
pub(super) fn test_runner_curried_modifier(member: &str) -> bool {
    TEST_RUNNER_CURRIED_MODIFIERS.contains(&member)
}

/// The modules the Node runtime provides under a bare name (Node 24
/// `module.builtinModules`, without the `_`-prefixed internals). A specifier
/// naming one, or any `node:` specifier, loads the runtime's module rather
/// than a project file. This assumes no bundler aliases one of these names
/// onto a project file (ADR 0219).
const NODE_BUILTIN_MODULES: &[&str] = &[
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Whether `specifier` names a module the Node runtime provides
/// ([`NODE_BUILTIN_MODULES`], or any `node:` specifier).
pub(super) fn is_node_builtin_module(specifier: &str) -> bool {
    specifier.starts_with("node:") || NODE_BUILTIN_MODULES.contains(&specifier)
}

/// Fresh-stack rows whose method calls its receiver's `then`
/// (`Promise.prototype.catch` and `finally`, ECMAScript 27.2.5.1, 27.2.5.3).
/// A subclass that overrides `then` can run the handler synchronously, and
/// the inherited method still resolves to the standard declaration, so a proof
/// that the handler never runs on its invoker's stack must refuse these.
const RECEIVER_THEN_DISPATCH: &[&str] = &["Promise.catch", "Promise.finally"];

/// Whether `call` is a fresh-stack scheduler whose stack depends on its
/// receiver's `then` ([`RECEIVER_THEN_DISPATCH`]).
pub(super) fn dispatches_through_receiver_then(call: &ResolvedCall) -> bool {
    call.declaration.as_ref().is_some_and(|declaration| {
        declaration.standard_library
            && RECEIVER_THEN_DISPATCH.contains(&declaration.qualified_name.as_ref())
    })
}

impl RuntimeArgumentBehavior {
    /// Whether the callback, when it runs, is proven to run with no caller's
    /// listener current -- the one fact a `deferred` summary row needs before
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
        && runtime_declaration(call).is_some_and(|declaration| {
            declaration.standard_library
                && FRESH_STACK_SCHEDULERS.contains(&declaration.qualified_name.as_ref())
        });
    Some(if fresh_stack {
        RuntimeArgumentBehavior::FreshStackCallback
    } else {
        behavior
    })
}

/// Only the reviewed global runtime calls consume the independent
/// default-library binding fact. It never promotes a package declaration or
/// a structurally typed method into the general library model.
/// `fetch` gains no callback timing row: the selected overload still decides
/// which arguments are values. As for ordinary default-library calls, implicit
/// conversions are outside this model, and the leaf walk refuses literal getters.
pub(super) fn augmented_global_runtime_call(call: &ResolvedCall) -> bool {
    call.validity == ResolvedCallValidity::Valid
        && call.kind == CallKind::Call
        && call.targets.is_none()
        && !call.target.is_empty()
        && call
            .default_library_binding
            .as_ref()
            .is_some_and(|declaration| {
                declaration.standard_library
                    && declaration.owners.is_empty()
                    && matches!(
                        declaration.qualified_name.as_ref(),
                        "setTimeout"
                            | "setInterval"
                            | "queueMicrotask"
                            | "requestAnimationFrame"
                            | "requestIdleCallback"
                            | "fetch"
                    )
            })
}

fn runtime_declaration(call: &ResolvedCall) -> Option<&typefacts::ResolvedDeclaration> {
    if augmented_global_runtime_call(call) {
        call.default_library_binding.as_deref()
    } else {
        call.declaration.as_ref()
    }
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
/// the pinned tsrs revision's TypeScript: every declaration of that name in the
/// bundled default library is the same `EventTarget` registration whose
/// argument 1 is the listener. That fact is used here for one purpose only,
/// to withhold a claim about *when* the listener runs; it does not widen the
/// timing table, whose rows also feed callback summaries.
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
    let declaration = runtime_declaration(call)?;
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
            // DOM addEventListener steps 1-2 only register the listener.
            "addEventListener" | "Window.addEventListener" | "EventTarget.addEventListener"
                if call.kind == CallKind::Call && argument == 1 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            // CSSOM View addListener delegates to DOM listener registration.
            "MediaQueryList.addListener"
                if call.kind == CallKind::Call && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            // Media Session: handle media session action, queued task, step 7.
            "MediaSession.setActionHandler"
                if call.kind == CallKind::Call && argument == 1 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::DeferredCallback)
            }
            // ECMA-262 Promise(executor), step 10 calls the executor inline.
            "PromiseConstructor.construct"
                if call.kind == CallKind::Construct && argument == 0 && argument_callable =>
            {
                Some(RuntimeArgumentBehavior::InlineCallback)
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
            // Web IDL iterable methods install Array.prototype.forEach
            // on NodeList (steps 1.2.4); the callback runs inline.
            "NodeList.forEach"
            | "NodeListOf.forEach"
            | "Array.forEach"
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

/// Whether `argument` of the compiler-resolved call is a callback an exact
/// standard-library declaration invokes before it returns (the audited
/// [`RuntimeArgumentBehavior::InlineCallback`] rows: `Array`/`ReadonlyArray`
/// `map`, `forEach`, `filter`, `some`, `every`, `find`, `findIndex`,
/// `flatMap`, `reduce`, `sort`, ..., `Set`/`Map`/`NodeList` `forEach`,
/// `Array.from`'s mapper, the `Promise` executor).
///
/// The standard-library bit is required of the selected declaration: a
/// project-defined or shadowed method with the same name resolves elsewhere
/// and inherits nothing from its spelling. "Invoked before it returns" is a
/// timing claim, not a count: an empty receiver never invokes the callback.
pub(super) fn is_inline_standard_callback(
    call: &ResolvedCall,
    actual_callability: Option<Callability>,
    argument: usize,
) -> bool {
    call.declaration
        .as_ref()
        .is_some_and(|declaration| declaration.standard_library)
        && argument_behavior(call, actual_callability, argument)
            == Some(RuntimeArgumentBehavior::InlineCallback)
}

/// Whether the valid resolved call is the standard `setTimeout` or
/// `setInterval`: HTML timer initialization steps, which queue the callback as
/// a task of its own. The timer's callback never runs before the caller's
/// microtasks have drained.
pub(super) fn is_timer_call(call: &ResolvedCall) -> bool {
    call.validity == ResolvedCallValidity::Valid
        && call.kind == CallKind::Call
        && runtime_declaration(call).is_some_and(|declaration| {
            declaration.standard_library
                && matches!(
                    declaration.qualified_name.as_ref(),
                    "setTimeout"
                        | "setInterval"
                        | "WindowOrWorkerGlobalScope.setTimeout"
                        | "WindowOrWorkerGlobalScope.setInterval"
                )
        })
}

/// The qualified name of the exact standard-library declaration a valid
/// resolved call selected (`Promise.then`, `PromiseConstructor.all`, ...), or
/// `None` for a project, package, unresolved or invalid call.
pub(super) fn standard_call_name(call: &ResolvedCall) -> Option<&str> {
    if call.validity != ResolvedCallValidity::Valid {
        return None;
    }
    call.declaration
        .as_ref()
        .filter(|declaration| declaration.standard_library)
        .map(|declaration| declaration.qualified_name.as_ref())
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
mod tests;
