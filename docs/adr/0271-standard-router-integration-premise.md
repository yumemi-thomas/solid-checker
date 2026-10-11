# ADR 0271: standard router integration premise

- Status: accepted owner decision; implemented (2026-10-11).
- Owners: the next.26 authored spec, authored index/tooling, and backend
  authored-tier selection. No Solid dialect or shared IR vocabulary changes.
- Extends ADRs 0198/0226; uses ADR 0266's two-state positive visible veto.

## Decision

For an admitted installed `@solidjs/router@2.0.0-next.26`, assume that router
hooks execute beneath the Router component returned by that package's
`createRouter`. Its history is absent or the direct result of that package's
`browserHistory`, `hashHistory`, or `memoryHistory`; the application supplies
no value for the exported `RouterContext`. This is the **standard router
integration premise**, conditional package knowledge rather than an inferred
absence of custom integration.

There are exactly two states: **Assumed**, by default, and **Vetoed(site)**.
Only positive, exact runtime identity evidence vetoes. Unresolvable references,
missing/ambiguous facts, alias cycles, type-only imports and typeof type queries
do not veto. Neither state asserts a defect. A veto selects open summaries for
only the exports the authored entry explicitly conditions on this premise;
their import obligations fall back to SC9005. Other contracts, `defineRoutes`,
`defineFileRoute` and independent Solid violations retain their meanings.

## Runtime authority

All citations below are published next.26 bytes installed at
`../solid-checker-rc13-corpus/apps/helge-dev/node_modules/@solidjs/router/`.

- `dist/routers/factory.jsx:171-254` mounts the package's router context and
  obtains its integration from the configured/default history. The bundled
  equivalents are `dist/index.js:1151,1166,1306`.
- `dist/routers/history.js:63-180` supplies the standard factory utilities.
  Browser/memory histories have no custom query or params wrapper; hash
  history supplies the package's own hash render/parse helpers.
- `dist/routing.js:490-528,724-751,947-982` constructs location, base/route
  params and path resolvers. `dist/types.d.ts:281-300` describes the utility
  substitution boundary. Without the premise, its callbacks are arbitrary.
- `dist/utils.js:124-147` captures the creating router/route owner in
  `createMemoObject`. Arbitrary first-use keys create memos inside
  `runWithOwner(owner, ...)`, then read their accessors. `ownKeys`/`has` read
  the existing source. The later reader does not supply the memo's parent.

The params result is the proxy itself: every property Get, ownKeys and has
trap reads reactive state. Its whole-object store role is exact. Both branches
of useSearchParams (`dist/routing.js:166-209`) return such a proxy as tuple
item 0; obtaining the tuple item is inert, while its proxy accesses are reactive.
The setter and typed validator behavior stay open. Captured-owner params/query
source plumbing imposes no ambient consumer owner registration requirement.

Location is instead a plain object (`dist/routing.js:509-527`). Pathname,
search, hash and state are reactive getters; key is an inert getter; query is
a plain data property holding the proxy. Retrieving query or enumerating
location does not read reactive state. The former whole-object store positive
was incorrect and is removed, including from next.16/17/18. Their independently
inspected location constructors are respectively `donegeon-client/web`'s
`dist/routing.js:436-474`, `compass-ui/apps/ui`'s `:464-502`, and
`solid-groove`'s `:471-509`, beneath each app's `node_modules/@solidjs/router`.

The older versions' `useParams` and `useSearchParams` store positives are
removed too. This premise conditions next.26 only, and in next.16/17/18 a
history's `utils.paramsWrapper` or `utils.queryWrapper` replaces the proxy
with any value its declarations allow (next.16 `dist/routing.js:473,600-604`;
next.17 `:501,648-652`; next.18 `:508,655-659`). No corpus finding moved.

Existing object outputs describe member values, not getter execution. A
`reactive` member is an accessor function, not a reactive scalar Get.
ADR 0255's lazy-getter-object has different observer-dependent semantics and
never proves an untracked violation; ADR 0235/0249 describe calls, and
property-get operations describe caller-argument Gets. None expresses this
mixed result exactly. This round adds no vocabulary: location returns stays
open and has no store positive. The real `location.search` seed defect at
donegeon-client's WaitlistRoute.tsx:33 therefore loses its native proof until
an exact getter representation exists; runtime regression pairs retain it.

## Exact visible veto

The authored spec records package-relative declaration **name-node byte
spans**, independently of source identifier spelling:

| Identity | Declaration | Bytes |
| --- | --- | --- |
| RouterContext's canonical RouterContextObj | dist/routing.d.ts | 1101..1117 |
| createRouter | dist/routers/factory.d.ts | 6426..6438 |
| browserHistory | dist/routers/history.d.ts | 758..772 |
| hashHistory | dist/routers/history.d.ts | 872..883 |
| memoryHistory | dist/routers/history.d.ts | 1156..1169 |

These are UTF-8 byte offsets, including the multibyte characters earlier in
the published declarations; JavaScript string indices are not these offsets.

The backend joins exact queried entities, canonical Type Facts aliases and one
declaration, plus existing owning-manifest evidence for the exact package and
version. Npm aliases and linked/store roots normalize through that evidence;
conflicting owners, duplicate entities and merged declarations supply none.
Oxc's binder also follows exact immutable identifier aliases. Transparent
TypeScript wrappers preserve identity. No contained-span approximation,
import spelling, guessed member dispatch or filename suffix is an identity.

Veto on any positively identified runtime value reference to RouterContext,
including renamed imports, aliases/escapes, namespace members and named
re-exports. A mere erased type query is not a runtime reference. Even a
read-only context lookup vetoes: this deliberately small rule does not
distinguish context inspection from supplying a provider value.

An identified createRouter call vetoes if its first argument is not an object
literal, or if its history property is not an identified direct call of one
of the three factories. Spread/computed config keys veto. Saved history values
veto even when initialized by memoryHistory; mutable history objects can be
patched before mount. Config history getters/methods, duplicate keys and
prototype-setting members are also conservatively refused by the existing
closed-literal fact. Nested route properties do not masquerade as config
properties. Quoted static keys and transparent wrappers remain supported.

The scan covers every analyzed project source, including unimported setup
files. Dependency internals, excluded sources, reflective/dynamic integration,
computed callee targets, unresolved mutable aliases, namespace escapes without
an exact member identity, unqueried re-export trails, prior/HMR installation
and custom providers reached through unsupported channels remain outside the
premise. The project must ensure the condition there; silence is not proof.

## Mechanism and cache identity

`spec.premises` names the affected exports, value-reference identities and
config-call/factory identities. The authored tier carries this metadata in its
repository-owned format-1 index. Producer and consumer validate the same
metadata; the producer binds it and each affected call graph into fresh probe
digests. There is at most one integration premise per entry in this slice.

The public schema-1 contract document does not change. It continues to state
the conditional authored claims, just as existing claims adopt ADR 0226's
premises. The backend loads both that accepted contract and an open weakening
of exactly the named exports. The weakening gets its own semantic/proof and
closed-claims digests via the ordinary authored acceptance path. Existing IR
and diagnostic cache fingerprints therefore distinguish Assumed and Vetoed
meaning without teaching shared IR about routers. Artifact admission and
project-catalog precedence are unchanged.

## Closures and remaining open domains

Under ADR 0226's cited-source policy and the named premise:

- useLocation closes reads, creates and callbacks; returns stays open with
  no positive return shape. useParams closes all four domains. Hook
  construction reads no reactive property and invokes no caller callback.
- useHref/useResolvedPath close reads, returns and creates, with explicit
  optional parent memo registration, tracked initial argument invocation and
  reactive accessor returns. Callbacks stays open: native projection
  (`contracts.rs:1503-1536`) requires a same-stack call-triggered invocation
  with min >= 1, excluding optional initial execution and external triggers.
  Hydration permits zero initial executions, so increasing min is unsound.
  The unexercised path-update rows are removed; callback pairs assert only
  initial execution. Remaining reads/returns/creates closures stand on the
  hook construction census and do not require those later invocation rows.
  useResolvedPath's internal route-path reads are tracked. Context
  absence/throws stays open. Native projection is unchanged in this round.
- useMatch closes returns and creates (two optional-parent memos). Reads and
  callbacks stay open for dynamic match-filter dispatch and path coercion.
- useSearchParams stays open: its typed-path branch executes arbitrary
  Standard Schema validators and creates a query proxy under the hook caller,
  while the returned setter retains the stateful navigation/composer protocol.
  The premise does not eliminate either blocker.
- useLinkState is unclaimed: the multi-memo active/current/pending graph,
  coercion and in-flight target behavior require their own complete census
  and positive probe pairs. No pending-only behavior is guessed.
- The already-closed useIsRouting/useRouteMatches claims also require this
  premise and are now explicitly withheld on a veto. createRouter/history
  methods/useNavigate/query/action lifecycles stay open.

## Read-only corpus veto census

Before finalizing the rules, a source grep plus TypeScript AST inspection
found 54 direct-import createRouter calls (excluding node_modules, generated
outputs, vendor bundles and prior probe scratch). 43 fit the premise; ten use
saved histories and one forwards a nonliteral config. No spread/computed config
occurs in those app calls. This is a source prediction, not a checker sweep.

| Rule | Top-level corpus projects and sites |
| --- | --- |
| RouterContext runtime value (1 project) | skyjtx-website/apps/skyjt-website/src/services/portfolioData.ts:115 |
| Nonliteral config (1 project) | skyjtx-website/packages/typed-routes/src/router/create-router.ts:16 |
| Non-direct history (4 projects; 10 calls) | app-game/apps/gpu-text-rendering (one test); jar-hell-web/web (one test); kui/frontend (seven calls across alerts, clusters, consumers, messages and topics test harnesses); solid-groove (one test) |
| Spread/computed config (0 projects) | None |

Normal production entry routers remain Assumed except Skyjtx's explicit
context use/nonliteral forwarding. The saved-history controls are legitimate
test harnesses, deliberately outside the direct-call premise; a veto says
nothing about their runtime correctness. Projects excluding those tests do
not veto merely because the files exist on disk. Skyjtx's inspection is not a
provider mutation; the conservative value-reference rule still withholds it.

## Verification

Focused fixture `fixtures/package-contracts/standard-router-integration`
pins selective weakening, receipt/cache identity, each veto and unresolved
controls, with no reduced package stubs. Published-typing checks use strict
noEmit and skipLibCheck=false. Round 1 passed fifteen Chrome pairs, including
five memo-owner/callback pairs. Round 2 adds search/key/query/enumeration
location controls; current probe and native results are recorded in
`rust/target/research/router-premise/RESULT-2.md`. Next.16/17/18 probes are
lead-owned. No corpus sweep or full verify is claimed here.

The native checker recognizes the three leaf-owner pairs as SC3001. The
tracked-callback write pairs demonstrate initial runtime behavior, but native
projection does not diagnose either write. The explicit open callbacks domain
now retains SC9005 at each hook import, independently of open createRouter/history
imports. useLocation retains its open returns obligation, and useMatch retains
its explicit open-domain SC9005 results. Inert location controls must have no
native violation; an open import obligation is not whole-program certification.
