# flush-closure-called-once

**Claim (ADR 1161).** `flush(fn)` runs `fn` on its caller's stack and keeps the caller's strict-read window. A `flush` call in a
closure that only in-place layers run (called once by name and referenced nowhere else, or an IIFE) runs where the outermost
layer's call runs. Main left the read inside unplaced (`function-run-unplaced`).

| component | outcome |
|---|---|
| `ClosureInBody`, `IifeInClosure`, `StoreMemberInBody`, `NamespaceFlushInBody` (called once in the body) | SC1001 violation (main: uncertifiable) |
| `ClosureInJsx`, `ClosureInHandler` (called once in a JSX expression or a handler) | clean (as on main) |
| `AlsoTimer` (also handed to `setTimeout`), `CalledTwice`, `HandedToForEach` | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/flush-closure-called-once` reports STRICT_READ_UNTRACKED at the four violations and at the body
calls of `AlsoTimer` and `CalledTwice`, and nothing at the rest. Stubs: `flush-write-path`'s (rc.21).
