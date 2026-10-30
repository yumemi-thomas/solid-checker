# named-apply-helper-callback

`strict-read-untracked` (SC1001) for a read in a function nested in an effect apply passed by name. A function the in-place layers run (a project helper's callback it calls in its own body, an IIFE) runs at a call in the callback's body, so its read takes the role the named callback gives that call, as the same read in an inline apply does (`climb_in_place_layers`). Before this fixture every function nested in a named callback was placed deferred, a role no rule reports: the read was a silent clean (fuzzer recall grid, `callback:*` forms, 2026-10-31).

Violations (rc.17 reports STRICT_READ_UNTRACKED at each, Chrome-witnessed by the fuzzer's recall grid):

- `NamedApplyHelper` (line 12): the helper's callback in a named `createEffect` apply.
- `NamedRenderApplyHelper` (line 23): the same in a named `createRenderEffect` apply.
- `NamedApplyIife` (line 34): an IIFE in a named apply.
- `InlineApplyHelper` (line 45): the inline control, reported before the change too.

No SC1001:

- `NamedApplyTimer` (line 55): a timer callback runs later, outside the apply.
- `NamedApplyStored` (line 67): a function stored and never called here; no layer runs it in place.
- `NamedComputeHelper` (line 78): the helper's callback in a named tracked compute, where a read tracks.

Declaration stubs: `callback-containment`'s `solid-js.d.ts` (published 2.0.0-rc.9 and rc.13 typings, as recorded there).
