# Solid 2 semantic inventory

This inventory was re-audited against the Solid `2.0.0-beta.31` public exports,
diagnostic union, and runtime call sites. “Conditional” means static certification
requires type, execution-region, ownership, async, or compiler facts; it does
not mean the checker may ignore the obligation.

Overload shape is part of the model. In particular, value-form `createSignal`,
`createStore`, `createOptimistic`, and `createOptimisticStore` create writable
sources but no child computation. Their function-form overloads create a
computed/projection child and are therefore forbidden inside leaf owners. The
optimistic variants otherwise participate in read provenance, setter-write,
refresh/affects-target, async, and `sync: true` checks like their non-optimistic
counterparts.

| Runtime diagnostic / condition | Static class | Initial proof obligation |
| --- | --- | --- |
| `REACTIVE_WRITE_IN_OWNED_SCOPE` | Conditional | Resolve a signal/store setter or `refresh()` target and prove whether it executes in an owned forbidden scope or an allowed event/action/untracked/leaf region. |
| `ACTION_CALLED_IN_OWNED_SCOPE` | Conditional | Resolve an `action()` result through calls and prove whether invocation occurs with a non-leaf owner. |
| `PENDING_ASYNC_UNTRACKED_READ` | Conditional | Prove async provenance and that the read executes in a tracked, suspendable region. |
| `ASYNC_OUTSIDE_LOADING_BOUNDARY` | Conditional | Prove an async render read is dominated by a compiler-recognized `Loading` boundary. |
| `CLEANUP_IN_FORBIDDEN_SCOPE` | Statically provable | Resolve `onCleanup` and prove its call region is `createTrackedEffect` or `onSettled`. |
| `PRIMITIVE_IN_FORBIDDEN_SCOPE` | Statically provable | Resolve primitive creation and prove the containing callback is a leaf-owner role. |
| Invalid cleanup return value | Conditional | Resolve callback role and prove every returned value is a function or `undefined`. |
| `SETTLED_CLEANUP_UNOWNED` | Conditional | Prove an `onSettled` callback can return cleanup and whether its call executes under a live, children-capable owner. |
| `flush()` in a forbidden scope | Statically provable | Resolve `flush` and prove the call region is a leaf-owner role. |
| Potential infinite loop | Runtime-only initially | Static rules reject known feedback writes, but the runtime iteration limit remains the oracle for data-dependent loops. |
| `STRICT_READ_UNTRACKED` | Conditional | Prove reactive provenance and that the read executes in an untracked component/effect-apply/rendering-function region. |
| Component props destructuring | Conditional checker policy | Prove a rendering component and props symbol (including stable aliases) before rejecting parameter/body destructuring. |
| Reactive read after guaranteed `await` | Conditional checker policy | Use tsgo function-expression and await-dominance facts to prove Solid accessor provenance, a tracked async computation, and an unconditional completed `await` before the read; reject conditional dominance and nested closures. |
| `PENDING_ASYNC_FORBIDDEN_SCOPE` | Conditional | Prove async provenance and a read in a non-suspendable leaf-owner region. |
| `NO_OWNER_EFFECT` | Conditional | Resolve effect creation and prove no live owner dominates it. |
| `NO_OWNER_CLEANUP` | Conditional | Resolve cleanup registration and prove no live owner dominates it. |
| `NO_OWNER_BOUNDARY` | Conditional | Use compiler facts to resolve boundary creation and prove no live owner dominates it. |
| `RUN_WITH_DISPOSED_OWNER` | Runtime-only initially | Owner disposal is generally value- and control-flow-dependent; reject unresolved cases when certification depends on them. |
| `INVALID_REFRESH_TARGET` | Conditional | Prove `refresh()` receives an original branded Solid accessor/store; reject proven wrappers, reads, literals, and invalid arity, and fail closed for unresolved targets. |
| `INVALID_AFFECTS_TARGET` | Conditional | Prove `affects()` receives a branded accessor/store, with at most one key and keys only for stores. |
| `MISSING_EFFECT_FN` | Statically provable | Resolve `createEffect` and require both compute and effect arguments, including calls with trailing commas. |
| `SYNC_NODE_RECEIVED_ASYNC` | Conditional | Resolve `sync: true` computations and prove whether their callback returns a Promise or AsyncIterable. |
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
