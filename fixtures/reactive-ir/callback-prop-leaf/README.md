# callback-prop-leaf

**Claim (ADR 1190, slice 1).** A component prop's function literal whose runs ADR 1171 proves are the child's calls of
`props.run` is walked at those calls. Two changes do it:
- The leaf walk resolves `props.run()` to the literal (`prop_literal_called_at`).
- The unread-package question asks only where those calls run (`package_hand_off`). A JSX attribute value is otherwise read
  as handed to the web runtime, so the B3 gate (`unread-package-may-run`) used to decline every census row in the literal.

| component | outcome |
|---|---|
| `Body` (called in the child's body) | clean (main: SC3001/SC6002 `leaf-operation-unaccounted`, `effect-creation-unaccounted`) |
| `Settled` (called in `onSettled`) | SC3001 violation at the call (main: uncertifiable) |
| `Tracked` (called in a tracked effect, the literal flushes) | SC3001 violation at the call; SC2008's flush census row stays |
| `Never` (never called) | rows kept (the dead-site ruling) |
| `Spread` (a spread on the element) | rows kept (ADR 1171 proves nothing there) |

Chrome: `runtime-oracle/callback-prop-leaf`. Settled is covered-violation and Spread covered-uncertifiable; Body is quiet.
Tracked's flush throws without frames, so its call-site violation is ledgered as `attribution` (ADR 1180's case).
