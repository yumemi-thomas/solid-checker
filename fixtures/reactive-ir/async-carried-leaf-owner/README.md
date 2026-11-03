# async-carried-leaf-owner

**Claim (ADR 0580).** A pending async read written in a named function that its binding hands to inline carriers runs where those carriers run, so its leaf owner (`onSettled`, `createTrackedEffect`) is the carrier's, never the read's own position: the first carrier under a leaf owner answers (following up to four named functions out), and ADR 0563's `carrier_provably_runs` decides whether that run is proven.

Violations (SC5001): `SettledNamedForEach` (`[1].forEach(hf)` in a named `onSettled` callback), `SettledSharedCall` (`hf.call(null)`, one callback handed to two `onSettled` calls), `TrackedNamedTagged` (``hf`x` `` in a named `createTrackedEffect` callback), `SettledTwoCarriers` (`Array.from([1], hf)` in a function `Reflect.apply` runs).

Uncertifiable, never a violation: `SettledNamedEmpty` (an empty receiver), `SettledInlineEmpty` (the `onSettled` literal is around `hf` lexically, but only an empty receiver runs it: a proven violation before), `SettledProjectForEach` (a project `forEach`: no carrier; SC9012 at the call).

Silent: `MemoNamedForEach` (the named function runs in a memo compute, which suspends).

Runtime: rc.17 throws `PENDING_ASYNC_FORBIDDEN_SCOPE` at each violation (fuzzer mx-pairs PENDING_ASYNC_FORBIDDEN_SCOPE `settled`/`tracked` × `handoff-*` × `cb-shared`, 96 false cleans, all certified, before this change). Stub: `async-carrier-call-attributed`'s. `tsc --noEmit` clean against the published rc.17 typings (tsrs).
