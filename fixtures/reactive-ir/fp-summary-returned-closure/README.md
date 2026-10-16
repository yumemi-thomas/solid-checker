# fp-summary-returned-closure

**Claim.** A closure a hook builds into its result -- an object-literal
property (`return { busy: () => state() }`) or an element of a returned tuple
(`return [state, () => state() > 0]`) -- is not run by the hook's call. Its
reads are neither the hook's summary reads nor reachable through the hook's
call edges, so a component body that merely calls the hook is not charged with
a strict-window read of what the closure reads later.

Before: `discover_summary_nodes` admits a function only when it is bound, a
method, or in the TypeScript function universe, so an anonymous property or
tuple closure is no node and the nearest node containing its reads is the
enclosing hook. `Screen` below was reported at `createMutation()` as "reads
`state`" -- the pattern behind about 192 of 1,962 measured
`strict-read-untracked` violations (kui's `createMutation().busy` alone is
49+). `interproc::runs_in_retained_value_literal` now answers the question the
summary path never asked, at the three places a read or a call edge is
attributed to a node (direct references, typed accessors, call edges).

Scope and approximations:

- Only a literal that is the value of a **data** object property or an element
  of a **returned** array literal, and **not** inside a call argument written in
  the hook (`track({ run: () => state() })`, `createStore([() => s()])`), is
  held back. A call may run an argument it receives, which is the
  callee-timing question and keeps its legacy attribution.
- A getter (`get big() {}`) is a property whose value is the function, so it is
  held back by the same test (the baseline folded it too: `createGetter`). A
  method (`small() {}`) is a summary node of its own and was never folded.
- A later call needs exact transport to the returned callable and structural
  body facts for that function. The backend now records direct returned
  expressions and direct property/tuple member function literals using their
  exact spans, preserving transparent TypeScript wrappers. Admitted calls can
  project the closure's reads at invocation; construction still reads nothing.
  Missing bodies, unstable receivers and unsupported transport stay explicitly
  uncertifiable. The IR does not synthesize a missing implementation or borrow
  JSX/event execution for an eager call.

| Case | Finding | Why |
| --- | --- | --- |
| `createMutation()` called in `Screen`, `m.busy()` only in a JSX attribute | none | the closure runs in a tracked position |
| `createResetter()` with a block-bodied property closure, called from a handler | none | same hook shape, handler |
| `createFlag()` returning `[s, () => s() > 0]` | none | the exact tuple element now has structural body facts and runs in tracked JSX |
| a property closure that calls a bound helper | none | the call edge is inside the closure |
| getter in the returned object | none | held back as a property function |
| method in the returned object | none | a node of its own, never folded |
| the same hooks imported from `hooks.ts` (`Remote.tsx`) | none | the project-level summary follows the same rule across files |
| `createSnapshot()` reads its signal while it runs | `SC1001` violation at the call | a direct read by the hook |
| `createEager()` calls its own helper while it runs | `SC1001` violation at the call | the call edge is in the hook body |
| `createRemoteSnapshot()` imported from `hooks.ts` | `SC1001` violation at the call | direct read by the hook, across files |

**Stub.** `solid-js.d.ts` copies `createSignal` and `createMemo` verbatim from
the rc.9 typings, as its header lists; `App.tsx` passes `tsc --noEmit`
(strict) against the stub and against the real rc.9 install.

`DirectReturns.tsx` adds a transparent-wrapper direct-return control. Its
eager invocation reports SC1001 violation at `read()`; invocation only from
tracked JSX is clean. It passes `tsc --noEmit` against published RC.13 typings
without the fixture stub.
