# `@solidjs/router` as the next certification target: scope and first wall

Measured 2026-09-28 at `227a7360`, on the release binary. Every number below is
**measured** unless it is marked *estimated*. The join script is
[`2026-09-28-router-scope.mjs`](2026-09-28-router-scope.mjs) (see
*Reproducing*). The demand side is the app-import metric's site list of the
same day ([baseline](2026-09-28-app-import-metric-baseline.md), wall 2).

The owner chose `@solidjs/router` as the next target on 2026-09-28: 27 of the
38 corpus apps import it, at 272 sites, and no installed version has a
contract.

## Headline

| | value |
| --- | ---: |
| app sites (`role: app`) / at a 2.0 prerelease / at a name the version exports | 272 / 263 / 259 |
| versions certified here (host free, `browser`, `node`) | next.30 (rc.9), next.26 (rc.9), next.21 (rc.6) |
| certification, all 9 runs | `certified`, every entrypoint (`.`, `./fs`, `./server`) |
| exports clean, any version, any host | **0** (next.30 0/32, next.26 0/31, next.21 0/31) |
| demanded sites any single wall unblocks alone | **22** (`./fs` `defineFileRoute` reads recipe, *estimated* by bytes; 2 measured), and 3 (`defineRoutes`, `defineRoute`: the accessor hazard; not under `node`) |
| sites the first four-cause bundle unblocks (callbacks, creates, reads, returns at the context-reading hooks) | 143 host free, 154 `browser`, 128 `node` (*estimated* over all versions); 60 / 62 / 59 at the measured versions |
| sites in an app environment on an audited runtime triple | 64 of 263 (24 %): next.26 × rc.9 and next.18 × rc.3 |
| harness wall, 3 versions per host | host free 60 s, `browser` 15 s, `node` 22 s (install cache warm after the first) |

- **The package certifies; nothing in it is clean.** All nine runs produce a
  complete certified catalog. Every demanded export stays partial or
  degenerate in every host, so no router site can be certified today, even
  with a tier bundle.
- **The top three exports are the same wall four times.** `useNavigate` (51
  sites), `useLocation` (41) and `useParams` (34) are one line each:
  `useRouter().navigatorFactory()`, `useRouter().location` and
  `useRoute().params`. `useRouter()` is `invariant(useContext(RouterContextObj), …)`,
  and `useRoute()` reads `RouteContextObj`, falling back to `useRouter().base`.
  Each of the four open domains stops at the same fact: the checker has no
  premise for a member of the value `useContext` returns. That is a missing
  claim form, not a generator bug, and it is not closeable soundly in one
  slice (see *The top wall*). This task stops after Part 1, with a proposal.
- **Bytes: the top hooks are identical in every installed version; the
  certification is not portable.** The closure of `useNavigate`,
  `useLocation`, `useParams` and 10 other demanded exports is byte-identical
  across all ten installed versions (next.16 to next.30). The tier admits a
  bundle only for its exact artifact and environment (ADRs 0123, 0126, 0131),
  so each (version, runtime) pair needs its own certification. Certifying is
  cheap (under a minute for three versions). What limits reach is the runtime:
  only 64 sites sit on an audited runtime triple (rc.3 or rc.9).

## Demand

272 app sites in 27 apps. 9 of them (kobalte-docs) are on the 1.x-era
`@solidjs/router@0.16.2` and are out of scope. That leaves 263 sites on ten 2.0
prereleases:

| version | sites | apps | apps' runtime (`solid-js` / `@solidjs/web` / signals) |
| --- | ---: | ---: | --- |
| next.24 | 59 | 2 | rc.8 / rc.8 / rc.8 |
| **next.26** | 47 | **7** | rc.9 / rc.9 / rc.9 (all seven) |
| **next.21** | 47 | **6** | rc.6 / rc.6 / rc.6 (all six) |
| next.18 | 44 | 4 | rc.3 ×3 (one with signals rc.8), rc.4 ×1 |
| next.23 | 26 | 3 | rc.7 |
| next.16 | 16 | 1 | rc.0 / rc.0 / rc.1 |
| next.20 | 11 | 1 | rc.4 / rc.4 / rc.5 |
| next.17 | 8 | 1 | rc.1 |
| next.19 | 5 | 1 | rc.4 |

The two "most installed" versions measured here are next.26 and next.21, by app
count (7 and 6). next.24 has more sites (59), but all 59 are in two apps on
rc.8, which has no audited archive and no install pin
(`scripts/ecosystem-benchmark/lib/runtime-pins.mjs`). A certification in its
apps' environment is refused before it starts. Each measured version was
certified on the runtime triple its own apps install: next.26 on rc.9 and
next.21 on rc.6. rc.9 is an audited triple. rc.6 is not: only
`@solidjs/signals@2.0.0-rc.6` is an audited archive, and `solid-js` and
`@solidjs/web` rc.6 are not (`rust/crates/solid-dialect/audited-archives.json`),
so next.21 was measured for its bytes but cannot be delivered (*Versions*).

| export | sites | apps | | export | sites | apps |
| --- | ---: | ---: | --- | --- | ---: | ---: |
| `useNavigate` | 51 | 18 | | `useHref` | 6 | 2 |
| `useLocation` | 41 | 16 | | `useResolvedPath` | 5 | 1 |
| `useParams` | 34 | 12 | | `memoryHistory` | 4 | 2 |
| `createRouter` | 29 | 25 | | `useAction` | 3 | 1 |
| `./fs` `defineFileRoute` | 22 | 5 | | `action`, `defineRoutes`, `useBeforeLeave`, `useRouteMatches` | 2 each | |
| `query` | 15 | 7 | | `RouterContext`, `defineRoute`, `hashHistory`, `useLinkState`, `useMatch` | 1 each | |
| `./fs` `fileRoutes` | 13 | 13 | | unattributed (`?`) | 4 | 3 |
| `revalidate` | 12 | 4 | | | | |
| `useSearchParams` | 11 | 7 | | | | |

## Per export: what holds each domain

Host free, identical at next.30, next.26 and next.21 unless a version is named.
A "class" is the certification metric's `causeOf`. The six refusal classes the
task names map onto these as follows:

- **generator decline**: `declined: …`;
- **missing claim form**: `missing claim form: …`, plus the census
  refusals (`census refusal: …`, `withheld operation: …`), where a census has no
  premise for the form it met;
- **recipe**: `recipe: …`;
- **dialect-silent**: `dialect-silent: …`;
- **dependency**: `unaccepted dependency: …`;
- **graph lane**: no router record is in this class. The metric's flags do not
  route through `--dependency-graph-lane`. The only graph-preparation refusal
  is next.30's `./fs` peer (below).

| export | callbacks | reads | returns | creates |
| --- | --- | --- | --- | --- |
| `useNavigate` | census: property-access-unknown-accessor | declined: runtime-accessor-installation | census: recursive-value-shape (completion not proven primitive) | declined: unresolved-callee |
| `useLocation` | census: property-access-unknown-accessor | declined: runtime-accessor-installation | census: recursive-value-shape (`openIndex`) | census: property-access-unknown-accessor |
| `useParams` | census: property-access-unknown-accessor | declined: runtime-accessor-installation | census: recursive-value-shape (root shape open) | census: property-access-unknown-accessor |
| `createRouter` | never proposed | declined: runtime-accessor-installation | never proposed (next.30), recursive-value-shape (next.26, next.21) | declined: unresolved-callee |
| `./fs` `defineFileRoute` | closed | next.26, next.21: **recipe: no probe recipe**; next.30: unaccepted dependency `filesystem-routing/flags` | closed (next.30: dependency) | closed (next.30: dependency) |
| `query` | never proposed | declined: runtime-accessor-installation | census: recursive-value-shape | declined: unresolved-callee |
| `./fs` `fileRoutes` | census: property-access-unknown-accessor | census: reads premise required (property-access-unknown-accessor); next.30: dependency | census: recursive-value-shape | declined: unresolved-callee |
| `revalidate` | census: property-access-unknown-accessor | declined: runtime-accessor-installation | closed | declined: unresolved-callee |
| `useSearchParams` | census: property-access-unknown-accessor | declined: runtime-accessor-installation | census: recursive-value-shape | dialect-silent `solid-js:createMemo` (`browser`: unresolved-callee) |
| `useHref`, `useResolvedPath` | proposed, not certified | declined: runtime-accessor-installation | never proposed | dialect-silent `solid-js:createMemo` |
| `memoryHistory` | census: coercion | declined: runtime-accessor-installation | never proposed | declined: unresolved-callee |
| `useAction` | attribution catch-all (next.26: invokes a caller-supplied callable) | declined: runtime-accessor-installation | census: recursive-value-shape (next.30), callable-path | dialect-silent `:action` |
| `defineRoutes`, `defineRoute` | closed | declined: runtime-accessor-installation | closed | closed |

Where the hosts differ:

- `browser` resolves `createMemo` so that the dialect row does not answer.
  `useSearchParams`, `useHref` and `useResolvedPath` then show
  `unresolved-callee` for `creates`, where host free and `node` show
  dialect-silent `solid-js:createMemo`.
- Under `node`, the contradiction veto for `revalidate`, `useBeforeLeave`,
  `defineRoutes` and `defineRoute` does not complete (`recipe: veto did not
  complete`, 17 sites). There are two reasons. First, "no jsx-free premise
  covers …/dist/routers/factory.jsx": under `node` the `solid` condition
  selects the untranspiled JSX module graph. Second, the probe worker "could
  not resolve `seroval`", which `@solidjs/web` reaches transitively.
  next.30 `node` has 12 partial and 20 degenerate exports, against 14 and 18
  host free.

### The reads wall is case-wide

`runtime-accessor-installation` opens `reads` for **every export of the
case**. Its closure hazard has empty `affected_exports`
(`contract_certification/module_closure.rs`). The default-condition `.` case is
one 150 KB bundle (`dist/index.js`). In next.30 its hazard sites are three
`new Proxy`, three `Object.defineProperty` and one `__proto__` literal:

- the three Proxies: `createMemoObject`'s, which is `params`; the `paths`
  builder's; and `action`'s submissions;
- `defineProperty` on the request event's `router.matches`, on a class
  instance's `paths`, and on a function's `name`.

The module-graph (`solid`) case carries the same sites in `utils.js`,
`paths.js`, `data/action.js` and `routers/*.jsx`. So `defineRoutes` (identity)
and `useNavigate` lose `reads` to a Proxy they never reach syntactically.

Narrowing the hazard to the exports that reach its site would **not** be sound.
The objects are shared through context. `useParams()` returns the router
context's `params` member, and that member **is** `createMemoObject`'s Proxy,
built by `createRouterContext` and handed over by the `Router` component
through `RouterContextObj`. Syntactic reachability from the export misses
exactly that flow. This is why the hazard is case-wide today, and it has to
stay so until a flow premise exists (see the proposal).

## Versions: can one certification answer several?

**Bytes.** Per demanded export, the table digests the export's closure inside
its entry file (the `default` condition: `.` is `dist/index.js`, `./fs` is
`dist/fs.js`). The closure is its top-level declaration, plus every top-level
declaration it names, transitively, plus the imported names it reaches. This
over-approximates, so an equal digest is the strong statement. Equal letters
mean equal bytes:

| export | n16 | n17 | n18 | n19 | n20 | n21 | n23 | n24 | n26 | n30 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `useNavigate`, `useLocation`, `useParams`, `useHref`, `useResolvedPath`, `useRouteMatches`, `useBeforeLeave`, `useLinkState`, `hashHistory`, `defineRoutes`, `defineRoute`, `RouterContext`, `./fs defineFileRoute` | A | A | A | A | A | A | A | A | A | A |
| `./fs fileRoutes` | A | B | B | B | B | B | B | B | B | C |
| `revalidate` | A | A | B | B | B | B | B | B | C | C |
| `memoryHistory` | A | A | B | B | C | C | C | C | C | C |
| `useMatch` | A | A | B | B | C | C | C | C | C | D |
| `query`, `useSearchParams` | A | B | C | C | D | E | E | E | F | G |
| `createRouter`, `action`, `useAction` | a different digest at almost every version | | | | | | | | | |

Whole files differ between every adjacent pair, because the bundle carries
every module. next.26 to next.30, for example, changes `routing.js`, `utils.js`,
`data/action.js` and `fs.js`, and adds `fsServer.js` and
`serverRouteComponent.js`.

The certification agrees with the bytes, in all three hosts. The 16 demanded
exports whose closure is identical at next.21 and next.26 get the same record
at both versions, domain for domain. Of the 15 identical at next.26 and next.30,
14 get the same record. The exception is `./fs` `defineFileRoute`: its own
bytes are equal, but next.30's `dist/fs.js` imports the `filesystem-routing`
peer, and that case-wide hazard opens every domain.

**What the admission rules allow.** One certification answers exactly one
artifact in one environment:

- the acceptance root binds name, version, lockfile integrity, entrypoint and
  conditions (ADR 0123, step 1);
- the installed files must reproduce the receipt's `snapshotRoot` (ADR 0131);
- every environment entry (`solid-js`, `@solidjs/web`, `@solidjs/signals` and
  their semantic closure) must be found by the recorded lookups (ADRs 0123 and
  0126).

A byte-identical export in another version is still another artifact, so each
(version, runtime) pair needs its own certification and its own bundle. This
report does not propose weakening that.

**What that costs.** Certification is not the bottleneck: three versions
certify in 15 to 60 s per host. The runtime is. The table below shows which
apps' environments can be certified today. A consumer-environment run needs
all three runtime packages to be audited archives (`AUDITED_ARCHIVES`), and
only the rc.3 and rc.9 triples are; rc.6 has an audited `@solidjs/signals`
alone:

| version × runtime | apps | sites | lockfile readable at admission (after `6ca77fb2` and `e10e58c7`) |
| --- | ---: | ---: | --- |
| next.26 × rc.9 | 7 | 47 | 47 (npm, yarn, pnpm one or two documents, `bun.lock` v1) |
| next.18 × rc.3 (beacon-web, solid-groove) | 2 | 17 | 7 (solid-groove's `bun.lock` v1; beacon-web's `bun.lockb` is not read) |
| everything else: next.21 × rc.6 (6 apps, 47 sites: `solid-js` and `@solidjs/web` rc.6 are not audited), next.18 × rc.3 with signals rc.8 and × rc.4 (27), and the rc.0/rc.1/rc.4/rc.5/rc.7/rc.8 runtimes | 17 | 199 | not certifiable: no audited triple, and ADR 0127 schedules no new work on older rcs |

The ceiling for the router is therefore **64 sites (24 %)**. When this scope
was measured, 30 of them had a lockfile the checker could read at admission;
since the lead's lockfile fixes (`6ca77fb2`, `e10e58c7`), 54 do. This assumes every export were
clean.

## Walls ranked by the sites they unblock

Sites at next.26 and next.21 read their own record. The table below also
counts sites at an unmeasured version, and each of those reads a proxy record:

- a measured version whose closure digest for that export is identical,
  nearest first (123 site-records);
- otherwise the nearest measured version (45 site-records, *estimated*).

A case-wide hazard follows the entry file, not the export, so both kinds of
proxy are estimates. The site-level numbers at the measured versions alone are
given after the table.

| wall | class | sites blocked | unblocks alone | top exports |
| --- | --- | ---: | ---: | --- |
| `runtime-accessor-installation` (reads, case-wide) | generator decline | 224 | 3 (`defineRoutes`, `defineRoute`; not under `node`) | `useNavigate` 51, `useLocation` 41, `useParams` 34, `createRouter` 29 |
| `recursive-value-shape` (returns census) | missing claim form | 196 | 0 | the same four |
| `property-access-unknown-accessor`, subject `call-result` (callbacks, creates) | missing claim form | 164 | 0 | `useNavigate`, `useLocation`, `useParams`, `fileRoutes` 13 |
| `unresolved-callee` (creates) | generator decline | 131 (`browser` 153) | 0 | `useNavigate`, `createRouter`, `query`, `fileRoutes` |
| callbacks never proposed | missing claim form | 50 | 0 | `createRouter` 29, `query` 15 |
| reads: no probe recipe | recipe | 22 | **22** | `./fs defineFileRoute` 22 (18 at next.24) |
| dialect row `solid-js:createMemo` (creates) | dialect-silent | 22 (host free, `node`) | 0 | `useSearchParams` 11, `useHref` 6, `useResolvedPath` 5 |
| returns never proposed | missing claim form | 21 | 0 | `useHref`, `useResolvedPath`, `memoryHistory`, `action` |
| veto did not complete (`node` only: jsx module graph, `seroval`) | recipe | 17 | 0 | `revalidate` 12 |
| reads premise required (`fileRoutes`) | missing claim form | 13 | 0 | `./fs fileRoutes` 13 |
| `filesystem-routing/flags` peer not installed (next.30 `./fs` only) | dependency | 0 app sites | — | no app is on next.30 |

The greedy unlock curve (host free) closes the wall set that clears the most
sites per wall added:

1. **+143**: the four walls at the context-reading hooks together. This
   clears `useNavigate` 51, `useLocation` 41, `useParams` 34, `revalidate` 12,
   `useRouteMatches` 2, `defineRoutes` 2 and `defineRoute` 1. It is +154 under
   `browser` and +128 under `node`.
2. **+46**: callbacks never proposed (`createRouter` 29, `query` 15,
   `useBeforeLeave` 2).
3. **+22**: the `defineFileRoute` reads recipe.
4. **+13**: the `fileRoutes` reads premise.
5. **+11**: the `createMemo` dialect row (`useSearchParams`).

Then come smaller steps, reaching 259.

At the measured versions alone (94 sites), step 1 is **+60** (`useParams` 22,
`useNavigate` 19, `useLocation` 18, `defineRoutes` 1). Step 2 is +18, the
`fileRoutes` premise +8, and the `defineFileRoute` recipe **+2**.

No single wall unblocks a context-reading hook. Each of the three is blocked
in four domains.

## The top wall

The top wall is one fact seen from four domains. **The checker has no premise
for a member of a package-owned context value.** It covers every demanded hook
that goes through `useRouter()` or `useRoute()`: `useNavigate`, `useLocation`,
`useParams`, `useSearchParams`, `useHref`, `useResolvedPath`, `useMatch`,
`useBeforeLeave`, `useRouteMatches` and `useLinkState`. That is 154 sites.

- **callbacks, creates.** The producer records `useRouter().location` as a
  `property-access-unknown-accessor` form, with subject derivation
  `call-result`. Only a parameter-rooted subject (ADR 0034), an own literal
  (ADR 0044) or a parameter-or-own-result (ADR 0093) has a disposition, so the
  census refuses. `useRouter().navigatorFactory()` is also a call to a member
  of that value: an unresolved callee.
- **reads.** The case-wide accessor hazard applies (above). Behind it, the
  reads census would refuse the same form, as it does for `fileRoutes`
  ("reads premise required").
- **returns.** The returned member's shape is open. `location` is an object
  literal of getters that read memos (`createLocation`). `params` is a
  `createMemoObject` Proxy. `navigatorFactory()` returns the navigate
  function. The census refuses each as `recursive-value-shape`.

This is **not ADR 0151 or ADR 0152.** ADR 0151 lets a wrapper cite a
dependency's accepted claim, but the context value here is the package's own
object reaching the export through a dialect primitive: there is no dependency
claim to cite. ADR 0152 concerns nested callbacks in described callables.
`createRouter`'s and `query`'s "callbacks never proposed" (wall 5, 46 sites)
may be in 0152's reach. That was not investigated here.

**Why it is not closed in this task.** A sound premise needs three things
that do not exist yet. Each of them is an ADR-sized decision.

1. **A provider census.** Which values can `useContext(RouterContextObj)`
   return? Every package-internal provider site of the context object has to be
   enumerated, and each provided value tied to a reviewed root. For the router
   that is the object literal `createRouterContext` returns, whose `location`,
   `params` and `navigatorFactory` are data properties, next to getters
   `pendingTarget` and `submissions`.
2. **An ownership rule for consumer-provided values.** `RouterContextObj` is
   exported as `RouterContext`, so a consumer can provide any value. Either
   such a value is the consumer's, as a parameter is under ADR 0034, or the
   premise must refuse whenever the context object is nameable. Refusing would
   refuse the router. That is a policy decision, not a mechanical one.
3. **A flow bound on the accessor hazard.** The hazard can leave `reads` of a
   context member only if no hazard site can install an accessor on the
   provided literal. Take `Object.defineProperty(e.router || (e.router = {}), "matches", …)`:
   it needs a proof that its target is not that literal. This is per-site
   alias reasoning.

Even with all three, `returns` stays open for the two largest hooks. Their
returned values are getter-bearing objects and a Proxy whose member reads are
tracked, and no returns claim form describes that today. So the context
premise alone is *estimated* to unblock 0 sites. It unblocks the 143 only
together with returns forms for `location` (a literal of tracked getters),
`params` (a `createMemoObject` proxy) and `navigate` (a returned callable,
ADR 0145, with its own writes).

## Proposal (as submitted; now ADR 0153)

The owner approved this proposal on 2026-09-28. It is now
[ADR 0153](../../adr/0153-a-member-of-a-package-owned-context-value.md), and its
first slice has landed (see *After ADR 0153's first slice* below). The text is
kept as submitted.

**ADR 0153 (proposed): a member of a package-owned context value.**

- **Scope.** A subject root `context-value`, which the producer states when a
  subject is the result of a dialect `useContext(C)`. `C` must be a
  module-level `const` initialized by the dialect's `createContext` in the
  artifact's own runtime source.
- **The census.** It enumerates every provider of `C` in the artifact's
  runtime source (a `C` component element, `createComponent(C, …)`, or the
  1.x-style `.Provider`, which the Solid 2 dialect does not have). Each
  provided `value` has to reach an own object literal (ADR 0044) through
  declarations the census can already follow. A member read of the subject is
  then dispositioned against that literal:
  - a data property is not an invoking form;
  - a `get` accessor is the literal's own code, and is walked like a callee;
  - a spread or computed key refuses.
- **The consumer-provider clause** needs an explicit decision. A value
  provided through the exported context object is the consumer's (the ADR 0034
  reading), or the premise refuses when `C` is nameable.
- **The hazard stays case-wide by default.** A `context-value` premise is
  admitted only when every `runtime-accessor-installation` site in the case
  has a target that is either a fresh allocation in the same expression
  (`new Proxy(<fresh>, …)` returns a *new* object, so its accessors are on the
  proxy, not on the literal) or a value that no provided literal can alias.
  Otherwise it refuses, naming the site.
- **Fixtures, against the real published typings.** A positive:
  `useLocation`-shaped, a literal data member. A negative: the member is a
  `get` that reads a signal, so `reads` has to stay open. Then:
  - a consumer-provided value, under whichever clause is chosen;
  - a context whose value is not an own literal, which refuses;
  - a `defineProperty` onto the provided literal, which refuses;
  - a namespace import and a re-exported context object.

  `tsc` has to be silent on every one of them. The stub for `solid-js`'s
  `createContext`/`useContext` must be byte-faithful to rc.9's
  `types/client/core.d.ts`.
- **Follow-on (separate ADRs):**
  - returns forms for a literal of tracked getters and for a memo-object
    proxy;
  - the navigate callable's call claims under ADR 0145.

  Only with those does a router hook become clean.

**The cheapest measured step, if the lead wants a site sooner:** a
hand-authored reads recipe for `./fs` `defineFileRoute` at next.26. It is
`function defineFileRoute(path, config)`, and it is byte-identical in every
version. It is the only demanded export whose one open domain is a missing
recipe. It unblocks 2 sites that sit on an audited runtime (next.26 × rc.9) and
22 by bytes, but the other 20 are at next.23 and next.24, on runtimes with no
archive. It needs a tier bundle for next.26 as well, and the lead regenerates
the tier.

## After ADR 0153's first slice and the installed versions

Measured 2026-09-28 on `router-scope`, on the release binary, with the same
flags, host free, `browser` and `node`.

**Certification inputs.** The benchmark manifest now carries a `solid2` row
for each (router version, audited runtime) pair the app-import corpus
installs:

- next.26 on the rc.9 triple, installed by 7 apps (47 sites);
- next.18 on the rc.3 triple, installed by beacon-web and solid-groove
  (17 sites);
- next.30, the corpus row, which no app installs.

Consumer environments match a package by its version, so a package may have
several rows. oscartbeaumont-website's environment, re-derived from its pinned
`pnpm-lock.yaml`, now delivers `@solidjs/router@2.0.0-next.26` beside
`@solidjs/meta`, and its delivery run certifies both.

These pairs are excluded, each for the reason given:

- next.21 × rc.6 (6 apps, 47 sites): `solid-js` and `@solidjs/web` rc.6 are
  not audited archives; only `@solidjs/signals` rc.6 is;
- next.18 with signals rc.8 (error-menu-web) or on rc.4 (app-game);
- next.16, next.17, next.19, next.20, next.23 and next.24: their runtimes (rc.0
  to rc.8) have no audited triple;
- consumer environments for the other next.26 and next.18 apps:
  - `derive-consumer-environment.mjs` reads `pnpm-lock.yaml` only. It now also
    reads pnpm 11's env document followed by the project document, under the
    checker's own reader's exact shape;
  - lutra-console and solid-validation-site write such pnpm 12 locks, but
    derivation still refuses both: each closure resolves two releases of
    `@solid-primitives/event-listener` (next.3 and next.5), and an environment
    pins one release per name;
  - en-passant (npm), jandibat-web (yarn), expenses-app, helge-dev and
    solid-groove (`bun.lock` v1) use formats the deriver does not read;
  - beacon-web uses the binary `bun.lockb`, which no reader reads.

**ADR 0153, slice A** (parts 1 and 2 plus part 3's fail-closed default, for
`creates` and `callbacks`). On the real router bytes at next.26 and next.18,
the producer states the context-member premise for `useLocation` and
`useIsRouting`. The chain is `useRouter()` then `invariant(useContext(RouterContextObj), …)`,
the provider is `createComponent(RouterContextObj, { value: routerState })`,
and the provided literal is `createRouterContext`'s. The census refuses both
exports with "the context escapes the package as `RouterContext`", as intended
until the admission gate (part 3) exists.

`useNavigate`'s refusal moves one leg deeper. Its form is now deferred, so the
walk first meets the member call `useRouter().navigatorFactory()`, whose callee
resolves to an interface member: `census refusal: unresolved callee`, 20 sites.

| | before | after |
| --- | ---: | ---: |
| exports clean, next.30 / next.26 / next.18, any host | 0 / 0 / 0 | 0 / 0 / 0 |
| app sites certified (package side, all versions, *estimated* by bytes) | 0 of 259 | 0 of 259 |
| app sites on a certifiable (version, runtime) pair | 47 (next.26) | 64 (next.26 + next.18) |

**Sites unblocked: 0.** No wall closes alone. The `useRouter()` hooks need all
of the following before any site is certified, in this order:

1. the admission gate (ADR 0153 part 3): a signed case premise, and a check
   that the consumer program never provides `RouterContext`;
2. a per-export bound on `runtime-accessor-installation` (item C): a complete
   enumeration of the member accesses an export executes, each on a receiver
   the census can prove is not a hazard site's object;
3. the `returns` shapes (part 4): a tracked-getter object for `location`, a
   memo-object proxy for `params`, and a described callable for `navigate`.

The app-import metric cannot move on this branch either way. It measures the
compiled-in tier, which the lead regenerates from these inputs.

## Reproducing

```sh
make build-checker-release
bun docs/package-contract-v2/phase22/2026-09-28-router-scope.mjs --prepare <dir>   # --no-scratch: committed rows only
for host in none browser node; do   # --host browser|node for the two hosts
  SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/release/solid-checker-rust \
  SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
  bun scripts/ecosystem-benchmark/run.mjs --manifest <dir>/manifest.json --solid 2 \
    --timeout 1800 --attempt-certification --recover-entrypoints \
    --probe-recipe-corpus scripts/ecosystem-benchmark/probe-recipes --keep-temp [--host $host] \
    --probe '@solidjs/router@2.0.0-next.30|solid2|only' \
    --probe '@solidjs/router@2.0.0-next.26|solid2|only' \
    --probe '@solidjs/router@2.0.0-next.18|solid2|only' \
    --probe '@solidjs/router@2.0.0-next.21|solid2|only' \
    --json <out>/run[-$host].json --markdown <out>/run[-$host].md
  bun scripts/certification-metric.mjs --run <out>/run[-$host].json --corpus <dir>/corpus.json \
    --json <out>/metric[-$host].json --markdown <out>/metric[-$host].md
done
# npm pack @solidjs/router@2.0.0-next.{16,17,18,19,20,21,23,24,26,30}, unpacked to <tarballs>/next.N/package
bun docs/package-contract-v2/phase22/2026-09-28-router-scope.mjs --closures <tarballs> \
  --sites rust/target/app-import-metric/metric.json --json <closures.json>
bun docs/package-contract-v2/phase22/2026-09-28-router-scope.mjs --join \
  --sites rust/target/app-import-metric/metric.json --closures <closures.json> --metric <out> [--exact] \
  [--measured 2.0.0-next.30,2.0.0-next.26,2.0.0-next.18]
```

The next.26 and next.18 rows are committed manifest rows, each on the runtime
triple its apps install. The next.21 row is a scratch clone of next.30's row,
because its rc.6 triple is not audited; its integrity is recorded in the
script. The certification metric's corpus still pins next.30 alone.
