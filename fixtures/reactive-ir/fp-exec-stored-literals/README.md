# fp-exec-stored-literals

**Claim.** Code written inside a component is claimed as running in the
component body (the strict-read window) only where the facts prove it runs
during the body. The body itself, an immediately invoked function, a
standard-library inline callback, a project function that calls its parameter
during the call, a control-flow component's render callback, and a named
helper every one of whose references is a call that is itself proven body-time
(including its default parameter, when every call omits the argument) all
qualify. Everything else nested in a component -- a function *stored* where it
is written (an object property, method or getter, an array element), a callback
handed to a consumer not proven to invoke it, a helper referenced as a value or
called from a handler or a tracked JSX attribute -- is not proven, and `SC1001`
is **uncertifiable** or absent, never a proven untracked read
(`execution_role::callee_callback_timing`, `named_helper_runs_during_body`,
`parameter_default_owner`).

Before, the lexical fallback gave every such literal the component body's role
and reported the read as a proven violation: `callee_callback_timing` answered
`false` for any literal that was not an argument of some call. Runtime ground truth (2.0.0-rc.9, dev, defect triage `repros-strict/rt*.mjs`):
a closure that is not called warns nothing; `[1].map(() => n())`, an IIFE and a
default parameter *called* in the body do.

| Case | Finding | Why |
| --- | --- | --- |
| `{ run: () => n() }`, `{ run() { n() } }`, `{ run: function () { n() } }`, `{ get value() { return n() } }`, `[() => n()]` | `SC1001` uncertifiable | stored; when it is invoked has no fact |
| `register({ run: () => n() })`, `keep(() => n())` (project helpers that keep it) | `SC1001` uncertifiable | the callee is not proven to invoke it during the call (unchanged) |
| `function capture(value = n())`, `(value = n()) => ...` called only from a handler | `SC1001` uncertifiable | the default is evaluated at the call, not at the declaration |
| helper called only from a handler / passed as a value / never called / called from a tracked JSX attribute (`title={label()}`) | none | its calls are not proven to run in the body |
| `const log = () => ...n()` called in the body, with or without another call from a handler | `SC1001` violation (through `log`) | a body-time call runs it |
| `inner`/`outer` chain called in the body | `SC1001` violation | every call is body-time |
| `function capture(value = n())` called in the body as `capture()` | `SC1001` violation | the call omits the argument and runs in the body |
| `capture(1)` (default never evaluated) | `SC1001` uncertifiable | the default is not shown to run |
| `const snapshot = n()` | `SC1001` violation | the component body |
| `(() => n())()` | `SC1001` violation | an immediately invoked function runs where it is written |
| `[1, 2].map((x) => x * n())` | `SC1001` violation | a standard-library inline callback runs during the call |
| `now(() => n())`, with `now` calling its parameter | `SC1001` violation | the callee calls it during the call (unchanged) |

Remaining approximation: a stored literal that is in fact invoked during the
body (`const o = { run: () => n() }; o.run();`) is uncertifiable, not a
violation; no fact here follows the object's member call.

**Stub.** `solid-js.d.ts` holds `createSignal` verbatim from the rc.9 typings
(plain-value overload). `App.tsx` passes `tsc --noEmit` (TypeScript 5.9.3,
`strict`) against the stub and against the real rc.9 install with
`jsxImportSource: "@solidjs/web"`.
