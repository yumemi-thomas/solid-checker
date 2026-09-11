# `reads` veto observation: the design, and why I recommend not shipping it

- **Status:** design for review. Nothing implemented;
  `reviewed_observation("reads")` still returns `None`.
- **Date:** 2026-09-10.
- **Recommendation:** **do not add a synthesized veto for `reads`.** Gate the
  domain on hand-authored probe recipes instead, and let gate 3 proceed
  without one.

## 1. What a synthesized veto is here

ADR 0036 derives a probe module from the export's Type Facts call signature and
runs it against the real artifact. The module samples argument tuples, calls
the export, and emits a marker if it observes something that **contradicts**
the proposed closure. It never proves the claim — the implementation census
does that — it only tries to falsify it.

`synthesized_vetoes.rs` keeps one `ReviewedObservation` per domain, each
stating its own exactness:

| domain | observation | exact? |
| --- | --- | --- |
| `returns` | any call whose result is not `undefined` | yes |
| `creates` | own-property additions to `globalThis` during the call window | **explicitly not** |

The module already has the scaffolding a `reads` observation would need: a
`callback` whose invocations are counted in `invoked.count`, a `before`
snapshot, and a `yielded` flag, all in scope where the observation's `emit`
snippet is spliced in.

## 2. The design

Two instrumented argument kinds, placed by `sample_tuples` according to the
declared signature, and one emit.

**Probe accessor** — for a slot the signature types as a nullary function
returning a value:

~~~js
const reads = { count: 0 };
const probeAccessor = () => { reads.count += 1; return undefined; };
~~~

**Tripwire object** — for a slot typed as an object, so a property access on a
caller-supplied receiver is visible:

~~~js
const tripwire = (keys) => Object.defineProperties({}, Object.fromEntries(
  keys.map((k) => [k, { enumerable: true, get() { reads.count += 1; return undefined; } }])
));
~~~

**Emit:**

~~~js
if (reads.count > 0) harness.emit({ marker: "read-operation", kind: "call", phase: "enter" });
~~~

with

~~~
observation: "calls of caller-supplied accessors and getters on caller-supplied
objects during the call window; observes no read of a source the export owns"
~~~

It is deterministic in the signature, needs no runtime internals, and uses
public API only. As a piece of engineering it works.

## 3. Why it is the wrong observation

**It cannot see the half that the domain is now about.**
`semantic-model.md` § reads **[Decision 2026-09-10]** scopes `reads` to a proxy
or source **the export owns**: a read through a caller-supplied value is the
*caller's*, and is explicitly not this export's `reads`. Everything the design
above observes is on the caller's side of that line. Everything on the export's
side — a module-level signal, a store it created, an accessor it imported — is
invisible to it.

So the two failure directions are:

- **False negative, systematically.** The observation would run clean for an
  export that reads a source it owns on every call, and the gate would record
  "nothing contradicted the closure". That is exactly the hazard ADR 0036
  closed when it made `reviewed_observation` return `None` for an unreviewed
  domain: *"a `cleanups` candidate gated by a veto watching `globalThis` would
  report 'nothing contradicted it' for a claim that observation says nothing
  about."* Shipping this would reopen that hazard under a different name.
- **False positive, arguably.** Invoking a caller-supplied accessor may be a
  `callbacks` item rather than a `reads` one — the census plan § 3.2 says the
  export's act on a caller's callable *is* the invocation. A veto firing on it
  would refuse a closure that is not contradicted. That direction is safe
  (refusal, not certification) but it is still the wrong signal.

**And the exact version needs internals.** Observing an owned read means
knowing which sources a computation subscribed to: run the call inside a
computation the probe owns and inspect its source list. In
`@solidjs/signals@2.0.0-rc.3` those fields are minified — `reporterBlocksSource`
reads `e.ie`, `e.o?.le`, `e.nt`, `e.it`, `e.ut`, `e.lt`, `e.S`, `e.o?._` — and
minified private fields move between builds. That is the premise class lever F
already flags for `cleanups`, and it is *worse* here: `cleanups` had a named
owner field per dialect, this has a minified graph.

`enableExternalSource` is not a way out. Its factory wraps external systems
into Solid's tracking; it sees computations, not the Solid-internal sources
they read.

## 4. Recommendation: no synthesized veto, hand recipes instead

The system already has the right behaviour for a domain with no reviewed
observation, and it is not "blocked" — it is "withheld for want of a recipe".
`contract certify --probe-recipe-corpus <DIR>` takes hand-authored,
claim-addressed recipes, and a hand recipe *can* observe an owned read,
because its author knows which source the export owns and can watch that
source specifically.

So:

- Leave `reviewed_observation("reads")` returning `None`, deliberately, and
  record here that it is a decision rather than an omission.
- Let **gate 3** proceed: `ClaimDomain::Reads` into `PROPOSABLE` and into
  `require_census_decides_closure`. Candidates are then proposed, censused,
  and withheld for want of a recipe — which is the measurement the accuracy
  roadmap asks for, with no unsound closure anywhere.
- Closure then arrives one hand recipe at a time, starting with the exports
  that matter to real consumers rather than with whichever export is first in
  a queue — the lesson ADR 0042 paid for.

**What this costs:** `reads` will not close in bulk. That is the honest price
of a domain whose contradiction cannot be observed generically without reading
a minified reactive graph.

**What it buys:** the census, the rows, and the proposal path all land and stay
sound, and the measurement shows exactly which exports a recipe would unblock.

## 5. Gate 3 landed (2026-09-10), on this recommendation — and was withdrawn the same day (§ 6)

`reviewed_observation("reads")` still returns `None`, deliberately, and the
domain was admitted anyway:

- `ClaimDomain::PROPOSABLE` is now `[Creates, Reads, Returns]`.
- `require_census_decides_closure` admits
  `(Implementation, Call(Reads))`, with the rationale recorded at the match
  arm: `reads` rides the **existing** form machinery rather than new analysis.
  `census_form_disposition` already clears exactly the two provenances § reads
  **[Decision 2026-09-10]** excuses — a parameter-rooted receiver (ADR
  0034/0040, the caller's) and an own object literal (ADR 0044, data
  properties the specification created) — and refuses every other subject
  root, *including a value returned by a call*. A store obtained from a Solid
  primitive therefore arrives as a call the walk enumerates, and an
  undispositioned access refuses the domain.
- The refusal message for the remaining domains now names "creates, reads and
  returns".

Four tests changed, none to go green:

- `contract_semantics::tests` — the `PROPOSABLE` array, `Reads.is_proposable()`,
  and the *undecidable domain* example, which was `reads` and is now `writes`.
- `contract_document::tests` — the "no closure proof mode" refusal case, same
  swap, same reason.
- `inferred_contract::tests` — a cleared walk now offers
  `{Creates, Reads}`. This is the intended coupling and not silent inheritance:
  `open_proposed_closure` weakens only the generator's own *complete-negative*
  claims into candidates, so a domain appears there because the generator
  concluded the closure, and the certifier's census still has to re-prove it.
  Adding a census widens what may be offered; it does not add a claim.

### What this now does end to end

A `reads: []` the generator infers is republished as a **proposed** closure,
reaches the certifier, is decided by the implementation census on the form
machinery above, and — with no reviewed observation — is **withheld for want
of a recipe**. No unsound closure anywhere, and the withheld candidates are
the measurement the accuracy roadmap asks for.

### The gap this leaves, named

There is no `reads_walk.rs`. `creates_walk.rs` is 1336 lines and
`returns_walk.rs` is 109; the generator's `reads` candidates come from wherever
it already infers a complete-negative `reads`, not from a walk that
understands § 4.4's proxy property-access forms. That is sound — the census
refuses what the generator over-offers — but it is noisier than it needs to be,
and a dedicated walk is the way to make the generator's offer precise. It is
not on the critical path to a closed claim; a hand recipe is.

## 6. Withdrawn (2026-09-10): the census cannot decide `reads` at all

Asked to verify that the census refuses an owned read,
`fixtures/package-contracts/implementation-census-reads` was written with
`readsOwnProxy` — an export that reads a `Proxy` the module built — expected to
refuse. **It does not refuse. The census never sees it.**

### What the producer emits for `ownProxy.value`: nothing

Pinned by a new case in
`TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`:

~~~go
const ownProxy = new Proxy({ value: 1 }, { get: () => 1 });
export function ownProxyRead(): unknown { return ownProxy.value; }
// {"ownProxyRead", []*int{}}  — zero uncensused invoking forms
~~~

TypeScript types a `Proxy` as its **target**, so `ownProxy.value` resolves to a
data property, and `PropertyAccessUnknownAccessor` is recorded only for a
member the checker *cannot* resolve. `invocation.go`'s own header says why
that is right: a trap "is out of the producer's reach entirely", and "a
consumer whose claim requires that no trap ran must obtain that premise
elsewhere".

### Why § 5's reasoning was wrong

Gate 3 was admitted on the argument that `census_form_disposition` clears only
parameter-rooted and own-literal subjects and refuses the rest. That is true
**of forms that exist**. It assumed a form would exist for an owned proxy
access. None does, so there is nothing to refuse, and the census would have
decided `reads: []` for `readsOwnProxy` — a false closure, on exactly the shape
the domain is about.

### Withdrawn

`ClaimDomain::PROPOSABLE` is back to `[Creates, Returns]` and
`require_census_decides_closure` no longer admits `Reads`, with the reason
recorded at the match arm. The corpus snapshots regenerated back; only the new
fixture and four rows that were already dirty differ from `HEAD`.

**Kept**, because none of it depended on the census:

- the 19 `reads` negative rows (audited dialect facts, inert until a sound
  reads path exists);
- `semantic-model.md` § reads' two 2026-09-10 decisions;
- the fixture and its two recipes, which now document the limitation;
- the Go test, which is the durable artifact — it pins the zero-form fact so
  the next attempt cannot repeat this.

### What this corrects in the roadmap

`phase21/2026-09-03-implementation-census-plan.md` § 4.4 says the `reads`
census is "same machinery, plus the proxy property-access forms". **There are
no proxy property-access forms.** The "plus" is not an extension of the census
— it is a premise the producer states it cannot supply, and `Store<T>` being
`T` means no declared-signature premise can supply it either.

So `reads` closure needs one of:

1. **A producer fact that does not exist today** — some way to state that a
   receiver is not a proxy. That is a Type Facts design question, not a census
   one, and the header above suggests it is out of reach syntactically.
2. **A runtime premise**, i.e. the internals route lever F flags for
   `cleanups`, here over a minified reactive graph.
3. **Per-export hand recipes with no census at all** — the closure never
   asserted by an implementation census, only ever by a reviewed runtime
   observation for that one export.

Option 3 is the only one available now, and it means `reads` closes one
reviewed export at a time and never in bulk. The domain is further from
closure than the accuracy roadmap's lever F implies.

## 7. Correction (2026-09-10): §6 is too strong — the hole is one shape, not the domain

§6 concluded "the census cannot decide `reads` at all" from a single measured
shape. Measuring the rest of the family narrows that a great deal. Four more
cases were added to
`TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`:

| shape | forms | kind |
| --- | --- | --- |
| `new Proxy({value:1},{get}).value` | **0** | — |
| typed object + `Object.defineProperty(…{get})`, then `.value` | **0** | — |
| typed object literal `get value()`, then `.value` | 1 | `get-accessor` |
| class `get value()`, then `.value` | 1 | `get-accessor` |
| `any` receiver `.first`, literal carrying a getter | 1 | `property-access-unknown-accessor` |
| `any` receiver + `defineProperty`, then `.value` | 1 | `property-access-unknown-accessor` |

**The producer is not blind to accessor reads.** A *declared* accessor —
object literal or class, direct resolvable access — records a `get-accessor`
form, which §6 did not know existed. An unresolvable member records
`property-access-unknown-accessor`, as before.

**The blind spot is exactly one shape, and the Proxy is an instance of it:**
an accessor **installed at run time** on a receiver whose declared type still
says data property. `Object.defineProperty` on a typed object is the same hole
as a `Proxy` trap, for the same reason — nothing is declared, so there is
neither a form to refuse nor a body to walk. Type erasure is what hides it: the
*identical* `defineProperty` becomes visible the moment the receiver is `any`.

### A fourth route §6 did not list

§6 offered three ways to close `reads` — a new producer fact, a runtime
premise, or per-export hand recipes with no census. There is a fourth, and it
is much cheaper than all three:

> **Refuse `reads` for an artifact case whose closure installs an accessor at
> run time; decide it for every other case.**

That is a syntactic, conservative condition over bytes the census *already*
walks — no new producer fact, no runtime premise, no per-export review. The
refusal set is the `Proxy` constructor and the descriptor-installing family
(`Object.defineProperty`/`defineProperties`/`create`, `Reflect.defineProperty`,
`__defineGetter__`), plus any computed member access on those namespaces, since
`Object["define" + "Property"]` has to refuse too.

**This is a proposal, not a decision, and it is a soundness claim — the kind
this document has already been wrong about once.** What has to be established
before it is worth implementing:

1. That the refusal set is genuinely exhaustive over ways to install an
   accessor at run time. Each member needs its own zero-form case in the Go
   test first, exactly as the two above were measured.
2. What fraction of the 8950 corpus exports survive the refusal. A sound census
   that refuses nearly everything is not worth the machinery.
3. Whether an accessor installed in a *dependency* is reachable by the same
   scan, given that dependency composition already bounds the closure.

Until (1) and (2) are measured, the domain's status is unchanged: `reads` is
closed for 0 exports, and `PROPOSABLE` stays `[Creates, Returns]`.

## 8. Survival fraction, measured (2026-09-10)

§7 named three things to establish before the fourth route is worth building
and said (2) — what fraction of exports survive the refusal — is the decisive
one. Measured.

**Basis.** All 381 measured rows of
`rust/target/ecosystem-investigations/2026-09-09-machine-full.json`, every one
of the 8950 exports, zero rows missing. Package archives come from the offline
registry cache under `rust/target/registry-cache/v1`, so this needs no network
and reads the exact published bytes. Scan:
[`2026-09-10-reads-survival-scan.mjs`](2026-09-10-reads-survival-scan.mjs),
result: [`2026-09-10-reads-survival-measurement.json`](2026-09-10-reads-survival-measurement.json).

**The unit is the published archive, not the artifact-case closure**, because
the report carries no per-case closure file list. An archive is a superset of
every closure inside it, so "archive clean" soundly implies "every closure in
it is clean": the survival numbers below are a **lower bound**.

### The numbers

| | packages | exports |
| --- | --- | --- |
| clean — no runtime accessor installation anywhere | 273 / 381 | 2372 / 8950 |
| only the bundler's CJS-interop alias (`var __defProp = Object.defineProperty`, no residual occurrence) | 14 | 3419 |
| genuinely refused | 94 | 3159 |

Excluding the four Solid-core rows, whose contracts never supplement the
runtime model anyway (`external_packages()` filters them):

- **2372 of 7638 non-core exports survive — 31%.** Against 0 today.
- **5791 of 7638 — 76% — if the bundler alias is special-cased.**

### Three things the numbers say that the headline does not

**The refusal set's breadth is a non-issue.** `Object.create`, `Reflect.` and
computed `Object[…]` access add *exactly zero* packages: every archive carrying
one already carries `Proxy` or `defineProperty`. §7 worried these would be
over-broad. They are free.

**The refusal hits the giants, not the tail.** Median exports per row is **5**;
the dirty set is `solid-js` (636), `@solidjs/web` (483), `solid-recharts` (327),
`@solidjs/signals` (183), `@solidjs/start`, `@solidjs/router`. Package-level
survival is 75%, export-level 31% — the corpus is a long tail of small clean
packages plus a few large dirty ones. Which of those two numbers matters
depends on whether the goal is "most packages a project installs" or "most
exports in the corpus".

**The 76% figure leans on two rows.** 3324 of the 3419 alias-only exports are
`@kobalte/core@0.13.13` and `@kobalte/core@2.0.0-alpha.0`. The alias
special-case is worth 12 more packages but almost no additional exports. Do not
quote 76% as if it were broad.

### What this does not establish

- **Survival is a ceiling, not a yield.** An export that passes the refusal
  still needs a `reads` census to *decide* it, and none exists. This measures
  how much of the corpus such a census could legally look at, not how much it
  would close.
- **A regex over minified bytes is a measurement, not an implementation.** The
  real refusal has to be an AST condition over the closure the census walks.
  The numbers here would move in both directions under that: finer granularity
  (closure, not archive) recovers exports; a sound AST rule may refuse shapes
  this scan missed.
- **§7 prerequisite (1) is still open** — whether the refusal set is exhaustive
  over ways to install an accessor at run time. It is now worth doing: each
  member needs its own zero-form case in
  `TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`,
  measured the way §7's two were.

### Verdict

**The route is worth building.** 31% of consumer-relevant exports could have
`reads` decided under a refusal this crude, against zero under every route §6
listed, and the cheap sensitivity worries turned out to cost nothing. The
ordering stands: prerequisite (1) next, then the census admission with a
control written first.

## 9. Exhaustiveness of the refusal set, measured (2026-09-10)

§7's prerequisite (1): is the refusal set exhaustive over ways to install an
accessor at run time? Pinned by a new producer test,
`TestRuntimeInstalledAccessorReadsAreInvisibleToTheProducer`. Every shape is a
typed receiver whose declared type says data property, read directly.

| shape | forms | in the refusal set? |
| --- | --- | --- |
| `Object.defineProperty(o, k, {get})` | **0** | yes |
| `Object.defineProperties(o, {k: {get}})` | **0** | yes |
| `Object.create(null, {k: {get}})` | **0** | yes |
| `Reflect.defineProperty(o, k, {get})` | **0** | yes |
| `o.__defineGetter__(k, fn)` | **0** | yes |
| `Object.setPrototypeOf(o, accessorProto)` | **0** | yes |
| `o.__proto__ = accessorProto` | **0** | yes |
| `new Proxy(target, {get})` | **0** | yes |
| `Proxy.revocable(target, {get}).proxy` | **0** | yes |
| declared getter on an object literal | 1 | no — `get-accessor` |
| declared getter on a class | 1 | no — `get-accessor` |
| unresolvable member on an `any` receiver | 1 | no — `property-access-unknown-accessor` |

The last three are controls in the nonzero direction: a silently degraded
analysis cannot make the nine zeros pass while they hold. The harness also
fails loudly on an export name it cannot find, checked by temporarily adding
one — without that, nine rows asserting *zero* would pass vacuously.

### Three shapes §8's scan missed

Prerequisite (1) was worth doing before implementing, because the enumeration
found real gaps in the set §8 scanned:

- **`Proxy.revocable`.** §8's pattern was `/\bProxy\s*\(/`, which does not match
  `Proxy.revocable(…)`. A second spelling of the same hazard, silently exempt.
- **`Object.setPrototypeOf`** and **`__proto__` assignment** were not scanned at
  all. Both install an accessor-bearing prototype at run time.
- **`Object.create`** was scanned only as a *sensitivity* check, treated as
  possibly over-broad. It is mandatory.

### Re-measuring §8 with the corrected set costs nothing

The scan was rerun with all eight patterns mandatory
([the same script](2026-09-10-reads-survival-scan.mjs), pattern block updated):

| | before | after |
| --- | --- | --- |
| packages clean | 273 / 381 | **273 / 381** |
| exports clean | 2372 / 8950 | **2372 / 8950** |
| alias-only (bundler preamble) | 14 pkg / 3419 exports | **14 / 3419** |

**Identical.** Every package the new patterns catch was already caught by
`defineProperty` or `Proxy`; `setPrototypeOf`, `__defineGetter__` and computed
`Object[…]` hit zero packages, and `__proto__` hits 13 that were already dirty.
So §8's numbers survive the correction unchanged: **31% of non-core exports, or
76% with the bundler alias special-cased.**

That is the useful result. The refusal set can be as broad as the enumeration
demands without costing a single export, which means there is no tension
between soundness and yield here — only between soundness and *effort*.

### What is still open

- **The enumeration is mine, not a specification.** It covers what I could
  name. A shape absent from the table above is a hole in the refusal set, and
  the table is the place to add it — a pattern without a case is unpinned.
- **A regex is still not the implementation.** The real refusal is an AST
  condition over the closure the census walks; these patterns are the
  measurement that says it is worth writing.
- **Dependency reach (§7 prerequisite 3) is untouched.** Whether an accessor
  installed in a dependency is caught by the same scan depends on how
  dependency composition bounds the closure, and nothing here tests it.

Both of §7's blocking prerequisites are now measured. The next step is the
census admission itself, with a control written before the change — the
fixture `fixtures/package-contracts/implementation-census-reads` already
carries `readsOwnProxy`, which must refuse.

## 10. The admission, attempted (2026-09-10): what landed and the one seam that stopped it

§9 said the next step was the census admission with a control written first.
Attempted. Most of it works; one seam does not, and it is not the one this
document has been circling.

### What landed, and is green

- **`ModuleHazardKind::RuntimeAccessorInstallation`** in `solid-facts`, an
  **AST** detector — not the regex §8 measured with — over all nine shapes
  §9 enumerated: `Proxy` by identifier (so `Proxy.revocable` and
  `const P = Proxy` are caught), the accessor-installing member names by name
  rather than by receiver, `Object.create` at arity ≥ 2, a computed member on
  `Object`/`Reflect`, and a literal `__proto__:` key. Two tests pin it in both
  directions, and a positive planted in the negative list was confirmed to
  fail before the pair was trusted.
- **`census_reads_domain`** in `type_facts.rs`: the *form* half of the
  `creates` census with none of its callee walk, since a `reads` claim is
  about accesses in the export's own body. No deferral — with no call walk, a
  form it cannot decide here it cannot decide at all.
- **The admission** in `require_census_decides_closure`, and the dispatch arm
  that routes a `Reads` closure demand to that census.
- **The fixture split.** `implementation-census-reads` now has two
  entrypoints: `.` for the exports that must close and `./owned` for the
  proxy. This was forced by the design and is worth stating on its own — the
  refusal is a fact about a *closure*, so a single `new Proxy` anywhere in a
  file withdraws `reads` for every export in it. The fixture had encoded an
  export-granular expectation the mechanism cannot deliver; attributing the
  hazard per export would need dataflow from the installing expression to each
  read's receiver, which nothing in this pipeline has.

Both halves were also verified end to end while the emission was live: the
hazard reached the plan, `installs_runtime_accessor()` answered, the proposed
`reads` closure was withdrawn, and no gate was scheduled — against a control
package byte-identical but for the `Proxy`, which *did* schedule one.

### What stopped it: the hazard census exists twice

**`ClosureHazard` is computed independently on both sides of the boundary** —
in `contract_certification/module_closure.rs` over the oxc AST, and in
`packages/cli/scripts/artifact-resolution.mjs`'s `syntaxHazards` over the
TypeScript AST — and the two manifests must agree **byte for byte**, spans
included, because the hazard list feeds the closure digest.

Emitting a kind on one side only makes the generated document stop planning
against its own artifact. That is not a hypothetical: it broke
`the_generated_census_fixture_carries_every_creates_candidate_into_planning`
with `Contract(NoArtifactCase)`, because `implementation-census-creates`'
tracer carries `const protoTable = { __proto__: untypedRegistry, first: 1 }`
and the certifier suddenly saw a hazard the CLI did not.

So the emission is **withdrawn at the mapping**, with the reason at the match
arm. Everything else stays: the detector and its tests, the census, the
admission. All inert, in the same sense the 19 `reads` negative dialect rows
are inert — correct facts waiting on one missing piece.

`PROPOSABLE` stays `[Creates, Returns]`. It was widened during this pass and
the corpus regenerated to see the effect: `reads` closed for 88 artifact cases
and the emitted documents said `closed: ["reads", "creates"]`. With the
generator blind to the hazard, that included `readsOwnProxy` — a document
asserting a closure that is false. Reverted, and the corpus regenerated back;
the 27 fixture files that remain changed are the split plus files that were
already dirty.

### Correction, same day: § 10's "step 2 has no home" was wrong

The agreement check already exists and is good.
`module_closure::verify_snapshot_closure` **recomputes** the closure from the
authenticated snapshot and compares it to the supplied manifest, and
`closure_difference` reports the disagreement per field — `entries`,
`dependencies`, `hazards` — with counts and samples. The
`Contract(NoArtifactCase)` I hit is a *downstream* symptom, not the comparator.

So the mirror was developed red-green against it. See § 11.

### What the seam actually costs

The remaining work is **not** more census design. It is mirroring one detector
across two AST libraries with identical byte spans:

1. Implement the nine shapes in `syntaxHazards` over the TypeScript AST.
2. Prove the two agree — the natural control is a fixture whose closure digest
   is computed by both paths and compared, which is what `NoArtifactCase`
   already tests implicitly and badly.
3. Then re-emit, re-admit `PROPOSABLE`, and regenerate the corpus.

Step 2 is the interesting one and has no home today. Two independent syntax
censuses that must agree exactly is a seam worth questioning on its own terms:
every hazard kind ever added has had to be written twice, and nothing checks
the agreement except a downstream digest mismatch with an unhelpful message.

## 11. Landed (2026-09-10): `reads` is proposable, and the census decides it

The mirror is written and the domain is admitted. `ClaimDomain::PROPOSABLE` is
`[Creates, Returns, Reads]`.

### The mirror

`syntaxHazards` in `packages/cli/scripts/artifact-resolution.mjs` now emits
`runtime-accessor-installation` over the TypeScript AST for the same nine
shapes the oxc detector emits, with the same byte spans, `affectedExports: []`
and `affectedDomains: ["reads"]`. Two things had to line up beyond the spans:

- **`HAZARD_DEBUG`'s insertion order is load-bearing.** `HAZARD_ORDER` derives
  from it, and Rust sorts by `ClosureHazardKind`'s derived `Ord`, which is
  declaration order. The new kind is appended in both, in the same position.
- **The kind needs allowlisting.** Emitting one the CLI validator does not
  know fails the whole closure with `unknown closure hazard …`, which is what
  turned 5 corpus refusals into 10 on the first attempt.
- **The checker can be absent.** `parseModule` builds a `ts.Program` only for
  a module that needs symbol identity, so `syntaxHazards` can run with
  `checker === null` — and `isLocallyBoundIdentifier` dereferenced it
  unguarded. The existing detectors never hit it because their outer
  conditions did not fire on such a module; the `Proxy` arm does.
  `esm-barrel`'s `new Proxy(factory, {})` crashed the whole artifact case with
  `null is not an object (evaluating 'checker.getSymbolAtLocation')`, and the
  corpus recorded it as a refusal and **deleted the fixture's expected
  documents**. The helper now reads an absent checker as "not locally bound",
  which is the conservative answer for every caller: each negates it to decide
  whether a name is the global, so a shadowed name over-refuses.

  This one is worth remembering for its shape rather than its fix: a new
  detector that fires on more nodes than the old ones reaches states the old
  ones never did, and the corpus's failure mode for that is a *deletion*.

### The control

`implementation-census-reads`, split into two entrypoints, same package, same
kind of code:

| entrypoint | exports | emitted |
| --- | --- | --- |
| `.` | `plainArithmetic`, `readsOwnLiteral`, `readsCallerMember`, `readsCallerElement`, `invokesCallerAccessor` | `closed: ["reads", "creates"]` |
| `./owned` | `readsOwnProxy`, `readsOwnProxyElement`, `observedReads` | `closed: ["creates"]` |

`observedReads` touches no proxy and refuses anyway. That is the design
stated as an outcome: the premise is a fact about a **closure**, so one
`new Proxy` withdraws the domain for every export in the file.

Corroborated by a fixture that was not written for this: `esm-barrel` carries
`const proxiedFactory = new Proxy(factory, {})` and closes `reads` for none of
its summaries, while every other generating fixture in the corpus does.

### What the hazard costs, measured

**Nothing, outside the fixture that tests it.** A direct file-by-file
comparison of the generated corpus with both censuses on against both off:
**zero differing files.** The hazard changes the closure manifest, and the
closure manifest is not what the emitted document's `closureSha256` hashes.

Admitting the domain is what moves the corpus, and it moves it the intended
way: every export whose closure installs nothing now proposes `reads`, so the
returns fixture's gate count goes 13 → 22 (4 returns + 9 creates + 9 reads).

### Four assertions changed meaning, none was worked around

- Two tests used `reads` as their example of a domain with **no** closure
  proof mode. They now use `writes`, with the date and reason inline.
- `a_cleared_creates_walk_reaches_…` asserted `{Creates}` and now asserts
  `{Creates, Reads}`, pointing at the fixture pair that shows the withdrawal.
- `the_generated_returns_fixture_…` gained an explicit
  `candidates_for(Reads)` row rather than only absorbing the gate-count bump.

### Still open

- ~~`reads` closes at proposal time only.~~ **Closed by § 12.**
- **The refusal is closure-granular.** Per-export attribution needs dataflow
  from the installing expression to each read's receiver.
- **The set is my enumeration.** § 9's table is the contract; a shape missing
  from it is a hole in both detectors.
- **The dual census stands.** Two independent syntax censuses that must agree
  byte for byte, now mirrored a fourth time. `verify_snapshot_closure` catches
  a disagreement, but only for a package something actually certifies.

## 12. A `reads` closure reaches a receipt (2026-09-10)

§ 11 left the domain closing at *proposal* time only. It now closes at
certification: `a_reads_closure_reaches_a_receipt_through_its_mandatory_veto`
plans `implementation-census-reads`' `.` entrypoint with `plainArithmetic`'s
`reads` closed, schedules the one mandatory veto that closure demands, runs
`plain-arithmetic.mjs` against the real artifact through the pinned harness,
and asserts three things: nothing withheld, `reads` closed in the canonical
main the receipt binds, and a probe-gate root that is not the empty one.

Its pair, `an_accessor_installing_closure_never_plans_a_reads_candidate`,
plans `./owned`'s real bytes as a synthetic package's root — because
`plan_for_test_package_closing` resolves `.` and nothing else — and asserts no
gate is scheduled at all.

### One defect the admission had, and how it stayed hidden

`withheld_weakening` mapped `creates` and `returns` and refused everything
else by name. With `Reads` proposable, every recipe-gated plan carrying a
reads candidate failed `UnknownDomain { domain: "reads" }` — five existing
tests plus both new ones.

**`cargo test -p solid-facts-backend --lib` did not show it.** Those tests
return early when the binary was compiled without the certification pins, so
the runs that gated § 11 reported green while asserting nothing about the
gate. `make test-rust` carries `CERTIFICATION_ENV` and
`SOLID_CHECKER_EXPECT_PROBE_PINS=1`, which turns the early return into a
failure; under it the suite was red. AGENTS.md documents this trap and it
still cost a commit — the fix is folded into the admission commit rather than
appended, because a commit that fails the pinned suite is not green.

The tell was visible and I nearly missed it: the first run of the new test
passed **in 0.00s**.

### What the recipe stopped needing

`plain-arithmetic.mjs` used to import `observedReads` and compare the proxy
trap's counter before and after the sample, because that counter was its
author's only way to claim the sample had observed every reactive-shaped
source the module owns. After the split it cannot — `observedReads` lives in
`./owned` — and it no longer needs to: the closure states syntactically that
`./index.js` installs no accessor, so there is no trap to count. The hazard
does the work the counter used to, and the recipe is now what a veto should
be: sample the export, emit only on contradiction.

### Still open

- The refusal is closure-granular.
- The nine shapes are one enumeration, mirrored by hand across two ASTs.
- No *real* package has had a `reads` closure certified. seroval cannot: its
  development bundle carries `defineProperty` ×7, `__proto__` ×2,
  `__defineGetter__` ×1 and `Object.create` ×2, so it sits in the 69% § 8
  measured as refused. The next real target has to come from the 273 clean
  packages, and has to be one a consumer actually imports.

## 13. Can any real package produce a clean verdict now? (2026-09-10)

§ 12 left `reads` certifiable and one question outstanding: pick a package
from the 273 that install no accessor, certify it, and see whether a consumer
goes clean. Answered by cross-referencing the clean set against what the
corpus already proves, before spending a certification on it.

**No, and the blocker is `returns`, not `reads`.**

| | exports |
| --- | --- |
| in the clean set (deduplicated rows) | 1746 |
| of which `creates` closes | 50 |
| of which `returns` closes | **0** |

SC9005 needs `reads ∧ returns ∧ creates`. `reads` can now close for all 1746.
`creates` closes for 50, spread over eight packages — `@corvu/utils` and
`@corvu-next/utils` (20 each), `@kobalte/solidbase` (6), two
`@solid-primitives/analytics` exports, one in `@solid-primitives/input-mask`.
`returns` closes for none of them, so every one still reports.

`@solid-primitives/utils`, the obvious candidate at 99 clean exports and a
dependency of most of the family, is worse than that: all nine domains are
unknown for all 99, so the creates census refuses it outright.

### Why `returns` is zero, and the one door still open

The returns census decides two shapes: empty completion (ADR 0035) and a
whole-parameter identity. A utility that computes and returns a value is
neither. `@corvu/utils` has no void-returning export at all — the closest is
`afterPaint`, which returns a `number`.

So the only route to a clean verdict today is the *other* half of the returns
work: the demand-scoping slice raises the obligation only where a consumer can
read the result. An export whose result the consumer discards sheds `returns`,
and then `creates ∧ reads` closed is the whole predicate.

That narrows the search to **those 50 exports, called for effect**. Whether
any of them is naturally called that way in real code is the next question,
and it is a question about consumers rather than about packages.

### What this says about the ordering

Every measurement today has pointed at `reads` as the blocker, and it was —
until it closed. The next one is `returns`, and it is a *census* limitation
rather than a premise the producer cannot supply: the shapes it decides are
empty completion and parameter identity, and nothing else. Extending it is a
census question with no proxy-shaped hole underneath it, which makes it a
smaller problem than `reads` was.

## 14. A real package certifies and silently loses every closure (2026-09-10)

§ 13 said the clean set's blocker is `returns`. Testing that against a real
package found something else first, and it is a defect rather than a limit.

### What the generator proposes

`@corvu/utils@0.4.2` is in the accessor-free set. Generated against the exact
published archive from the registry cache, its `./dom` entrypoint proposes
closures for every export in both artifact cases:

~~~
case 0  ./dist/dom/index.js    contains  closed=["reads","creates"]  reads=["read-0"]
case 1  ./dist/dom/index.jsx   contains  closed=["reads","creates"]  reads=["read-0"]
~~~

Twelve exports across the package close both `creates` and `reads` — the first
time `reads` has closed for a real published package anywhere.

Note the shape: `contains` reads `.contains` off parameter 0, so its `reads`
is a **complete positive** (`Complete([read-0])`), not an absence proof. The
domain is closed over exactly one operation.

### What certification produces

Certified with the test-scoped issuer and an empty recipe corpus so ADR 0036
synthesis can run:

| | |
| --- | --- |
| status | `certified` |
| withheld closures | **0** |
| refusals | **0** |
| `domain-exhaustiveness` demands planned | **0** |
| `closed` arrays in the accepted document | **0** (the proposal had 5) |

The same run reusing the closure-carrying proposal via `--proposal`, bound to
certify's own certification importer, is identical: nothing withheld, nothing
planned, nothing closed.

**Control.** This afternoon's `seroval@1.5.6` accepted document, produced by
the same command, carries 3 `closed` arrays. So an accepted contract *can*
carry closure; corvu's were dropped.

### Why this matters more than the missing verdict

Every closure vanished between proposal and receipt with **no withheld record
and no refusal**. That is exactly the failure mode `WithheldClosure` exists to
prevent: a domain that cannot be proved is supposed to be named, not to
disappear. A consumer reading this catalog cannot tell the difference between
"the census refused this" and "nobody asked".

### The named suspect, not yet confirmed

The two documents differ in the *shape* of what they close. seroval's
surviving closures are `creates: []` — an absence — and `returns: ["return"]`
whose single operation the returns census decides by name. corvu's `reads` is
a complete positive over an arbitrary operation the census has no rule for.

`census_creates_domain` and `census_reads_domain` both refuse a candidate
whose claim enumerates any operation ("a … closure candidate must enumerate no
operation"). If a complete-positive claim is therefore never a *candidate*,
then nothing plans it, nothing withholds it, and the generator's `closed`
marker is dropped at normalization with nobody accountable for it.

That is a hypothesis with a clear test: plan a document whose only closure is
a complete positive and assert either a candidate or a withheld record exists.
It has not been run.

### What it blocks

The clean-verdict experiment. `contains` has no callback and SC9005's
predicate is exactly `reads ∧ (returns ∧ demanded) ∧ creates`, so a consumer
that discards its result should certify clean. It cannot be tried until a
certified document actually carries the closures its proposal offered.

## 15. Correcting § 14: the suspect is falsified and one of its runs was void

Two things in § 14 were wrong. The observable it reports is not.

### The suspect is dead

§ 14 guessed that a **complete positive** closure (`Complete([read-0])`, as
opposed to an absence) is never a candidate, so nothing plans it and the
marker is dropped at normalization. Tested directly, with the real document's
operation copied verbatim into a round-trip case in
`a_proposed_closure_labels_a_stated_closure_and_is_otherwise_refused`:

~~~
{"closed":["reads"],"reads":["read-0"],"proposedClosures":["reads"],"operations":[…]}
~~~

It survives decode, normalize **and** re-encode with the domain closed and
still labelled proposed. `KnowledgeSet::open_proposed_closure` returns true
for any `is_closed()` claim, positive or not, and
`derive_demand_graph` turns *every* closure candidate into a
`DomainExhaustiveness` demand with no filtering. So the document layer and the
candidate layer both handle this shape correctly. The guess was wrong.

### And one of § 14's two runs proved nothing

§ 14 says "the same run reusing the closure-carrying proposal via `--proposal`
… is identical". That run **silently regenerated**:
`certificationImporterPathFor` hashes the package root *and the catalog path*,
and the proposal had been generated for the first run's catalog while the
second wrote to a different one. Certify rejected the mismatch and made its
own, exactly as documented — I did not check.

Redone with the importer computed for the catalog actually being written, and
verified afterwards by comparing the binding to the path generated for:

| | |
| --- | --- |
| proposal `closed` arrays | 5 |
| `domain-exhaustiveness` demands planned | 0 |
| withheld closures | 0 |
| refusals | 0 |
| accepted document `closed` arrays | **0** |

### What is actually established

A proposal carrying five closures certifies into a document carrying none,
with nothing withheld and nothing refused. That is unchanged and it is the
thing that matters: a closure disappears without being named.

What is *not* established is where. Ruled out: the document round trip, the
candidate derivation from a closed claim, and demand-graph filtering. Still
open, and the next thing to test: whether the proposal certify plans from is
the one on disk at all. Certify regenerates in its own acquired workspace, and
if that regeneration produces a closure-free proposal where generation against
an extracted copy produces five, then the census is environment-dependent and
*that* is the defect rather than a dropped marker.

The way to settle it is to make certify emit the proposal it planned from.
Nothing does that today, which is why two rounds of this investigation have
been inference.

## 16. The instrument, and what it eliminated

`certification-audit.json` now records **`plannedProposal`**: the proposal
certification actually planned from, and which domains it states as closed,
per artifact case and export. It also records `reusedProposal` unconditionally
rather than only when true, because its absence used to be ambiguous between
"regenerated" and "this build does not say".

Diagnostic only — the audit is already `authoritative: false`. It exists
because § 14 and § 15 were both inference across this exact boundary, and both
were wrong.

### What it says about `@corvu/utils`

~~~
plannedProposal.closures        10
domain-exhaustiveness demands    0
withheldClosures                 0
accepted document closures       0
~~~

**The proposal certification planned from carries ten closures.** So § 15's
remaining hypothesis — that certify's regeneration in its own acquired
workspace produces a closure-free proposal where generation against an
extracted copy produces closures — is false. Both produce them.

### Three hypotheses now dead

| hypothesis | how it died |
| --- | --- |
| a complete-positive closure never survives normalization | round-trip case: it survives decode, normalize and re-encode (§ 15) |
| certify's regeneration is environment-dependent | `plannedProposal` shows ten closures in the planned document |
| a closure hazard opened every domain | `certificationInputs[].resolution.closure.hazards` is empty for this package |

The document also carries `proposedClosures` alongside `closed`, so it is
exactly the shape the round-trip case proved good.

### Where it now has to be

Between Rust reading that proposal and `inspect_candidates` producing closure
candidates. Everything on the CLI side is accounted for: the right document,
with the right markers, no hazards, reaching the planner.

Operation-derived demands *are* planned from the same document
(`operation-cardinality` 8, `operation-reachability` 8), so Rust is reading
its operations. Only the closures are missing.

The next instrument is the mirror of this one on the Rust side: record what
`inspect_candidates` returned for the selected candidate. **Not** another
round of reading the code and guessing — that is what produced the two
corrections above, and the rule this document keeps relearning is that a
boundary you cannot see through is a boundary to instrument, not to reason
across.

## 17. The Rust-side mirror, and the exact boundary

`solid-checker-rust` now emits `solid-checker:closure-candidates=` — one line
per certified artifact case, naming every closure candidate the planner
derived, before any gating. The CLI sums them across cases into the audit's
`closureCandidates`. Both certify paths emit it; the single-case one was
instrumented first and reported nothing for `@corvu/utils`, because a
two-case entrypoint takes the other.

Three boundaries, one run:

| | |
| --- | --- |
| proposal states closed (rows) | 10 |
| planner derived closure candidates | **18** (9 per case × 2) |
| withheld by gating | 0 |
| accepted document closures | **0** |

And the candidates are exactly the right ones:

~~~
afterPaint            Domain(Call(Creates)), Domain(Call(Reads))
callEventHandler      Domain(Call(Reads))
combineStyle          Domain(Call(Creates)), Domain(Call(Reads))
contains              Domain(Call(Creates)), Domain(Call(Reads))
sortByDocumentPosition Domain(Call(Creates)), Domain(Call(Reads))
~~~

### What this settles

Every earlier explanation is dead. The proposal carries the closures, the
planner derives call-domain candidates from them, gating withholds none, and
the receipt binds a document that closes nothing. The loss is **after
candidate derivation and outside the withholding mechanism**.

A correction to § 14 and § 16 while here: the audit's `demandPlans` come from
the CLI's *separate* `contract plan-demands` invocation, not from the certify
transaction. "Zero domain-exhaustiveness demands" was a fact about that other
call, and I read it as a fact about certification. The candidate count above
is the first number in this investigation that is actually about the run that
issued the receipt.

### The remaining question, stated as a question

`inspect_candidates` derives a candidate by calling `open_proposed_closure()`,
which *weakens* the claim — the candidate universe is built by opening the
closures it enumerates. Something must close them again for a proven
candidate, and seroval's document shows that something works there.

So: what re-closes a proven candidate, and why does it not fire here? That is
one more instrument — record the canonical main's closed set beside the
candidate set — and deliberately **not** another guess. Two guesses in this
document were wrong and a third would cost more than the measurement.

## 18. The full accounting, and a regression today's own change caused

A fourth marker, `solid-checker:certified-closures=`, reports what the
canonical main a receipt binds actually closes, via a new diagnostic
`document_closed_call_domains` in the backend lib. With the three before it
the accounting is complete: **offered → derived → withheld → bound.**

### The instrument is validated, on a package that keeps its closures

| | `seroval@1.5.6` | `@corvu/utils@0.4.2` |
| --- | --- | --- |
| proposal offered | 4 | 10 |
| planner derived | 6 | 18 |
| gating withheld | 3 | **0** |
| receipt binds | **3** | **0** |

seroval balances: 6 derived − 3 withheld = 3 bound. corvu does not: 18
derived, nothing withheld, nothing bound. Eighteen closures unaccounted for by
any mechanism.

That control matters more than the corvu number. A "0 bound" from an
uncalibrated instrument says nothing; a "0 bound" from one that reports 3 for
a package known to keep 3 is a measurement.

### And a regression this document's own change caused

The seroval control run first came back with **both** `returns` recipes
withheld as `no recipe in corpus`. Admitting `reads` to `PROPOSABLE` moved
every claim id in every emitted contract, and a `claimId` is a content digest
— so the recipe committed this afternoon addressed nothing, and this
afternoon's measured result (a consumer's SC9005 going from
`reactiveReads,returns` to `reactiveReads`) had silently reverted.

The corpus README names this as the corpus's weakness and says to re-read the
ids after any generator change. I made the generator change and did not.

Repaired by reading the live ids out of `withheldClosures` and repointing the
two development recipes. seroval now binds `createPlugin: creates,returns` and
`createReference: creates,returns` — better than this afternoon, because
`createPlugin`'s returns closes too.

**`seroval-create-plugin-identity-production.mjs` is still stale.** Its claim
belongs to the production condition and this run certified development; it
needs its own run to reprint. Left as is rather than guessed.

### What is now established about the defect

Offered, derived, withheld and bound are all measured, on two packages, with
one balancing and one not. The loss is after candidate derivation, outside
withholding, and before the canonical main — a span with exactly one thing in
it: whatever re-closes a proven candidate. seroval proves that step exists and
works.

## 19. Located: the case-set batch loses every closure

The instruments narrowed it to one span; a controlled pair of runs names it.

`@corvu/utils@0.4.2` `./dom`, identical but for the conditions, which decide
how many artifact cases are certified:

| conditions | cases | derived | withheld | bound | balances |
| --- | --- | --- | --- | --- | --- |
| `solid` | 1 | 9 | 6 | 3 | yes |
| `default` | 1 | 9 | 6 | 3 | yes |
| *(none)* | 2 | 18 | **0** | **0** | **no** |

Both single-case runs are correct and identical, so the difference is not the
artifact — `index.jsx` and `index.js` behave the same. It is the **count**.
`seroval@1.5.6`, the package that kept its closures throughout, certifies one
case.

**A multi-case entrypoint takes the case-set batch (ADR 0036, "the shared
batch is the fast path") and comes out with every closure gone and nothing
withheld.** The single-case path withholds what it cannot serve and binds what
it can.

This is the defect recorded in `docs/precision-backlog.md`, now with a
reproduction that is two commands apart.

### A second thing the single-case run shows

Its six withheld records are *every* `reads` candidate, all
`no recipe in corpus`, while all three bound closures are `creates`.
ADR 0036's synthesized veto keeps one reviewed observation per domain and the
table has `returns` and `creates` — `reviewed_observation("reads")` is still
`None`, which is what § 1 of this document opened with and § 2–§ 5 designed a
fix for that § 6 then declined to ship.

So the admission works and `reads` closures are real, but every one of them
needs a **hand recipe**: exactly the "one reviewed export at a time, never in
bulk" outcome § 6 predicted for option 3. The 31% survival measured in § 8 is
the ceiling for what a census may decide; the recipe requirement is a second,
tighter gate on top of it.

That reframes § 13's ordering again. The blocker for a clean verdict on a real
package is no longer `returns` alone — it is that `reads` cannot close in bulk
without a synthesized observation for the domain, which is the very thing this
document was written to design.

## 20. § 19 does not survive its control

§ 19 concluded the case-set batch loses closures because two cases lose them
and one does not. Written as a test —
`a_case_set_accounts_for_every_closure_candidate_it_was_given`, certifying a
synthetic two-case package and asserting `bound + withheld == candidates` —
**it passes**. The set certifies and the accounting balances.

So two cases alone do not cause the loss. The correlation across three
`@corvu/utils` runs was real and reproducible, and it is still not the
mechanism.

The test stays. It asserts the invariant that actually matters — every
candidate a certification is given is either bound in the receipt or named in
a withheld record — and nothing else in the suite asserts it. It will catch
this class of loss wherever it comes from, including the corvu case once
something reproduces it.

That is five corrections in this investigation. The pattern in all of them is
the same and it is worth naming: a difference that *correlates* across a
handful of runs is not a mechanism, and I have repeatedly written it up as
one. The instruments were the right response and they are what caught this;
the discipline still missing is writing the control **before** the conclusion,
not after.

## 21. Fixed: the fallback re-gated an already-weakened plan

The bisect that mattered was not case count. It was **`probes`**.

`certify_value_only_case_set` falls back to per-plan certification when a
candidate withheld for want of a recipe could be served by a synthesized
veto. That fallback was handed `gated.plan()` — the plan whose candidates the
gating weakening had already opened. `certify_value_only` re-gates what it is
given, found no candidate left, bound nothing, and had nothing to withhold
either. Every proposed closure vanished with no record.

The same mistake sat in the incomplete-gate fallback beside it. Both now take
the original plan.

Without a probe configuration the fallback is never reached — every candidate
is withheld for want of a recipe and the batch finalizes that honestly. That
is exactly why § 20's control passed while asserting nothing: it called the
batch with `None`. Supplying an empty corpus, as every real run does,
reproduced the loss in a unit test in seconds.

| | before | after |
| --- | --- | --- |
| synthetic two-case, probes supplied | 2 given, 0 bound, 0 withheld | 2 given, accounted |
| `@corvu/utils` `./dom`, two cases | 18 given, 0 bound, 0 withheld | 18 given, **12 withheld, 6 bound** |

Six bound is exactly twice the single-case result, which is the answer the
one-case runs said it should be all along.

### On how this was found

Five wrong conclusions preceded it, all the same shape — a correlation across
a few runs written up as a mechanism. What ended it was not a better guess.
It was the four audit markers, which turned "a closure went missing" into
"18 given, 0 accounted", and then a unit test that reproduced the same
signature in seconds instead of a two-minute certification.

The one discipline that would have saved most of it: the control before the
conclusion. § 20 already said so and § 20's own control was still vacuous
until its `probes` argument was checked.

## 22. Making the recipes mechanical, and the part that cannot be

§ 6 is why `reads` has no synthesized veto: an unenumerated read is a read of
a source the export **owns**, and a veto derived from the export's call
signature can only instrument values the *caller* supplied.
`reviewed_observation`
([synthesized_vetoes.rs](../../../rust/crates/solid-facts-backend/src/contract_certification/synthesized_vetoes.rs))
registers nothing for the domain and has no fallback arm, so every `reads`
candidate stays withheld until somebody writes a recipe by hand.

That leaves the domain gated on hand authorship. This section is about
removing everything from that job except the part § 6 proves is irreducible.

### 22.1 A recipe is three things, and two of them are transcription

| part | who can supply it |
| --- | --- |
| the **claim id** | mechanical — a content digest an author copies out of a run's material, and re-copies whenever the contract moves |
| the **manifest entry** | mechanical — `importKind`, `scenario`, `drain`, `expectedEvent`, and a `coverageLimitations` line the corpus test requires to be non-empty |
| the **observation** | not mechanical, and for `reads` provably not (§ 6) |

The second row hides the error this repository has no test for. A manifest's
`expectedEvent.marker` and the module's `harness.emit` have to name the same
string, and nothing checks that they do: a mismatch does not fail, it makes
the gate **unmatchable**, and an unmatchable gate is a clean non-observation —
the veto passes and the closure certifies. Emitting both from one place is
worth more than the typing it saves.

### 22.2 The trap that decides the design: a vacuous scaffold certifies

A recipe that runs to completion without emitting its marker is a
`CleanNonObservation`
([runtime_probes.rs:984](../../../rust/crates/solid-facts-backend/src/runtime_probes.rs)):
the mandatory veto **passes** and the closure certifies on the implementation
census alone. That is correct for a finished recipe — `plain-arithmetic.mjs`
is exactly this, and it is how a true `reads: []` closes. It is catastrophic
for a generated one. A scaffold that merely called the export would convert
"nobody has written the observation yet" into "nothing contradicted the
closure", silently, across every candidate a run emits at once.

So the generated module **throws** until an author deletes its `UNFINISHED`
guard. A throw makes the gate incomplete, and an incomplete gate withholds the
candidate (`WITHHELD_CLOSURE_VETO_INCOMPLETE_PREFIX`) exactly as having no
recipe at all does, with the throw's own text carried into the withheld
record. The failure mode of an unfinished scaffold is the state it was
generated from, never a weaker one.

`contract_certification::tests::a_recipe_that_throws_withholds_its_candidate_rather_than_certifying_it`
pins that against the real harness rather than against a reading of the code,
and it has a control: its sibling
`a_reads_closure_reaches_a_receipt_through_its_mandatory_veto` runs the same
plan with a finished recipe and closes the domain. Same fixture, same claim,
same gate — the recipe body is the only difference.

### 22.3 What landed

`scripts/probe-recipe-scaffold.mjs`. It reads a certification plan or audit —
both carry the same `withheldClosures` array — and for every candidate
withheld as `no recipe in corpus` emits a module and its manifest entry,
addressed by the claim id verbatim.

~~~sh
bun scripts/probe-recipe-scaffold.mjs \
  --plan /tmp/run/certification-plan-0.json \
  --corpus scripts/ecosystem-benchmark/probe-recipes \
  --domain reads --specifier seroval
~~~

Four properties, each with a test in `scripts/probe-recipe-scaffold.test.mjs`:

- **It never overwrites.** A module already on disk, or a claim the manifest
  already addresses, is skipped. The only recipe worth having is one somebody
  finished, so the tool cannot destroy one.
- **It serves only the recipe gap.** `census refused: …` has no proposed
  closure to veto and `veto did not complete: …` already has a recipe whose
  author a scaffold must not overwrite; neither is touched.
- **It has no fallback domain.** `DOMAIN_SCAFFOLD` covers `reads`, `returns`
  and `creates` and refuses anything else, mirroring `reviewed_observation`
  for the same reason: a domain that inherited a neighbour's marker would be
  gated by a veto watching for a contradiction nobody defined, and would pass.
- **A scaffold cannot reach a commit.** `ecosystem-probe-recipes.test.mjs`
  fails on a checked-in module carrying the guard. A scaffold is a safe
  working state and a pointless shipped one — every run would launch a worker
  per gate to arrive back at the withholding it started from.

The `reads` scaffold's header carries the three obligations § 6 says the
author owns: enumerate every reactive-shaped source the *closure* owns,
make each observable, and assert your own observation fired so a package edit
that removes the source fails the recipe instead of passing the gate.

### 22.4 Found while doing it

`fixtures/package-contracts/implementation-census-reads/probe-recipes/recipes.json`
carried `"policy": 2` where `WireRecipeCorpus` requires the policy object, so
`RecipeCorpus::load` would have refused it outright. It had never been read —
the fixture's tracer tests assemble their own corpus from the modules and the
live claim ids — and nothing checked it. Fixed, and
`ecosystem-probe-recipes.test.mjs` now checks the envelope of every
checked-in fixture manifest, which is where the class was invisible.

## 23. Using the scaffold on seroval: there is no `reads` candidate, and there should not be

- **Asked:** run `probe-recipe-scaffold.mjs` on `seroval@1.5.6`'s `reads`
  candidate — the one domain
  [the returns recipe](2026-09-10-real-package-returns-recipe.md) § 5 left as
  "the whole remaining gap" on `createReference`.
- **Answer:** there is no such candidate. The scaffold emits nothing, which
  is correct, and the reason is not a missing recipe.

### 23.1 What the tool says

~~~
$ bun scripts/probe-recipe-scaffold.mjs --audit …/certification-audit.json \
    --corpus … --domain reads --specifier seroval
no candidate in certification-audit.json is withheld for want of a recipe
  reads: no candidate at all in this material. …
~~~

The same audit under `--domain creates` reports the census refusal on
`resolvePlugins`, and under `--domain returns` emits the scaffold for the
claim the hand recipe later filled. So all three of the domain's states are
distinguishable from the same material, which is what the run was for.

### 23.2 The retained certification agrees

The catalog at `/private/tmp/solid-checker-rc7-reproduction/seroval-consumer-certification`
holds the real contract document its receipt binds. Five summaries:

| summary | `closed` | states `reads` |
| --- | --- | --- |
| `…f8be3897` (`createReference`) | `["creates", "returns"]` | no |
| `…d8e909ab`, `…f22a4831` | `["creates"]` | no |
| `…5ba387bc`, `…ee9d83c3` | — | no |

`reads` is not closed, not open-with-items, not withheld. It is absent: no
candidate was ever planned for it.

### 23.3 Why: six hazard sites, and they are not all false positives

`runtime-accessor-installation` is a fact about a **file**, so one site
withdraws `reads` from every export in it. Running the four rules of
`syntaxHazards` over the certified artifact case
(`dist/esm/development/index.mjs`, 4209 lines) finds six:

| site | what it installs |
| --- | --- |
| ×4 `Object.defineProperty(globalThis‖window‖self‖global, REFERENCES_KEY, {value: …})` | a **data** property on the global, one per environment branch |
| `Object.defineProperty(object, key, {value, configurable, enumerable})` in `assignStringProperty` | a **data** property |
| `Object.defineProperties(result, Object.getOwnPropertyDescriptors(fields))` in `deserializeDictionary` | **whatever the deserialized input describes** |

Five of the six are provably data properties, and a read through a data
property runs no code — so on those the hazard is an over-approximation of
the member-name rule. The sixth is not. `deserializeDictionary` copies own
property descriptors off an object the deserializer built from its input, and
a descriptor can carry a getter. seroval is a deserializer; installing
arbitrary descriptors is the job.

Across the whole package (both formats, 26 sites) the CJS cases add the
sharper case. esbuild's CJS prelude is

~~~js
var __defProp = Object.defineProperty;
… (e, r) => { for (var t in r) __defProp(e, t, { get: r[t], enumerable: true }); }
~~~

— a real accessor installation reached through an **alias**, which no rule
keyed on `Object.defineProperty(` as a call would see. Only the bare-member
rule catches it, at the point the intrinsic is named. That is the rule
earning its conservatism.

### 23.4 What this settles

**Narrowing the hazard to spare literal `{value: …}` descriptors would not
unblock seroval.** It clears 20 of the package's 26 sites and leaves the four
`defineProperties` and the two aliased references — so the domain stays
withdrawn, correctly. A refinement that *did* unblock it would need dataflow
from the installing expression to each read's receiver, which § 10 already
recorded this pipeline does not have.

So `reads` on seroval is not a recipe gap and not a tooling gap. It is the
right answer for a package that installs the property descriptors its input
describes.

**Where a first real `reads` recipe could exist.** Scanning the retained
`node_modules` for hazard sites, four of ten packages are clean —
`@solid-primitives/memo`, `@solid-primitives/trigger`,
`@solid-primitives/utils` and `csstype` — against `solid-js` (144 sites, 140
provable accessors), `@solidjs/web` (162), `@solidjs/signals` (76),
`seroval` (26), `typescript` (17) and `seroval-plugins` (2). The population
where `reads` can close is small unbundled packages, not bundled ones; that
is a claim about ten packages in one tree, and the ecosystem number is not
measured.

### 23.5 One caveat the run demonstrated

The retained audit predates the `reads` admission, which moved every claim
digest in this package. The scaffold copied its `returns` claim id verbatim —
`…9e82b17c` — while the checked-in hand recipe now addresses `…f1b996ab`. The
tool is exactly as fresh as the material it is given, and a scaffold generated
from a stale plan addresses nothing. Regenerate the plan first.

## 24. `@solid-primitives/memo`: a third reason, and it is upstream of everything

§ 23 named the four hazard-clean packages in the retained tree as where a
first real `reads` recipe could exist. Tried the smallest of them.

- **Asked:** run the scaffold on `@solid-primitives/memo@2.0.0-next.2`.
- **Answer:** again no candidate — and for a reason that is neither seroval's
  nor a recipe's. The generator resolves nothing about this package at all.

### 24.1 It certifies, and the contract says nothing

First end-to-end certification of the package (published archive acquired at
its lockfile integrity, test-scoped local issuer, entrypoint `.`):

~~~
status                certified
refusals              0
withheldClosures      0
closureCandidates     0   (1 artifact case)
certifiedClosures     0
~~~

A clean run by every summary field. The contract it published:

~~~json
"summaries": { "summary-ee9d83c3…": { "call": {}, "shape": "callable" } }
~~~

**One summary, empty `call`, shared by all seven exports.** Nothing closed,
nothing open-with-items, no domain stated. The generator's own plan sidecar
says why in one number: **70 unresolved claims** — seven exports × ten
domains — and zero closure candidates.

That is worth stating plainly because the audit does not: a certification can
be `certified` with no refusal and no withholding and still carry a contract
that describes nothing. "Certified" is a statement about the *proof of what
the document says*, not about the document saying anything.

### 24.2 The cause is three lines in the inputs sidecar

~~~json
{ "kind": "unaccepted-external-dependency", "source": "./dist/index.js:solid-js" }
{ "kind": "unaccepted-external-dependency", "source": "./dist/index.js:@solidjs/web" }
{ "kind": "unaccepted-external-dependency", "source": "./dist/index.js:@solid-primitives/utils" }
~~~

Every export of this package runs through `createSignal`, `createMemo`,
`getOwner`, `onCleanup`, `runWithOwner` or `isServer`. With no accepted
contract for the packages those come from, the generator correctly declines
to describe any domain of any export. `dependencies: []` in the plan.

**And the chain does not terminate for `reads`.** The deepest dependency is
`solid-js@2.0.0-rc.0`, which § 23's scan found carries **144
accessor-installation sites, 140 of them provably accessors** — a reactive
runtime really does install accessors. Its `reads` is correctly withdrawn,
so nothing composed above it can close `reads` through it. Certifying
`@solid-primitives/utils` and `@solidjs/web` through the dependency-graph
lane could unblock memo's *other* domains; it cannot unblock this one.

### 24.3 Three reasons, none of them a recipe

Across the three packages tried, "there is no `reads` candidate" has three
distinct causes, and they sit at three different stages:

| package | stage lost | cause |
| --- | --- | --- |
| `@solid-primitives/memo` | **proposal** | every domain unresolved; three unaccepted external dependencies |
| `seroval` | **planning** | the closure hazard withdraws `reads`; the deserializer installs the descriptors its input describes (§ 23) |
| `implementation-census-reads` (fixture) | — | proposes, censuses, gates, closes |

So the honest position: **no real package in the retained tree can produce a
`reads` recipe gap today.** The scaffold is not the bottleneck and neither is
recipe authorship. The bottleneck for a leaf package is its dependency
closure, and for a bundled one the accessor hazard — and for the reactive
runtime at the bottom of every chain, the hazard is *right*.

### 24.4 What the tool learned from it

Two silences made this take a certification run and four artifacts to answer,
and one of them is now closed.

`probe-recipe-scaffold.mjs` takes `--proposal-plan <file>.proposal.json` and
distinguishes the stages, because "no candidate" meant two different things
that need different work:

~~~
  reads: no candidate at all in this material. …
    The generator proposed no reads closure: it is unresolved for 7
    export(s) (createLatest, createLatestMany, createLazyMemo, …).
    No recipe applies before the generator can decide the domain; read the
    .refusals.json sidecar's declinedClosures and certificationInputs for why.
~~~

The other silence is not closed and is recorded here as a gap. **The
generator emits decline records for `creates` only.** memo's refusals carry
eleven `declinedClosures`, every one of them `creates`, with a `kind`
(`unresolved-callee`, `dialect-silent`, `refusing-callee-fixpoint`), a
location and a spelling. For the other nine domains — `reads` among them —
an undecided claim is recorded as a bare `unresolvedClaims` entry with **no
reason at all**. ADR 0008 built the decline records for `creates` and nothing
extended them. Until they are extended, "why did `reads` not propose here"
cannot be answered from the material; it has to be re-derived by reading the
package.

## 25. Extending the decline records past `creates`

§ 24 left one silence open: the generator explained why it declined a
`creates` closure and said nothing at all about the other eight domains. An
undecided `reads` claim reached the material as a bare `unresolvedClaims`
entry — export, domain, claim id, and no reason — so "why did `reads` not
propose here" could only be answered by reading the package. It cost a
certification run and four artifacts to answer for one package in § 24.

### 25.1 The two reasons are different facts

`DeclinedClosureRecord` already carried `domain` explicitly, "so a second
domain does not have to change the record's shape". The shape was ready; the
*reason* type was not, because it was `CreatesDecline` — a call site inside
the export that the `creates` walk would not propose across.

A `reads` decline is usually not that. It is a **closure hazard**:
`bind_exports` opens every domain the closure's hazards name
([artifact_resolution.rs](../../../rust/crates/solid-facts-backend/src/artifact_resolution.rs),
`open_domains`), before any walk is consulted. Nothing about the export is
unknown — the closure is.

So `decline` became `ClosureDecline`, with two variants that reach the same
ten-column channel and are told apart by the `kind` column:

| variant | fact |
| --- | --- |
| `Call(CreatesDecline)` | a call site inside the export the walk refused across — this build's ignorance of one callee |
| `Hazard { kind, source }` | a hazard that opened the domain for every export it names — a fact about the artifact closure |

No wire change: a hazard fills `kind` with the hazard's own kebab-case name
and `location` with its `source`, and leaves `package`, `callee`,
`declaration`, `shape` and `spelling` empty, which the parser already
preserves rather than guesses at.

### 25.2 Proposable domains only, and that is not a shortcut

`hazard_declines` records a hazard against `creates`, `returns` and `reads`
and no other domain. A hazard opening `writes` explains nothing: `writes`
has no census, so it would be open whatever the closure looked like, and
naming the hazard as its blocker would state a cause that is not one. The
three proposable domains are exactly the ones where a closure *could* have
been proposed, so they are the only ones where "why was it not" has an
answer.

One record per (export, domain, hazard). A hazard whose `affected_exports`
is empty is a fact about every export of the case, so three unaccepted
dependencies across seven exports is sixty-three rows for three facts. That
is the cost of each row standing alone, which is what a reader filtering for
one export needs.

### 25.3 What it says now

`@solid-primitives/memo`, the § 24 case, regenerated:

| domain | kind | rows |
| --- | --- | --- |
| `creates` | `dialect-silent`, `refusing-callee-fixpoint`, `unresolved-callee` | 11 |
| `creates`, `returns`, `reads` | `unaccepted-external-dependency` | 21 each |

~~~json
{ "export": "createReducer", "domain": "reads",
  "kind": "unaccepted-external-dependency",
  "location": "./dist/index.js:solid-js" }
~~~

The question § 24 could not answer from the material is now one row.

And `implementation-census-reads`, the fixture the whole domain design turns
on, records its own hazard for the first time: `./owned`'s three exports
each decline `reads` on `runtime-accessor-installation` at
`./owned.js:1119-1124` — the `new Proxy` site — while the `.` entrypoint's
five exports record nothing, because nothing blocks them.

`seroval@1.5.6` closes the loop back to § 23. Its `createReference` now
declines `reads` on six `runtime-accessor-installation` sites per artifact
case:

~~~
runtime-accessor-installation  ./dist/esm/development/index.mjs:7505-7526
runtime-accessor-installation  ./dist/esm/development/index.mjs:7702-7723
runtime-accessor-installation  ./dist/esm/development/index.mjs:7893-7914
runtime-accessor-installation  ./dist/esm/development/index.mjs:8084-8105
runtime-accessor-installation  ./dist/esm/development/index.mjs:47141-47162
runtime-accessor-installation  ./dist/esm/development/index.mjs:51013-51036
~~~

Those are exactly the six § 23 found by running the hazard rules over the
bundle by hand — the four environment branches writing `{value: …}` to the
global, `assignStringProperty`, and `deserializeDictionary`. The
investigation § 23 recorded is now a field in the generator's own output.

### 25.4 Corpus effect

Declined closure proposals across the 97 generator fixtures go from **43 to
821**, and every added row is a fact that was previously invisible:

| domain | kind | rows |
| --- | --- | --- |
| `reads` | `unaccepted-external-dependency` | 212 |
| `reads` | `runtime-accessor-installation` | 127 |
| `reads` | `nonliteral-dynamic-loading` | 5 |
| `creates` | `unaccepted-external-dependency` | 212 |
| `creates` | walk kinds (unchanged) | 43 |
| `creates` | `nonliteral-dynamic-loading` | 5 |
| `returns` | `unaccepted-external-dependency` | 212 |
| `returns` | `nonliteral-dynamic-loading` | 5 |

Thirty `expected-refusals.json` snapshots move and **nothing else does** — no
contract document, no proposal plan, no closure candidate. The channel is
measurement, exactly as ADR 0008 built it: a `runtime-accessor-installation`
record is the closure's syntax, not a claim that any read occurs.

### 25.5 One duplicate removed on the way

The domain wire names existed twice — `call_claim_domain_name` in the
certifier and, implicitly, the `"creates"` literal in the generator's record.
A third copy was about to be written. `ClaimDomain::wire_name` is now the one
mapping and the certifier delegates to it, because a name that drifts between
producer and consumer is the dual-census failure in miniature (§ 10).

## 26. The dependency-graph lane on `@solid-primitives/memo`: a silent no-op

§ 24 said certifying memo's three dependencies "could unblock memo's *other*
domains" through the graph lane. Tried it.

`--dependency-graph-lane` **changed nothing**. Both runs publish the same
contract document (`e2976f1d…`), the same zero closure candidates, the same
zero withheld closures; the audits differ only in a scratch temp path, and
`graphPreparation` carries no `partialProposalFrontier`, so the lane never
engaged at all.

### 26.1 Why: the lane keys on a refusal memo does not produce

`preparedGraphForPartialProposal` gates on
`partialProposalHasDependencyFrontier(audit.refusals)`, which needs a
**non-empty** `refusals` array carrying an exact `dependency-composition`
class. memo's `refusals` is `[]`. Its artifact case generates perfectly well
— it just generates an empty contract.

The same underlying fact has two representations, and the lane sees one:

| representation | when | lane fires |
| --- | --- | --- |
| artifact-case refusal, class `dependency-composition` | the case **cannot generate** | yes |
| `UnacceptedExternalDependency` closure hazard | the case generates, every domain open | **no** |

memo is the second, and that is plausibly the more common shape for a small
well-formed package: nothing about its bytes is malformed, it simply calls
into packages this build has no contract for. The flag was accepted and
ignored without a word, which is the part worth fixing whatever is decided
about the routing.

### 26.2 And extending the trigger would not unblock `reads`

Worth settling before anyone builds it. Two of memo's three dependencies —
`solid-js` and `@solidjs/web` — are **primitive-defining packages** under the
v2 dialect (`solid_2.rs`'s `primitive_defining_packages`). Generating either
selects `GenerationScope::DialectDefiningPackage`, whose
`publishes_bootstrapped_reactive_domains()` is false: callbacks, reads,
creates and cleanups are withheld wholesale (ADR 0017, ADR 0005). So even a
fully certified `solid-js` would state nothing about `reads`, and memo's
`reads` could not compose through it.

The graph lane is therefore the wrong instrument here twice over: it does not
fire, and it would not help if it did.

### 26.3 The tension the new records make visible

One export, `createPureReaction`, now carries both channels for the same
import:

~~~
dialect-silent                  solid-js  runWithOwner    …/dist/index.js:1433:1544
dialect-silent                  solid-js  createReaction  …/dist/index.js:1459:1543
unaccepted-external-dependency                            ./dist/index.js:solid-js
~~~

The walk asked the *dialect* about `solid-js`'s primitives and named the two
it had nothing to say about; the closure declares the whole package opaque
and opens every domain regardless. ADR 0008's Pinned section already records
this interaction — "opens every domain of that artifact case whatever the
walk found" — so it is expected rather than newly broken. What is new is
that it is now countable per export and per domain, which is what would let
somebody decide whether a core-runtime import should be an opaque frontier
at all.

**Not decided here.** Whether `record_opaque_frontier` should exempt a
`core_runtime_contract_reference` specifier is a question about where the
trust boundary sits, not a cleanup: the dialect's primitive recognition and
a contract's authority over an archive's bytes are different questions, and
they could legitimately both hold.

## 27. Exempting the built-in runtime from the opaque frontier

§ 26 left this as a trust-boundary question rather than a cleanup. Decided:
**a core-runtime specifier no longer records an opaque frontier.**

### 27.1 What was wrong with the frontier

`record_external` recorded an `UnacceptedExternalDependency` hazard for any
external specifier with no supplied dependency edge, and that hazard opens
**every domain of every export** of the case. `solid-js`, `@solidjs/signals`
and `@solidjs/web` have no package contract *by design*:
`core_runtime_contract_reference` withholds one, and generating one selects
`GenerationScope::DialectDefiningPackage`, which withholds the reactive
domains wholesale. So the frontier was a demand that can never be met, levied
on every package that imports Solid at all — which is every Solid package.

### 27.2 The objection, and why the exemption is still admissible

`primitive_defining_package`'s own doc says its basis "may only ever
*withhold* a claim (open a domain), never establish one — see ADR 0005",
because it compares a name with no version and no integrity behind it. The
exemption is a name-only predicate that **stops** withholding, which is the
direction that doc forbids.

It is admissible here for a different reason, and the difference is exact:
**clearing the frontier establishes no claim.** Every claim still comes from
the generator's derivation over *this* package's bytes, and every proposed
closure is re-proved by the certifier's implementation census, which walks
the archive's own implementation and refuses any form it cannot census. What
the name gates is whether a blanket withdrawal applies, not whether anything
is true. A proposal is the generator's inference (ADR 0008); a wrong one is
withheld, not certified.

**The limit, stated rather than papered over.** The dialect is the authority
for these packages' *primitives*, and it answers domain questions about them
— that is why `creates` still declines on `dialect-silent` where the dialect
has nothing to say. It is **not** an authority for arbitrary *values* these
packages export. `@solid-primitives/utils`' `createHydratableSignal` reads
`sharedConfig.hydrating`, a property access on an imported object whose bytes
are outside the closure; the generator now proposes `reads: []` for it,
where before the frontier said "unknown". That proposal rests on the census
refusing an unknown accessor at certification, not on the generator knowing
anything. It is the class of case to watch.

**Verified in § 36.** Both guards that class depends on now have named,
falsifiable tests, and the generator's `reads: []` proposal was reproduced
directly rather than assumed.

### 27.3 Measured

Corpus, 97 generator fixtures:

| | before | after |
| --- | --- | --- |
| declined closure proposals | 821 | **221** |
| proof candidates | 940 | **1342** |
| local open claims | 5454 | **5048** |

Nineteen contracts move, and they move by *closing* domains the frontier had
blanket-opened. `creates-decline-records` is the clearest read: its
`dialectSilent` export now publishes `closed: ["reads", "returns"]` while
`creates` stays open, because the `creates` walk still declines on the
dialect's silence about `createEffect`. The guard that has an opinion keeps
it; the blanket one is gone.

`torture-getter-exports` is the other control worth naming: its `getterResult`
now closes `creates`, and its `reads` stays withdrawn — the file installs an
accessor with `Object.defineProperty`, so the
`runtime-accessor-installation` hazard still fires. Removing one hazard did
not remove the other.

On real packages, `@solid-primitives/utils@7.0.0-next.4` — whose only import
is `solid-js` — goes from **0 closure candidates to 118, of which 45 are
`reads`.** That is the first time a real published package has produced a
`reads` closure candidate at all. `@solid-primitives/memo` is unchanged: two
of its three frontiers clear, and `@solid-primitives/utils` is the third, so
its contract stays empty until that dependency is certified and supplied as
an accepted edge.

### 27.4 The dual census, again

The closure is computed twice, and the two must agree byte for byte or a
supplied closure never matches the recomputed one. Rust derives the list from
`Dialect::primitive_defining_packages`; the TypeScript census hard-codes it,
because nothing checked in carries the list for it to read — the dialect
manifests' `contracts[]` is the *bundled contract* list, which is broader
(`@solid-primitives/scheduled` is in it, and is not core runtime; that
mistake was made and caught while writing the test).

So the agreement is asserted from the side that owns the list:
`module_closure::tests::the_core_runtime_package_list_agrees_with_the_typescript_census`
reads the constant out of the mirror and compares it. Confirmed falsifiable —
deleting one entry from the mirror fails it with the intended message.

## 28. Certifying `@solid-primitives/utils`: it refuses, and that unmasks a gap in § 27

§ 27 ended with the chain to try: certify `utils`, supply it to `memo` as an
accepted edge, and `memo`'s last frontier clears. **The chain stops at the
first step.**

### 28.1 The refusal

~~~
witness-acquisition refused for demand sha256:c15404fe…:
  Type Facts certification failed during live export-value verification:
  demand sha256:c15404fe… is locally open: recursive-value-shape
  (artifact-case:1bea9ecd…:createHydratableSignal):
  operation value root shape has no verifiable premise: the demand asserts
  no callability and the producer's root observation is open
~~~

`--recover-entrypoints` does not help: it isolates refused *artifact cases*,
and here the single case is fine — one export's demand refuses the whole
transaction.

It is `createHydratableSignal` again, the export § 27.2 named as the class to
watch, though not for the reason predicted. The refusal is a **returns**
value-shape demand, not a reads one: the contract declares an operation whose
returned root shape the producer could not observe. The checker is being
honest — it says exactly what it cannot prove, and it refuses rather than
certifying.

`memo` is therefore unchanged: its hazard records fall from 21 per domain to
**7** (seven exports × the one remaining specifier, `@solid-primitives/utils`)
and its closure candidates are still **0**.

### 28.2 The regression class § 27 shipped unmeasured

Before the exemption, `utils` **certified** — vacuously, with a contract
stating nothing about any export. After it, `utils` **refuses**.

That is the correct direction on the precision contract: a receipt binding a
document that says nothing is worth nothing, and a named refusal is worth
something. But it is a behaviour change of a kind § 27 did not measure, and
the honest statement is:

- `make verify` is green and covers every gate it runs, but it deliberately
  excludes the ecosystem benchmark.
- `benchmarks/ecosystem/report.json` holds **381 rows with contract content**,
  and essentially all of them import `solid-js`. Every one now proposes
  domains the frontier used to blanket-open, so the report is stale and would
  move substantially.
- How many rows go from *certifies vacuously* to *refuses* is **not
  measured**. `utils` is one data point and it went the wrong way.

The refusal it unmasks looks pre-existing rather than new: a generator that
declares a value root shape the producer's observation leaves open is a
producer/generator mismatch that the frontier was hiding for every
Solid-importing package. Unmasking it is progress; paying for it across the
ecosystem in one step was not measured before landing.

### 28.3 What would settle it

`make ecosystem-benchmark` with `--attempt-certification` (release build,
roughly twenty minutes) against the pre- and post-exemption binaries, counting
rows by outcome. That is the measurement § 27 owed and did not take. Until
it is taken, `cf905a58` should be read as *sound but unmeasured at ecosystem
scale* — the direction is right, the magnitude is unknown.

## 29. Measured: the exemption fixes thirteen rows and regresses none

§ 28 said `cf905a58` was "sound but unmeasured at ecosystem scale" and named
`@solid-primitives/utils` as one data point that "went the wrong way". The
measurement is now taken, and **§ 28's worry was wrong in the aggregate**.

`make ecosystem-regression` — the purpose-built gate for exactly this
question, which fails on any row the pin certified that this commit does not:

~~~
418 probes, 349 complete contracts, 32 partial          (531 s)
certificationRegressionCount   0
certificationFixCount         13
regressionCount                0
~~~

Thirteen rows **gained** certification, twelve of them `refused → certified`:

| row | was |
| --- | --- |
| `solid-js@1.9.14` (solid1) | refused |
| `@solidjs/web@2.0.0-rc.3` (solid2) | refused |
| `@solidjs/element@2.0.0-rc.3` (solid2) | refused |
| `@tanstack/solid-db`, `solid-form`, `solid-hotkeys`, `solid-store` | refused |
| `corvu@0.7.2`, `@corvu/popover`, `@corvu-next/popover` | refused |
| `solid-devtools@0.34.5` | refused |
| `@solid-primitives/visibility-observer@2.0.1` | refused |
| `@kobalte/solidbase@0.6.13` | not attempted |

Certification totals: 399 attempted, 381 verified, 324 complete, 57 partial,
654 certified entrypoints.

### 29.1 Not a vacuous pass

The threshold is `maxCertificationRegressions: 0`, and the report records
`baseline.provided: true`. The repository already closes the obvious hole —
`report.test.mjs` pins that "a run that supplied no baseline **fails** the
threshold rather than passing it silently" — so a green run here is a
comparison that happened, not one that was skipped.

### 29.2 Reconciling `utils`

`@solid-primitives/utils` still refuses the manual `contract certify` of
§ 28, and it is not in either list: it was not certifying before the
exemption either, so nothing regressed. Its refusal is a real, named
`recursive-value-shape` gap that the exemption **unmasked** rather than
caused — the generator declares a returned root shape the producer's
observation leaves open. That remains open work, and it now has thirteen
counterexamples showing the unmasking is worth having.

### 29.3 The pin is now stale in the direction that hides regressions

`benchmarks/ecosystem/report.json` still records those thirteen rows as
refused. A future change that takes one of them from certified back to
refused would compare against the *old* pin, match it, and pass. Repinning
with `make ecosystem-benchmark` closes that hole and locks in the gains; it
rewrites a checked-in artifact the phase ledgers read, so it is left as a
deliberate separate step rather than folded into this measurement.

## 30. The repin is blocked: the exemption costs 4.5× benchmark wall time

Asked to repin `benchmarks/ecosystem/report.json` with § 29's improved run.
**Not done.** Producing the candidate to scratch first — rather than writing
the pin and discovering this afterwards — showed why.

| | pinned | candidate |
| --- | --- | --- |
| `durationMs` | 116,582 | **527,689** |
| `harnessDurationMs` | 1,373,821 | **5,733,467** |
| `generationDurationMs` | 631,684 | 857,281 |
| verified rows | 368 | 381 |
| certified entrypoints | 537 | **654** |

`performance-budget.test.mjs` asserts the pinned report's `durationMs` is
**strictly below 150,000 ms**. The candidate is 3.5× over it. A pin that
fails its own budget test is worse than a stale pin, so the file is
unchanged.

### 30.1 It is not contention, and it is not the host

The first measurement (§ 29, 531 s) ran while this session was also building
and testing, so contention was the obvious suspect. The candidate run had
the machine to itself and a warm registry cache: **527 s**. And this *is* the
14-core authority host the 150 s budget was calibrated on — `ecosystem-
benchmark.md` records a no-contention rerun there at 116.6 s, which is the
current pin.

### 30.2 The cause is the exemption, and the mechanism is the veto

Harness time went up **4.2×** while generation time moved only 1.4×. The
harness is the probe worker, and every closed claim domain schedules a
**mandatory contradiction veto** — a real worker launch. The exemption
creates far more closed domains (+117 certified entrypoints, and many more
candidates per entrypoint), so far more gates execute.

The cost therefore sits where the benefit does: this is what it costs for
the checker to actually prove the domains the blanket frontier used to
withdraw. It is concentrated in `creates` and `returns`, which ADR 0036
serves with *synthesized* vetoes that launch; `reads` candidates have no
synthesized veto and stay withheld without a launch.

### 30.3 Why no gate caught it

- `make verify` excludes the ecosystem benchmark entirely.
- `make ecosystem-regression` passed, because its thresholds file carries
  only `maxCertificationRegressions`. It measures **outcomes, not time**.
- `performance-budget.test.mjs` reads the *pinned* report, so it cannot see
  an unpinned regression — it only fires once somebody repins.

So a change can quadruple the corpus wall time and be green on every gate
that runs before landing. That is a gap in the gates, not only in this
change.

### 30.4 The decision this needs

Four options, none of them mine to take unilaterally:

1. **Accept and raise the budget.** The test comment says the budget "is the
   ceiling the project holds itself to, not a description of the current
   measurement" — raising it to fit a change is precisely what that sentence
   warns against.
2. **Optimise the gate schedule** so the extra vetoes cost less — for
   instance batching synthesized vetoes per artifact case instead of per
   claim, which is where the 4.2× lives.
3. **Narrow the exemption** so fewer domains close, trading some of the
   thirteen certification fixes back.
4. **Revert `cf905a58`.**

Until one is chosen the pin stays where it is, and § 29's thirteen
certification fixes stay unpinned — with the detection hole § 29.3 already
named.

## 31. Batching the synthesized vetoes: why the obvious form is not available

> **Vindicated by § 33.** This section's premise — "the cost is launches, and
> launches are sessions" — is correct against the release binary: launching is
> 47.4% of a probe-gate batch. § 32 appeared to refute it, but measured a debug
> build. Read § 33 before § 32.

§ 30.4 proposed "batching synthesized vetoes per artifact case instead of per
claim" as the option that keeps § 29's gains. That proposal was
under-informed. Mapping it found the real constraint.

### 31.1 Launches cannot be shared, by design

The cost is launches, and launches are `sessions`:

~~~
targets  = one per subject (artifact case × export × domain)
sessions = targets × modes × policy.repeat_runs
launches = one per session, sequential
~~~

`launch_every_session` is sequential deliberately — "without the intermediate
census session N could tamper with what session N+1 reads and restore it
before the final check" — and one workspace materialization already covers
the whole batch, so per-batch setup is amortised.

Two claims cannot simply share one launch. `isolation_collisions` flags any
two runs sharing a process, realm or module instance, and a collision
**refuses the mode**: *"repeat runs reused process, realm, or module-instance
state"*. That check exists precisely to catch a shared realm, so smuggling a
shared launch past it would defeat the property it guards.

Emitting one module for an export's several domains does not help on its own
either: the two manifest entries would differ only in `expectedEvent.marker`,
but sessions are keyed per claim, so the launch count is unchanged.

### 31.2 So the literal form is a gate-model change

Fewer launches requires fewer *targets*: a gate would have to become **one
isolated observation of one export that can falsify several of its claims**,
instead of one per claim. That is a coherent model — one run of the samples
really does observe `creates`, `returns` and `reads` at once, and the
generated module already computes all three observations in one body — but
it is not an optimisation. It changes:

- **what a gate attests**, from one claim to several, and therefore
  `probe_gate_root` — **every policy-2 receipt in the repository moves**;
- **what a hand recipe addresses**, because the corpus is claim-keyed
  (`recipe_for(claim_id)`) and every checked-in recipe, plus the scaffold
  emitted in § 22, names one claim;
- **withheld attribution**, since `incomplete_gate_withholding` maps a gate
  back to exactly one `WithheldClosure` today.

Sizing, from the regenerated fixture corpus: 201 summaries carry proposed
closures, 151 of them close two or three domains. 395 gates would become
201 — **49% fewer launches**, roughly 527 s → ~265 s. Real, and still 1.8×
over the 150 s ceiling.

### 31.3 The better lever: overlap the boot, keep the isolation

Per-launch cost is process spawn plus Node boot plus import plus run plus
census. The sequencing requirement is only that **session N+1 must not read
the workspace until N's census has passed** — not that N+1's interpreter
cannot already be running. A pre-booted worker that has been handed no
session yet has read nothing.

So a small pool of pre-booted workers, each handed exactly one session after
the preceding census and each still getting its own fresh process and realm,
amortises boot latency **without weakening isolation and without changing
what a gate means**. Receipts move only through the harness image digest,
which any harness change moves anyway — not through a semantic change to
what a gate attests.

It needs a `PROBE_WORKER_PROTOCOL` bump (the worker must wait for its session
rather than receive it at spawn) and is confined to the harness launcher and
worker.

### 31.4 Recommendation

Do § 31.3 first: it is confined, it preserves the isolation argument intact,
and it does not touch the claim-keyed recipe model that § 22's scaffold and
every checked-in recipe depend on. Measure, then decide whether § 31.2's
gate-model change is still worth a repository-wide receipt migration for the
remaining factor.

Neither is started. § 30's budget breach stands, and the pin stays where it
is.

## 32. Measured: the launches are 1.3% of the cost, so the pre-boot pool is not worth building

> **Superseded by § 33.** Every measurement in this section was taken against
> a *debug* build, and the debug profile inflates the census by roughly 19×.
> The conclusion below — that launching is 1.3% of a batch — is false for the
> release binary the benchmark actually runs, where launching is the *largest*
> phase. The section is kept as written because § 33 is about how it went
> wrong; do not cite its numbers.

§ 31.4 recommended a pre-booted worker pool as the confined lever. Measuring
the probe-gate batch before building it killed the proposal.

One batch, `implementation-census-reads`, two sessions, debug build,
`SOLID_CHECKER_TIMINGS=1`:

| phase | time | share | per session |
| --- | --- | --- | --- |
| `censusNs` | 2.835 s | **59.0%** | 1418 ms |
| `workspaceNs` | 0.957 s | 19.9% | — (per batch) |
| `pinVerificationNs` | 0.913 s | 19.0% | — (per batch) |
| **`launchNs`** | **0.061 s** | **1.3%** | **31 ms** |
| `conditionsNs` | 0.039 s | 0.8% | — (per batch) |

**Launching is 1.3% of the batch and 2% of the per-session cost.** A pool
that amortised Node boot perfectly would remove at most that. § 31.3 was
optimising the one phase that costs nothing.

### 32.1 What actually costs: the watched census, per session

`launch_every_session` re-verifies the whole watched census after every
session, and the census hashes the pinned images. On this host the two
largest are the Node executable at **117 MB** and the verifier image at
119 MB debug / 24.7 MB release. The census is parallel across labels, so it
costs its largest input — and it pays that once per session.

Per session: census 1418 ms, launch 31 ms. **The census is 98% of what a
session costs**, and it is what makes an extra gate expensive.

That also explains § 30 exactly. The exemption did not make launches
slower; it made more *sessions*, and every session buys another full census.

### 32.2 Which reframes the two candidates

- **§ 31.3 pre-boot pool — abandoned.** It targets 1.3%.
- **§ 31.2 gate-model batching — better than it looked, for a different
  reason than given.** Halving sessions halves the *census*, not the
  launches: 395 gates → 201 would take roughly 30% off the harness. The
  49%-fewer-launches figure was the right arithmetic attached to the wrong
  cost.

### 32.3 The untried lever, and the reason to be careful with it

The census re-hashes ~240 MB of *pinned, immutable* images between every
pair of sessions. Hashing them once per batch instead would remove most of
59%.

It is not obviously sound, and the code says why in the sentence that
justifies the current ordering: "without the intermediate census session N
could tamper with what session N+1 reads and restore it before the final
check." That is exactly the attack a once-per-batch hash reopens, and the
Node executable is the extreme case — tampering it changes the interpreter
the next session runs under.

Whether a narrower answer exists — verifying the large pinned images
*before each exec* rather than after each run, which is the same count;
or splitting the census by what a session can actually write — is a
security-model question and is not settled here.

Nothing was built. § 30's budget breach stands.

## 33. Corrected: § 32 measured a debug build, and its conclusion inverts in release

§ 32 concluded that launching is 1.3% of a probe-gate batch and abandoned the
pre-boot pool unbuilt. That conclusion is wrong. It was measured with
`cargo test`, which is a debug build, and the debug profile distorts precisely
the phase the comparison rested on.

### 33.1 How the debug basis inflated the census

Two independent factors, both hitting the census and neither hitting launches:

- **`verifier-image` is a different file.** The label hashes
  `std::env::current_exe()`. Under `cargo test` that is the *test executable*
  — measured here at **116.3 MB** — not the 23.6 MB release
  `solid-checker-rust` the benchmark runs. Nearly 5× the bytes.
- **Debug SHA-256 is ~4× slower.** Per-label timings give a uniform
  **124 MB/s** in debug (`node-executable` 112.1 MB in 905 ms; `verifier-image`
  116.3 MB in 940 ms), against **510 MB/s** measured with `shasum` on the same
  files on the same host.

Together they inflate the census by roughly 19×, which is exactly the gap
between § 32's 1418 ms/session and the release figure below. Launching a
process is unaffected by the checker's build profile, so it was compared
against a census nineteen times too expensive and looked like noise.

`AGENTS.md` already carries this trap for finding *timing*, and the local
memory note spells it out — "debug hashing made graph rows look 15× slower".
It was not applied here.

### 33.2 The release measurement

`@kobalte/utils`, release binary, one package, `SOLID_CHECKER_TIMINGS=1`:
313 probe-gate batches, 6,198 sessions, 1,043.5 s of probe-gate time.

| phase | total | share | per session |
| --- | --- | --- | --- |
| **`launchNs`** | 495.0 s | **47.4%** | **80 ms** |
| `censusNs` | 451.6 s | 43.3% | 73 ms |
| `workspaceNs` | 43.4 s | 4.2% | 7 ms |
| `pinVerificationNs` | 26.3 s | 2.5% | 4 ms |
| `conditionsNs` | 24.2 s | 2.3% | 4 ms |

Against § 32: census **1418 ms → 73 ms** per session, and launch
**1.3% → 47.4%** of a batch. The two phases swap places.

### 33.3 No single package generalises

The other heaviest row behaves nothing like this one. `solid-js@1.9.14`,
same release binary, runs **zero** probe sessions (`gateSessions: 0`,
`gateNs` 0.18 ms) and spends **95.7%** of its certification in
`live-export-value-acquisition-and-verification` — a sequential loop over
35–44 plans on a single `TypeFactsCertificationSession`.

So the corpus has at least two distinct bottlenecks, and a one-package
profile — § 32's included, and both of § 33's — cannot rank them. A
corpus-wide ranking needs one full run with `SOLID_CHECKER_TIMINGS=1`.

### 33.4 What this does to the candidates

- **§ 31.3 pre-boot pool — reinstated, and it is the largest measured
  lever.** 47.4% of probe-gate time on the package that spends the most
  time there.
- **§ 31.2 gate-model batching — better still.** Session count multiplies
  launch *and* census, so halving sessions takes ~45% off the harness, not
  the ~30% § 32.2 computed against the census alone.
- **§ 32.3 census hoisting — devalued.** It buys 43.3%, but at 73 ms/session
  the absolute prize is far smaller than § 32.3 implied, and the
  tamper-then-restore objection is unchanged. Not worth the security
  argument on these numbers.
- **Certification concurrency — untested and free.** Width computes to
  `min(20, 14+6, 24) = 20` on this host, justified by a comment measuring
  children that "spend most of their slot time waiting". A single
  certification child was sampled here at **606% CPU** — about 6 of 14
  cores — so that calibration is stale in the direction of
  over-subscription. No code change; a sweep would settle it.

### 33.5 What § 30 keeps

The budget breach is real and is not a measurement artifact. Decomposing the
pinned report against the current one, over the same 418 rows:

| component | old | new | ratio | share of the increase |
| --- | --- | --- | --- | --- |
| install | 74.9 s | 97.4 s | 1.30× | 0% |
| generation | 631.7 s | 857.3 s | 1.36× | 5% |
| certification | 1373.8 s | 5733.5 s | **4.17×** | **95%** |

Install time is the environmental control — no code change can move it — and
it is flat, so no host-level slowdown explains the breach. Two further
readings: `declinedClosures` rose 40,051 → 411,906 (10.3×) but correlates
with added certification time at **r = 0.062**, so the records are a symptom
of the larger closure and not its cost; and effective parallelism *fell*
17.84× → 12.67×, which is what § 33.4's last bullet is about.

§ 30's four options stand. Nothing measured so far closes a 527 s run to the
150 s ceiling, so optimisation narrows the gap rather than removing the
decision.

## 34. Built: the pre-boot pool, measured at ~16% of a certification's wall

§ 33.4 reinstated § 31.3. This is it, built and A/B'd.

### 34.1 What had to move first

A worker could not be booted ahead because the recipe module was a
*spawn-time environment variable*: a booted process cannot be told afterwards
which recipe to run. Worker protocol v7 moves it into the session frame, read
beside `id` and `mode.environment` — before the recipe, and therefore the
package, runs — and fails closed when absent, so a pooled worker can never
inherit an earlier session's value.

The launch nonce deliberately did **not** move. It binds the *process* to the
harness that spawned it, which is true from boot and is what the startup frame
answers with before any session exists. A parked worker keeps exactly the
binding it has today. An earlier draft of this section claimed the nonce would
have to move and that its meaning would change; that was wrong, and nothing
about what the harness proves changed here.

### 34.2 Where the boot is hidden

Not during the previous session's *run* — the obvious placement, and the
unsound one. Today exactly one worker exists at a time; booting the next one
alongside a running session would put a parked process next to hostile package
code, which is a new surface for no reason.

It is hidden inside the **between-session census** instead. In that window the
finished session's process group is already dead and the next session's worker
has no session, has read nothing a session names, and is blocked on `stdin`.
`verify_unchanged` still stands between one session's run and the next
session's first read, which is the property the ordering exists for. Parking
changes *when a process exists*, not *when it reads*.

`launch` split into `spawn_parked` (pinned Node, allowlisted environment,
nonce, startup frame verified — all session-independent) and `run_parked`
(writes the session, reads the one run frame). The policy timeout now starts
at dispatch rather than at boot, so a worker parked early cannot spend a
session's budget waiting; `spawn_parked` bounds the startup frame with
`STARTUP_BUDGET` on its own. A refused or panicked pre-boot is discarded
rather than reported: the next session boots inline and surfaces the real
error there, and the census verdict always takes precedence.

### 34.3 Measured

`@kobalte/utils`, release binary, 313 batches / 6,198 sessions per run, both
orders to rule out an order effect. `SOLID_CHECKER_PROBE_NO_PREBOOT=1` is the
control.

| trial | launch (pool) | launch (off) | wall (pool) | wall (off) | delta | outcome diffs |
| --- | --- | --- | --- | --- | --- | --- |
| pool first | 169.5 s | 498.0 s | 208.7 s | 247.7 s | −15.7% | 0 |
| no-pool first | 155.2 s | 521.7 s | 210.2 s | 252.2 s | −16.7% | 0 |

Launching falls ~68%, which is the mechanism working. The phase sum falls only
6%, because `censusNs` now includes joining the boot thread — the boot does
not *entirely* hide inside the census, it mostly does. End to end the
certification is **~16% faster**, and **zero** outcomes, statuses, or exit
statuses differ across 12,396 sessions.

### 34.4 What it does not do

It does not close § 30. ~16% off a 527 s run is ~440 s against a 150 s
ceiling. The ranked remainder is unchanged from § 33.4: session count is still
the larger lever because it divides launch *and* census, and the certification
concurrency sweep is still free and still untested. A corpus-wide run with
`SOLID_CHECKER_TIMINGS=1` is still the only thing that can rank the two
bottlenecks against each other, since `solid-js` runs no probe sessions at all.

## 35. Corrected: the pool is worth ~1.5% corpus-wide, not the ~16% § 34.3 claimed

§ 34.3 measured the pre-boot pool on one package and reported ~16%. The
commit that landed it (`f0de1b0e`) carries the same figure. Corpus-wide it is
wrong, for a reason § 33.3 had already written down and this section's author
then ignored: **no single package generalises.**

### 35.1 The controlled corpus A/B

Full 418-row runs, back to back, no-pool first, nothing else on the host,
`SOLID_CHECKER_PROBE_NO_PREBOOT=1` as the control:

| metric | pool | no-pool | delta |
| --- | --- | --- | --- |
| corpus wall | 548.0 s | 556.5 s | **−1.5%** |
| row-time sum | 7713.0 s | 7059.0 s | **+9.3%** |
| outcome / status / exit-status differences | — | — | **0** |

### 35.2 Why the single-package figure did not transfer

`@kobalte/utils` alone has the whole host to itself, so a boot started beside
the census runs on an idle core and genuinely disappears. The benchmark runs
20 certification children on 14 cores — one child was sampled at 606% CPU —
so there is no idle core for the boot to hide on. It does not hide; it
competes, which is exactly what the +9.3% row time is.

This is the same saturation argument that killed the parallel-producer-session
idea earlier in the day. It applied here too and was not applied.

### 35.3 What the pool is actually worth, and why it stays on

Two different cases, and both are real:

- **A package certified with slack** — one package, a developer's machine, an
  interactive `--package` run: ~16% faster, as § 34.3 measured. This is the
  case a person waits on.
- **The saturated corpus run** — 20-way concurrency: ~1.5% faster, at 9.3%
  more total row CPU.

It stays on by default because the case a human waits on is the one it helps,
it is semantically neutral across 418 rows in both configurations, and
`SOLID_CHECKER_PROBE_NO_PREBOOT=1` turns it off for a host where the extra
CPU is not wanted. A smaller host than this 14-core one may well want it off;
that is not measured here.

§ 34.3's table stands as a measurement of the unsaturated case. It does not
stand as the pool's value, and `f0de1b0e`'s commit message overstates it.

### 35.4 What still has not been measured

Unchanged and still the cheapest remaining work: the certification
concurrency sweep — free, no code change, and now doubly motivated, since a
+9.3% row-time cost that barely moves wall is another sign that 20 slots on
14 cores is past the useful width. And § 31.2's session count remains the
larger lever, because it divides launch *and* census rather than trying to
hide one behind the other.

## 36. Verified: the § 27.2 class refuses, and the untested half was the producer

§ 27.2 named a class to watch and left it as an argument: after the exemption
a generator proposes `reads: []` for an export whose only read is a property
access on a core-runtime import, and that is admissible *only because the
certifier's implementation census refuses the form*. Nothing tested it.

### 36.1 The proposal, reproduced

A throwaway package — `import { sharedConfig } from "solid-js"` and one
export returning `sharedConfig.hydrating`, beside an own-literal control —
generates a contract in which **both exports share one summary**:

~~~json
"closed": ["reads", "creates"], "reads": []
~~~

with an empty refusals sidecar: no declined closure, no withheld claim. So
the generator really does propose `reads: []` over a core-runtime value
access, exactly as § 27.2 predicted, and treats it identically to a value the
module built itself. The whole safety of the case is downstream of that.

### 36.2 Two guards, and only one of them was tested

**The census side was already covered.** `census_form_disposition`'s
`own-literal` arm requires the subject's declaration to resolve to a path
inside the artifact and to sit in `run.runtime_sources`; a unit test in
`type_facts.rs` already asserts that a declaration *outside* the artifact's
runtime source refuses, alongside no declaration at all.

**The producer side was not.** The gap is subtler than § 27.2 guessed. The
producer roots an *imported* binding as `own-literal` when its declaration
carries a visible object literal — `importedTableRead` in the producer's own
fixture is imported and is rooted, legitimately, because ADR 0044's premise
is about the literal, not about where the binding came from. Nothing asserted
what happens when the declaration has **no initializer**, which is precisely
the shape a core-runtime package exports: `declare const sharedConfig`.

Had the producer rooted that as an own literal, the form would have arrived
at the census carrying a clearing premise, the `runtime_sources` guard is the
only thing that would have stood between it and a certified `reads: []`, and
no test anywhere named the situation.

### 36.3 What was added

`TestDeclaredImportedReceiverIsNotRootedAsAnOwnLiteral` in the producer:
a `declare const` with no initializer, imported and read, must record an
uncensused invoking form (so the census has something to refuse) and must
root **none** of them. It carries its own control — `importedTableRead`, the
imported binding that *does* have a literal and *is* rooted — so the test
fails if the own-literal premise stops working altogether rather than
silently proving nothing.

Confirmed falsifiable: pointed at `importedTableRead` it fails with
`a form is rooted "own-literal"; a declaration with no initializer states no
premise`.

### 36.4 Status

The class § 27.2 flagged is **sound, and now evidenced at both layers**. The
gap was in the evidence, not the behaviour — which is the good outcome, but
it was not knowable without checking, and the producer-side shape that could
have broken it was not the one § 27.2 anticipated.

## 37. Corrected: `utils` certifies, its `reads` is fully closed, and the chain's real gap is the edge supply

§ 28 recorded that the chain "stops at the first step" because
`@solid-primitives/utils` refused with a `recursive-value-shape` demand on
`createHydratableSignal`. Re-measured against the current release binary,
**that refusal is gone.**

### 37.1 `utils` certifies, with content

`@solid-primitives/utils@7.0.0-next.4`, release binary, `--attempt-certification`:

| | |
| --- | --- |
| artifact cases emitted | **3**, all three certified, **0 refused** |
| certification | `certified` at `catalog-publication` |
| declines | `unresolved-callee: 4` — and *nothing* else |
| exports | 99 |

The `unaccepted-external-dependency` declines are gone entirely, which is
§ 27's exemption doing exactly what it was for.

And the domain this whole phase is about:

~~~
unknownByDomain: { reads: 0, creates: 68, returns: 96, callbacks: 99, writes: 99, … }
~~~

**`reads` is unknown for zero of 99 exports** — closed across the entire
package. That is the first real published package whose `reads` domain closes
completely, and it is the thing § 6 onwards was built for.

The three `@solid-primitives/source` entries in the report are *inapplicable*
artifact cases, not refusals: that condition routes to sources the tarball
does not ship, and skipping it is correct. An earlier reading of this section's
author took `artifactCasesTotal` for the whole population and concluded the
package had nothing to certify; it counts the *emitted* cases, and the
inapplicable one is additional.

### 37.2 `memo` is blocked by the edge, not by `utils`

`@solid-primitives/memo@2.0.0-next.2` emits one real case
(`/exports/./import/default` → `./dist/index.js`, 0 refused) and declines:

~~~
unaccepted-external-dependency: 21   unresolved-callee: 5
dialect-silent: 4                    refusing-callee-fixpoint: 2
~~~

All seven exports remain unknown in every domain. The 21 records are the
`@solid-primitives/utils` specifier with no accepted contract behind it.

### 37.3 The gap is a missing capability, not a defect

Certifying `utils` does not by itself help `memo`: the benchmark runs each
package independently and never supplies one row's certified contract to
another's generation. Naming both in one invocation does not change it —
`--package @solid-primitives/utils --package @solid-primitives/memo` leaves
`dependencyPlan: null` on both rows and `memo`'s 21 declines untouched.

The generator does accept `--accepted-contracts` and
`--proposal-dependencies`; the benchmark does not drive them. So the chain
§ 27 proposed is sound and its first step now works — what is missing is the
step that hands step one's receipt to step two.

### 37.4 Status

- § 28.1's refusal: **resolved**, and § 28 should be read through this section.
- `utils`: certifies, `reads` closed on 99/99 exports.
- `memo`: unchanged, waiting on an accepted edge that nothing currently
  supplies.
- Next: drive `--accepted-contracts` from the benchmark (or once by hand) and
  measure how many of `memo`'s seven exports close when the edge is real.

## 38. Why the `memo` edge cannot be supplied: the composing lane is keyed to refusals

§ 37 left the chain needing one step — hand `utils`' receipt to `memo`'s
generation. This is why that step has no path today, and why § 26's graph-lane
attempt was a silent no-op.

### 38.1 An accepted contract is resolution-bound, not a file

A catalog entry is not a contract document sitting in a directory. It carries
`bindings` naming the exact `importer` shim path, the `specifier`, a
`resolvedImportRoot` digest and a `semanticDigest`. Copying `utils`' certified
document into `memo`'s catalog would forge a binding for a resolution that was
never performed, so "supply the edge by hand" is not a file operation.

### 38.2 The lane that exists for this cannot be requested

`certificationLaneRequest` routes to a composing lane only when the row is
`class: "partial-success"` **and** `partialProposalHasDependencyFrontier`
matches — and that predicate reads *artifact-case refusals*.

`memo` is `class: "success"` with **zero** refused cases. Its frontier is a
decline census: 21 `unaccepted-external-dependency` records against
`@solid-primitives/utils`, with all seven exports unknown in every domain. So
the lane is never requested, and `--dependency-graph-lane` does not force it
— the class check precedes the flag. Measured: with the flag, `memo` still
reports `laneRequested: reused-proposal`.

**This is not something § 27 introduced.** The pinned pre-exemption report
classes `memo` `success` with zero refused cases as well, and the corpus-wide
`partial-success` count is identical before and after (32 of 418). The lane
policy has never covered this row. That is the explanation § 26 lacked.

### 38.3 And making it requestable is not enough

Extending the predicate so an explicit `--dependency-graph-lane` accepts a
decline-based frontier was tried and **reverted**. It does route the request —
`memo` then reports `laneRequested: published-graph` — but `contract certify`
performs no composition: the audit records `graphPreparation: {reusedProposal:
false}` with no `rootCases`, the run reports the `generated-proposal` lane,
and the 21 declines and seven unknown exports are untouched.

The reason is structural. The frontier-only lane *publishes the refused cases
instead of the generated ones*. With zero refused cases there is nothing for it
to publish, so it degenerates to generating the proposal. The lane is keyed to
refusals end to end, not only in the benchmark's predicate.

The change was reverted rather than kept: it would have made `laneRequested`
read `published-graph` while nothing composed, which is a worse report than an
honest `reused-proposal`.

### 38.4 What would actually close it

A decline-based dependency frontier needs a composing path of its own — the
certifier would have to treat "this export's closure declined on specifier X"
as a demand for X's accepted contract, the way it treats a refused case. That
is a certifier change, not a benchmark one, and it is the real prerequisite for
`memo` and for every dependent package behind the same shape.

Until then: `utils` certifies with `reads` closed 99/99 (§ 37), and `memo`
stays at zero closed domains regardless of `utils`' state.

## 39. Measured at corpus scale: the bottleneck is the dependency edge, not hand observations

The question behind this whole phase is how much of certification can run
without a person. It has been answered from one package at a time. This is the
corpus answer, read off the 418-row report (381 rows carry a contract, 8,950
exports between them).

### 39.1 Where the domains stand

| domain | exports closed | rows fully closed |
| --- | --- | --- |
| `reads` | 11.4% | 63 / 381 |
| `creates` | 5.9% | 14 / 381 |
| `returns` | 1.6% | 0 / 381 |
| everything else | 0% | 0 / 381 |

### 39.2 What is actually blocking it

411,906 declined closures, by kind:

| share | kind | rows affected |
| --- | --- | --- |
| **86.8%** | **`unaccepted-external-dependency`** | 273 |
| 4.5% | `unresolved-callee` | 168 |
| 3.3% | `runtime-accessor-installation` | 78 |
| 3.0% | `refusing-callee-fixpoint` | 126 |
| 2.2% | `dialect-silent` | 153 |
| 0.3% | `opaque-wasm`, `mutable-unbound-global`, `nonliteral-dynamic-loading` | 7 |

And the review plan — the queue of work a person would pick up — holds **3,000
closure candidates and zero probe candidates.** Nothing in the corpus is
currently asking for a hand-authored recipe.

### 39.3 The split that matters

Partitioning the rows by whether they have a dependency frontier at all:

| | rows | exports | `reads` closed | rows fully closed |
| --- | --- | --- | --- | --- |
| with a dependency frontier | 273 | 7,720 | **4.7%** | **0 / 273** |
| without one | 108 | 1,230 | **53.7%** | **63 / 108** |

**This is a selection effect and must not be read as a forecast.** Packages
with no dependency frontier are systematically simpler — fewer dependencies
usually means less of everything — so 53.7% is not what the other 273 rows
would reach if the edge were supplied. What the split does establish is that
no row carrying a dependency frontier closes completely, in any domain,
anywhere in the corpus.

### 39.4 What this changes

Every earlier statement in this document — and every answer this session gave
— put the human cost at "one hand-authored observation per export whose
`reads` you want closed". At corpus scale that is not the binding constraint:

- `@solid-primitives/utils` closes `reads` on **99 of 99 exports with zero
  recipes**. There is no recipe for it in the corpus at all. § 6's argument is
  about exports that genuinely read a source they own; most exports do not,
  and their `reads: []` is decided by the implementation census with no person
  involved.
- The corpus-wide probe-candidate count is **zero**.
- 86.8% of all declines are one missing capability — the decline-based
  dependency-edge composition § 38.4 describes.

So the ranked work for human-less certification is: **the dependency edge
first, by a wide margin**, then `unresolved-callee` and the hazard kinds.
Hand-authored observations are a real cost but a small one, and they are not
what is holding the corpus.

§ 6 stands as a statement about what a synthesized veto can never do. It does
not describe where the effort goes.

## 40. The decline-only frontier now says so

§ 39 put 86.8% of all declined closures behind one missing capability. That
capability — a composing path for decline-based dependency frontiers — is not
built here. What is built is the part that stops the gap being invisible.

### 40.1 The silence, and why it was a defect

`preparedGraphForPartialProposal` returned `{ graph: null, trace: null }`
whenever `partialProposalHasDependencyFrontier(audit.refusals)` did not match.
For a row like `@solid-primitives/memo` — whose frontier is 21
`unaccepted-external-dependency` *declines* and zero refused cases — that is
an early exit with no record at all.

The same function's other exits are careful about exactly this, in a comment a
few lines below: "a lane that was requested, attempted and could not be
prepared leaves the partial proposal as the answer — but it must leave a
trace, or the audit reads exactly like a row that never wanted the lane."
The early return broke that rule, and that silence *is* § 26's unexplained
no-op.

### 40.2 What it reads now

`memo`, `--dependency-graph-lane`:

~~~json
{
  "partialProposalFrontier": "declined-only",
  "reason": "the dependency frontier is recorded as closure declines, not as refused artifact cases; this lane publishes refused cases and has none to publish",
  "declinedDependencyRecords": 21,
  "declinedDependencySpecifiers": ["./dist/index.js:@solid-primitives/utils"],
  "declinedDependencySpecifiersTotal": 1
}
~~~

The specifier and the record count, in the audit. "Why did this not close" is
a sidecar line rather than a package to read.

### 40.3 On restoring the routing § 38.3 reverted

§ 38.3 reverted the benchmark change that lets an explicit
`--dependency-graph-lane` route a decline-based frontier, because on its own
it made `laneRequested` read `published-graph` while nothing composed — a
report claiming a lane that never engaged.

It is restored here, and the reason it is no longer the same change: with
§ 40.1 in place, routing is what lets certify *reach the point where it can
say why*. Without it certify is never asked and the trace never runs. The two
halves are one change and are both gated behind the explicit flag; the default
policy is untouched, because the frontier-only lane still publishes refused
cases instead of generated ones.

### 40.4 What this does not do

It composes nothing. `memo` still has 21 declines, seven exports unknown in
every domain, and zero closed domains. § 38.4 remains the work: the certifier
must treat "this export's closure declined on specifier X" as a demand for
X's accepted contract, the way it treats a refused case. § 39 is the argument
for doing it — 86.8% of every decline in the corpus, and not one row carrying
a dependency frontier closing anywhere.
