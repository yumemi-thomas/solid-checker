# carrier-call-row

**Claim (ADR 1186).** A call that runs what a carrier held keeps an SC9012 uncertifiable row of its own, beside the carrier's.
Here the call is `(f as any)(...xs)`, through a `for…of` binding over `fns = [setW1]`. The census program
`mx-reactive-write-in-owned-scope-memo-shared-signal-loop-callee-module-helper` was a false clean by the fuzz judge: its only row stood at
the carrier `fns` (7:52 in the census text), where no Chrome frame is. The call's role was read as a handler's and cleared the row
(the button also calls `g1()`).

| site | outcome |
|---|---|
| `fns` (11:52) | SC9012 uncertifiable, as on main |
| `(f as any)(...xs)` (11:65) | SC9012 uncertifiable (main: none) |
| `DirectInHandler` | clean |

Chrome (`runtime-oracle/carrier-call-row`): REACTIVE_WRITE_IN_OWNED_SCOPE at line 11, covered-uncertifiable. The oracle judges by line;
the fuzz judge needs the column the frame records.
