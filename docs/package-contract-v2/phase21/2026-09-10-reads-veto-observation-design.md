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
