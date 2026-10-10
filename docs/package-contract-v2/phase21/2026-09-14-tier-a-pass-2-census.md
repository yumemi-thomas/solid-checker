# Tier A pass-2 census: what a recipe can still buy (2026-09-14)

The 2026-09-13 depth plan sized its Tier A rows by withheld count and guessed
at decidability for all but two of them. This is those guesses measured. It was
run after the twenty-four-recipe batch of 2026-09-14 (`63db68fb`), against that
commit's pin — 418 rows, 2,090 recipe-less `reads` detail rows.

Nothing here is authored. The whole point of a pass-2 run is to learn, before
writing anything, which candidates a recipe can serve at all: "no recipe in
corpus" weakens a candidate out of the plan before its census runs, so a
refusal underneath stays masked (§ 43.3 of the reads-veto design record).

## Correction: a standalone project does not reproduce a graph-lane case

The plan's § 1 says "case ids equal the corpus's, so a scratch project is
enough". **That is true only for cases the corpus certifies as a root row**
(`lane: reused-proposal`). Ten of the twelve Tier A cases are
`lane: published-graph` — the package is a *dependency node* of some root — and
a standalone certification of that package produces a different artifact case
with different claim ids:

| case | corpus lane | standalone `--entrypoint` run produced |
| --- | --- | --- |
| `6f867f0c` `@solid-primitives/scheduled@2.0.0-next.2` | reused-proposal | `6f867f0c` — **exact match** |
| `b70ad6d1` `@solid-primitives/utils@7.0.0-next.4` `./immutable` | reused-proposal | `b70ad6d1` — **exact match** |
| `1116a60f`, `0cf47e45` `@corvu/utils@0.4.2` `./create/keyedContext` | published-graph | `5514c3a5`, `2b83335f` — different |
| `e1a524fa` `@solid-primitives/scheduled@1.5.3` | published-graph | `3706615c` — different |
| `b97f9095` `@solid-primitives/refs@1.1.4` | published-graph | no candidate at all |

The fix is to certify the **root** package with `--dependency-graph-lane
--recover-entrypoints` and let the dependency come in transitively at whatever
version the root pins. Doing that against `@corvu/accordion@0.2.5` reproduced
all eight corvu Tier A case ids exactly; `@kobalte/utils@0.9.2` reproduced
`b97f9095` and `@corvu/drawer@0.2.4` reproduced `e1a524fa`. A second trap sits
just past it: the scaffold's `--specifier` decides what each emitted module
imports, so on a graph-lane run it must name the **dependency** under study,
not the root. Naming the root makes every gate fail with "the probe worker
could not resolve …", which reads like a verdict and is not one.

`@solid-primitives/refs@1.1.4` certified standalone to `status: certified` with
*zero* `reads` candidates — neither decidable nor refused. That is worth saying
plainly because it is the shape most likely to be misread as "nothing to do":
the candidates exist only in the graph-lane planning of a consumer.

## Measured verdicts

Decidable = the census did not refuse, and the candidate is withheld only
because the emitted scaffold still throws its `UNFINISHED` guard. Refused = the
implementation census refused, which no recipe can serve.

| case | rows | gainable | blocked | decidable exports | refused exports |
| --- | --- | --- | --- | --- | --- |
| `1116a60f` | 48 | **48** | 0 | `createKeyedContext`, `getKeyedContext`, `useKeyedContext` | — |
| `0cf47e45` | 48 | **48** | 0 | same three | — |
| `a18d59ef` | 18 | **18** | 0 | `default` | — |
| `0702583f` | 18 | **18** | 0 | `default` | — |
| `1b08bdc4` | 16 | **16** | 0 | `default` | — |
| `9b53a55f` | 16 | **16** | 0 | `default` | — |
| `b97f9095` | 36 | **24** | 12 | `defaultElementPredicate`, `getFirstChild`, `getResolvedElements`, `resolveFirst` | `Ref`, `resolveElements` |
| `e1a524fa` | 24 | **12** | 12 | `createScheduled`, `debounce`, `leading` | `leadingAndTrailing`, `scheduleIdle`, `throttle` |
| `6f867f0c` | 12 | **6** | 6 | same three | same three |
| `f34410e8` | 33 | 0 | 33 | — | `combineStyle` |
| `fd42a1d9` | 33 | 0 | 33 | — | `combineStyle` |
| `b70ad6d1` | 22 | 0 | 22 | — | all eleven |
| **total** | **324** | **206** | **118** | | |

`d22fd9a2` and `3df9bd5d` (`@solid-primitives/refs@3.0.0-next.0`, 24 rows) stay
**unmeasured**. Their only corpus root is `motion-solidjs@0.7.0-beta.4`, whose
graph finalization hits the already-recorded `motion-utils@12.39.0` blocker
("runtime implementation does not match the snapshot-replayed export binding").
They are not counted above in either column.

## Where the plan's guesses were wrong

Four of six, in both directions, which is the argument for running pass 2
rather than authoring from a reading of the source:

- **`./create/keyedContext` (96 rows).** Guessed "expect *waits on a withheld
  `solid-js` claim*". Measured **fully decidable** — the largest single
  authoring opportunity left in Tier A, and the plan had it as doubtful.
- **corvu `./create/*` `default` (68 rows).** Guessed "likely dependency-claim
  waits". Measured **fully decidable**.
- **`./immutable` (22 rows).** Guessed "pure array/object helpers, cheap".
  Measured **zero gainable**: every one of the eleven exports the corpus still
  withholds is census refused. This is the guess that would have cost the most
  — eleven recipes written against candidates no recipe can serve.
- **`@solid-primitives/refs`.** Guessed "expect `instanceof Element` refusals on
  the predicates". The predicates are exactly the decidable half; the refusals
  are `Ref` and `resolveElements`, on unrooted property access.
- **`@solid-primitives/scheduled`.** Guessed `createScheduled` would wait on a
  `solid-js` claim through `createSignal`. It is decidable; the refusals are
  `throttle`, `scheduleIdle` and `leadingAndTrailing`.
- **`combineStyle` (66 rows).** Guessed census refused. **Confirmed.**

## The 118 blocked rows are Tier B evidence, not noise

Every refusal in this census is one of the premises the plan already names,
which is a useful corroboration that Tier B is aimed correctly:

- `iteration-protocol (SpreadElement)`, no reviewed subject root — `throttle`,
  `scheduleIdle`. This is **B-4** (a spread of a rest-parameter alias).
- `coercion (BinaryExpression)`, no reviewed subject root — `add`, `divide`,
  `multiply`, `power`, `substract`. **B-5** territory.
- `property-access-unknown-accessor` on an element or property access with no
  reviewed root — `concat`, `omit`, `pick`, `split`, `update`, `sortBy`,
  `flatten`, `leadingAndTrailing`, `Ref`, `resolveElements`. **B-5/B-6**.
- "the invocation of a member of parameter 1 … sits inside a callable nested in
  the implementation, so its execution point is not the call event" —
  `filterInstance`, `filterOutInstance`. This is **B-6** (a captured parameter
  root) stated almost in the ADR's own words.

So the immutable case that yields nothing to authoring is the one that most
directly motivates B-5 and B-6.

## Reproducing

`--dependency-graph-lane --recover-entrypoints` on the root, `--specifier` on
the dependency, two passes, scratch corpus never the checked-in one:

~~~sh
# root-row case: the standalone project reproduces the corpus case id
node pass2.mjs --label immutable --dep @solid-primitives/utils \
  --dep-version 7.0.0-next.4 --solid 2.0.0-rc.3 --entrypoint ./immutable

# graph-lane case: certify the root, scaffold against the dependency
node pass2.mjs --label gl-accordion --dep @corvu/utils --dep-version 0.4.2 \
  --root-package @corvu/accordion --root-version 0.2.5 --solid 1.9.14 \
  --entrypoint . --graph-lane --recover-entrypoints
~~~

Audited Solid versions are 1.9.14 and 2.0.0-rc.3; `motion-solidjs@0.7.0-beta.4`
resolves `solid-js@2.0.0-rc.8` transitively, which is deliberately **not**
substituted here.

## Addendum: the two largest remaining cases, censused (2026-09-14)

Run after Tier A authoring closed, against the 1,884-row frontier.

### `9887e137` — `@solid-primitives/utils@6.4.1`, 336 rows

**Zero gainable by authoring.** All eight exports the corpus still withholds
are census refused. The thirty decidable exports on this case are already
closed, which is why they carry no recipe-less rows.

What the eight refuse with matters more than the count, because they do not
share a premise — they split across five:

| export | refusal | premise |
| --- | --- | --- |
| `keys`, `entries` | `domain-exhaustiveness` / **`callSignatureNotUnique`** | B-1 |
| `defaultEquals`, `tryOnCleanup` | `domain-exhaustiveness` / `implementationUnavailable` | B-2 |
| `createHydratableSignal`, `createHydrateSignal` | `property-access-unknown-accessor` at `index.js:5439..5459` | B-3 (`sharedConfig`) |
| `createMicrotask` | `iteration-protocol (SpreadElement)` at `4711..4718` | B-4 |
| `defer` | `property-access-unknown-accessor (ElementAccessExpression)` at `3427..3435` | B-6 |

So the single largest case in the corpus is not one premise away from closing;
it is five, and B-1 reaches 84 of its 336 rows.

### `9bc68a12` — `@floating-ui/utils@0.2.12`, 80 rows

`floor`, `max`, `min`, `round` refuse with `domain-exhaustiveness` /
`implementationUnavailable`; `getOppositePlacement` and
`getOppositeAxisPlacements` refuse on a `coercion (BinaryExpression)` with no
reviewed subject root (B-5). The other eighteen exports are decidable and
already closed.

### The correction this forces on B-1

The depth plan describes B-1's refusal as `implementationUnavailable`, "the
producer's alias hop lands on `lib.es2017.object.d.ts`, which has no body to
census". That is right for the `Math.*` aliases and **wrong for the `Object.*`
ones**: `keys` and `entries` refuse with `callSignatureNotUnique`, because
`Object.keys` and `Object.entries` are overloaded and the census cannot pick a
signature. A premise built to the plan's single description would have closed
`floor`/`max`/`min`/`round` and left `keys`/`entries` exactly where they are —
two thirds of B-1's rows.

B-1 therefore has to lift **two** refusal reasons on the same stated fact: that
the binding is an immutable alias of a reviewed default-library callable. Which
signature the checker would otherwise have selected stops mattering once the
identity is stated, so `callSignatureNotUnique` is not an obstacle to the
premise — but it is an obstacle to a premise that only looks for a missing
body.

### Measured reach

| premise | rows | exports |
| --- | --- | --- |
| B-1 default-library alias | **188** | `entries` 62, `keys` 62, `floor`/`max`/`min`/`round` 16 each |
| B-2 dependency-export alias | **124** | `defaultEquals` 62, `tryOnCleanup` 62 |

B-1 is the largest single premise on the frontier and is still only 10% of the
1,884 recipe-less rows. Nothing here is authoring: both tiers need a producer
fact and a certifier arm.
