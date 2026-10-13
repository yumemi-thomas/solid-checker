# Parameter forwarding on every completion (ADR 0285)

Claim: `function f(cb) { return g(cb) }` calls `cb` during the call when the
forward is on `f`'s every-completion path, the argument is `f`'s own
parameter passed unchanged at an exactly mapped position, and `g` calls that
position on every completion. Chains compose to a fixpoint.

| Case | Shape | Expected |
| --- | --- | --- |
| `Forwarded` | `readNow(source)` | SC1001 at the call |
| `Chained` | `useForwarded(source)` | SC1001 at the call |
| `Guarded` | `if (flag) readNow(source)` | silent |
| `Deferred` | `() => readNow(source)`, read in JSX | clean |
| `Maybe` | helper calls it behind its own branch | silent |
| `Timer` | `setTimeout(() => readNow(source))` | silent |
| `Replaced` | forwards another value | clean |
| `AfterPattern` | a destructured parameter precedes `source` | silent |
| `ArrowTarget` | forwards into a `const` arrow | silent |
| `Evaluated` | a module with direct `eval` (`evaluated.ts`) | silent |

`Guarded`, `Maybe` and `Timer` are silent rather than uncertifiable: a
forward that is not proven to run was silent before this change too.
`ArrowTarget` is refused because only a function declaration is initialized
whenever the forward can run; a `const` callee may be in its temporal dead
zone (ADR 0218, review round 3). `AfterPattern` is refused because summary parameter positions skip
destructured parameters while the completion table's do not. The two SC9005
rows in `helpers.ts` are the open program's export obligations for the
callbacks `useDeferred` and `useTimer` hand to closures.

`solid-js.d.ts` is `context-member-identity`'s stub (claim-bearing signatures
copied from published 2.0.0-rc.13). The sources pass `tsc --noEmit`.
