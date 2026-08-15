# Solid 2.0 semantic inventory

This inventory was re-audited against the Solid `2.0.0-rc.0` public exports,
diagnostic union, and runtime call sites. “Conditional” means static certification
requires type, execution-region, ownership, async, or compiler facts; it does
not mean the checker may ignore the obligation.

Overload shape is part of the model. In particular, value-form `createSignal`,
`createStore`, `createOptimistic`, and `createOptimisticStore` create writable
sources but no child computation. Their function-form overloads create a
computed/projection child and are therefore forbidden inside leaf owners. The
optimistic variants otherwise participate in read provenance, setter-write,
refresh/affects-target, and async checks like their non-optimistic
counterparts. `sync: true` checks apply only to the signal family
(`createSignal(fn)`, `createMemo`, `createOptimistic(fn)`, `createEffect`,
`createRenderEffect`, `createTrackedEffect`): the store-family constructors
(`createStore(fn)`, `createProjection`, `createOptimisticStore`) rebuild their
node options with only `loadingValue`/`name`, so `options.sync` never reaches
their node and is inert there (probed, rc.0). Construction form also decides
refreshability: only the derived store forms own a compute node, and
`refresh()` on a value-form `createStore(obj)`/`createOptimisticStore(obj)`
store — or any of its child records — throws `INVALID_REFRESH_TARGET` in dev.

| Runtime diagnostic / condition | Static class | Initial proof obligation |
| --- | --- | --- |
| `REACTIVE_WRITE_IN_OWNED_SCOPE` | Conditional | Resolve a signal/store setter or `refresh()` target and prove whether it executes in an owned forbidden scope or an allowed event/action/untracked/leaf region. |
| `ACTION_CALLED_IN_OWNED_SCOPE` | Conditional | Resolve an `action()` result through calls and prove whether invocation occurs with a non-leaf owner. |
| `PENDING_ASYNC_UNTRACKED_READ` | Conditional | Prove async provenance and that the read executes in a tracked, suspendable region. Declared-first-paint exception (probed, rc.0): a `loadingValue`/`seedLoadingValue: true` node is born committed and its first flight cannot throw here — but the exception ends at the first real answer, when a pending re-ask throws again, so the obligation stays with conditional wording. An unreadable options argument makes the throw unprovable: fail honest by downgrading to uncertifiable. |
| `ASYNC_OUTSIDE_LOADING_BOUNDARY` | Conditional warning | Prove an async render read is dominated by a compiler-recognized `Loading` boundary; without one, report deferred atomic mount rather than a thrown runtime error. Declared-first-paint exception (probed, rc.0): a `loadingValue`/`seedLoadingValue: true` source never suspends its first flight and never trips a boundary, so no warning is reported for proven declarations. |
| `CLEANUP_IN_FORBIDDEN_SCOPE` | Statically provable | Resolve `onCleanup` and prove its call region is `createTrackedEffect` or `onSettled`. |
| `PRIMITIVE_IN_FORBIDDEN_SCOPE` | Statically provable | Resolve primitive creation and prove the containing callback is a leaf-owner role. |
| Invalid cleanup return value | Conditional | Resolve callback role and prove every returned value is a function or `undefined`. |
| `SETTLED_CLEANUP_UNOWNED` | Conditional | Prove an `onSettled` callback can return cleanup and whether its call executes under a live, children-capable owner. |
| `flush()` in a forbidden scope | Statically provable | Resolve `flush` and prove the call region is a leaf-owner role. |
| Potential infinite loop | Runtime-only initially | Static rules reject known feedback writes, but the runtime iteration limit remains the oracle for data-dependent loops. |
| `STRICT_READ_UNTRACKED` | Conditional | Prove reactive provenance and that the read executes in an untracked component/effect-apply/rendering-function region. |
| Component props destructuring | Conditional checker policy | Prove a rendering component and props symbol (including stable aliases) before rejecting parameter/body destructuring. |
| Reactive read after guaranteed `await` | Conditional checker policy | Use tsgo function-expression and await-dominance facts to prove Solid accessor provenance, a tracked async computation, and an unconditional completed `await` before the read; reject conditional dominance and nested closures. |
| `PENDING_ASYNC_FORBIDDEN_SCOPE` | Conditional | Prove async provenance and a read in a non-suspendable leaf-owner region. Declared-first-paint exception (probed, rc.0): the first flight of a `loadingValue`/`seedLoadingValue: true` node serves the declared value without warning or throw; later re-asks warn and throw like undeclared nodes, so the obligation stays with conditional wording. |
| `ssrSource: "client"` read during SSR outside a `Loading` boundary | Conditional | Prove a source declares `ssrSource: "client"` with no `loadingValue`/`seedLoadingValue` in an exact object-literal options argument, a tracked JSX read with no dominating `Loading` boundary, and a server rendering entry point imported somewhere in the project (the throw lives in `dist/server.js`, unconditionally — including for fully synchronous computes, so detection keys on option presence, not async provenance). CSR-only projects never run the throwing path and must stay silent. |
| `NO_OWNER_EFFECT` | Conditional | Resolve effect creation and prove no live owner dominates it. |
| `NO_OWNER_CLEANUP` | Conditional | Resolve cleanup registration and prove no live owner dominates it. |
| `NO_OWNER_BOUNDARY` | Conditional | Use compiler facts to resolve boundary creation and prove no live owner dominates it. |
| `RUN_WITH_DISPOSED_OWNER` | Runtime-only initially | Owner disposal is generally value- and control-flow-dependent; reject unresolved cases when certification depends on them. |
| `INVALID_REFRESH_TARGET` | Conditional | Prove `refresh()` receives an original branded Solid accessor or refreshable store (member chains on a store base count — child records carry the brand); reject proven wrappers, reads, literals, zero-argument calls, and value-form store targets, and fail closed for unresolved targets. Extra arguments are ignored by the runtime and are not rejected. |
| `INVALID_AFFECTS_TARGET` | Conditional | Prove `affects()` receives a branded accessor/store — including nested store records reached through member chains — with at most one property key and a key only for a store record. Multiple keys are separate calls, not a path array. |
| `MISSING_EFFECT_FN` | Statically provable | Resolve `createEffect` and require both compute and effect arguments, including calls with trailing commas. |
| `SYNC_NODE_RECEIVED_ASYNC` | Conditional | Resolve `sync: true` computations and prove whether their callback returns a Promise or AsyncIterable. |
| `Cannot call resolve inside a reactive scope` | Conditional | Prove a `resolve()` call runs under an active observer: directly inside a tracked, non-deferred callback (memo/effect compute, `createTrackedEffect`, boundary body) or in a compiler-proven tracked JSX region. `untrack` callbacks and component bodies clear the observer and are runtime-legal (probed, rc.0 — the guard is `getObserver()`, dev-only; production silently takes a one-shot snapshot). |
| `httpStatus`/`httpHeader` after the head commits | Conditional warning | Prove the call renders below a `Loading` boundary's *children* (fallbacks are shell content) in a project that server-renders. The drop is committed-gate behavior (`dist/server.js` gates write and retraction on `!response.committed`; there is no queue) but conditional — a boundary settling pre-flush still applies — so the finding stays a warning. Client builds are unconditional no-ops and CSR-only projects stay silent. |
| Module-level `"use server"` with a non-function export | Statically provable (doc-derived) | Prove the module's directive prologue contains `"use server"` and an export is a wrapped call, a non-function default expression, or a re-export; the compiler turns only direct function exports into client references (RFC 10 §Compiler implications — the directive pass is build-plugin territory, so the RFC text is the spec). |
| Rich server-function arguments without a serializer | Conditional | Prove the callee carries a `"use server"` directive (function-level, or module-level in another module), the argument's resolved type is or top-level-contains `Date`/`Map`/`Set`/`RegExp`/a typed array, and nothing imports `enableRichArguments` or configures `serializeArgs` (probed, rc.0 client: plain-JSON default, directed throw). Lone/trailing `Uint8Array` has a natural body encoding and is exempt; unresolvable argument types route to silence. |
| `REACTIVITY_HALTED` | Runtime-only | This is a secondary runtime scheduler/error state after an escaped reactive error, not an independent source rule. |
| `INVARIANT_VIOLATION` | Runtime-only, internal | Internal engine consistency assertions remain runtime or fuzzing oracles and are not user-program proof obligations. |

## Explicit unsupported boundaries

Each boundary creates an `uncertifiable` finding when it can affect a reactive
proof obligation:

- `any` or unknown values used as reactive sources, writes, calls, or callbacks;
- `eval`, `Function`, or code generated at runtime;
- unresolved dynamic call targets or property dispatch;
- dependencies with neither analyzable source nor a valid trusted contract;
- compiler options not represented by the compiler-facts protocol;
- mismatched source hashes, paths, spans, or UTF-8/UTF-16 mappings;
- unsupported JavaScript syntax or TypeScript project configurations;
- analyzer failures, missing backends, or stale package contracts.

## Result policy

Findings may include adapter-neutral `analysisContext` and `subjectKind`
fields when a broader runtime diagnostic has a statically proven sub-context,
such as a `createEffect` apply callback or a proven component-props read.
Adapters may use them to provide exact
compatibility rule names without changing certification status.

- No violation and no unresolved obligation: `certified`.
- At least one proven breach: `violation`.
- Otherwise, at least one unresolved obligation: `uncertifiable`.
- `--certify` fails for both `violation` and `uncertifiable`.
