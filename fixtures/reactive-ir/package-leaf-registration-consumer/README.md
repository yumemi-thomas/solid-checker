# A package registration inside a leaf owner

Pins ADR 0179: an accepted contract that states an export registers a
cleanup or a computation on its caller's owner, at the call and on every call
(unguarded, `ambient-at-call`, call-scoped `min: 1`), tells a consumer that
calling it inside a leaf owner is a forbidden operation. Solid 2 throws
`CLEANUP_IN_FORBIDDEN_SCOPE` or `PRIMITIVE_IN_FORBIDDEN_SCOPE` there and
halts reactivity.

| Case | Finding | Why |
| --- | --- | --- |
| `listen()` inside `createTrackedEffect` | `SC3001 leaf-owner-forbidden-call` violation | a cleanup on every call |
| `startTickerAlways()` inside `createTrackedEffect` | violation | a computation on every call |
| `listen()` inside an owner-backed `onSettled` | violation | the same leaf owner |
| `startTicker()` inside `createTrackedEffect` | no violation (the callback stays unresolved) | it only may register |
| `startTickerSilent()` inside `createTrackedEffect` | no violation (the callback stays unresolved) | it registers nothing |
| `queueMicrotask(() => listen())` inside `createTrackedEffect` | no leaf finding; `SC4001 missing-owner` | the microtask runs later on an empty stack, with no owner |
| `listen()`, `startTickerAlways()` in a component body | none | a normal owner |

The contract is `package-computation-consumer`'s with one export added,
`listen`, whose `cleanup` operation has the shape the generator publishes for
an `onCleanup` the export's body always runs (raf's `createRAF`). The manifest
bytes are that fixture's, so the closure digest is unchanged. The `solid-js`
stub is `fp-owner-leaf-helper-after-await`'s, verbatim from rc.9, and
`App.tsx` type-checks against it.
