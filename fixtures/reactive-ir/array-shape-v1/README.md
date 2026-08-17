# array-shape-v1

The `arrayShape` (solid-ts-facts ADR 0015) and `tupleShape` (ADR 0016) facts at
their consumers on Solid 1.x: `v1/no-array-handlers` (SC8007), which reports a
`[handler, data]` bound-handler pair on a JSX event prop, and `v1/prefer-for`
(SC8012), whose `<For each>` autofix requires the `.map` receiver to be a proven
array.

Both questions were once answered by matching `TypeDescriptor.text` against `[`,
`readonly `, `Array<`, `ReadonlyArray<`, and a trailing `[]`. This fixture pins
what text could not reach — and, for SC8007, the boundary with the type checker,
which is what makes it two files that must each stay exactly as they are:

- **`handler-cases.tsx`** — the positives, all of them bound-handler tuples. An
  **aliased tuple renders as its alias**, so `type Handlers = [...]` matched no
  prefix and the rule went silent on a real defect. Also here: a doubly-aliased
  tuple, a tuple returned from a call, a readonly tuple, and an inline literal.
- **`clean-cases.tsx`** — the negatives, and the fixture's other half. Four are
  the shapes the `tupleShape` narrowing removed (a plain array, a tuple with a
  non-callable first slot, a one-slot tuple, `[1, 2, 3]`); two are the `on:`
  cases; the rest are negatives a weaker classification would break —
  `arrayReturning: () => string[]`, which renders with the same trailing `[]` as
  an array of functions, and `SafeArray<T> extends Array<T>`, which is
  array-*like* and deliberately `notArray`.
- **`fail-closed-cases.tsx`** — absence, `mixed`, and `unknown` are "not proven",
  never "not an array". Type parameters (constrained or not), a union mixing both
  shapes, `any`, and an unresolved import all report nothing. These are
  deliberate false negatives; see `docs/precision-backlog.md`.
- **`map-receiver-cases.tsx`** — `prefer-for`. Every case reports; only the
  proven-array receivers carry the autofix.

## What each fact contributes

Both rows below are measured by reverting the consumers and re-running this
fixture.

**`arrayShape` (ADR 0015) closed false negatives that text could not reach.**
The text screen matched `TypeDescriptor.text` against `[`, `readonly `,
`Array<`, `ReadonlyArray<`, and a trailing `[]`:

| Case | Text screen | `arrayShape` |
| --- | --- | --- |
| `onClick={aliased}` (aliased tuple) | silent | **SC8007** |
| `onClick={nested}` (aliased twice) | silent | **SC8007** |
| `onClick={makeHandlers()}` (call returning the alias) | silent | **SC8007** |
| `aliasedArray.map(...)`, `type Rows = string[]` | reports, **no fix** | reports **+ fix** |
| `aliasedTuple.map(...)` | reports, **no fix** | reports **+ fix** |

An alias renders as its own name, so every prefix test missed it. The two
`prefer-for` rows are the same hole reaching a second rule: the receiver was
never *proven* an array, so the `<For each>` rewrite was withheld from code it
was correct for.

**`tupleShape` (ADR 0016) closed the duplicates `arrayShape` could not.**
`arrayShape` alone reports 9 SC8007 here; `tupleShape` narrows that to 5,
removing exactly the four `tsc` already rejects:

| Case | `arrayShape` only | `tupleShape` | `tsc` says |
| --- | --- | --- | --- |
| `onClick={plainArray}` | SC8007 | silent | missing properties `0`, `1` |
| `onClick={notCallableHead}` | SC8007 | silent | property `0` incompatible |
| `onClick={oneSlot}` | SC8007 | silent | property `1` missing |
| `onClick={[1, 2, 3]}` | SC8007 | silent | element 0 not a handler |

No legitimate finding was lost in either step.

## Why these findings are the checker's own

Checked with

~~~sh
node scripts/tsc-oracle.mjs check --dialect v1 --file <case>
~~~

against the **real** solid-js@1.9.14 typings, in both the strict and non-strict
passes.

`handler-cases.tsx` — which holds every SC8007 in this fixture — produces
**zero** `tsc` diagnostics. `onXxx` is typed `EventHandlerUnion = EHandler |
BoundEventHandler`, and `BoundEventHandler` is an *interface* with members `0`
and `1`, so a `[handler, data]` tuple satisfies it however it is spelled: bare,
aliased, aliased twice, readonly, inline, or returned from a call. Its first
member is typed `(data: any, ...e) => void` — `any`, so the data the handler
receives is never checked against the data the tuple carries. That unchecked
seam is the finding, and only this rule can report it.

`clean-cases.tsx` is the mirror image: **seven** `tsc` diagnostics and **zero**
findings. The rule and the type checker partition the space exactly, and `tsc`
names each reason:

~~~
SafeArray<number>                 missing the following properties: 0, 1
((event: MouseEvent) => void)[]   missing the following properties: 0, 1
[number, number]                  Types of property '0' are incompatible
[(event: MouseEvent) => void]     Property '1' is missing
[1, 2, 3]                         Type 'number' is not assignable to (data: any, e) => void
on:click={boundPair}              EventHandlerWithOptionsUnion has no bound arm
on:click={[plain, 1]}             the same
~~~

The two `on:` rows are the 2026-08-18 arm removal; see
`fixtures/upstream-parity/deviations.json`'s `no-array-handlers__invalid__03`.

## About the stub

`solid-js.d.ts` carries the **real** `EventHandler`, `BoundEventHandler`,
`EventHandlerUnion`, and `EventHandlerWithOptionsUnion` signatures from
`solid-js@1.9.14`'s `types/jsx.d.ts`. That is load-bearing, not thoroughness.

Contextual typing is what gives an inline `[handler, data]` literal its fixed
slots — the same literal in an unconstrained position stays a plain array. A
permissive `IntrinsicElements` index signature, which this fixture used before
2026-08-18, erases that: every literal arrives as an array, `tupleShape` is
absent everywhere, and the fixture silently stops exercising the path it exists
for while still passing. Everything else in the stub is reduced; only these
signatures are byte-faithful, and only they need to be.

The `node_modules/solid-js/package.json` stub pins the 1.x dialect; without it
the whole fixture silently runs the v2 catalog and reports nothing.
