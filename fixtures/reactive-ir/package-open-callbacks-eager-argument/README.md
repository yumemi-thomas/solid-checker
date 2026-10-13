# An eager argument to an export with open `callbacks`

Pins ADR 0237. A contract that leaves `callbacks` open says nothing about when
the export runs a *function it receives*. It says nothing about when the caller
evaluates an argument, because the caller does that before the call. Before
the ADR, the whole argument expression was treated as callback code of
unknown timing, so `withDefaults({}, overlay())` in a component body hid a
proven untracked read.

| component | argument | verdict |
| --- | --- | --- |
| `EagerRead` | `overlay()` at 1 | SC1001 violation |
| `EagerSpread` | `...overlays()` | SC1001 violation, plus the SC9005 the spread already carried |
| `EagerFirst` | `overlay()` at 0 of `withOverrides` | SC1001 violation |
| `DeliveredCallback` | `() => overlay()` | no SC1001: the timing is unknown |
| `WrappedCallback` | the literal behind `as` | same as `DeliveredCallback` |
| `SelectedCallback` | a literal chosen by `?:` | the literal's body keeps unknown timing |

An IIFE argument, `(() => overlay())()`, is eager too. Its body is a function
inside the argument, so it stays shielded: the region is narrowed to function
bodies rather than proven per function. That is an under-report, not covered
here.

The contract is accepted out of band through `.solid-checker/authorize-contract.json`,
as in `package-merged-props-consumer`; an unaccepted contract would make every
component report the same `SC9005`. `solid-js.d.ts` copies `createSignal` from the
published 2.0.0-rc.9 typings, and `tsc --noEmit` is clean on this directory.
