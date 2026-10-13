# Plan: the next depth tiers for `reads` closure (2026-09-13)

A working plan for an implementing agent. It is written against commit
`4978af90` (ADR 0101) and the pin it re-took: 418 rows, 10,290 certified
closure entries, 2,242 `reads` detail rows withheld as `no recipe in corpus`,
225 as visible census refusals. Every number below comes from
`benchmarks/ecosystem/report.json` at that commit or from scaffold-pass audits
taken the same day; re-measure before trusting one.

Read first: `AGENTS.md`, `CLAUDE.md`, `docs/adr/0100-described-callbacks-enumeration.md`,
`docs/adr/0101-described-reads-enumeration.md`, the top four entries of
`docs/precision-backlog.md`, and § 22 and § 43 of
`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`.

## 0. Rules that decide what counts

- **Never certify from silence.** A refusal that is a correct verdict on a
  false claim is not a lever. Rank levers by the finding a closure discharges
  for a consumer, never by the withheld count alone.
- **"No recipe in corpus" masks census verdicts** (design record § 43.3): a
  candidate with no recipe is weakened out of the plan before its census runs.
  The two-pass scaffold (§ 2 below) is the only way to learn whether a recipe
  can help. Never write a recipe before pass 2 has said the export is
  decidable.
- **A recipe is a falsifier, not a proof.** It must call the export; a module
  that completes without calling it is vacuous and certifies (§ 22.2). A
  recipe may install a same-turn shim for a host API the harness lacks
  (`requestAnimationFrame`, `requestIdleCallback`) and must declare it in
  `coverageLimitations`. It may never hand `session` or `harness` to the
  package.
- **Build through `make`** (`make build-checker-debug`, `make build-checker-release`);
  a bare `cargo` build drops the certification pins and yields a binary that
  silently refuses certification. One Cargo process at a time. Library tests
  need the pin environment — see § 1.
- **Corpus runs use the release binary with `--timeout 1800`**, compare to the
  pin, and are accepted only with **no row below baseline and no status move**.
  Clean `$TMPDIR/solid-checker-*` before timing.
- **Snapshots** (`scripts/contract-corpus.mjs --update`) only after the
  non-updating run showed the change; review every changed export.
- Commits are individually green, end with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`; nothing is
  pushed.

## 1. Mechanics to reproduce

**Pinned library tests** (a script; the pins are compile-time inputs):

~~~bash
#!/bin/bash
cd /path/to/solid-checker
export SOLID_TYPEFACTS_CERTIFICATION_SHA256="sha256:$(shasum -a 256 bin/solid-typefacts | awk '{print $1}')"
export SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256="sha256:$(node scripts/typefacts-source-identity.mjs --build-id dev --digest)"
export PROBE_NODE="$(command -v node)"   # the pinned Node 24.11.1
export SOLID_CHECKER_PROBE_HARNESS_SHA256="sha256:$(node scripts/probe-harness-source-identity.mjs --build-id dev --write-stamp --digest)"
export SOLID_CHECKER_PROBE_NODE_SHA256="sha256:$(shasum -a 256 "$PROBE_NODE" | awk '{print $1}')"
export SOLID_CHECKER_EXPECT_PROBE_PINS=1 SOLID_CHECKER_BUILD_ID=dev TYPEFACTS_BUILD_ID=dev
export SOLID_CHECKER_RC3_ARCHIVE_ROOT="$PWD/rust/target/tsc-oracle/v2/node_modules" SOLID_CHECKER_SOLID1_ARCHIVE_ROOT="$PWD/rust/target/tsc-oracle/v1/node_modules"
export SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib -- "$@"
~~~

**Standalone certification of one dependency case** (case ids equal the
corpus's, so a scratch project is enough):

1. `mkdir proj && cd proj && bun init -y && bun add <pkg>@<exact> solid-js@<exact>`;
   take the integrity from `bun.lock`.
2. Certify with `packages/cli/scripts/certify-contract.mjs`'s `certifyContract`
   using `--package-root proj/node_modules/<pkg> --integrity <sha512> --catalog
   <dir>/accepted-contracts.json --issuer-configuration <issuer.json>
   --trust-configuration-output <trust.json> --audit-output <audit.json>
   --probe-recipe-corpus <corpus> --entrypoint <ep>`, with
   `SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/release/solid-checker-rust` and
   `SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts`. Compute repo paths before
   any `cd`.
3. **Two-pass scaffold**: certify against an empty corpus → `bun
   scripts/probe-recipe-scaffold.mjs --audit audit0.json --corpus <scratch>
   --domain reads --specifier <specifier>` (emits `UNFINISHED` modules) →
   certify against the scratch corpus → run the scaffold again on `audit1.json`:
   it lists `unserviceable` (census refused) candidates; everything withheld as
   `veto did not complete … unfinished` is **decidable** and worth a recipe.
4. Motion-utils cannot be certified standalone ("runtime implementation does
   not match the snapshot-replayed export binding"); size it through a corpus
   run instead.

**Recipe conventions**: one module per (claim id, case); header names the
case and states `NEVER EMITS:` — the same sentence must appear in the
manifest entry's `coverageLimitations`, because
`scripts/ecosystem-probe-recipes.test.mjs` pins the sorted silent list from
the manifest. Carrying a module onto another case = new file + that case's
claim id (from `withheldClosureDetails[].semanticClaimId`) + header. Batch
precedents: `corvu-utils-*`, `solid-primitives-utils-colors-*`,
`tanstack-store-*` in `scripts/ecosystem-benchmark/probe-recipes/`.

**Re-pin**: copy the run's report json/md to `benchmarks/ecosystem/`, `bun
scripts/package-contract-v2-phase21-ledger.mjs --write`, replace the report
digest in `scripts/package-contract-v2-phase21-ledger.test.mjs`, run the
vitest files `package-contract-v2-phase21-ledger`, `package-contract-phase19`,
`ecosystem-probe-recipes`, `ecosystem-benchmark/run`.

## 2. Tier A — finish the recipes (authoring, no Rust)

The recipe-less `reads` set is 594 unique (case, export) pairs on 203 cases.
Rows multiply per root artifact case, so rank by rows a case would clear.
Known decidability is stated where a pass-2 audit exists; run pass 2 on
everything else before writing.

| rows | case | package | exports | known state | action |
| --- | --- | --- | --- | --- | --- |
| 80 | `08b3dc34`, `ff5cac09` | `@corvu-next/utils@0.1.4` `./reactivity` | `access`, `chain`, `mergeRefs`, `some` | decidable — same bodies as `@corvu/utils` `./reactivity`, whose recipes exist (`corvu-utils-reactivity-*`) | **carry** the four modules onto both cases with their claim ids |
| 64 | `1d311acb` | `@floating-ui/utils@0.2.12` `./dom` | `getContainingBlock`, `getFrameElement`, `getNearestOverflowAncestor`, `getNodeScroll`, `getWindow`, `isLastTraversableNode`, `isTableElement`, `isWebKit` | decidable (pass 2, 2026-09-13); the other 12 DOM exports refuse on `window`/`instanceof` forms | write 8 recipes; pass caller-owned node-like objects (`{ ownerDocument: { defaultView } }`) — under ADR 0034 what they read is the caller's; `isWebKit` needs a `CSS.supports` or `navigator` shim, declared |
| 96 | `1116a60f`, `0cf47e45` | `@corvu/utils@0.4.2` `./create/keyedContext` | `createKeyedContext`, `getKeyedContext`, `useKeyedContext` | unknown; `createContext`/`useContext` are `solid-js` dependency claims — expect "waits on a withheld solid-js claim" (§ 43) | pass 2 first; if the census decides, recipes are trivial (`Map` lookups) |
| 48 | `b97f9095`, `d22fd9a2`, `3df9bd5d` | `@solid-primitives/refs` 1.1.4 / 3.0.0-next.0 | `Ref`, `defaultElementPredicate`, `getFirstChild`, `getResolvedElements`, `resolveElements`, `resolveFirst` | unknown; expect `instanceof Element` refusals on the predicates | pass 2; write only what decides |
| 36 | `e1a524fa`, `6f867f0c` | `@solid-primitives/scheduled` 1.5.3 / 2.0.0-next.2 | `debounce`, `throttle`, `leading`, `leadingAndTrailing`, `scheduleIdle`, `createScheduled` | unknown; timers, `requestIdleCallback` | pass 2; shims for `requestIdleCallback` declared; `createScheduled` uses `createSignal` → dependency claim |
| 22 | `b70ad6d1` | `@solid-primitives/utils@7` `./immutable` | `add`, `concat`, `divide`, `multiply`, `omit`, `pick`, `power`, `sortBy`, `split`, `substract`, … | partly refused (`filterInstance`, `flatten` visible) | pass 2; pure array/object helpers, cheap |
| 21 | `ae3d9b64` | `@tanstack/devtools-ui@0.7.1` | 21 icon components | need a window (JSX components) | **skip**; policy, not authoring |
| 66 | `f34410e8`, `fd42a1d9` | `@corvu/utils@0.4.2` `./dom` | `combineStyle` | census refused (`SpreadAssignment` on parameter — ADR 0041 form with no root in this producer) | skip; Tier B-6 shape |
| 68 | `a18d59ef` … | `@corvu/utils@0.4.2` `./create/*` | `default` | unknown; Solid primitives | pass 2; likely dependency-claim waits |

Expected gain for Tier A if the unknowns decide as guessed: roughly 250–350
detail rows and 30–50 closure entries. Write recipes bottom-up (dependency
nodes before roots) and re-pin once per batch, not per recipe. Record each
batch in `scripts/ecosystem-benchmark/probe-recipes/README.md` and
`docs/precision-backlog.md` with before/after numbers.

## 3. Tier B — `@solid-primitives/utils` (census premises, Rust and producer)

The `6.4.1` root case `9887e137` is composed by seven Solid 1 rows and still
carries 336 recipe-less rows on eight exports; the four `7.0.0-next.4` root
cases carry 180 more on the same names plus `globalRegistry`. Every one is a
census verdict no recipe can change. The exports, their source, the refusal,
and the premise that would decide them:

### B-1. Aliases of default-library callables — `keys`, `entries` (also `floor`, `max`, `min`, `round` in `@floating-ui/utils`; 80 rows there)

~~~js
export const entries = Object.entries;
export const keys = Object.keys;
~~~

Refusal: `domain-exhaustiveness … implementationUnavailable` — the producer's
alias hop (`aliasedRuntimeImplementationLocked`) lands on `lib.es2017.object.d.ts`,
which has no body to census.

Premise (an ADR, "a default-library alias"): the producer states
`defaultLibraryAlias: { declaration }` when the binding is an immutable alias
whose terminal declaration is in a default library file and is a **reviewed**
callable — a small allowlist owned by the certifier, e.g. `Object.keys`,
`Object.entries`, `Object.values`, `Object.assign`, `Object.freeze`,
`Math.*`, `Array.isArray`, `Number.isFinite`, `JSON.parse`, `JSON.stringify`
(no callback slot, no reactive read of anything but its arguments). The census
closes `reads: []`, `creates: []`, `callbacks: []` for such an export on the
stated fact — a default-library callable invokes no caller callable (the ones
that do, `Array.prototype.map`, `Object.defineProperty` with accessors, are
kept off the list) and reads nothing but its arguments' own properties, which
is the caller's (ADR 0034). `Object.entries` does invoke getters on its
argument: caller's code, the same disposition as a property access. `returns`
stays open. **Veto**: a synthesized module observing `Object.is(subject,
Object.keys)` in the probe realm — an *exact* runtime witness of the alias,
stronger than any sampling; emit on inequality. Mirror ADR 0099's shape
(`Observation::NotCallable` → `Observation::DefaultLibraryAlias(name)`).
Fixture: a new `default-library-alias` generator fixture with `keys`,
`entries`, `floor`, and a refusing `mapAlias = Array.prototype.map` and a
`const later = Object.keys; later = …` (written binding refuses).
Approximate corpus effect: `keys`+`entries` on five utils cases (~120 rows),
four floating-ui exports on `9bc68a12` (80 rows).

### B-2. Alias of a dependency's export — `defaultEquals`, and `tryOnCleanup` in production

~~~js
export const defaultEquals = equalFn;           // from "solid-js"
export const tryOnCleanup = isDev ? fn => (getOwner() ? onCleanup(fn) : fn) : onCleanup;
~~~

Refusal: `implementationUnavailable` (`defaultEquals`: the alias hop lands on
`solid-js`'s `.d.ts`); `tryOnCleanup` is a conditional, not an alias, and
stays refused — correctly; leave it.

Premise: an immutable alias of an **accepted dependency export** is that
export. The dependency lane already composes closed claims for a *call* to a
dependency (ADR 0036/0098); this is the identity case: every closed domain of
`solid-js`'s `equalFn` (the dialect tier audits it) is the export's own. The
producer already states the alias (`immutableCalleeAlias`/`implementationOf`
family); the certifier needs an arm in `require_named_export_implementation`
that, when the transcript's terminal declaration is in an accepted dependency
package, binds to that dependency's accepted document instead of refusing.
Veto: `Object.is(subject, dependencyModule.equalFn)` in the probe realm, the
same exact witness as B-1. Small (2 exports × 5 cases ≈ 40 rows) but the
premise is reusable across the corpus (every `export { x } from` alias that
the resolver did not already fold).

### B-3. A property read on a dialect data object — `createHydratableSignal`, `createHydrateSignal`

~~~js
if (sharedConfig.context) { … }               // sharedConfig from "solid-js"
~~~

Refusal: `property-access-unknown-accessor (PropertyAccessExpression)` at the
`sharedConfig.context` read, "no reviewed subject root": the receiver is an
imported binding, and TypeScript types it as a plain object, which proves
nothing about a proxy.

Premise: a **dialect axiom** — the Solid 1.x and 2.0 dialect tiers state that
`sharedConfig` (and `DEV`, `$PROXY`-free config objects) are data objects the
runtime never proxies; the census dispositions a property access whose root is
an import of such a name from the dialect package as `DialectDataObject`
(ADR 0007's tier is the precedent for dialect-stated facts). Both dialect
crates own the list; the certifier reads it through the `Dialect` seam. The
rest of the body is `createSignal`/`onMount` calls, already dependency claims.
Effect: 2 exports × 5 cases ≈ 100 rows, and the axiom recurs in every
primitive that checks hydration.

### B-4. A spread of a rest-parameter alias — `createMicrotask`

~~~js
return (...a) => { (args = a), calls++; queueMicrotask(() => --calls === 0 && fn(...args)); };
~~~

Refusal: `iteration-protocol (SpreadElement)` on `args`, no reviewed root:
`args` is a module-local `let` written from the rest parameter `a` of a
nested callable. ADR 0042 says a rest parameter's array is engine-built and
records no form, but the alias through `args` loses that.

Premise: a local binding **written only from rest parameters** (of any
callable in the body) is engine-built; the producer's `local-binding-written`
leg already classifies writes to a binding — extend it with "every write's
right-hand side is a rest-parameter binding", and let the census disposition
the spread as `ParameterRootedIterable`-equivalent (`EngineBuiltIterable`).
Small on its own (1 export × 5 cases ≈ 30 rows); worth doing only if the
producer change is a few lines beside B-6.

### B-5. A default-library call result as a receiver — `ndjson`, `handleDiffArray`

~~~js
export const ndjson = raw => raw.split("\n").filter(line => line !== "").map(line => JSON.parse(line));
~~~

Refusal: `property-access-unknown-accessor` on a receiver that is a **call
result** (`raw.split(…)`, typed `string[]`): not a parameter, not a value this
program built by literal, so no reviewed root. (`handleDiffArray`: verify the
exact byte range in the audit before assuming — it is `current.length`/`prev[i]`
territory and may instead be the `i++`-written index the ADR 0034 root rule
refuses; if so it is a different, smaller premise: an index binding written
by `++` on a numeric type is not the receiver.)

Premise: **a default-library call whose declared result type is a primitive
or an engine-built array/object** (`String.prototype.split → string[]`,
`Object.keys → string[]`, `Array.prototype.filter/map → T[]`) is "a value this
program built" in ADR 0044's sense: the engine created the array with data
properties. Extend the producer's subject-root derivation with
`default-library-result`, and the certifier's `census_form_disposition` with
the matching arm. This is the largest *shape* on the corpus after B-1: chained
array/string helpers are everywhere in utility packages (`lines`, `ndjson`,
floating-ui's `getOppositePlacement` coercion on `placement.replace(...)`,
`mix`/`colorToOKLCH` in `./colors`). Size it with `rg` over the pinned
report's refusal reasons for `(PropertyAccessExpression)` / `(BinaryExpression)`
forms whose receiver text is a member call before writing.

### B-6. A parameter read inside a nested callable — `defer` (and corvu's `combineStyle`)

~~~js
return prevValue => { … for (let i = 0; i < deps.length; i++) input[i] = deps[i](); … };
~~~

Refusal: `property-access-unknown-accessor (ElementAccessExpression)` at
`deps[i]` (reach `unknown`), no reviewed root — the access sits in the
returned closure, and the producer states `subjectParameter` only for forms in
the declaration's own body.

Premise: ADR 0034's argument is about **authorship**, not timing: a getter on
an object the caller passed is the caller's code whenever it runs, so a
parameter-rooted form inside a nested callable is still the caller's read. The
producer already marks `captured`; extend the root derivation to nested
callables (the binding is the same uninitialized, unwritten parameter), and let
the census accept `ParameterRootedAccessor` with `captured: true` for the
**reads** domain (not for `callbacks`, where timing is the claim). Check
`combineStyle`'s `SpreadAssignment` on a parameter gets the same root — ADR
0041 already admits the form when rooted. Effect: `defer` ≈ 30 rows,
`combineStyle` 66 rows, and the shape recurs in every primitive that returns a
closure over its arguments.

### Order and expected effect for Tier B

1. **B-1** (default-library alias): one ADR, producer fact + certifier arm +
   synthesized identity veto + fixture. About 200 rows. (`motion-utils`'
   `SubscriptionManager` is a class, not an alias — Tier C.)
2. **B-5** (default-library result root): producer subject-root leg +
   certifier arm; measure first with the report. Potentially the largest.
3. **B-6** (captured parameter root for reads): producer + certifier; small
   code, wide shape.
4. **B-3** (dialect data object): dialect seam change — both dialect crates
   move together; keep it after the two above.
5. **B-2**, **B-4**: only if cheap beside the others.

Each is its own ADR (`docs/adr/0102…`), its own fixture pair (closing and
refusing exports), its own protocol bump when the producer states a new fact
(`TYPE_FACTS_HANDSHAKE_PROTOCOL`, `bin/solid-typefacts` rebuilt with
`scripts/build-typefacts.sh`, `apps/solid-typefacts` Go tests), and its own
corpus run. After a producer change run workspace `clippy --all-targets`
before `make verify`: test-only wire literals break only there.

## 4. Tier C — named and deferred

- **Classes** (`Store`, `ReadonlyStore` in `@tanstack/store` — 160 rows;
  `SubscriptionManager`, `EventClient`, `TriggerCache`, the `Lite*` pacers):
  `domain-exhaustiveness` because a class export's call domains are about
  construction. `apps/solid-typefacts/internal/typefacts/tsgo/class_constructions.go`
  exists; the census of a constructor body is the premise. An ADR of its own.
- **Consts bound to a call result** (`easeIn = cubicBezier(…)` in
  `motion-utils`, 234 rows): the value is a closure a local factory returned;
  closing needs the factory's returned-body census (ADR 0035/0096 territory).
  Defer.
- **Window-rooted host reads** (`window.document`, `navigator.*` in floating-ui
  DOM, `@corvu-next/utils` DOM, devtools icons): a `lib.dom`-declared property
  on a host-rooted receiver is not a reactive source; premise `HostRooted` for
  a root declared in `lib.dom.d.ts` **and** a property declared on that
  interface. Design record § 63 already found the classification seam. Do
  after B-5, it shares the producer leg.
- **Solid-primitive wrappers** (`@solid-primitives/memo`, `scheduled`, `timer`,
  `async`, corvu `./create/*`): wait on `solid-js` dependency claims; the
  bottom-up rule from § 43 applies — nothing to do at this layer until the
  dialect tier publishes more positive `reads` closures.

## 5. Verification and reporting cadence

Per tier: focused tests while iterating (`-p solid-facts-backend --lib` with
the pins; `facts-lib`/`ir-lib` where touched; Go tests for the producer),
`contract-corpus` non-updating then `--update` with a per-export review, the
receipt-level tracer test for any new closure shape, then clippy + fmt + `git
diff --check`, `make build-checker-release`, a corpus run compared to the pin
with the accounting script (entries, per-domain uncapped closures, withheld by
class, rows below baseline, wall). Re-pin only on a run with no row below the
pin. Report what changed, which checks ran, what moved and what did not, and
name the remaining refusals by class — never "done".
