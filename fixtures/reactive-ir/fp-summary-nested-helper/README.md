# fp-summary-nested-helper

**Claim.** A helper written inside a component (`const b = () => a() * 2`, a
nested function declaration) does not make the component body read what the
helper reads. A read through a helper is a strict-window read only at the
call that runs the helper: the summary path of `strict-read-untracked`
(`interproc::interprocedural_result_reads_for_file`) now skips a call written
inside a nested non-component function, exactly as the direct-read path
already did, so the read is reported where the helper is *called* in the body
(or in an untracked callback position) and nowhere else.

Before: the call `a()` inside `b` took the component's lexical role
(`UntrackedRendering`) because it is written inside the component, and every
derived chain whose end was only read in JSX, a handler or a memo was reported
as a proven untracked read. Measured on 1,962 findings over 38 real apps, this
pattern was about 573 false positives.

Measured against the published `solid-js`/`@solidjs/signals`/`@solidjs/web`
`2.0.0-rc.9` bytes (dev build): `STRICT_READ_UNTRACKED` is raised for the
direct body read and for a helper called while the body runs; a helper read
from a JSX child expression, a handler, or a memo raises nothing.

| Case | Finding | Why |
| --- | --- | --- |
| derived chain read in a JSX child | none | tracked JSX expression |
| chain through `createMemo`, read in JSX | none | same |
| helper passed to `onClick` by reference, or called in a handler literal | none | runs at event time |
| helper stored in an object, never called | none | no call, no read |
| helper called from an anonymous closure stored in an object property or an array element | none | nothing proves the closure runs while the body does (`runs_in_unproven_stored_literal`) |
| `read()` / `b()` called in the body | `SC1001` violation at the call | the call runs the helper inside the strict window |
| helper called from an immediately invoked literal in the body | `SC1001` violation | the literal runs where it is written |
| `Button` reading `props.variant` once in the body | `SC1001` violation | direct read, unchanged |

**Stub.** `solid-js.d.ts` copies `createSignal` and `createMemo` verbatim from
the rc.9 typings, as its header lists; `App.tsx` passes `tsc --noEmit`
(strict) against both the stub and the real rc.9 install.

The summary path attributes a call to the body only when its invocation during
that body is proven. The lexical component role it falls back to proves nothing
of the kind, so a call written in a literal that is stored, returned or set as a
JSX attribute value is asked first (`runs_in_unproven_stored_literal`) and a
literal that is a call argument keeps the callee-timing answer (a not-proven
result, never a proven one).

Remaining approximation: a helper handed to an unknown callee in the body
(`run(helper)`) is not reported here at all -- the summary path claims a read
only at a call that names the helper; whether the callee invokes it is the
callee-callback-timing question (`fixtures/reactive-ir/callee-callback-timing`).
