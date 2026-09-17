# `implementationUnavailable`, diagnosed in part (2026-09-14)

135 recipe-less `reads` rows across three exports, and until now the only large
frontier block with no measured cause:

| rows | export | package |
| ---: | --- | --- |
| 62 | `defaultEquals` | `@solid-primitives/utils` (6.4.1 and 7.0.0-next.4) |
| 62 | `tryOnCleanup` | `@solid-primitives/utils` (both) |
| 11 | `EventClient` | `@tanstack/devtools-event-client` |

Probed against the **installed package**, not a synthetic — the lesson from the
`motion-utils` attempt earlier the same day, where a fixture that omitted the
packaging answered a different question.

## `defaultEquals` — 62 rows, and no premise in this family can close them

~~~js
import { …, equalFn, } from "solid-js";
export const defaultEquals = equalFn;          // dist/index.js:16
~~~

~~~
tdecl=VariableDeclaration @ @solid-primitives/utils/dist/index.js
path[0] seg=0 complete=true decl=nil
implementation: complete=false open=[implementationUnavailable] implDecl=nil
~~~

The export's value **is** `solid-js`'s `equalFn`. Its body is in a dependency,
so this package has no implementation to census and the producer says so
correctly. `implementationUnavailable` is the right answer, not a gap.

Nothing in the subject-root family reaches this. Closing it would mean reading
the closure out of `solid-js`'s own contract — dependency composition, a
different mechanism from every premise this phase has built, and one whose
reviewed surface today is a three-name list (`REVIEWED_DEPENDENCY_MEMBERS`).
Whether these 62 rows are worth that is a question about composition, not about
the `reads` census.

## `tryOnCleanup` — 62 rows, not reproduced, cause still unknown

~~~js
export const tryOnCleanup = isDev
    ? fn => (getOwner() ? onCleanup(fn) : fn)
    : onCleanup;                                // one arm is solid-js's
~~~

The obvious reading is that it shares `defaultEquals`' cause through its second
arm. **The measurement refuses to confirm it**: against the installed package
the producer states `complete=true` with the local arrow as the implementation,
exactly as the control `trueFn` does. Whatever makes the corpus refuse it is not
reproduced by demanding the transcript at that binding.

So this is recorded as undiagnosed. The obvious reading may still be right; it
is not measured, and this phase has already paid twice for reasoning from an
unreproduced shape.

## One probe artifact, recorded because it looked like a finding

A tsconfig whose `include` listed both `dist/index.js` and `dist/index.d.ts`
made **every** export lose its implementation — `trueFn` included, which
certifies fine in the corpus. TypeScript resolves the module to the declaration
file and the `.js` is never analyzed. The js-only project is the valid one.

A probe that breaks the control is a broken probe, and the control is what
caught it.

## Where this leaves the block

- **62 rows** have a measured cause that no premise in this family addresses.
- **73 rows** (`tryOnCleanup` 62, `EventClient` 11) remain unexplained.

The block should no longer be ranked as one 135-row premise opportunity. At
least 62 of it is a composition question, and the rest is not yet understood
well enough to cost.
