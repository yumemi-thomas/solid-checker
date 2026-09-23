# What holds an import open

- Measured 2026-09-23 with
  [`2026-09-23-what-holds-an-import-open.mjs`](2026-09-23-what-holds-an-import-open.mjs)
  on that day's `make contract-coverage-census` run (finished 01:08:42Z). The
  per-export evidence is
  [`2026-09-23-what-holds-an-import-open.json`](2026-09-23-what-holds-an-import-open.json),
  kept because the run's catalogs live in a temporary directory and will not.
- The run was built from a working tree ahead of 859db58b: the 2026-09-18
  recipes and ADR 0112, and later producer work that closes `callbacks` and
  `reads` on `substract`, `multiply`, `divide` and `power`. Those four stay open
  at every import either way, and no conclusion below rests on them.
- The census's consumer view (859db58b) says what an import finds open. This
  says why, for the 994 of 1,152 in-surface sites it does not clear: 533 open at
  every import, 461 on some uses.

## `returns` is open because nothing proposes it

Each open consumer domain has exactly one status, read from the row whose
document gave the census its answer:

| status | `returns` | `callbacks` | `creates` | `reads` |
| --- | ---: | ---: | ---: | ---: |
| never proposed | **887** | 163 | 196 | 4 |
| withheld: census refused | 0 | 474 | 216 | 80 |
| withheld: other | 0 | 0 | 8 | 22 |
| declined at generation | 71 | 68 | 111 | 22 |
| proposed, not certified | 26 | 32 | 0 | 1 |

**On 887 sites `returns` is open with no proposal and no recorded decline.** The
generator proposes a `returns` closure in exactly two shapes: `returns: []` for
a body that completes without a value (`returns_walk.rs`, ADR 0035), and one
return of a whole parameter (ADR 0075). Anything else a function returns,
`() => true` included, leaves the domain unknown. The pinned 2026-09-14 report
shows the same on both majors: `@solid-primitives/utils` has `returns` unknown
on 35 of 35 exports at 6.4.1 and 96 of 99 at 7.0.0-next.4.

**It was invisible for a structural reason.**
[The 2026-09-14 ranking](../phase21/2026-09-14-which-closures-change-a-consumer-finding.md)
counted `returns` open on 36 sites (§ 5) and listed `noop`, `asArray`, `trueFn`
and `accessWith` among exports "ALL CLOSED" (§ 3). Its ledger was built from
withheld closures, and a claim nobody proposed is never withheld: the pinned
demand rows still record `@solid-primitives/utils` `clamp` as `"state":
"all-closed"` with `returns` open. This is the probe-recipe trap again at
another layer: a record names only what was asked.

## The largest lever: a `returns` closure for a primitive completion

Nine exports have `returns` as their only open consumer domain, 242 sites (the
demand lists `@solid-primitives/utils` `clamp` twice). Six of them return a
primitive on every completion, read off the published bytes:

| export | sites | implementation |
| --- | ---: | --- |
| `@solid-primitives/utils` `noop` | 98 | `(() => void 0)` |
| `@solid-primitives/utils` `trueFn` | 27 | `() => true` |
| `@kobalte/utils` `clamp` | 16 | `Math.min(Math.max(value, min), max)` |
| `@solid-primitives/utils` `isObject` | 10 | `value !== null && (typeof … \|\| typeof …)` |
| `@solid-primitives/utils` `clamp` | 7 | `Math.min(Math.max(n, min), max)` |
| `@solid-primitives/utils` `falseFn` | 3 | `() => false` |

**161 sites.** Closing `returns` on these makes them the first callables in the
corpus that a consumer imports with nothing open; the census's `clean` bucket is
0 today.

Each piece already exists on its own:

- the semantic model allows it: `returns` closed with one `return` operation
  whose `output` is `plain` (§ returns);
- ADR 0045's `primitiveCompletion` is the producer's statement that every
  completion of an implementation is a union of primitive types, under the
  premise the census already classifies the body with;
- ADR 0075 is the shape of a nonempty `returns` closure the census accepts
  (exactly one return operation, a plain completion form, an authenticated
  control-flow census), and ADR 0096 is the synthesized-veto pattern (observe
  every normal completion; here, a contradiction when `typeof` is `object` or
  `function`).

What is missing is the joint: a generator proposal of one `return` with a
`plain` output, a census rule that accepts it on `primitiveCompletion`, and that
veto observation. That is an ADR the size of ADR 0075. **Not measured:** whether
the producer states `primitiveCompletion` for these six, because the run keeps
no implementation transcripts. Establishing it is the ADR's first step.

The other 81 sites on the list, `asArray` (51), `accessWith` (27) and
`createIdGenerator` (3), return the caller's value, a new array holding it, or a
function. Those are structured outputs ADR 0075 excludes, and a later step.

## What holds `creates` open: the every-import half

`creates` is open on 531 of the 533 sites that raise SC9005 wherever the name is
imported (`reads` on 129), so it is the domain that decides that bucket.

| holding | sites | status |
| --- | ---: | --- |
| `@kobalte/utils` `callHandler` (112), `composeEventHandlers` (48) | 160 | census refused: an unresolved callee, `handler[0](handler[1], event)`, a call through an element of a parameter |
| `@solid-primitives/utils` `entries` (37), `keys` (22) | 59 | never proposed: aliases of `Object.entries` / `Object.keys`; ADR 0112 closes their `reads` by identity, and nothing states their `creates` |
| `@solid-primitives/utils` `tryOnCleanup` | 37 | never proposed: wall 1b of the 2026-09-18 backlog entry |
| calls into `getOwner`, `runWithOwner`, `onCleanup`, `createSignal`, `createEffect`, `createMemo`, `createContext`, `useContext` | 74 | declined: dialect-silent |
| the `./immutable` family (`pick`, `push`, `update`, `concat`, …) | 100 | never proposed |

The dialect-silent row is three different things. `solid-js` 2.0 re-declares
`createSignal` and `createMemo` with bodies that reach `ctx.serialize`, and the
audit withholds their rows on purpose (`creates_walk.rs`). `runWithOwner`,
`createEffect`, `createContext` and `useContext` have no `creates` row at all.
`getOwner` and `onCleanup` have rows under `@solidjs/signals` that these calls
do not reach, and why is not settled here. These are the exports that would
carry an owner requirement, such as `createTween` calling `createEffect`, so
this is the row that stands between the contracts and more `missing-owner`
findings on consumer code.

## Order

1. The primitive-completion `returns` closure: 161 sites to nothing open, on a
   fact the producer already has a form for (ADR 0045), once it is shown to be
   stated for these six.
2. `creates` for the default-library aliases `entries` and `keys` by the same
   identity ADR 0112 uses for `reads`: 59 sites from every-import to some-uses.
3. The dialect's `creates` rows for the owner-requiring primitives: 74 sites,
   audit readings rather than premises, and the one step here that turns
   contracts into misuse findings.
