# array-shape-v1

The `arrayShape` fact (solid-ts-facts ADR 0015) at its two consumers on Solid
1.x: `v1/no-array-handlers` (SC8007), which asks whether a JSX event-handler
value is an array, and `v1/prefer-for` (SC8012), whose `<For each>` autofix
requires the `.map` receiver to be a proven array.

Both questions were previously answered by matching `TypeDescriptor.text`
against `[`, `readonly `, `Array<`, `ReadonlyArray<`, and a trailing `[]`. This
fixture pins what text could not reach:

- **`handler-cases.tsx`** — the positives, all of them bound-handler tuples. An
  **aliased tuple renders as its alias**, so `type Handlers = [...]` matched no
  prefix and the rule went silent on a real defect. Also here: a doubly-aliased
  tuple, a tuple returned from a call, a readonly tuple, and an inline literal.
- **`clean-cases.tsx`** — the negatives that a weaker classification would break.
  `arrayReturning: () => string[]` renders with the same trailing `[]` as an
  array of functions, which is why the text screen needed a second fact
  (callability) to tell them apart; the compiler's predicate separates them
  directly. `SafeArray<T> extends Array<T>` is array-*like* and deliberately
  classifies as `notArray`: the fact uses the compiler's own
  `isArrayOrTupleType`, which requires the global `Array`/`ReadonlyArray` type or
  a tuple, because an author who declares a wrapper chose it over an array. That
  is the same vouching upstream honours for a cast.
- **`fail-closed-cases.tsx`** — absence, `mixed`, and `unknown` are "not proven",
  never "not an array". Type parameters (constrained or not), a union mixing both
  shapes, `any`, and an unresolved import all report nothing. These are
  deliberate false negatives; see `docs/precision-backlog.md`.
- **`map-receiver-cases.tsx`** — `prefer-for`. Every case reports; only the
  proven-array receivers carry the autofix.

## What the text screen actually missed

Measured by reverting the two consumers and re-running this fixture. It found 6
findings where the fact finds 9, and it lost both autofixes.

| Case | Text screen | `arrayShape` |
| --- | --- | --- |
| `onClick={aliased}` (aliased tuple) | silent | **SC8007** |
| `onClick={nested}` (aliased twice) | silent | **SC8007** |
| `onClick={makeHandlers()}` (call returning the alias) | silent | **SC8007** |
| `aliasedArray.map(...)`, `type Rows = string[]` | reports, **no fix** | reports **+ fix** |
| `aliasedTuple.map(...)` | reports, **no fix** | reports **+ fix** |

Every row is an alias question; no case lost a finding.
The `prefer-for` rows are the same alias hole reaching a second rule: an alias for
`string[]` renders as `Rows`, so the receiver was never *proven* an array and the
`<For each>` rewrite was withheld from code it was correct for.

## Why these findings are the checker's own

Checked with

~~~sh
node scripts/tsc-oracle.mjs check --dialect v1 --file <case>
~~~

against the **real** solid-js@1.9.14 typings, in both the strict and non-strict
passes.

`handler-cases.tsx` — which holds every SC8007 in this fixture — is **completely
silent** to `tsc`. `onXxx` is typed `EventHandlerUnion = EHandler |
BoundEventHandler`, and `BoundEventHandler` is an *interface* with members `0`
and `1`, so a `[handler, data]` tuple satisfies it however it is spelled: bare,
aliased, aliased twice, readonly, inline, or returned from a call. Only this rule
can object to those, and its claim — the value is a bound-handler tuple where a
function was meant — is not a type error's claim.

`clean-cases.tsx` contains one case `tsc` *does* reject — `onClick={safe}`, where
`SafeArray<T> extends Array<T>` — and the checker reports nothing there, so it is
not a duplicate either.

A **plain array** on `onXxx` (`X[]`, `ReadonlyArray<X>`, `any[]`, `unknown[]`)
has no `0`/`1` members and is likewise already TS2322. The rule still reports
those, which is a known duplicate recorded in `docs/precision-backlog.md`; no
such case is pinned here, because this fixture must not bless it.

## About the stub

`solid-js.d.ts` types `JSX.IntrinsicElements` with an index signature. That is
looser than the real package, which is exactly why the claims above are settled
by the oracle rather than by this fixture. It cannot manufacture a finding here
regardless: no rule in this fixture reads the *attribute's* declared type. Every
proof reads the **value's** type (`Handlers`, `SafeArray<number>`,
`() => string[]`, `Rows`), declared in the case files themselves and
byte-faithful to what a real project would write.

The `node_modules/solid-js/package.json` stub pins the 1.x dialect; without it
the whole fixture silently runs the v2 catalog and reports nothing.
