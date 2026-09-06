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

### B. A compiled-JSX controlled profile — mostly not a compilation problem (2026-09-06)

**Corrected by measurement, and the measured part taken as ADR 0039.** This
entry assumed the `vetoThrew` class was a `.jsx` runtime target needing
compiled bytes. The reasons say otherwise: all 61 `.jsx` failures were the
pinned interpreter refusing an *extension*
(`TypeError: Unknown file extension ".jsx"`), on modules that are not markup —
`@corvu/utils` publishes 23 `.jsx` files under its `solid` condition and 20 of
them contain no JSX at all. A `.jsx` module the checker proves JSX-free now
executes as ECMAScript under a stated premise (ADR 0039): `vetoThrew` 62 → 1,
withheld 866 → 805, statuses unchanged. What remains of this lever is the
genuine case — a `solid`-condition module that really does carry JSX (three in
`@corvu/utils`) — for which the original design below stands.


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

**Step 2, taken (2026-09-06, handshake protocol 23).** Of the 15 coercion
sites left after step 1, 11 were **local helpers** reached from an export whose
own body cleared: `scalePoint` from `applyPointDelta`/`removePointDelta`,
`calcLength` from `aspectRatio`, `mixNumber` from `transformAxis`, `wrap` from
`getEasingForSegment`, `clamp` from `steps`, `binarySubdivide` from
`cubicBezier`, `formatErrorMessage` from `warnOnce`, `hueToRgb`, `fillOffset`,
`distance`. A helper has no declared signature; its premise is the **argument
types at the call that reached it, on the caller's twin**, recorded by the
caller's premised census (`callArgumentPremises`), carried on the helper's
local-declaration demand (`parameterPremises`), spelled as one `@param` per
slot on the helper's own twin, and bound the same way — text and declaration
identity, at every hop. A helper reached from two calls with different argument
types is two demands and two transcripts rather than a union, so each census
holds under exactly the condition the receipt records. Measured: withheld
904 → 866, `censusRefused` 786 → 748, coercion 80 → 30 (15 → 7 sites);
`vetoThrew` 45 → 62 as newly closed censuses reach the `.jsx`-entry veto
(ADR 0037's open class). Statuses unchanged. The 7 sites left: the 4 honest
ones (`unknown` and generic parameters, `@solid-primitives/utils`'s
`compare<T>`, `any`-typed declarations) and three new classes named in the
ADR — a helper's `any` return type in the caller's body (`scalePoint(…) +
translate`; a fixpoint over the local call graph), a `.d.ts` name the helper's
module cannot spell (`calcLength(axis)` under `Axis`), and a nested arrow's
untyped parameter (`binarySubdivide` through `getTForX`).

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

**The census half is already domain-parameterized; the veto half is the lever
(investigated 2026-09-06).** `census_dialect_axiom_for_callee` takes the domain
and asks `primitive_performs_no_operation(archive, name, domain)`, and the
generator's proposal walk asks `some_audit_denies_primitive(spelling, domain)`
the same way, so generalizing the terminator is mostly threading a parameter.
What is *not* in place is a falsifier. ADR 0036 synthesizes a veto from the
export's call signature and observes the domain's contradiction — exactly
`undefined`-vs-value for `returns`, own-property additions to `globalThis` for
`creates` — and a domain added to `ClaimDomain::PROPOSABLE` without an
observation of its own would have inherited the `creates` one silently. That
fallback is now closed: `reviewed_observation` returns nothing for an
unreviewed domain, so its candidates stay withheld for want of a recipe rather
than being gated by a veto watching the wrong thing.

An observation for `cleanups` does exist, and it is *precise* — measured on
both dialects, with the export's own owner:

| dialect | owner field | empty | one registration | two |
| --- | --- | --- | --- | --- |
| `solid-js@1.9.11` | `cleanups` | `null` | `array(1)` | `array(2)` |
| `solid-js@2.0.0` (`@solidjs/signals`) | `_disposal` | `null` | the function | `array(2)` |

`createSignal` perturbs neither field, so the observation is specific to
cleanup registration rather than to resource creation. But both fields are
**private runtime internals**, and the Solid 2 one is an underscore-prefixed
field of a prerelease whose shape moves between builds. Reading it is a premise
about the runtime's internals, which the dialect would have to state and audit
per version — a new premise class, and the reason this lever is one ADR of its
own rather than a thread-the-parameter change. `disposals` and `invalidates`
have no observation identified at all: a disposal of the caller's resource and
a store-proxy invalidation are not visible from the veto's side without the
same internals, and `invalidates` additionally needs the proxy question lever C
makes decidable.

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
4. ~~Lever C step 1~~ — taken as ADR 0038; ~~step 2~~ — taken as its
   amendment (protocol 23): helper premises from call-site argument types.
   Open under C: a helper argument type spelled by a `.d.ts` name the
   JavaScript module cannot resolve (spell it as `import(…).name` from the
   identity's declaration file).
5. **Lever B** ADR once A runs, Solid 2 first, 1.x with Babel reproduction.
6. **Lever F** — investigated 2026-09-06 and *not* taken: the census
   terminator generalizes by parameter, but every one of the three domains
   needs a veto observation, and the only one that exists (`cleanups`) reads
   private runtime internals. Its own ADR, with the dialect stating the owner's
   cleanup registry per version. The silent-inheritance hazard is closed in the
   meantime. Then `callbacks`, then `reads`/`writes`.
7. **Lever G** — the accessor class. Its first slice is taken as ADR 0040:
   a parameter-rooted accessor is the caller's code in **write** position too,
   which was the shape that dominated the class (`axis.min = …`, the
   `set-accessor` question ADR 0034 deferred to the `writes` domain). Measured:
   withheld 805 → 745, accessor refusals 404 → 284, statuses unchanged. What
   remains, on the same measurement: 101 element reads and 110 property reads
   whose receiver is not parameter-rooted — module-level untyped receivers
   (`isDragging[axis]`, `scaleCorrectors[key]`) whose initializer the census
   could inspect — plus 42 destructuring elements and 31 spreads. A
   module-level *object literal* needs no premise at all: the compiler binds
   its members as data properties and records no form.

   The third slice is ADR 0042, and it is the one that paid: the iteration
   protocol dispositioned by *whose value is iterated* rather than by its type.
   One helper — `chain`, published verbatim by five packages — carried 58 of
   the 90 iteration refusals and needed three premises at once: the caller's
   iterable, the engine-built rest array, and the value that iterable yielded.
   Measured: withheld 739 → 675, iteration 90 → 18, **64 candidates**, with the
   accessor class rising only 277 → 285 because `chain` is a leaf. Picking a
   leaf is what made the difference; the two slices before it cleared forms
   that were merely first in a queue.

   The fourth slice is ADR 0043, and it takes the case ADR 0041 deferred by
   name: **the root set is closed under the reads the census already
   dispositions**. A name a parameter's own object pattern bound, and a name a
   local declaration bound from an already-rooted initializer, hold the
   caller's value as surely as the chain they abbreviate — erase the
   intermediate and the census already certifies the result — while a
   parameter's *default* naming another parameter is caller-supplied under
   either branch and travels under its own derivation so the two never read
   alike. Measured: withheld 675 → 662, `censusRefused` 618 → 605, the accessor
   class 285 → 248. **Thirteen candidates**, which is the ADR 0041 pattern
   rather than the ADR 0042 one: the bodies it unblocked refuse at their next
   form, and **coercion is now the largest class in the lever** at 107
   refusals, up from 83. That is the measurement naming the next slice, and it
   is the same argument one more time — `point -= translate` on a
   parameter-rooted operand runs the caller's `Symbol.toPrimitive`, `valueOf`
   or `toString`, which is code in the caller's own artifact. ADR 0034 listed
   coercion among the forms it did not review; nothing since has reviewed it.

   The second slice is ADR 0041: a spread's operand and an object pattern's
   source are subjects under the same premise. It removed the destructuring
   class (`BindingElement` 42 → 6) but closed only **six** candidates, because
   the bodies it unblocked refuse at their next form — usually a spread of a
   local the engine just built (`{ ...parentTransition, ...rest }`). That is
   the measured next slice, and it is a **different premise**: the own
   properties of a spread or rest-element result are data properties, so
   reading one invokes nothing, *provided* nothing installed an accessor on the
   local between its creation and the read. Proving that is an escape
   question — the local must not reach code the census cannot see — and it is
   the first premise in this lever that is not about provenance. After
   ADR 0043 that residue is 98 property reads and 103 element reads on
   receivers that are neither parameter-rooted nor engine-built — module-level
   lookup tables (`scaleCorrectors[key]`, `transformPropOrder[i]`,
   `supportedWaapiEasing[easing]`), locals the engine built (`vars[key]`,
   `match[2]`, `value.split("/*")[0]`), and call results, which is a third
   premise again.

Nothing here is a `tsc` duplicate: every claim is about runtime reactive
behavior the type system cannot express. Nothing here loosens a refusal without
a stated premise the receipt carries.
