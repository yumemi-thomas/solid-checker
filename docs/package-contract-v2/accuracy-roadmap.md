# Accuracy roadmap for package-contract certification

- Status: proposal, not yet decided. Every lever below needs its own ADR or a
  measured prototype before implementation; this page ranks them.
- Date: 2026-09-06
- Measured on: the full ecosystem corpus, release checker, recipe corpus on
  both targets, 88 s wall on mains. 418 probes, 398 attempted, 368 certified,
  30 refused. All counts come from the kept `certification-audit.json` files
  and the run's report, not from the pinned report, which carries no reasons.

## What "accuracy" is here

A certified row is a contract whose *closed* claims are authenticated. It says
nothing about how many claims are closed: a row certifies with every export
still open. The benchmark's honest accuracy number is `exportsProven` — an
export with every behavioral domain closed — and it is **0 of 8950**.

Open claims by domain, corpus-wide:

| domain | exports open | exports closed |
| --- | --- | --- |
| `creates` | 8706 | 244 |
| `returns` | 8906 | 44 |
| `callbacks`, `reads`, `writes`, `invalidates`, `throws`, `cleanups`, `disposals` | 8950 each | 0 |

So no amount of work on `creates` alone moves `exportsProven`. It moves the
count of closed claims (80 316 open today), which is the number the
ecosystem report should also carry per domain. A second caveat: these columns
are computed from the generator's *proposal*, not from the certified document,
so a certifier-side closure (a veto that ran and passed) shows up only as a
smaller withheld count. The report should read the accepted catalog for the
certified state. The levers below are ranked by
closed claims per unit of design, with the domain lever last because it is the
only one that can move `exportsProven` at all.

## Where the loss is, measured

### 1. `creates` candidates withheld: 1367

| bucket | count | what it is |
| --- | --- | --- |
| `censusRefused` | 1086 | the implementation census declined the body |
| `vetoUnreproducible` | 220 + 12 + 1 | a synthesized veto the pinned Node cannot run for the artifact case |
| `vetoThrew` | 24 | `.jsx` runtime targets under the `solid` condition |
| `noRecipe` | 0 | — |

The 1086 census refusals are **146 distinct source sites**; every other
refusal is the same site reached from another export or another artifact
case. By form:

| form | refusals | distinct sites | dominant shape |
| --- | --- | --- | --- |
| coercion (binary operator) | 280 | 44 | `a - b`, `p <= 0`, `v > max` in untyped `motion-dom` / `motion-utils` JS |
| property access, unknown accessor | 312 | 52 | `axis.min`, `treeScale.x`, `currentElement._$host` on untyped parameters |
| callee is not a function declaration | 127 | 24 | `const isMotionValue = (value) => …`; callback parameters (`cb`, `signal`); one class method |
| iteration protocol | 66 | 12 | `for (const callback of callbacks)` over a rest parameter; spreads |
| callee outside own runtime source | 30 | — | `isObject` from `motion-utils` (an accepted dependency, ADR 0008 § 4.5) |
| `instanceof` | 29 | 8 | `value instanceof CancelledError`, `x instanceof EventTarget` |
| template / computed-name coercion | 27 | 5 | `` `${name}:${pseudoElement}` `` |
| local declaration with no binding identifier | 56 | — | `chain` in `@corvu/utils` |
| by-reference call (`CallableFunction.apply`) | 2 | — | honest |
| `await` on a non-Promise | 3 | 1 | honest |

The single root cause behind roughly 700 of the 1086 is that the census walks
**compiled, untyped JavaScript**, where every parameter is `any`, so every
binary operator may reach `valueOf` and every property access may reach a
getter. The census is right to refuse under that premise; the premise is what
can change (lever C).

The 220 unreproducible vetoes are one shape: the artifact case was certified
under conditions without `node`, selecting `solid-js/dist/solid.js` (or
`dist/dev.js` under `development`), and Node's own condition set adds `node`,
which selects `dist/server.js`. 12 more are `@tanstack/custom-condition`,
which is not a plain condition name the harness will hand to Node. One is a
`.ts` target under `node_modules`, which Node refuses to strip. Rows:
`@solid-primitives/form` 84, `@corvu/drawer` 46, `motion-solidjs` 38,
TanStack query 36, corvu accordions 16.

The 24 `.jsx` cases are `@corvu/drawer` 12 and `@corvu/accordion` 6 (Solid
1.x) and `@corvu-next/accordion` 6 (Solid 2), all from `@corvu/utils` or
`@corvu-next/utils` selecting `dist/index.jsx` under `solid`.

### 2. Artifact cases refused before any claim exists: 351

Whole cases emit nothing, so their exports are not even counted as open.

| reason | cases |
| --- | --- |
| `dependency-composition`: accepted dependency has no exact runtime binding for an export (`solid-js` → `ErrorBoundary`, `Errored`, `NotReadyError`, `createComponent`; `@solidjs/signals` → `$PROXY`; `@tanstack/router-core` → `DEFAULT_PROTOCOL_ALLOWLIST`; `motion-utils` → `MotionGlobalConfig`) | ~70 |
| `dependency-composition`: unresolved dependency module for an `export *` (`@tanstack/query-core`, `@tanstack/query-persist-client-core`, `@tanstack/router-core/ssr/server`) | ~30 |
| `published-artifact`: entry has no runtime ESM exports (`@kobalte/core` publishes `src/**/*.test.tsx` through a wildcard export; `jsx-runtime` entries) | 60 |
| `published-artifact`: declared target is not a file (`@solid-devtools/debugger`, `@kobalte/themes`, three `@solid-primitives` `dist/index.jsx`) | 36 |

By row: `solid-js` 50, `@kobalte/core` 45, `@kobalte/solidbase` 33,
`@solid-devtools/debugger` 28, `@tanstack/solid-start` 27, `@solidjs/web` 25,
`corvu` 18. The published-artifact class is honest — the publisher's bytes are
what they are. The dependency-composition class is the graph lane's own
frontier, and the fact that `solid-js@1.9.14` refuses a case because
*`solid-js`* has no runtime binding for `ErrorBoundary` needs an answer: either
the bundled contract's runtime-binding index omits component-valued and
symbol-valued exports, which is a gap, or the binding is genuinely not exact,
which the reason should then say.

### 3. Inapplicable cases: 579

355 unpublished private-condition targets, 141 non-module targets (`.map`,
`.json`, `.css`), 83 declaration-only targets. All honest; not a lever.

## Levers, ranked

### A. An executor for the client build — taken as ADR 0037 (2026-09-06)

The cheap, sound path turned out to be inside the Node lane. Node's exports
resolution takes the first key in object order that is in its condition set,
and every `solid-js` and `@solidjs/web` in the corpus orders `browser` before
`node`. So a launch carrying `--conditions=browser` lands on the client build
the case certified. ADR 0037 adds a bounded reproduction-condition search:
the requested set first, then `browser`, admitted only when the planning-time
replay passes for every planned case *and* every `exports`/`imports` object in
the authenticated closure selects identically under the resulting set. It
also closed a gap the tracer tests exposed: the value-only lane never replayed
its dependency edges at all, so its vetoes ran against Solid's server build
with no refusal. Both lanes now replay every closure edge.

What remains for the browser lane (ADR 0033) is what genuinely needs a DOM,
not the condition gap. The 12 `@tanstack/custom-condition` cases are a
condition-name grammar question, separate and small.

**Census CPU policy — measured, deferred.** On the release binary one census
costs about 0.09 s (the pinned Node executable dominates and labels hash in
parallel), so the corpus's ~590 censuses are ~14 s of wall across eight
workers. Not a prerequisite for anything above; revisit only if the browser
lane's 196 MB bundle census becomes a real cost.

Rejected alternative: certify the `node`-condition artifact case instead, so
Node reproduces it. The server build of `solid-js` has no reactivity, so a
`creates` veto run against it observes nothing and proves nothing about the
client build the consumer runs. Unsound for the claim; do not take it.

Rejected alternative: a checker-owned resolver inside Node via module
customization hooks. It reverses ADR 0006's premise that Node's resolver is
the arbiter and its echo the proof, and duplicates the browser lane's
resolution premise in a second lane. The browser lane already owns it.

### B. A compiled-JSX controlled profile (24 candidates, one new premise)

Both compilers are already workspace crates: `solidjs-compiler` (the official
Solid 2 compiler, Rust) and `dom-expressions-compiler` (the 1.x port). A
`compiled-jsx` profile derives the runtime bytes for a `.jsx` target with the
pinned compiler at the pinned revision, the way ADR 0030 derives erased
TypeScript, and the receipt names *compiled by `<crate>@<rev>` with
`<options>`* as a premise the consumer must match.

- **Solid 2 (6 cases):** the crate is the compiler applications use. The
  profile can be admitted on the crate's identity alone once `default-features
  = false` is confirmed to expose the compile entry (it does; the `node`
  feature only adds N-API bindings).
- **Solid 1.x (18 cases):** applications compile with
  `babel-plugin-jsx-dom-expressions`; the port targets 0.40.10 and records 64
  probe divergences from it. A receipt over the port's output is a receipt
  about the port. Two honest shapes: require an independent reproduction, as
  ADR 0030 does with Node's strip — run the pinned Babel plugin in the harness
  and refuse when the bytes differ — or emit a capability that names the port
  and let a consumer that compiles with Babel refuse it. The first is the one
  that lets Babel consumers accept the contract.

The 1.x reproduction is also the natural place for the `solid`-condition
`.jsx` cases to gain the same conditions treatment as lever A: the compiled
output imports `solid-js/web`, which is again resolved under the artifact
case's conditions, so lever B depends on lever A's executor to run.

### C. Type premises for the census over untyped runtime source — root slice taken as ADR 0038 (2026-09-06)

The census refuses `a - b` because `a` is `any`. But the veto for the same
export already samples its arguments from the *declared* call signature, so
the closure is already conditional on typed callers. Making the census state
the same premise — "under the export's declared signature" — is consistent,
not a relaxation.

**Step 1, taken.** The producer classifies the root implementation's form
census on a checked twin of the JavaScript file carrying
`@type {typeof import("<declaration module>").<name>}`, so the compiler's own
contextual typing carries the declared types into the body (parameters, the
returned arrow's parameters through the return type, a `map` callback through
the declared element type). Each parameter's type on the twin must print
identically to the declared signature's, the transcript states the premise,
the verifier binds it byte for byte to the one signature the synthesized veto
samples from, and the receipt records `census-premise:` sites. Measured on the
corpus: withheld candidates 1093 → 904, `censusRefused` 992 → 786, coercion
refusals 357 → 80 (60 → 15 distinct sites), iteration 97 → 90; the accessor
class *rose* 348 → 398 because the cleared coercions were hiding accessor
refusals behind them. Statuses unchanged (368 / 30).

**Step 2, open.** Of the 15 coercion sites left, 11 are **local helpers**
reached from an export whose own body cleared: `scalePoint` from
`applyPointDelta`/`removePointDelta`, `calcLength` from `aspectRatio`,
`mixNumber` from `transformAxis`, `wrap` from `getEasingForSegment`, `clamp`
from `steps`, `binarySubdivide` from `cubicBezier`, `formatErrorMessage` from
`warnOnce`, `hueToRgb`, `fillOffset`, `distance`. A helper has no declared
signature; the premise for it is the **call-site argument types in the
caller's twin**, spelled as `@param` tags per slot from the caller twin's
checker, carried on the local-declaration demand, and bound the same way. A
helper reached from two callers with different argument types takes the union.
The remaining 4 sites are honest: `unknown` and generic parameters
(`@solid-primitives/utils`'s `compare<T>`), and `any`-typed declarations.

**Step 3, not a lever.** Accessor forms are unchanged by design (ADR 0034's
rejection stands: a declared property type says nothing about a getter). The
398 accessor refusals are the largest census class now; they are lever G
below.

### D. Census callee vocabulary (≈130 refusals, ≈25 sites) — first slice taken 2026-09-06

- ~~A `const` binding whose initializer is a function or arrow expression~~ —
  taken: the verifier binds an arrow or function expression that is the whole
  initializer of a plain `const`/`let`/`var` declarator to that identifier and
  holds it to the same written/redeclared checks as a named declaration
  (ADR 0008 amended). A `let` is admitted on the same terms; `invariant`
  refuses because it is *written*, not because of its keyword. A declarator
  initialized by a call (`supportsLinearEasing = memoSupports(…)`) refuses by
  name.
- A method on a class declared in the artifact's own runtime source, called on
  `this`: follow it (`getAll`).
- A callee that is a *parameter* (`cb`, `signal`, `b`) is caller-supplied code.
  Its `creates` is not the export's to close; it belongs to `callbacks`. Keep
  refusing, and let the reason say the domain that owns it.
- A callee in an accepted dependency (`isObject` from `motion-utils`): ADR 0008
  § 4.5's terminator — a dependency export whose `creates` is closed by an
  authenticated receipt in the same graph transaction. The graph lane has the
  receipts; the disposition has not been taken. This is what turns the graph
  lane from "certify dependencies too" into "compose closures across them".

### E. Dependency-composition refusals — investigated (2026-09-06)

The report's `artifactCaseRefusals` are the *first* generation's refusals,
before the published-graph lane re-generates with the dependency accepted, so
the 351 overstates the loss. After certification, 46 rows are partial and
227 declared entrypoints are uncertified; 112 of those are one row
(`@tanstack/charts`, 113 entrypoints, no refusals recorded — its own
investigation). The residual dependency-composition refusals sit on rows that
ran the **reused-proposal** lane (`@tanstack/solid-pacer` 13,
`@tanstack/solid-start` 7 per row, `@tanstack/solid-table` 6,
`@tanstack/solid-router` 4–6): the runner asks for the graph lane whenever a
partial proposal has a dependency frontier, so these are graph preparations
that fell back — the next question is why, per row. The `solid-js@1.9.14`
`./web` case refusing because *`solid-js`* has no binding for `ErrorBoundary`
is the graph lane's own frontier applied to a dialect-defining archive, and it
is moot while that row refuses on `createResource`'s recursive value shape.
Not an indexing bug: the binding comes from the accepted catalog the graph lane
builds, not from the bundled contract, which does carry `ErrorBoundary`.

### F. Domains beyond `creates` — the only lever that moves `exportsProven`

The `creates` census walk enumerates every reachable callee and decides the
claim against a per-dialect negative table. `cleanups`, `disposals` and
`invalidates` are the same shape of claim — "no reachable callee is a dialect
primitive of this class" — over the same walk with a different table. The 244
exports whose `creates` closed would very likely close these three in the same
transcript, and the negative rows come from the same audited dialect
documents. That is one ADR generalizing the census terminator per domain and
one veto family per domain.

The remaining domains each need something new:

- `callbacks`: whether the export invokes its callable parameters and in which
  phase. The parameter-use census exists in the producer; the claim shape and
  veto do not.
- `reads` / `writes`: every property access on a value that may be a store
  proxy is a read or write. This is the proxy property-access form ADR 0008
  § 4.4 names, and it interacts with lever C, which is what makes "may be a
  proxy" decidable at all.
- `throws`: not a census target under schema version 1. A schema decision
  before any code.
- `returns`: 44 closed via ADR 0035's valueless-completion census; a value
  shape census is the extension.

`exportsProven` stays 0 until every one of these has a census; the first
exports proven will be the ones with trivial bodies. The report should carry
closed claims per domain so the intermediate progress is visible.

## Suggested order

1. ~~Lever A~~ — taken as ADR 0037 without the browser lane or a census policy
   change; see above.
2. ~~Lever E~~ investigation — done; the residual is a lane-fallback question
   on four TanStack rows and one 113-entrypoint row, recorded above.
3. **Lever D** — const-bound callees taken 2026-09-06; own-class methods and
   `new` on an own-source class remain (18 refusals, 3 sites in `motion-dom`).
4. ~~Lever C step 1~~ — taken as ADR 0038; step 2 (helper premises from
   call-site argument types) is the next census slice, ≈11 sites.
5. **Lever B** ADR once A runs, Solid 2 first, 1.x with Babel reproduction.
6. **Lever F** ADR: per-domain census terminators over the existing walk, then
   `callbacks`, then `reads`/`writes`.
7. **Lever G** (new): the accessor class — 398 refusals at 73 sites after
   ADR 0038, mostly writes into a parameter-rooted object (`axis.min = …`, a
   `set-accessor` question ADR 0034 left to the `writes` domain), module-level
   object-literal receivers whose initializer the census could inspect
   (`isDragging[axis]`, `scaleCorrectors[key]`), and destructuring of locals.
   Each needs its own premise; none is a type question.

Nothing here is a `tsc` duplicate: every claim is about runtime reactive
behavior the type system cannot express. Nothing here loosens a refusal without
a stated premise the receipt carries.
