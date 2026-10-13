# `@tanstack/solid-router` and `@tanstack/solid-query` against what apps import

Measured 2026-09-29 at `f09d0930`, on the release binary. Every number below is
**measured** unless it is marked *estimated*. The method is the one in
[`2026-09-28-router-scope.md`](2026-09-28-router-scope.md), with one change:
every installed version is certified here, so each app site reads the record
of its own version and no byte digest stands in for a version that was not
measured. The join script is
[`2026-09-29-tanstack-scope.mjs`](2026-09-29-tanstack-scope.mjs) (see
*Reproducing*). The demand side is the app-import metric's site list of
2026-09-28 ([baseline](2026-09-28-app-import-metric-baseline.md), walls 3, 6
and 9).

## Headline

| | router | query |
| --- | ---: | ---: |
| app sites (`role: app`) | 437 in 7 apps | 89 in 7 apps |
| installed versions, all certified here on the rc.9 triple, host free, `browser`, `node` | rc.8, rc.6, rc.4, beta.29 | rc.3, rc.0 (and the corpus row rc.4) |
| exports clean / partial / degenerate, host free | 2 / 2 / 94 of 98, every version | rc.3 11 / 18 / 23 of 52; rc.0 10 / 12 / 35 of 57 |
| demanded exports clean, any version, any host | **0** | **0** |
| sites on an app's audited runtime, and so deliverable | **308** (rc.8 × rc.9: viviana 213, sefer 64, finds-team 31) | **0** |
| sites any single wall unblocks alone | 0 | 0 |
| sites the first greedy bundle unblocks, host free (all / deliverable) | 330 / 258 | 23 / 0 |
| harness wall, 7 probes | host free 128 s, `browser` 196 s, `node` 17 s | (same runs) |

- **The top wall is not a claim form. It is attribution.** In the metric,
  `callbacks`, `returns` and `creates` of `createFileRoute` (251 sites) and
  `useNavigate` (39) read "never proposed". The generator's
  attribution records show why. A dependency export with an unknown claim is
  imported, and the obligation at that import falls through every rung of the
  attribution ladder to `fallback-all`. That marks every export of the case in
  every domain. No exact rung names `createFileRoute` or `useNavigate` at all.
  This happens at 22 sites in the router (13 of them import specifiers) and
  at 2 import sites in `@tanstack/router-core`. It is the reason 94 of the 98
  router exports are degenerate.
- **The reads wall is case-wide and small at its source.** Both packages lose
  `reads` on every export to `runtime-accessor-installation`. The router has
  three sites, all `Object.defineProperty(linkProps, …)` inside `useLinkProps`
  on a function-local `const linkProps = {}`. Query rc.3 has three sites, and
  each one allocates its target in the same expression:
  `new Proxy({}, …)`, `Object.defineProperties({}, …)` and `new Proxy([], …)`.
- **Query cannot be delivered to any app today.** No app runs it on an audited
  runtime triple. Router rc.8 on rc.9 is the only deliverable pair, and the
  manifest already has its row, so no manifest change was needed (*Versions*).
- **Under `node` nothing certifies at `.`.** The router's published-graph
  lane refuses a module cycle through `@tanstack/router-core`'s
  self-referencing `./isServer`. Query's lane refuses at `@solidjs/web` rc.9
  under `[import, node]`.

## Demand

| version | sites | apps (runtime `solid-js` / `@solidjs/web` / signals) | audited runtime |
| --- | ---: | --- | --- |
| router rc.8 | 308 | viviana-ui-web 213, sefer 64, finds-team 31 (all rc.9) | **yes** |
| router rc.4 | 65 | probus-hk 53 (rc.8), spotify-desk-thing 12 (rc.4) | no |
| router rc.6 | 47 | ai-memory-ui (rc.6) | no |
| router beta.29 | 17 | civil (rc.6) | no |
| query rc.3 | 66 | derp-media-server 56 (rc.8), probus-hk 4 (rc.8), ai-memory-ui 3 (rc.6), inferay 3 (rc.6) | no |
| query rc.0 | 23 | spotify-desk-thing 10 (rc.4), donegeon-client 7 (rc.0 / rc.0 / rc.1), compass-ui 6 (rc.1) | no |

| router export | sites | | query export | sites |
| --- | ---: | --- | --- | ---: |
| `createFileRoute` | 251 | | `useQuery` | 29 |
| `useNavigate` | 39 | | `useQueryClient` | 18 |
| `Link` | 27 | | `useMutation` | 15 |
| `useLinkProps` | 17 | | `QueryClient` | 8 |
| `Outlet` | 16 | | `QueryClientProvider` | 7 |
| `useLocation` | 12 | | `useInfiniteQuery`, `createQuery` | 4 each |
| `redirect`, `useSearch` | 9 each | | `QueryObserver`, `useQueries`, `hydrate`, `createMutation` | 1 each |
| `createRouter` | 7 | | | |
| `useRouterState`, `notFound` | 6 each | | | |
| 14 others | 38 | | | |

## Versions: which pairs are certifiable

Admission binds one artifact in one environment (ADRs 0123, 0126, 0131). A
consumer-environment run also needs all three runtime packages to be audited
archives, and only the rc.3 and rc.9 triples are
(`rust/crates/solid-dialect/audited-archives.json`).

- **Certifiable, and already a manifest row: router rc.8 × rc.9.** viviana's
  environment is in `consumer-environments.json`, and finds-team's tree matches
  the same bundle: the baseline's lockfile rerun shows its 31 sites *open*,
  not *no contract*.
- **Certifiable in principle, excluded: sefer (64 sites, rc.8 × rc.9).** The
  baseline records sefer as "the dependency environment differs". Sefer's
  pinned lock (digest `990520d5…`, matching the corpus) resolves the router's
  own `@tanstack/router-core` to 1.171.22, the certified release. That release
  depends on `seroval` 1.6.4, where the bundle recorded 1.6.7. The tree also
  holds router-core 1.171.32, for `@tanstack/router-generator`. A sefer
  consumer environment would fix the recorded environment. The deriver refuses
  it: "`@corvu-next/dialog`: the closure resolves two releases, 0.1.4 and
  0.1.5". An environment pins one release per name. This is the same refusal
  the router scope met in lutra-console, so it is not a small manifest change.
- **Excluded, no audited triple:** router rc.4 (rc.8 and rc.4 runtimes), rc.6
  and beta.29 (rc.6), query rc.3 (rc.6 and rc.8) and query rc.0 (rc.0, rc.1 and
  rc.4). ADR 0127 schedules no new work on older rcs. These were certified here
  on rc.9 **for their bytes only**, as scratch rows the script writes. Every
  router version gets the same counts (2 / 2 / 94). At 18 of the 24 demanded
  exports every version also gets the same record. At the other 6 (`Outlet`,
  `useSearch`, `useParams`, `RouterProvider`, `lazyRouteComponent`,
  `Scripts`), only the `creates` cause differs between versions.

## Per export: what holds each domain

This is the certification metric's `causeOf`, host free, at the version the
apps install (router rc.8 where it is installed). `never proposed` is the
metric's label for "in `unresolvedClaims`, with no decline or withheld record".
For these packages it is the attribution widening (next section), not a claim
form the generator lacks.

| router export | sites | callbacks | reads | returns | creates |
| --- | ---: | --- | --- | --- | --- |
| `createFileRoute`, `useNavigate`, `redirect`, `createRouter`, `notFound`, `createRootRoute`, `useRouter`, `createRootRouteWithContext`, `getRouteApi`, `createRoute`, `linkOptions` | 330 | never proposed | accessor hazard | never proposed | never proposed |
| `Link` (rc.8), `useLinkProps` (rc.4) | 44 | never proposed | accessor hazard | never proposed | dialect-silent `createSignal` |
| `Outlet` | 16 | never proposed | accessor hazard | never proposed | dialect-silent `useContext` (15), `createMemo` (1) |
| `useLocation`, `Scripts`, `useBlocker` | 16 | never proposed | accessor hazard | never proposed | dialect-silent `createMemo` |
| `useSearch`, `useRouterState`, `useParams` | 19 | never proposed | accessor hazard | never proposed | unresolved-callee |
| `HeadContent` | 4 | never proposed | accessor hazard | never proposed | refusing-callee-fixpoint |
| `lazyRouteComponent` | 4 | never proposed | accessor hazard | never proposed | dialect-silent `solid-js:lazy` |
| `RouterProvider` | 4 | never proposed | accessor hazard | never proposed | dialect-silent `@solidjs/web:merge` (3) |
| `createMemoryHistory` (beta.29) | 1 | census: domain-exhaustiveness | accessor hazard | never proposed | never proposed |

| query export | sites | callbacks | reads | returns | creates |
| --- | ---: | --- | --- | --- | --- |
| `useQuery` | 29 | never proposed | accessor hazard | never proposed | refusing-callee-fixpoint (22), dialect-silent `createSignal` (7) |
| `useQueryClient` | 18 | census: declared callee refused (17) | accessor hazard | withheld operation: recursive-value-shape (17) | census: declared callee refused (17) |
| `useMutation` | 15 | never proposed | accessor hazard | never proposed | dialect-silent `createMemo` |
| `QueryClient`, `createQuery`, `QueryObserver`, `createMutation` | 14 | never proposed | accessor hazard | never proposed | never proposed |
| `QueryClientProvider` | 7 | never proposed | accessor hazard | never proposed | unresolved-callee (4), dialect-silent `createSignal` (3) |
| `useInfiniteQuery` | 4 | never proposed | accessor hazard | never proposed | dialect-silent `createMemo` (3), `createSignal` (1) |
| `useQueries` | 1 | never proposed | accessor hazard | never proposed | refusing-callee-fixpoint |
| `hydrate` | 1 | census: domain-exhaustiveness | accessor hazard | closed | never proposed |

The router's dialect-silent `createSignal`, `createMemo` and `useContext`
records print as `undefined:…` in the metric. The router calls them through a
namespace import (`import * as Solid from "solid-js"`, then
`Solid.createSignal(null)`), and the record's package half is empty for a
namespace member. That is cosmetic, because the audit row that would clear the
record is keyed by the dialect spelling. The call is still silent: the audit
has no creates denial for `createSignal`, whose function form does create.

Where the hosts differ:

- `browser` resolves the Solid primitives through another module graph. The
  router's dialect-silent rows mostly turn into `never proposed` (rc.8 `Link`,
  `Outlet`, `useLocation`), rc.4's `useLinkProps` and `Link` show
  dialect-silent `createEffect` (28), and `useLocation` shows
  `create-publishing-callee` (6). For query, `useQuery` and
  `QueryClientProvider` show `unresolved-callee`, and `useMutation` shows
  `create-publishing-callee`. No export is clean. The greedy first bundle is
  +381 for the router and +32 for query.
- `node`: see *Node*.

## The top wall: the attribution ladder falls back to every export

This was measured by re-certifying router rc.8 and query rc.3 and rc.0 host
free, with the checker's stderr captured (`native-tee.sh` in *Reproducing*).
The generator writes one `solid-checker:unknown-claim-attribution=` record per
obligation it cannot close, and it names the rung that attributed it
(`rust/crates/solid-facts-backend/src/main.rs`,
`attribute_unresolved_obligation`). The certification report does not keep
these records.

**Router rc.8: 252 distinct records.** Most are exact (`identity-widening`,
`enclosing-chain`, `reexport-specifier`, `class-construction`, …). The ones
that set the result are the `fallback-all` records. Each marks all 95 exports
of the case in all five consumer domains:

- 13 at import specifiers of `@tanstack/router-core` exports whose claims are
  unknown: `BaseRoute`, `BaseRootRoute`, `BaseRouteApi` and `notFound`
  (`route.jsx`, `route.js`), `RouterCore` (`source/router.js`),
  `getLocationChangeInfo` and `trimPathRight` (`Transitioner.jsx`),
  `isNotFound` (`not-found.jsx`), and `createNonReactiveMutableStore` and
  `createNonReactiveReadonlyStore` (`routerStores`);
- 4 at arguments whose callback claims are unknown: `resolvedLocation`
  twice, `error?.cause`, and `getStoreFactory`;
- 5 on exported-parameter member dispatch (`applyResponseMetadata`,
  `toHeadTags`, `serializeMatchTransfer`, `primeRouterFromRegistry`, and the
  argument of `getScrollRestorationScriptForRouter`). These open `reads` and
  `returns`.

**`@tanstack/router-core` 1.171.22 has the same problem one level down.** Two
`fallback-all` records sit at its `router.js` import of `@tanstack/history`'s
`createBrowserHistory` and `parseHref`, whose `ownerRequirements` and `returns`
are unknown. They mark all 71 exports of the case. 71 of router-core's
exports are open in all four consumer domains (79 in `creates` and
`returns`). That includes a pure string function, `trimPathRight`. The router then imports those open exports.

`createFileRoute` and `useNavigate` have **no** exact-rung record at all. Every
open domain on them comes from these widenings, plus the reads hazard.

**Why the ladder fails here** (*diagnosed from the source, not proven by an
experiment*). The references that should carry each import to its exports
are:

- inside class fields and methods, for example
  `this.history = createBrowserHistory()` in `RouterCore`;
- inside module-private components used only as JSX (`<Transitioner />` in
  `Matches.jsx`).

Every module in the router's `dist/esm` and `dist/source` also has a sibling
`.d.ts`. The ladder's own comment documents that shape as unpairable: the
consumer's reference binds to the declaration file, so the implementation has
no reference. The graph then reports incomplete, and emission widens to
`fallback-all`.

**Query.** The same widening occurs. rc.0 has `fallback-all` at the imports of
`hydrate` and `QueryCoreClient` (`class QueryClient extends QueryCoreClient`)
and at `createServerDehydrationChannel`'s parameter dispatch, over all 47
exports of the case. rc.3 has it at `QueryCoreClient` only, over the 41
exports of the `src` case, which is why rc.3 keeps 11 clean exports. Seven
dispatch records in `@tanstack/query-core` widen to its 24 exports.

## The reads wall

`runtime-accessor-installation` opens `reads` for every export of the case
(`contract_certification/module_closure.rs`: `affected_exports` is empty).
Every site is listed here:

- **Router** (rc.8, `.` in both conditions): three `Object.defineProperty`
  calls in `useLinkProps` (`link.js` 9091, 9222, 9428; `link.jsx` 13319,
  13469, 13742). The target of all three is `linkProps`, a function-local
  `const` initialized by `{}`. The first copies descriptors from
  `propsSafeToSpread`, and the other two install getters over
  `resolvedStateProps()` and the caller's getters.
- **Query rc.3**: `new Proxy({} as MetaState, …)` (`useBaseQuery`),
  `Object.defineProperties({}, …)` (`useInfiniteQuery`) and
  `new Proxy([], …)` (`useQueries`), the same in `build/` and `src/`. rc.0 has
  the first only.

None of the targets can be an object that existed before the call. Binding
the hazard to the exports that reach the allocating function would lift
`reads` from every other export: *estimated* at up to 393 router sites (all
but `Link` and `useLinkProps`) and up to 35 query sites (`useQueryClient`,
`QueryClient`, `QueryClientProvider`, `QueryObserver`, `hydrate`). The hooks
that do reach the allocation keep `reads` open, correctly: their results are
getter and proxy objects that track reads.

## Walls ranked by the sites they block

Host free, at each site's own version. "Alone" is the number of sites whose
only open wall this is.

| wall | class (as re-read above) | router sites (deliverable) | query sites | alone | top exports |
| --- | --- | ---: | ---: | ---: | --- |
| `runtime-accessor-installation`, case-wide | generator decline, 3 sites per package | 437 (308) | 89 | 0 | `createFileRoute` 251, `useNavigate` 39, `useQuery` 29 |
| `fallback-all` attribution, shown as returns "never proposed" | attribution precision | 437 (308) | 71 | 0 | `createFileRoute` 251, `useNavigate` 39, `useQuery` 29, `useMutation` 15 |
| the same, callbacks | attribution precision | 436 (308) | 71 | 0 | the same |
| the same, creates | attribution precision | 331 (258) | 16 | 0 | `createFileRoute` 251, `useNavigate` 39, `QueryClient` 8 |
| dialect-silent `createSignal` (creates) | dialect audit | 44 (16) | 11 | 0 | `Link` 27, `useLinkProps` 17, `useQuery` 7 |
| `unresolved-callee` (creates) | generator decline | 19 (6) | 4 | 0 | `useSearch` 9, `useRouterState` 6, `QueryClientProvider` 4 |
| dialect-silent `createMemo` (creates) | dialect audit | 17 (9) | 18 | 0 | `useMutation` 15, `useLocation` 12 |
| `refusing-callee-fixpoint` (creates) | generator decline | 4 (3) | 23 | 0 | `useQuery` 22, `HeadContent` 4 |
| `useQueryClient`: declared callee refused (callbacks, creates), recursive-value-shape (returns) | census refusal | — | 17 | 0 | `useQueryClient` 17 |
| dialect-silent `useContext` (creates) | dialect audit | 15 (12) | — | 0 | `Outlet` 15 |
| dialect-silent `lazy`, `merge` | dialect audit | 7 (4) | — | 0 | `lazyRouteComponent` 4, `RouterProvider` 3 |
| domain-exhaustiveness (callbacks) | census refusal | 1 | 1 | 0 | `createMemoryHistory`, `hydrate` |
| graph cycle, `node` only | graph lane | 437 under `node` | — | — | every router site |
| `@solidjs/web` rc.9 `[import, node]` refused, `node` only | dependency | — | 89 under `node` | — | every query site |

**Greedy unlock curve, router, host free** (each step closes the wall set that
clears the most sites per wall added):

1. **+330**: hazard + attribution in three domains. This clears
   `createFileRoute` 251, `useNavigate` 39, `redirect` 9, `createRouter` 7,
   `notFound` 6, `createRootRoute` 5, `useRouter` 5 and six smaller exports.
   The deliverable share is +258, of which `createFileRoute` is 212.
2. +44 dialect `createSignal` (`Link`, `useLinkProps`).
3. +19 `unresolved-callee`.
4. +17 `createMemo`.
5. +15 `useContext` (`Outlet`).
6. +4 `refusing-callee-fixpoint`.
7. +4 `lazy`.
8. +3 `merge`.
9. +1 domain-exhaustiveness. The total is 437.

**Query:**

1. +23: hazard + attribution + `refusing-callee-fixpoint` (`useQuery` 22,
   `useQueries` 1).
2. +18 `createMemo`.
3. +15 creates attribution.
4. +11 `createSignal`.
5. +17 `useQueryClient`'s census walls.
6. +4.
7. +1.

**What the curve does not show.** Step 1 assumes an exact attribution lands
nowhere. It will land somewhere. `createFileRoute` is
`(options) => createRoute(options)`, and `createRoute` constructs
`class Route extends BaseRoute`, so exact attribution moves `BaseRoute`'s
unknown claims onto it instead of onto everything. `BaseRoute` is open in
router-core, today through router-core's own `fallback-all`. So the 251 sites
need exact attribution in **both** packages. Behind that they need whatever
exact walls `BaseRoute` then shows, which is *unmeasured*. The honest
statement is that no measured bundle short of that clears a router site.

## In flight elsewhere

- **ADR 0152 (nested callbacks in described callables).** It is not in reach
  as measured. No demanded export's `callbacks` domain is held by a nested
  callback form. It is held by the widening, or for `createRouter` by exact
  `class-construction` records: `RouterCore`'s constructor takes `options`
  and `getStoreFactory`, and their claims are unknown. That is a
  dependency-claim question (ADR 0151 citation, once router-core certifies)
  before it is a nested-callback one.
- **ADR 0153 (a member of a package-owned context value).** It is behind the
  attribution wall, and its shape differs here. `useRouter()` is
  `Solid.useContext(routerContext)`. The provided value is `RouterProvider`'s
  `router` **prop** (`createComponent(RouterContext, { value: router })`),
  that is, the app's own `createRouter()` result, not an own object literal.
  Slice A's own-literal census would refuse it. The consumer-provider clause
  (part 2) decides these hooks: `useNavigate` 39, `useLocation` 12,
  `useSearch` 9, `useRouterState` 6, `useRouter` 5, `useParams` 4, and so on,
  about 76 sites. Query's `useQueryClient` (18) has the same shape, through
  `QueryClientProvider`'s `client` prop.
- **ADR 0156 (withheld forwards).** `redirect` 9, `notFound` 6 and
  `createMemoryHistory` 1 are re-exports of `@tanstack/router-core` and
  `@tanstack/history` (16 sites). They are open today because the dependency's
  claims are unknown, not withheld. ADR 0156 moves nothing here until those
  dependencies certify with a withheld export.

## Node

- **Router** (every version): the published-graph lane refuses the `.` case
  with "published dependency graph cycle". Under `[import, node, solid]`,
  `@tanstack/router-core/isServer` selects `isServer/server.js`. That file
  imports `../load-server.js`, which imports `router.js`, which imports
  `path.js`, which imports the package's own `@tanstack/router-core/isServer`
  again. The browser and default variant, `isServer/client.js`, imports
  nothing, so only `node` meets the cycle. The fallback lane
  (reused-proposal) certifies `./ssr/client` alone. `.` is uncertified, and so
  are all 437 sites.
- **Query**: the graph node `@solidjs/web@2.0.0-rc.9 . [import, node]` refuses:
  "accepted dependency solid-js/internal has no exact runtime binding for
  export ssrScope". The plain lane refuses at artifact-or-demand planning. All
  89 sites are uncertified. The `@solidjs/web` refusal is not query's: any
  package whose graph reaches `@solidjs/web` under `node` will meet it.

## Proposals (not implemented)

**P1: attribute an import's unknown claim through its references into class
members and components.** This is the top wall for both packages and their
cores. A precision ADR on the ladder, not a claim form:

1. **Class members.** A reference inside a field initializer, method or
   constructor of a module-level class `C` attributes to `C`'s creators and
   instance-member exports. This is the existing ADR 0134 attribution, keyed
   from the reference's enclosing chain rather than from the obligation's own
   location.
2. **Components.** A reference inside a module-private function whose every
   use is a component reference (`createComponent(F, …)` compiled, `<F />`
   in JSX source) attributes to the exports that reach those uses, by the
   existing reachability.
3. **The sibling-declaration gap.** This is where most of the router's
   widenings plausibly originate, and it is a fact gap rather than a rung
   gap. It needs a producer fact that pairs `x.js` or `x.jsx` with the `x.d.ts`
   beside it, so that a reference to the declaration counts as a reference to
   the implementation. Whether (1) and (2) clear any record without (3) has to
   be measured first: re-run the capture above, with a count of `fallback-all`
   records per rung before and after.
4. **Fail closed.** A reference in module top-level code executed at import
   stays `fallback-all`, correctly. So does any computed or member-dispatched
   reference.
5. **Fixtures, against the published typings (`tsc` silent).**
   - A class-field reference to an import whose claim is unknown: only the
     class's creators open.
   - The same through a sibling `.d.ts`.
   - A JSX-only private component.
   - Negatives: a top-level call stays wide, and a namespace-member reference
     stays wide.

**P2: bind `runtime-accessor-installation` to the allocating function when
its target is fresh.** This is the second wall, and it is the same item C that
ADR 0153 part 3 and the router scope name. The narrowest sound form covers all
six measured sites:

- **The target.** It is either an allocation in the same expression
  (`new Proxy(<literal>, …)`, `Object.defineProperties(<literal>, …)`) or a
  `const` whose initializer is an object or array literal in the same
  function body, reassigned nowhere.
- **The hazard.** It is then attributed to the exports that reach that
  function, not to the case.
- **Refusal.** The hazard stays case-wide if the object escapes other than by
  being returned or passed as a component prop. Being stored into module
  state, into a context value, or through a parameter's member counts as
  escaping.

The hazard is also part of the closure digest mirrored in
`packages/cli/scripts/artifact-resolution.mjs`, so the binding must be emitted
beside the site, not recomputed by the replay. This lifts `reads` from up to
393 router and 35 query sites (*estimated*). It clears none of them alone.

Neither proposal clears a site alone. P1 then P2 is the order, and the site
count behind them is not measurable until P1 lands.

## Backlog notes

- The certification metric labels "unresolved with no decline" as
  `missing claim form: … never proposed`. For attribution widenings that label
  is wrong. The generator's `unknown-claim-attribution` records are the
  evidence, and they are not persisted: a run keeps no copy.
- A namespace-member dialect callee (`Solid.createSignal`) prints its package
  as `undefined` in the metric's wall key.

## Reproducing

```sh
make build-checker-release
bun docs/package-contract-v2/phase22/2026-09-29-tanstack-scope.mjs --prepare <dir>
for host in none browser node; do   # --host browser|node for the two hosts
  SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/release/solid-checker-rust \
  SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
  bun scripts/ecosystem-benchmark/run.mjs --manifest <dir>/manifest.json --solid 2 \
    --timeout 1800 --attempt-certification --recover-entrypoints \
    --probe-recipe-corpus scripts/ecosystem-benchmark/probe-recipes --keep-temp [--host $host] \
    $(jq -r '.packages[].probe | "--probe", .' <dir>/corpus.json) \
    --json <dir>/run[-$host].json --markdown <dir>/run[-$host].md
  bun scripts/certification-metric.mjs --run <dir>/run[-$host].json --corpus <dir>/corpus.json \
    --json <dir>/metric[-$host].json --markdown <dir>/metric[-$host].md
done
bun docs/package-contract-v2/phase22/2026-09-29-tanstack-scope.mjs --join \
  --sites rust/target/app-import-metric/metric.json --metric <dir> [--hosts "none browser node"] [--certifiable]
```

The attribution records come from the same run with the native binary
wrapped so that its stderr is kept:

```sh
#!/bin/bash
# native-tee.sh: pass SOLID_CHECKER_NATIVE_BIN=<this script>. Absolute paths:
# the benchmark runs the checker from its temporary tree.
exec 3>&2
"<repo>/rust/target/release/solid-checker-rust" "$@" 2> >(tee -a <dir>/native-stderr.log >&3)
```

Then `grep '^solid-checker:unknown-claim-attribution=' <dir>/native-stderr.log`.
Each record carries `mechanism`, `exports`, `domains` and a byte span into the
retained tree (`--keep-temp`).
