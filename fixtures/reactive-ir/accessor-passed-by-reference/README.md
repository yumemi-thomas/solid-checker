# accessor-passed-by-reference

**Claim.** An accessor passed by reference to a project function, `run(count)`,
is read at that call when the callee's own synchronous body calls the parameter
(`execution_role::invokes_parameter_during_call`, applied in
`interproc::interprocedural_result_reads_for_file`). The read then takes the
call's role: `SC1001` `strict-read-untracked` in a component body, clean in a
tracked JSX expression or an event handler.

Before, only a function *with a summary* passed by reference contributed its
reads, so a bare accessor argument contributed none and `run(count)` in a body
was silent: a missed true positive.

A call of the parameter counts only when it is written in the callee's own
body. A call inside a timer callback, a returned closure, or an async body is
not proven to run during the call, so it contributes no read.

| Case | Finding | Why |
| --- | --- | --- |
| `run(count)` in the body; `run` returns `read()` | `SC1001` violation | the callee's body reads `count` during the call |
| `readNow(count)`, the same helper from `helpers.ts` | `SC1001` violation | cross-module resolution of the callee |
| `<p>{run(count)}</p>` | none | a tracked JSX expression |
| `onClick={() => run(count)}` | none | an event handler |
| `later(count)`; `later` calls it from `setTimeout` | none | the call is in the timer callback, not in `later`'s body |
| `keep(count)`; `keep` pushes it into an array | none | nothing calls it during the call |
| `wrap(count)`; `wrap` returns `() => read()` | none | the call is in the returned closure |
| `settle(count)`; `settle` is `async` and calls it after an `await` | none | an async body is not proven synchronous |

`SC9005` on `keep` is the existing callback-timing obligation of an exported
function that stores its parameter, and is unchanged.

**Stub.** `solid-js.d.ts` holds `createSignal` and its types, copied verbatim
from the rc.9 typings as its header lists. The project type-checks cleanly
against it (`tsc --noEmit`, `strict`).
