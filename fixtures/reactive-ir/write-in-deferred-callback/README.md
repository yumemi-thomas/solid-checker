# write-in-deferred-callback

**Claim.** `SC2001` `reactive-write-in-owned-scope` reports a write in a
function literal written in a component body only when the literal provably
runs during the body (`execution_role::nested_literal_runs_during_body`). It
must be a literal that is:

- handed to a project function that invokes that parameter during the call;
- a standard-library inline callback;
- an IIFE;
- a primitive's or a control-flow component's callback;
- a named closure whose call site is in the body.

Anywhere else, the write's role is what its invocation sites prove. With none,
it is unclassified and nothing is reported. An unproven write position is
never a violation.

Before, the rendering role placed every literal written in a component body in
the body, so each clean case below was reported as a proven violation. The
matching read was already a proof obligation (`strict-read-untracked`
uncertifiable, "passed to a function that is not proven to invoke it during
the call"). The `@solid-primitives/history` misuse-ledger twin was one such
case (`docs/package-contract-v2/phase22/2026-09-29-primitives-misuse-criterion3.md`).

| Case | Finding | Why |
| --- | --- | --- |
| `later(() => setCount(1))`, where `later` stores the callback for a timer | none | the literal runs from the interval, after the body |
| `keep(() => () => setCount(1))`, where `keep` calls its source and keeps the returned closure | none | the write is in the returned closure, which runs from a click |
| `UndoHistoryShape`: the source reads, and returns the setter closure | none for the write; `SC1001` violation for the read | the source runs during `keep`'s call in the body, so its read is untracked there; the setter closure runs later |
| `const reset = () => setCount(0)`, called only from `onClick` | none | its one call site is the event handler |
| `now(() => setCount(1))`, where `now` calls its parameter | `SC2001` violation | proven to run during the call, in the body |
| `const reset = …; reset();` in the body | `SC2001` violation | its call site is the body |
| `[1].forEach(() => setCount(1))` | `SC2001` violation | a standard-library inline callback |
| `(() => setCount(1))()` | `SC2001` violation | an IIFE runs where it is written |

`SC9005` on `later` is the existing callback-timing obligation of an
exported function that stores its parameter, and is unchanged.

**Stub.** `solid-js.d.ts` holds `createSignal` and its types, copied verbatim
from the rc.9 typings as its header lists. `App.tsx` type-checks cleanly
against it (`tsc --noEmit`, `strict`).
