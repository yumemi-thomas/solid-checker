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

## 41. The dependency edge composes

§ 38.4 named the missing capability and § 39 measured it at 86.8% of every
decline in the corpus. It is built here, and it turned out to be much smaller
than the framing suggested, because the machinery already existed and only the
key it was looked up by was wrong.

### 41.1 Why it was one derivation, not a feature

`preparePublishedGraphCases` carries this comment, written long before this
phase:

> Exact case coordinates are acquisition requests, not claims that generation
> refused them. Keep graph preparation separate from the refusal-driven lane
> selector so a bounded investigation can request a case without fabricating a
> refusal.

That is the whole permission. The lane never needed a refusal; it needed an
exact `(entrypoint, conditions)` pair. `recoveryGraphCases` reads those pairs
off `audit.refusals`, and a row whose cases all *generated* has none — but
`writeProposalRefusalAudit` records `entrypoint` and `conditions` on every
declined-closure row too, and has since the census was introduced. The
coordinates were in the file the whole time, under a different key.

So the change is `declinedDependencyGraphCases`: the distinct coordinate pairs
named by `unaccepted-external-dependency` declines, `import` folded in the way
every other acquisition request here folds it, and cases whose only declines
are of another kind excluded — `dialect-silent` must not cause an acquisition.
`preparedGraphForPartialProposal` hands those to `preparePublishedGraphCases`
instead of returning § 40's trace.

### 41.2 Measured on `memo`

Same probe, same corpus, same recipes, with and without
`--dependency-graph-lane`:

| | reused-proposal | published-graph |
| --- | --- | --- |
| certification status | certified | certified |
| canonical graph nodes | — | 7 |
| published artifacts acquired | — | 6 |
| closure candidates, `memo` | **0** | **9** |
| closure candidates, `utils` | — | 55 |
| closures actually closed | 0 | **0** |
| certification wall | 291 ms | **17,275 ms** |

The nine `memo` candidates are seven `reads` and two `creates`. Nothing about
them is a dependency any more:

- the seven `reads` are withheld **`no recipe in corpus`** — ADR 0036's
  mandatory contradiction veto, one hand-authored recipe away;
- the two `creates` are withheld **`census refused`**, on `createSignal` and
  `createMemo` reached through `solid-js`' declaration file.

Before this change none of those nine existed. The certifier was never asked
whether `memo`'s `reads` could close, because the closure declined at the
dependency edge before a candidate was ever formed.

### 41.3 What it costs

59× on that row — 291 ms to 17.3 s, essentially all of it
`witnessAcquisition` (283 ms to 16.0 s) for acquiring and generating six extra
published artifacts. That is the honest price of the lane and it is why it
stays behind the explicit flag rather than becoming the default policy. It also
lands squarely on § 30's parked budget question: turning this on corpus-wide is
not a 150 s decision, it is a different order of run.

### 41.4 What it does not do

**Nothing closed.** All 64 candidates across both packages are withheld.

And the row's own `class` and `declinedClosuresByKind` are byte-identical
between the two runs — 21 `unaccepted-external-dependency` in both. That is a
reporting seam, not a composition failure: the benchmark reads
`contractContent` from the generation pass, which runs *before* the lane is
chosen, so that census can never see what the graph's regenerated root found.
The result is not invisible — every number in § 41.2 was read off
`certificationAttempt.withheldClosureDetails`, which the report does carry —
but the two censuses now describe different pipelines for the same row, and
anything reading `declinedClosuresByKind` to judge the lane will conclude it
did nothing.

### 41.5 What it changes about the answer to "how much needs a person"

§ 39 concluded the dependency edge was the binding constraint and hand-authored
observations were a small cost. Crossing the edge on one package refines that,
and not in the comfortable direction: **behind the dependency edge is the
recipe requirement.** `memo`'s seven `reads` candidates are blocked on
`noRecipe` and nothing else. The corpus-wide probe-candidate count was zero
(§ 39.2) because no row was getting far enough to want one.

That does not restore § 6 to the top of the list — the edge still had to be
crossed first, and 273 rows are still behind it — but it does mean the next
corpus-scale measurement has to be taken *with the lane on*, and read off the
withheld-closure census rather than `declinedClosuresByKind`, or it will keep
reporting a recipe demand of zero for a reason that is now an artifact of
where the pipeline stops. At 17 s a row that is a different order of run from
the one § 30's 150 s budget describes, which is the next thing this needs.

### 41.6 What the corpus says about turning it on

`make ecosystem-regression` against the pinned `benchmarks/ecosystem/report.json`:
418 probes, 349 complete contracts, 32 partial, **0 regressions and 0
certification regressions**. That is the expected answer and it is worth
naming why it is not evidence of anything about the lane: the default policy
never routes a decline-only frontier, so the gate exercises the path not
taken. It establishes that the capability costs the corpus nothing while it is
off, and nothing more. What it would cost with the lane on is unmeasured, and
§ 41.3's 59× on one row is the only number there is.

## 42. The lane measured on a sample, and what is actually behind the edge

§ 41.6 said the corpus number was unmeasured and one row was all there was.
This is the measurement: a stratified sample of the decline-only population,
run twice on the same ten probes, with and without `--dependency-graph-lane`.

### 42.1 The population the new path reaches

Read off the `edf1c988` regression report, 418 rows:

| | rows |
| --- | --- |
| carrying an `unaccepted-external-dependency` frontier | 273 |
| …whose frontier is **also** a refused artifact case (the old path) | 17 |
| …whose frontier is **only** closure declines (the new path) | **256** |

So the refusal-keyed lane could ever see 17 of them. 256 is the size of what
was unreachable, and 247 of those rows are `class: success` — rows that look
finished.

### 42.2 The sample

Ten rows, the deciles of the 256 by decline count (3 → 252; population median
24). `@solid-primitives/memo` at 21 sits just below the middle, so the worked
example of § 41 is a typical row rather than a favourable one. The
250,329-decline outlier and `@kobalte/core`'s 508 entrypoints are outside the
decile spread and were not sampled; nothing here describes them.

### 42.3 Result

| | control | `--dependency-graph-lane` |
| --- | --- | --- |
| rows certified | 10 / 10 | **10 / 10** |
| entrypoints certified | 11 / 12 | **11 / 12** |
| lane used | 10 reused-proposal | 8 published-graph, 2 fallback |
| canonical graph nodes | — | 55 |
| closure candidates | **0** | **431** |
| certification wall | 8.5 s | **202.8 s** |

**No receipt was lost.** That is the result that distinguishes this from the
frontier-only lane on *refusal* frontiers, which trades case sets and cost the
2026-09-03 corpus six rows. This path composes cases that generated, so there
is nothing to trade: a preparation that fails falls back to the proposal the
row already had.

Both fallbacks did exactly that, and named why:

- `@solid-primitives/keyed` — `@solid-primitives/utils is not installed above
  …/keyed/dist/index.js`. An install-shape fact, not a semantic one.
- `@tanstack/form-devtools` — 43 distinct specifiers, refused at a transitive
  node: `dayjs@1.11.23` … `entry file <package-root>/dayjs.min.js has no
  runtime ESM exports`. A fact about that publisher's bytes.

### 42.4 What blocks the 431

| domain | reason | n |
| --- | --- | --- |
| `reads` | **`noRecipe`** | 332 |
| `creates` | `censusRefused` | 53 |
| `returns` | **`noRecipe`** | 26 |
| `returns` | `dependencyWithheld` | 20 |

**83% of every candidate the edge exposed is blocked on a hand-authored
recipe.** § 41.5 inferred this from `memo` alone; it holds across the sample.

### 42.5 The correction this forces on § 39

§ 39.4 said, and I repeated it as the session's answer: *"Hand-authored
observations are a real cost but a small one, and they are not what is holding
the corpus."* That was measured on a pipeline that stopped at the dependency
edge. With the edge crossed it is wrong. Recipes are the dominant blocker
behind it, and the corpus-wide probe-candidate count of zero (§ 39.2) was an
artifact of where the pipeline stopped, not a property of the corpus.

What survives from § 39 is the ordering: the edge had to be crossed first, and
it still gates 256 rows. What does not survive is the conclusion about where
the human cost sits.

### 42.6 How much hand-authoring, actually

The 358 `noRecipe` candidates are **160 distinct claims**, and they concentrate
hard:

| package | candidates | distinct claims |
| --- | --- | --- |
| `@solid-primitives/utils` | 305 | **130** |
| `solid-js` | 26 | 3 |
| seven others | 27 | 27 |

And the claims are shared across rows — 45 of the 160 are wanted by four of the
ten rows, 40 by two:

| rows sharing a claim | claims |
| --- | --- |
| 4 | 45 |
| 3 | 3 |
| 2 | 40 |
| 1 | 72 |

So the unit of work is not "a recipe per export per package". In this sample it
is **one package's ~130 claims unblocking 305 candidates across eight
dependents**, because `@solid-primitives/utils` is what almost everything in
this ecosystem imports. That is a bounded, shareable, and very unequally
distributed cost — and it is the first time this phase has had a real number
for it.

### 42.7 Cost, and the amortization nobody has taken — the four-hour figure is wrong, see § 49.1

23.8× on certification for the sample (8.5 s → 202.8 s). Applying that
multiplier to the 256 decline-only rows' 618 s of certification in the full
corpus run projects **roughly four hours** for a lane-on corpus pass. Treat
that as an order of magnitude, not a forecast: the multiplier is measured on
median-sized rows, and the two biggest rows in the sample did not compose at
all.

The obvious lever is visible in the same numbers. Eight composing rows built
**55 canonical nodes and ran 55 proposal generations** — about seven each, and
most of them the *same* `@solid-primitives/utils` and `solid-js` acquisitions
re-acquired and re-generated once per row. Nodes are shared within a graph
(`byKey`) but nothing shares them between rows. A cross-row node cache is a
speed lever with a measured size, unlike the concurrency guesses parked in
§ 30.

## 43. Five recipes, measured: nothing closed, and three reasons why

§ 42.6 put the recipe cost at "one shared package's ~130 claims unblocking 305
candidates across eight dependents" and called it the first real number for it.
This tests that number by paying a small part of it: five hand-authored `reads`
recipes for `@solid-primitives/utils@7.0.0-next.4` on
`artifact-case:2bf41ff69a22…`, the case four of the ten sampled rows share.

The answer is **zero closed closures**, and the three reasons are each worth
more than the recipes were.

### 43.1 What was written

`scripts/probe-recipe-scaffold.mjs` emitted 45 scaffolds from the `props` row's
`certification-audit.json`; five were finished and committed. The other 40 were
left in a scratch corpus — each throws until its `UNFINISHED` guard is deleted,
so generating them costs nothing and certifies nothing.

Four are short by right rather than by laziness: `clamp`, `compare`,
`arrayEquals` and `trueFn` own no reactive-shaped source, and the artifact case
carries no `runtime-accessor-installation` hazard, so § 12's argument applies —
the census states syntactically that `dist/index.js` installs no accessor and
there is no trap for the recipe to count. `access` is the one that earns its
length: it executes caller code, so it is where ADR 0034's boundary sits. It
passes an accessor reading a getter *the recipe owns*, asserts the getter ran,
and deliberately does not emit — the assertion is what keeps it from being
vacuous, because it proves the apparatus can see a read on every run.

### 43.2 The measurement

Per row, before → after (`withheldClosureReasons`):

| | event-listener | pagination | props | queue |
| --- | --- | --- | --- | --- |
| `noRecipe` | 45 → **40** | 45 → **40** | 45 → **40** | 50 → **45** |
| `censusRefused` | 7 → **9** | 7 → **9** | 7 → **9** | 7 → **9** |
| `dependencyWithheld` | 3 → **6** | 3 → **6** | 3 → **6** | 3 → **6** |
| **total withheld** | 55 → 55 | 55 → 55 | 55 → 55 | 60 → 60 |

Every one of the five left `noRecipe`. Not one closed. `vetoIncomplete` stayed
0, so no recipe threw — the three that were serviceable ran clean and were
stopped by the *next* thing.

### 43.3 Reason one: `no recipe in corpus` masks a census refusal

Two of the five — `arrayEquals` and `compare` — moved to `censusRefused`:

- `arrayEquals`: *"a reads closure candidate must enumerate no operation, but
  the proposal names 1"*. `PROPOSABLE` admits the empty closure only, and this
  one is not empty.
- `compare`: *"the coercion form (BinaryExpression) at …index.js:2339..2344
  (reachable) states no reviewed subject root, so whose value it reads is
  undecided"*. `a < b` on `any`-typed parameters is an uncensused invoking
  form.

Neither refusal is new, and **no recipe could ever have served either**. They
were reported as `noRecipe` because that is the first blocker the pipeline
reaches; writing the recipe is what surfaced the real one.

So § 42.4's headline — *83% of candidates blocked on a hand-authored recipe* —
is an **upper bound on recipe-serviceable candidates, not a count of them**.
Two of five here were not serviceable. Five is far too small to put a fraction
on 358, and this document should not pretend otherwise; what is established is
that the number is smaller than 358 and that nothing currently distinguishes
the two populations without writing a recipe to find out.

This also makes the scaffold more valuable than it looked. A throwing scaffold
is a cheap probe for "is this candidate even a recipe's problem", and 45 of
them cost one command.

### 43.4 Reason two: the chain does not terminate at `utils` — WRONG, see § 44

The three that *were* serviceable — `clamp`, `trueFn`, `access` — ran their
vetoes clean and then moved to `dependencyWithheld`. `@solid-primitives/utils`'
own `reads` closures compose from **`solid-js`** claims, and those are withheld
too.

That reorders § 42.6's plan. Writing `utils`' 130 claims first cannot close
anything, because every one of them that survives its census waits on a
`solid-js` claim underneath. Recipes have to be written **bottom-up from
`solid-js`**, and the leverage calculation in § 42.6 — one package unblocking
many dependents — applies to `solid-js` first and to `utils` only after.

> **This conclusion is withdrawn.** § 44 traces the record it rests on and
> finds the composition check compares a claim id against a contract that
> cannot contain it. The three candidates are not waiting on any `solid-js`
> claim; they are failing a comparison that has no satisfying assignment.
> Nothing here establishes a recipe ordering, and § 42.6's plan is neither
> confirmed nor reordered by it.

### 43.5 Reason three: the diagnostic that says this names the wrong claim

All six `dependencyWithheld` records read:

~~~
composed from a withheld dependency claim: claim:v1:sha256:<x> of solid-js
~~~

and in all six, `<x>` is byte-identical to the withheld candidate's **own**
`semanticClaimId` (compared on the full 64-hex digest, six of six). It cannot
be: `NormalizedContract::claim_id` digests package identity along with the
artifact case, export and path, so a `@solid-primitives/utils` claim and a
`solid-js` claim cannot share an id.

The message is therefore unusable exactly where it matters — it tells an author
to go find a `solid-js` claim and hands them the `utils` claim they are already
looking at. `composed_from_withheld_dependency`
(`contract_certification/dependencies.rs:812`) builds the record from the
parent coordinate and interpolates the `MissingClosedClaim`'s
`semantic_claim_id` as the child; one of those two is not what its name says.
Not diagnosed further here, and not fixed blind.

### 43.6 What this costs the § 42 plan

§ 42.6 estimated the work as ~130 claims on one package. After this:

- the denominator is smaller than 358 by an unmeasured amount (§ 43.3);
- the order is wrong — `solid-js` comes first (§ 43.4);
- and the one diagnostic that would tell an author *which* `solid-js` claim to
  write next currently does not (§ 43.5).

The five recipes stay. Two of them can never fire and are kept deliberately,
because their presence is what converts a masked `noRecipe` into the census
refusal underneath it, and the README records that.

## 44. The wrong claim id is not a message defect, and § 43.4 does not survive it

§ 43.5 reported that `composed from a withheld dependency claim: <id> of
solid-js` names the candidate's own claim id, and filed it as a diagnostic to
fix. Traced, it is not a diagnostic defect. The message is printing the field
it was given, and the field is the parent's claim id **by construction**.

### 44.1 Where the id comes from

`ProofDemandSubject::DependencyClosure` is built in
`contract_semantics/certification.rs:472`:

~~~rust
for closure in &candidates.closure_candidates {
    let semantic_claim_id = candidates.proposal.claim_id(closure)?;
    requested.insert((ProofFamily::AcceptedDependencyComposition,
        ProofDemandSubject::DependencyClosure {
            dependency: dependency.clone(),
            parent: closure.clone(),
            semantic_claim_id: semantic_claim_id.as_str().into(),
        }));
}
~~~

`candidates.proposal` is *this node's* proposal and `closure` is *this node's*
closure candidate. So the field holds the parent's claim id, deliberately, and
neither of the two `MissingClosedClaim` sites § 43.5 named is mislabelling
anything — they pass along what the demand carries.

### 44.2 Two of its three readers agree with that; one does not

| reader | reads the field as | correct |
| --- | --- | --- |
| `type_facts.rs:503` `creates_census(parent, claim)` | the parent's claim — finds the parent's `DomainClosure` demand by it | yes |
| `type_facts.rs:456` `dependency_creates_claims(parent, claim)` | the parent's claim — same lookup | yes |
| `dependencies.rs:3137` `receipt.contains_closed_claim_id(…)` | the **dependency's** claim — asks the dependency's contract for it | **no** |

`NormalizedContract::claim_id` digests package identity alongside the artifact
case, export and path, so the third comparison has no satisfying assignment:
a `@solid-primitives/utils` claim id is never present in a `solid-js`
contract. The condition is guarded by `independent_creates_census.is_none()`,
so ADR 0020's live creates census is the one thing that skips it.

Read together: **a `reads` or `returns` closure candidate on a node that has
any accepted dependency cannot compose.** It survives gating, runs its
mandatory veto, and is then withheld by a comparison that cannot succeed.
`creates` escapes only through the ADR 0020 census. This is deduced from the
code and consistent with all six records observed in § 43; it is not an
exhaustive measurement, and `certifiedClosures` is `null` on all 418 rows of
the regression report, so that field cannot corroborate it either way.

### 44.3 What § 43.4 actually established

Nothing. The three serviceable recipes' candidates are not waiting on a
`solid-js` claim — no `solid-js` claim was ever identified, and none appears
in the row's own withheld census. They are failing an unsatisfiable check.
"Recipes have to be written bottom-up from `solid-js`" was inferred from a
record whose content is an artifact, and it is withdrawn.

§ 43.3 is unaffected: `arrayEquals` and `compare` are `census refused` on
their own facts, and the masking argument stands.

### 44.4 Why this is not fixed here

The correctly shaped check already exists a few lines above, at
`dependencies.rs:2748`: `dependency_creates_claims` returns claims filtered to
`requirement.dependency().package` and checks
`receipt.contains_closed_claim_id(&claim.semantic_claim_id)` with the
*dependency's* id. That is what the doc comment at `dependencies.rs:3029`
describes, and it is creates-only — the census emits
`CENSUS_DEPENDENCY_CLAIM_PREFIX` sites for `creates` and for nothing else.

So there are three candidate fixes and they are not equivalent:

1. **Delete the check at 3137.** One line, and it *loosens a trust-boundary
   condition*: parent closures would compose with no dependency-claim
   requirement at all. The doc comment says this check is what makes a parent
   that relied on a withheld dependency closure refuse on its own. Cheap and
   wrong to do on one reader's judgment.
2. **Carry the dependency claim the parent actually composes from**, and check
   that. This matches the doc, matches the creates path, and is the real fix —
   but the information does not exist for `reads`/`returns`. The census would
   have to state, per parent closure, which dependency claims it composes
   from, the way it already does for `creates`. That is producer/IR work, not
   a certifier patch.
3. **Correct the diagnostic only**, leaving the check. The reason would stop
   claiming a dependency claim it cannot name. This contradicts the
   instruction the defect was filed with — keep the id, an author needs it to
   know what to write next — and the honest answer to that instruction is
   that no such id exists to keep.

(1) is a soundness decision, (2) is a feature, (3) admits the pointer is not
available. The diagnosis is recorded here rather than any of them being taken
unilaterally.

## 45. Measured, not deduced: the check refuses a dependency that closed the exact claim

§ 44 derived the defect from the code and flagged that it was not an
exhaustive measurement. This measures it, in
`a_dependency_closure_requirement_carries_the_parents_claim_the_dependency_cannot_close`
(`contract_certification.rs`).

### 45.1 What the existing suite already proved, and why it missed this

`one_dependency_receipt_cannot_exchange_callbacks_for_throws` shows the
composition check is **correct and satisfiable**: given the leaf's `callbacks`
claim it passes, given the leaf's `throws` claim it refuses. It reaches the
check through `authenticate_dependency_claim_for_test`, which *overwrites*
`requirement.semantic_claim_id` with an id the test computed from
`leaf_plan.selected_candidate`.

That overwrite is the blind spot. Production never chooses the id; demand
planning does, from the parent's own proposal. No test had ever asserted what
the field contains when planning fills it.

### 45.2 The measurement

A two-node graph with callbacks closed on **both** nodes:

| | |
| --- | --- |
| parent's callbacks claim | `claim:v1:sha256:60736bba3e03a5c5…` |
| dependency's callbacks claim | `claim:v1:sha256:00023cbc68c1b902…` |
| what the planned requirement carries | **the parent's** |
| is the parent's id in the dependency's contract | **no** |
| does the dependency's receipt close its own callbacks claim | **yes** |
| does composition succeed | **no — `MissingClosedClaim`** |

The last two rows together are the finding. The dependency closed exactly the
domain the parent's candidate is about, published a receipt carrying that
claim, and composition refused anyway — because the id it looks for is the
parent's, and nothing the dependency could ever publish would contain it.

### 45.3 It is not a `reads` problem

The vehicle here is `callbacks`, and that widens § 44. `closure_candidates` is
every domain the proposal closed, not the proposable three, so *any* closed
domain on a node with an accepted dependency meets this check. The domain only
decides what happens next:

- **proposable** (`reads`, `returns`, `creates`) —
  `composed_from_withheld_dependency` returns a record and the candidate is
  withheld, which is what § 43 saw;
- **anything else** — it returns `None` at the `is_proposable` guard, the
  error propagates, and the **row refuses**.

The second path has never been hit in the corpus only because no row closes a
non-proposable domain (§ 39.1: "everything else 0%"). It is not guarded
against; it is unreached.

### 45.4 Status — resolved in § 46

The test is committed green as a characterization, asserting the current
meaning rather than the intended one, with the defect named at the assertion
and the three fix options left in § 44.4. Whichever is chosen, this test fails
and has to be updated deliberately — which is the property it exists for.

## 46. Fixed: the field is split, and three `reads` closures certify on a real package

§ 44.4 left three options. None of them is what landed, because § 45 changed
what the question was: the check is correct and satisfiable, and the field
feeding it carried two different claims for three different readers.

### 46.1 The change

`DependencyCompositionRequirement` now has two fields instead of one:

- `semantic_claim_id` — the **parent's** closure-candidate claim, which is
  what `creates_census` and `dependency_creates_claims` resolve against the
  parent's own `DomainClosure` demand;
- `dependency_semantic_claim_id` — the **dependency's** claim this
  requirement demands closed in the dependency's receipt.

Demand planning sets the first and leaves the second `None`. The receipt check
reads the second, so it no longer compares a parent claim against a dependency
contract.

**Planning naming no dependency claim is a fact about the domains, not a gap.**
`creates` is the one domain whose census follows callees, so it is the one
whose closure a dependency can contradict — and it names its dependency claims
exactly, through `dependency_creates_claims`, checked against the same receipt
by the caller. `census_reads_domain` and `census_returns_domain` both state in
their own doc comments that they have no callee walk: a `reads` claim is about
accesses in the export's own body, and a read reached through a caller-supplied
value is the caller's (ADR 0034). There is no dependency claim for those
domains to name.

### 46.2 The attempt that was wrong, and the test that caught it

The first cut guarded the existing condition on "is this id closed in the
dependency's accepted contract", as a proxy for "is this a dependency claim".
`one_dependency_receipt_cannot_exchange_callbacks_for_throws` failed
immediately, and correctly: a claim can belong to the dependency and *not* be
closed, which is exactly the case that test exercises, so the proxy would have
disabled the check for the case it exists to catch. Splitting the field says
what the proxy was approximating.

### 46.3 Two tests were green for the wrong reason

`dependency_composition_requires_the_receipt_to_close_the_exact_claim`
asserted its refusal through the planning path, where the comparison could
never have succeeded — so it never tested "the leaf did not close the claim".
It now asserts against a real dependency claim, which is what its name always
meant.

§ 45's characterization test inverts into the property it was pinning the
absence of: planning names the parent's claim and no dependency claim, and
composition **accepts** a dependency that closed what it was asked for.

### 46.4 Measured: the first `reads` closures on a real published package

The four rows sharing `@solid-primitives/utils@7.0.0-next.4`'s `.` case,
`--dependency-graph-lane`, with the § 43 recipes:

| | before | after |
| --- | --- | --- |
| `dependencyWithheld` per row | 6 | **0** |
| total withheld per row | 55 / 55 / 55 / 60 | 49 / 49 / 49 / 54 |

Nothing reappeared under another reason. In the published contract:

| export | closed | recipe |
| --- | --- | --- |
| `clamp` | **`reads`**, `creates` | written |
| `trueFn` | **`reads`**, `creates` | written |
| `access` | **`reads`**, `creates` | written |
| `arrayEquals` | `creates` | written; census refuses `reads` |
| `compare` | — | written; census refuses `reads` |
| `afterPaint`, `withAccess` | `creates`, `returns` | none |
| `handleDiffArray` | `returns` | none |
| `noop` | `creates` | none |

§ 12's "Still open" recorded that no *real* package had ever had a `reads`
closure certified. That is no longer true.

**The negative controls are what make it a result rather than a loosening.**
`arrayEquals` and `compare` carry recipes and still do not close `reads`,
because their census refuses (§ 43.3), and that refusal is now visible in the
contract rather than masked. `noop` is the sharpest: `() => void 0`, trivially
read-free, closes `creates`, and does **not** close `reads` — because nobody
wrote it a recipe. The mandatory veto is intact, and `noop` is precisely where
an over-permissive fix would have shown.

### 46.5 Corpus

`make ecosystem-regression`: **0 regressions, 0 certification regressions**,
381 rows certified, unchanged. Corpus-wide withheld closures fall from 11,833
to **11,665** across the same 132 rows, and `dependencyWithheld` disappears
from the reason census entirely.

That last number is the honest measure of what the defect was costing, and it
is smaller than it looks: the default policy does not route the graph lane, so
only the 41 rows that compose could move at all. What a lane-on corpus pass
now yields is unmeasured — § 42.7's four-hour estimate still stands in front
of it.

### 46.6 What is still open

- `§ 43.3`'s masking result is unchanged: `no recipe in corpus` is reported
  before a census refusal underneath it, so § 42.4's 358 remains an upper
  bound on recipe-serviceable candidates.
- `certifiedClosures` and `closureCandidates` are `null` on every row of the
  benchmark report, so corpus-scale closure yield still cannot be read off it
  — every number in § 46.4 came from the published contract in the run's own
  catalog.
- The recipe cost is unchanged and now has a measured unit: three recipes,
  three closures, on one artifact case shared by four rows.

## 47. Half the recipe backlog was never worth writing, and one run finds out

§ 43.3 found that `no recipe in corpus` masks a census refusal underneath it,
and could only say the real number was smaller than 358. This measures the
fraction on one artifact case, and turns the finding into a workflow rather
than a caveat.

### 47.1 The mechanism, and why a throwing scaffold defeats it

A candidate withheld as `no recipe in corpus` is weakened out of the plan by
`CertificationPlan::recipe_gated` *before* its demands are discharged. Its
implementation census therefore never runs, and a refusal that would have
decided it unconditionally is never computed. That is the masking: the
pipeline reports the first blocker it reaches, and the missing recipe is
always reached first.

A **throwing** scaffold is a recipe as far as the gate is concerned. The
candidate stays in the plan, its census runs, and a `census refused: …`
surfaces ahead of the incomplete gate. Nothing can certify from it — a throw
withholds exactly as no recipe does, which
`a_recipe_that_throws_withholds_its_candidate_rather_than_certifying_it`
pins — so the pass is free of risk as well as of hand-authoring.

### 47.2 Measured — the 53% is one case and not the corpus, see § 49.5

45 `reads` scaffolds for `@solid-primitives/utils@7.0.0-next.4`'s `.` case,
certified once with a scratch corpus:

| | candidates |
| --- | --- |
| `census refused` — no recipe can serve them | **24** |
| `veto did not complete` — the scaffolds, worth finishing | **21** |

The 24 split three ways: 10 `domain-exhaustiveness`, 8 *"a reads closure
candidate must enumerate no operation, but the proposal names 1"*, 6
*"reads-census premise required"*.

**53% of the batch was unserviceable**, and one run with no hand-authoring
found out. My § 43 sample put it at 2 of 5 and was too small to say so.

### 47.3 What changed

`probe-recipe-scaffold.mjs` now reports it. `censusRefusedCandidates` reads
the same material `recipeGaps` does and prints one `unserviceable` line per
candidate the census refused — before the no-gaps exit, because a second pass
is exactly when there are no gaps left and the report is all there is to say.
The two-pass workflow is documented in the script header and the corpus
README.

### 47.4 What it does not settle

The corpus-wide fraction. 53% is one artifact case of one package, and this
document has twice extrapolated a number from a sample and been wrong. § 42.4's
358 stays an upper bound with no fraction attached to it until a lane-on
corpus pass measures one.

## 48. The closure accounting a run reports, and the two gaps that hid it

Every closure number in §§ 41–47 was read out of a run's own catalog by hand,
because `certifiedClosures` and `closureCandidates` were `null` on all 418 rows
of the benchmark report. That made a lane-on corpus pass pointless: it could be
paid for and then not read. Two separate gaps produced that one `null`.

### 48.1 The certifier emitted only a third of the accounting on a graph

`main.rs` has four certification entry points. The two value-only ones call
`report_closure_candidates`, `report_certified_closures` and
`report_withheld_closures`. **Both graph ones called only the third.** So a
composed row reported what gating took away and never what the planner derived
or what the receipt binds — and "nothing closed" and "everything closed" were
the same report.

Both now emit all three, per node, with the node's package and version on the
record. Attribution is the point rather than a nicety: a composed row carries
several packages' closures, and in the worked example below they are
`@solid-primitives/utils`' and `solid-js`'.

### 48.2 The runner never read them

`scripts/ecosystem-benchmark/run.mjs` reads `withheldClosures` off the audit in
two places and nothing else. `certificationAttempt.certifiedClosures` was
therefore **absent**, not empty — and `jq` answers `null` for a missing key
exactly as it does for a null value.

That is worth naming as a trap, because it fooled this document. § 46.5 read
the `null` and concluded corpus-scale yield was unreadable. The conclusion was
right and the evidence did not support it: it could not distinguish "the
certifier emitted nothing" from "the runner never asked", and both were true.
Fixing only the first changed nothing visible, which is what exposed the
second.

`closureAccounting` now carries both through, preserving `null` rather than
defaulting to `{}` — an absent accounting and an empty one are different
answers and only the audit knows which.

### 48.3 A tally beside the rows

`closed` is capped at 64 rows by the consumer. A corpus pass reading a domain
breakdown off a capped list would report a *smaller yield* rather than a
partial one, which is the failure mode this whole section is about. The record
therefore carries `closedByDomain` as well, tallied natively and summed across
nodes; nine domains bound it, so it never truncates.

### 48.4 What a run says now

`@solid-primitives/props@4.0.0-next.3`, `--dependency-graph-lane`, read
straight off `report.json`:

~~~
certifiedClosures: { cases: 5, count: 25 }
closed domains, by package:
  @solid-primitives/utils  creates 24
  @solid-primitives/utils  reads    3
  @solid-primitives/utils  returns  3
~~~

Those three `reads` are `clamp`, `trueFn` and `access` — § 46.4's result, now
countable from the report instead of from a hand-read catalog.

### 48.5 What this unblocks

The lane-on corpus pass (§ 42.7, ~4 hours). It was the only thing that could
answer how large the recipe backlog actually is, and until now it would have
produced a report that could not answer it.

## 49. The corpus measured with the lane on: 1,515 recipes

§ 48 made a lane-on corpus pass readable. This is it: all 418 probes,
`--dependency-graph-lane`, the checked-in recipe corpus, the release checker.

### 49.1 It took ten minutes, and § 42.7's four-hour estimate was wrong

| | |
| --- | --- |
| wall | **610,947 ms (10 min)** |
| certification work summed over rows | 10,806,205 ms (180 min) |
| lanes used | 253 `published-graph`, 102 `reused-proposal`, 44 `generated-proposal` |

§ 42.7 projected "roughly four hours" by taking the 23.8× per-row multiplier
and applying it to the corpus's summed certification time. The summed time is
right — 180 minutes — and the projection was still wrong, because **the runner
executes rows concurrently** and the estimate silently assumed serial
execution. On 14 cores the 180 minutes of work landed in 10 minutes of wall.

The redundancy § 42.7 identified is real and unchanged: nothing shares
canonical nodes *between* rows, so `solid-js` and `@solid-primitives/utils` are
re-acquired and re-generated once per consuming row. It is a cost in CPU, not
in waiting, and it is not a defect — `graphCaseSet` already shares nodes across
roots within one transaction, and receipts are graph-root-local by design, so
what could be shared across rows is the generation and never the receipt.

### 49.2 The number

Withheld candidates, **deduplicated by semantic claim id** — the unit a recipe
addresses:

| | rows | distinct claims |
| --- | --- | --- |
| `no recipe in corpus` — **a recipe serves these** | 22,618 | **1,515** |
| `census refused` — no recipe serves one | 4,046 | 466 |
| veto ran and failed | 107 | 76 |

The 1,515 are 1,486 `reads` and 29 `returns`. At the ~10–30 lines a finished
recipe runs to, that is the whole hand-authoring bill for this corpus.

**Read the row counts as a warning, not a quantity.** 22,618 and 1,515 differ
by 15× because a shared dependency's claim is withheld once per consuming row.
Every per-row total in this document is inflated the same way.

### 49.3 It concentrates, which is the part that makes it tractable

64 packages hold all 1,515:

| | claims | share |
| --- | --- | --- |
| top 5 packages | 934 | **62%** |
| top 20 packages | 1,331 | **88%** |

| package | claims |
| --- | --- |
| *(the row's own package, no dependency node)* | 461 |
| `@solid-primitives/utils` | 299 |
| `@corvu/utils` | 72 |
| `framer-motion` | 51 |
| `motion` | 51 |
| `motion-dom` | 50 |
| `@floating-ui/utils` | 44 |

### 49.4 What closes today, distinct rather than per-row

| domain | distinct closures |
| --- | --- |
| `creates` | 362 |
| `returns` | 26 |
| `reads` | **3** |

The three `reads` are `clamp`, `trueFn` and `access` — the recipes written in
§ 43, and the corpus's only `reads` recipes. The per-row figure for the same
three is **300**, because `@solid-primitives/utils` is a dependency of about a
hundred rows. That 100× gap between the same fact counted two ways is the
single easiest mistake to make with this report.

### 49.5 § 47's 53% does not survive the corpus

§ 47 measured 24 of 45 candidates unserviceable on `@solid-primitives/utils`'
`.` case and declined to extrapolate. It was right to decline: corpus-wide the
unserviceable share is **466 / 1,981 = 23.5%**, less than half what that case
showed. The backlog is more serviceable than the sample suggested, and the
two-pass workflow § 47 added is what separates the two populations without
writing anything.

### 49.6 Two things this run surfaced and did not investigate

- **76 distinct claims whose veto ran and failed** (54 `vetoUnreproducible`,
  53 `vetoThrew` by row). Characterized in § 50, which corrects this entry:
  none of them is a hand-authored recipe, and none is a correctness problem.
- **461 claims carry no dependency node** — the largest single group, the
  row's own package rather than a dependency. Uncharacterised, and 30% of the
  total, so the top-20 table above should not be planned against until it is.

## 50. The 76 failing vetoes are not broken recipes

§ 49.6 flagged 76 distinct claims whose veto "ran and failed" and called it a
correctness signal. Characterized, the framing was wrong on the first word:
**none of them is a hand-authored recipe.** The corpus holds 19 recipes and
none appears here. All 76 are ADR 0036 *synthesized* vetoes, and every one
failed on a property of the artifact or the harness rather than on anything
about the observation.

### 50.1 Five causes

| cause | distinct claims | rows | packages |
| --- | --- | --- | --- |
| TypeScript source under `node_modules` | 35 | 35 | `@kobalte/utils`, root |
| custom export condition | 20 | 45 | `@tanstack/query-core` |
| export-condition binding | 9 | 9 | `component-register` |
| missing package in the probe workspace | 9 | 15 | root |
| `.jsx` extension | 3 | 3 | `@kobalte/core`, root |

- **TypeScript under `node_modules`** — *"Stripping types is currently
  unsupported for files under node_modules, for
  `…/node_modules/@kobalte/utils/src/index.ts`"*. Exactly the trap
  `kobalte-utils-noop.mjs` documented in 2026-09-04: the package's artifact
  cases are `.ts` source files, the private workspace puts them under
  `node_modules`, and the pinned interpreter will not strip types there. Known
  and not a defect.
- **Custom export condition** — *"export condition
  `"@tanstack/custom-condition"` is not a plain condition name, so it cannot be
  given to the pinned interpreter"*. Node's `--conditions` takes plain names;
  a scoped one cannot be passed. Refusing is right.
- **Missing package in the probe workspace** — *"Cannot find package
  `'seroval'` imported from `…/node_modules/solid-js/web/dist/server.js`"*.
  This is the one that looks like a defect rather than a limit, and it carries
  its own clue: the probe resolved `solid-js`' **server** entry, whose runtime
  dependency the workspace does not carry. Whether the bug is the missing
  dependency or the condition resolution that reached a server build is not
  established here.

### 50.2 Why this is a capability gap and not a correctness one

An incomplete veto **withholds** its candidate — the same outcome as having no
recipe at all, pinned by
`a_recipe_that_throws_withholds_its_candidate_rather_than_certifying_it`. So
none of these 76 certified anything it should not have. Every one is
fail-closed.

What they cost is closure: a synthesized veto that ran and did not contradict
would have closed its domain on the census alone, with **no hand authoring**.

### 50.3 What that changes about the § 49 bill

The 76 are not part of the 1,515. They are a separate population with a much
better ratio:

| | distinct claims | what unblocks them |
| --- | --- | --- |
| `no recipe in corpus` | 1,515 | one hand-written observation each |
| veto did not complete | **76** | **three or four harness fixes** |

Two of the five causes are interpreter limits that are honest refusals. The
other three — 38 claims between them — are harness work, and the missing-package
one is the only place in this document where something still looks like a bug.

## 51. The missing-package failure: the precheck is one level deep

§ 50.1 left *"Cannot find package `'seroval'` imported from
`…/node_modules/solid-js/web/dist/server.js`"* as the only thing still looking
like a defect. It is one, and a small one, but not where the error text points.

### 51.1 The mechanism, end to end

`solid-js`' `./web` subpath resolves by condition:

~~~json
"node":    { "import": "./web/dist/server.js" }
"browser": { "import": "./web/dist/web.js" }
~~~

The six affected rows — `@solid-primitives/cookies` ×2, `mutable` ×2, `timer`,
`@solidjs/meta` — carry artifact cases whose conditions are `[]`, folded to
`["import"]`. The probe worker is Node, so Node's own default `node` condition
selects **`server.js`**, which imports `seroval`. `seroval` is not in the
private workspace, and the worker throws.

### 51.2 Why the gate did not refuse by name

The module header states the intended behaviour: *"A dependency the analyzed
package imports and this transaction did not authenticate refuses the gate by
name (`require_authenticated_dependency_closure`) instead of the probe reaching
unauthenticated bytes."* Here it did not — a raw Node error surfaced instead.

`require_authenticated_dependency_closure` iterates
`plan.verified_closure.manifest().dependencies`: the **analyzed package's own**
declared dependencies, one level. `seroval` is not a dependency of
`@solid-primitives/cookies`; it is a dependency of `solid-js`, which *is*
authenticated and copied in. So the precheck passes and the runtime import
fails one level deeper.

**The precheck is one level deep; the runtime closure is not.** That is the
defect, and it costs diagnosis rather than safety: the outcome is an incomplete
veto either way, the candidate is withheld either way, and nothing certified
that should not have.

### 51.3 Three fixes, and they are not the same size

1. **Make the precheck match the runtime closure.** Small, matches the
   documented intent, turns a raw worker throw into a named refusal. Closes no
   claim.
2. **Authenticate the dependency closure transitively**, so `seroval` is
   acquired, verified and copied. This is what would actually close the 9
   claims — and it widens what the transaction authenticates, which is a
   trust-scope decision rather than a bug fix.
3. **Reconsider the conditions the probe runs under.** The affected cases name
   no `browser`, so Node's default `node` condition picks a *server* build.

### 51.4 An open question, stated as a question — answered in § 52.1

(3) raises something this section does not answer and should not pretend to:
the census analysed one resolution of `solid-js/web` and the probe loaded
whichever Node's default conditions selected. Whether those are the same bytes
is not established here. If they are not, a veto is observing a build the
closure was never about — which would be a soundness question and not a
diagnostic one. Establishing it means comparing the census's resolved entry
against the probe's for one of these six rows, and that has not been done.

## 52. § 51's question answered, and its fix (1) replaced

### 52.1 The probe cannot observe a different build than the census read

§ 51.4 asked whether the census and the probe might resolve `solid-js/web`
differently, and called it a possible soundness question. **For the subject it
is already checked.** `verify_reported_resolution` compares the worker's
reported resolution against the artifact case with `names_same_file`, and its
refusal text says so outright:

> the probe worker resolved … to …, but the artifact case this transaction
> certifies names …: the probe ran against a different file than the Type
> Facts witness read

So the probe cannot run against a different build of the package under test.
The question is answered and the answer is sound.

### 52.2 The gap that is real, one level down

`verify_reported_dependency_resolutions` checks a declared dependency with
`path_is_inside(&resolved, root)` — **containment in the authenticated copy,
not file identity.** Which build *within* `solid-js` a probe resolves to is
therefore not pinned.

On these six rows that surfaced as a crash, which is loud. A package whose
`node` and `browser` branches both resolve cleanly would diverge quietly: the
census reads one, the probe vetoes the other, and nothing compares them. That
is recorded here and not fixed; it is a narrower statement than § 51.4's, and
unlike that one it is not hypothetical.

### 52.3 Why § 51.3's fix (1) was not taken

Fix (1) was "make the precheck match the runtime closure", i.e. walk the
analyzed package's declared dependencies transitively. That would **refuse
gates that work today**: what an artifact case imports is a subset of what its
closure declares, and `web.js` needs no `seroval` even though `server.js` does.
It trades a bad diagnostic for lost capability.

What landed achieves the same intent without that trade. A worker throw whose
summary matches `Cannot find package '<name>'` is rewritten into a named
incompletion:

> the probe worker could not resolve `"seroval"`: the private workspace carries
> only this transaction's authenticated dependency closure, and `"seroval"` is
> reached transitively rather than declared by the analyzed package

It cannot over-refuse, because the resolution has already failed by the time it
runs. Every other throw keeps its own text — rewriting one the classifier does
not understand would replace a real diagnosis with a guess — and the controls
in `a_missing_transitive_dependency_is_named_rather_than_echoed` pin that,
along with both quote spellings Node uses and the truncated-frame cases.

The outcome is unchanged: an incomplete veto withholds its candidate. Only the
reason improves, and § 51.3's fixes (2) and (3) remain the ones that would
actually close the nine claims.

## 53. § 51.3's fixes (2) and (3), measured: one is unsound alone, the other is not a thing that can be done (2026-09-11)

> **§ 54.3 corrects this section.** The divergence described here is real, but
> the detection § 53.5 designs cannot reach it: solid-js is exempted by name
> from the dependency edge set both checks key on. Read § 54 before acting on
> § 53.5's ordering claim.

§ 52.3 closed by saying fixes (2) and (3) "remain the ones that would actually
close the nine claims". Reading what each one would do, that sentence is wrong
about both. This section replaces it.

### 53.1 The divergence is not hypothetical and not about `./web`

§ 52.2 recorded the containment gap as a thing that *could* diverge quietly.
It diverges by default, on solid-js' **main** entry, for every recipe that
imports it. `exports["."]` in the installed copy:

| condition branch | target |
| --- | --- |
| `worker` | `dist/server.js` |
| `browser` | `dist/solid.js` (`dist/dev.js` under `development`) |
| `deno` | `dist/server.js` |
| `node` | `dist/server.js` |
| bare `import` | `dist/solid.js` |

The Type Facts private program resolves with `"moduleResolution": "bundler"`
and, in `type_facts.rs`' own words, "no custom conditions" — so it falls past
`worker`, `browser`, `deno` and `node` and lands on `import`: **`dist/solid.js`**.
The probe worker is Node. Node always applies `node`. It lands on
**`dist/server.js`**.

Those are not two spellings of one build. In this copy they are 36,315 and
59,901 bytes: the client runtime and the server runtime.

`verify_reported_dependency_resolutions` passes both, because
`path_is_inside(&resolved, root)` asks only whether the file sits in the
authenticated copy. It does.

### 53.2 So the seroval crash was the symptom, not the defect

`dist/server.js` is the file that reaches `seroval`; `dist/solid.js` does not.
The six rows of § 51 that crash with `Cannot find package 'seroval'` are not
six rows with an incomplete dependency closure. They are six rows where **the
probe was already loading a different build than the census read**, and the
missing package is the only reason anyone found out.

### 53.3 Fix (2) alone converts a loud crash into a silent unsound veto

Fix (2) was "authenticate the dependency closure transitively". Do that and
`seroval` is present, `dist/server.js` loads, the veto runs — and it observes
the server runtime for a claim certified against the client runtime. The
crash goes away and the wrong answer stays, unsigned. On the nine claims this
was supposed to close, the loud failure is the only correct thing currently
happening.

Fix (2) is not wrong; it is out of order.

### 53.4 Fix (3) as stated is not available

Fix (3) was "reconsider the conditions the probe runs under". Node has no
switch that stops it applying `node`. `--conditions` adds to the set; it does
not replace it. The census cannot move to Node's set either — a bundler-
resolution census is what the artifact case was selected under, and
`needs_exact_conditions` exists precisely so a case selected under a custom
condition is not re-chosen by the host's defaults.

Nothing to implement. § 53.5 subsumes the intent.

### 53.5 Two gaps, not one — and both close with data already on hand

**Gap one: on most launches nothing is reported to compare.** The worker
resolves and reports only the specifiers the *session frame* asked for, and
`synthesized_vetoes.rs` writes

~~~json
"dependencySpecifiers": []
~~~

for every synthesized veto — which is all 76 of § 50. So on those launches no
dependency resolution is observed at all, and `path_is_inside` never runs on
anything. The containment check of § 52.2 is not weak on these rows; it is
absent.

That closes without touching the worker. The specifiers do not have to come
from the recipe: `plan.verified_closure.manifest().dependencies` is the
independently replayed set of accepted edges — "what the closure replay proved
this artifact case's modules actually import", in
`require_authenticated_dependency_closure`'s own words. Asking for those on
every launch reports a resolution for every edge the package really has,
regardless of what a recipe declared.

**Gap two: the reference to compare against.** For a dependency placed from a
`graph_dependencies` entry the reference is exact and local — that entry is a
full `CertificationPlan`, and `verified_resolution().runtime_path()` is the
file its own certification selected. The published-graph lane already requires
`edge.artifact_case == identity.artifact_case`, so that plan's case is the case
the parent's census bound. Comparing with `names_same_file`, as
`verify_reported_resolution` already does for the subject, is the whole check.

For a dependency placed from `certification_sources` there is no such
reference, and deliberately: those are declaration-only packages that "never
contribute a semantic claim, a dependency receipt, or a runtime module to this
plan". The census bound no runtime case for them, so containment stays and the
refusal text has to say which of the two checks ran.

That split is also the soundness boundary. A composed dependency's *claims* are
what the parent's closure is built from; running a different build of it makes
the composition describe code that did not run. A declaration-only dependency
contributes no claim, so a build mismatch there is under-specification rather
than a false proof — worth naming, not the same defect.

The ordering that follows: **identity first, then (2)**. With identity in
place, fix (2) is safe, because a transitively authenticated dependency that
resolves to the wrong build refuses by name instead of running.

### 53.6 What it will cost

This tightens a check that six or more rows currently pass. Every recipe that
imports solid-js is a candidate to start refusing — correctly, but a refusal
is still a lost gate. The size of that is a corpus measurement, not a
prediction, and it is the next thing to take.

## 54. Both gaps closed, measured at zero cost — and § 53's headline case is not among them (2026-09-11)

### 54.1 What landed

Two checks, both in `probe_harness.rs`, neither touching the worker.

**Every launch asks about the closure's own edges.** `closure_dependency_requests`
turns `plan.verified_closure.manifest().dependencies` into resolution requests
the workspace holds for the whole transaction, and
`launch_dependency_requests` unions them with whatever the recipe declared.
`ResolutionSubject.dependencies` carries `RequestedDependency { specifier,
package_name }` rather than a bare string, because an edge may name a subpath
and `dependency_roots` is keyed by package.

**A composed dependency's build is pinned, not just its copy.**
`AuthenticatedDependency` gained `certified_entry`, set only for a
`graph_dependencies` entry — a full plan, whose `verified_resolution()` names
the exact runtime file its own certification selected. The workspace
materializes that file and `verify_reported_dependency_resolutions` requires
`names_same_file` against it, the way the subject has always been checked.
A `certification_sources` snapshot is declaration-only, the census bound no
runtime case for it, and containment stays.

Pinned by `every_launch_asks_about_the_closures_edges_and_not_only_the_recipes`
and by three added assertions in
`a_declared_dependency_resolution_outside_the_authenticated_copy_refuses_the_gate`:
the certified file accepted, a *second file inside the same copy* refused, and
containment-only where no certified entry exists.

### 54.2 Measured: three corpus runs, identical on every number

`make ecosystem-regression` at HEAD, with the first check, and with both:

| | packages certified | closures certified | `vetoRunRefused` |
| --- | --- | --- | --- |
| control | 192 | 4,478 | 0 |
| edges asked about | 192 | 4,478 | 0 |
| + build identity | 192 | 4,478 | 0 |

No package moved. Every withheld-reason count is identical across all three —
`noRecipe` 9,667, `censusRefused` 1,859, `vetoThrew` 66, `vetoUnreproducible`
54, `vetoIncomplete` 19. § 53.6 predicted this would cost gates. It costs
none.

### 54.3 And that has a single explanation, which corrects § 53

`module_closure.rs` does not record a dependency edge for the built-in runtime
at all:

~~~rust
[] if solid_dialect::core_runtime_specifier(specifier) => {}
~~~

`solid-js`, `@solidjs/signals` and `@solidjs/web` — and every subpath of each,
`solid-js/web` included — are exempted by name from the opaque frontier, and
nothing is pushed onto `self.dependencies`. So a package that imports solid-js
has **no accepted edge for it**, no artifact case bound for it, and no accepted
contract for it: § 27 records why, and the reason is sound.

The consequence for this section's work is exact. Both new checks key on that
edge set. Neither can see solid-js. **The client-versus-server build divergence
of § 53.1 — the case that motivated all of this — is not covered by what
landed.** The zero in § 54.2 is not evidence that the divergence is absent; it
is evidence that the instrument does not point at it.

What landed is still worth having: the "nothing is reported at all" hole of
§ 53.5 is closed for every ordinary dependency, and a composed dependency's
build is now pinned rather than merely contained. But § 53.5's claim that this
ordering makes fix (2) safe is **wrong as stated**, and § 53.3 stands
unchanged: authenticating `seroval` transitively would still let `server.js`
load with nothing noticing, because solid-js carries no edge for either check
to reach.

### 54.4 Where the real blocker now sits

It is one line, and it is not a defect. The core-runtime exemption removes
solid-js from the dependency system on purpose, because an opaque frontier for
it is a demand that can never be met. Detecting which *build* of it a probe
loaded therefore cannot come from the edge set, and needs a channel that does
not exist yet — the probe reporting what it resolved for a specifier nobody
declared. That is the loader instrumentation § 53 hoped to avoid, and it is
now the only route left to fix (2).

### 54.5 What the measurement does not establish

An A/B that moves nothing cannot tell a check that always agrees from a check
that never runs. The identity check's wiring is pinned by unit tests; its
incidence on this corpus is not established by § 54.2, and given § 54.3 the
likeliest reading is that it fires rarely or not at all. A test that builds a
graph dependency and asserts a certified entry reaches the workspace would
close that, and is not written.

## 55. The seroval class closes, and fix (2) was never the fix (2026-09-11)

### 55.1 The machinery already existed; it was being skipped

§ 54.4 concluded the only route left was instrumenting the probe loader. That
was wrong, and the thing that makes it wrong was three functions away.

ADR 0037's `reproduce_artifact_cases` already owns this problem. It knows what
conditions the pinned interpreter applies (`observe_conditions` asks Node), and
`require_condition_neutral_closure` already walks every `exports` and `imports`
object in the authenticated closure — the built-in runtime included — comparing
the target selected under the requested set against the one selected under the
applied set. `selects_identically` is a real `PACKAGE_TARGET_RESOLVE` walk, not
a name match.

It was never reached for the common case. When the requested set reproduces
every *planned* artifact case, `reproduce_artifact_cases` returns immediately:

~~~rust
match replay_every_artifact_case(plan, graph_dependencies, kinds, &observed, closure) {
    Ok(()) => return Ok(ReproducedConditions { flags: requested.to_vec(), … }),
~~~

The neutrality walk runs only for a condition this verifier *adds*. The
requested set is exempt on the grounds that ADR 0006's per-node replay
dispositions the interpreter's own defaults — and it does, for everything that
has a plan or an accepted edge. The built-in runtime has neither (§ 54.3), so
nothing dispositioned it.

### 55.2 What landed

`require_condition_neutral_unplanned_dependencies` runs on that early-return
path, over exactly the closure entries nothing else covers: no
`certified_entry`, and no accepted edge in any planned closure naming them. For
each, the manifest's `exports` and `imports` must select the same target under
the applied set as under the requested one.

For solid-js they do not, and the existing test says so in its own fixture:
`[import]` selects the client build, `[import, module-sync, node, node-addons]`
selects `server.js`. Pinned now by an added assertion in
`the_neutrality_walk_admits_browser_only_where_it_moves_no_target`.

### 55.3 Measured: nothing lost, the seroval class gone

`make ecosystem-regression`, control versus this change, keyed on `probeId`:

| | control | with the check |
| --- | --- | --- |
| rows moved | — | 9, all in one direction |
| certified closure count | 5,174 | **5,187** |
| `vetoIncomplete` | 19 | **4** |
| details naming `seroval` | 16 | **1** |
| `vetoRunRefused` | 0 | 0 |
| every other reason bucket | — | identical |

`@solid-primitives/cookies`, `@solid-primitives/mutable` and
`@solid-primitives/timer` leave the incomplete-veto set entirely. No row lost a
closure and no row moved to a refusal.

The mechanism is ADR 0037 doing its job once it is asked: refusing the
requested set makes `reproduce_artifact_cases` try `browser`, solid-js lists
`browser` before `node`, both lead to the client build, the walk admits it —
and `server.js`, which is the only file that reaches `seroval`, is never
loaded.

### 55.4 § 51.3's fix (2) is withdrawn, not deferred

Fix (2) was "authenticate the dependency closure transitively", and every
section since has treated it as the thing that would close these claims. It
would have closed them the wrong way: authenticating `seroval` makes
`server.js` *load*, which is the build nothing certifies against. § 53.3 said
that would convert a loud crash into a silent unsound veto, and that reasoning
survives — what it got wrong was assuming the crash had to be resolved by
supplying the missing package. The right resolution was to stop reaching the
file that wants it.

Fix (3) — "reconsider the conditions the probe runs under" — turns out to have
been the correct instinct after all, and § 53.4 was wrong to call it
infeasible. It read the fix as "stop Node applying `node`", which is indeed
impossible. The available form is "add a condition that outranks `node`", which
is what ADR 0037 was built to do.

### 55.5 What is still open

One detail still names `seroval`, and six packages still carry incomplete
vetoes — `@kobalte/core`, `@kobalte/utils`, `@solidjs/element`, `@solidjs/meta`,
`@tanstack/solid-query`, `@tanstack/solid-query-persist-client`. Those are not
diagnosed here; `vetoIncomplete` fell from 19 to 4, and the remainder has not
been read.

§ 54.5's vacuity question is unchanged: the identity check of § 54.1 still has
no established incidence on this corpus.

## 56. The withheld accounting, normalized to distinct claims (2026-09-11)

### 56.1 Details over-count about fivefold

Every measurement in §§ 48–55 counted `withheldClosureDetails` rows. A row is
one (package × artifact case × claim), so a claim withheld across several cases
and several probe rows is counted several times. Ranked that way,
`noRecipe` 9,667 dwarfs everything and `coercion` looks like the second-biggest
census problem. Both readings are artifacts of the unit.

Counted by distinct `semanticClaimId`, from the § 55 run:

| bucket | details | **distinct claims** | share |
| --- | --- | --- | --- |
| `noRecipe` | 9,667 | **1,489** | 75% |
| `censusRefused` | 1,859 | **397** | 20% |
| `vetoIncomplete` | 124 | **99** | 5% |

against 5,187 certified closures. And `noRecipe` splits **reads 1,411 /
returns 78** — the `returns` half was 1,146 details, which is one claim
repeated across 22 packages.

### 56.2 The largest bucket is a decision, not a gap

The 1,411 `reads` claims have no synthesized veto because
`reviewed_observation("reads")` returns `None`, and § 4 is explicit that this
is "a decision rather than an omission". § 3 gives the reason: `reads` is
scoped to a source the export *owns*, a generic observation can only see the
caller's side of that line, and the exact version means reading a minified
reactive graph — `e.ie`, `e.o?.le`, `e.nt` — that moves between builds.

Nothing since has changed that. § 55 moved the probe onto the client build
rather than the server one, which does not make those fields any less minified.
So the honest statement of where certification stands: **71% of the remaining
withheld claims are hand-recipe labor by design.** That is the answer to
"can this be human-less" for the `reads` domain, and it is no.

The `returns` 78 are a different shape — a nonempty enumeration, or an export
whose Type Facts evidence states no call signature, both of which `synthesize`
filters out before `reviewed_observation` is consulted. Which of the two
dominates is not answerable from the report, because a withheld claim does not
appear in the published contract to be inspected.

### 56.3 The actionable lever, sized

`censusRefused` is 397 distinct claims and 99% `creates`; the `reads` census
refuses 20 details in total, so the census that was this document's subject is
not the one that is failing. By family:

| claims | details | pkgs | family |
| --- | --- | --- | --- |
| **122** | 508 | **32** | `invoking-form: property-access-unknown-accessor` |
| 50 | 122 | 12 | other |
| 44 | 80 | 19 | callee outside artifact, not a primitive |
| 41 | 364 | 16 | `invoking-form: coercion` |
| 33 | 128 | 6 | no function-like declaration node |
| 28 | 235 | 13 | binding initializer is not a function literal |
| 21 | 63 | 2 | transcript declaration outside runtime source |
| 17 | 96 | 15 | `invoking-form: iteration-protocol` |
| 15 | 193 | 15 | nested callable parameter is caller-supplied |

`property-access-unknown-accessor` is 31% of census-refused claims and touches
32 packages — the widest spread in the table, and the top entry on either unit.
`coercion` is the one the details view got most wrong: 364 details, 41 claims.

By syntactic shape the 508 details are `PropertyAccessExpression` 216,
`ElementAccessExpression` 146, `SpreadAssignment` 113, `BindingElement` 30.
ADR 0034 already certifies a read off an *unwritten parameter*; what remains is
its boundary — a written parameter, a module-level receiver, a nested callable's
own parameter, a setter — which the refusal text does not distinguish, so
choosing a premise needs a focused run rather than this report.

### 56.4 What this does not say

Which boundary case dominates the 122 is unmeasured, and the § 55.5
stragglers — 99 claims across eight unrelated classes, including a
`@tanstack/custom-condition` the harness config rejects and a `@kobalte/core`
that cannot resolve `solid-js` at all — are still undiagnosed.

## 57. The 122 are one premise, and it is the producer's (2026-09-11)

### 57.1 The refusal now names which premise it wanted

§ 56.3 could not choose a premise to review because every boundary of ADR 0034
arrived as the same sentence. `census_transcript_calls`' `refuse_form` printed
the form kind, the node kind, the location and the reach — and not
`subject_root`, which is the field `census_form_disposition` actually switches
on.

It prints it now, with the reason it was not enough: no derivation offered, a
parameter-family root whose parameter the producer did not identify, a root on
a form that does not read its subject, or a root this form has no reviewed
disposition for. Write position is named too.

Behaviour-neutral by measurement — a `make ecosystem-regression` against § 55's
run moves no row and certifies the same 5,187 closures. Only the text changed.

### 57.2 All 122, one answer

| claims | pkgs | premise |
| --- | --- | --- |
| **122** | **32** | the producer offered no subject derivation |

There is no second row: in every one of the 122, `subject_root` is empty, so no
derivation was emitted at all.

§ 56.3 framed this as choosing which premise to review next. That framing was
wrong in one direction — there is nothing to *review*, because nothing was
offered.

**But the sentence that followed here was wrong in the other.** It read the
empty root as proof that none of the 122 is an ADR 0034 boundary case — "not a
written parameter, not a module-level receiver". That does not follow, and § 58
falsifies it directly: a written parameter is *absent from*
`parameterSubjectRootsLocked`' map, so it produces no root at all rather than a
parameter root that then fails. A written parameter and a subject that was never
a parameter arrive here identically.

### 57.3 Withdrawn: the producer already attempts that derivation

This subsection claimed the producer "never roots an *accessor* form that way"
and that one `own-literal` derivation in `apps/solid-typefacts` would close the
122. **Both are wrong.** It was inferred from the assignment site —
`uncensusedInvokingFormCensusLocked` fills `SubjectRoot` only when
`accessorFormSubjectParameterLocked` returns a subject — without reading what
that function does.

It calls `subjectRootLocked` for exactly this form kind, and `subjectRootLocked`
carries ADR 0044's own-literal leg:

~~~go
if declaration := p.ownLiteralDeclarationLocked(symbol); declaration != nil {
    return &resolvedSubject{derivation: typefacts.SubjectRootOwnLiteral, …}
}
~~~

`ownLiteralDeclarationLocked` is not narrow either: a variable declaration whose
initializer is a data-only literal and which is never assigned, or an object
rest element, at any scope including module level. The verifier's `own-literal`
arm and the producer's derivation are both already in place and already wired to
accessor forms.

So the 122 are subjects that fail *every* leg — parameter family, own-literal,
caller-supplied-callee result, and the ADR 0050 join. `subjectRootLocked`'s own
comment names what is left: "a call result, a module binding, a nested callable's
own parameter, a literal". Which of those, and in what proportion, is not
recorded anywhere.

### 57.4 What is not established

What the 122 subjects actually are. The verifier's refusal says only that no
derivation was offered; the producer knows which leg it fell off and does not
say. Sizing any candidate premise needs the producer to report that — the same
shape of instrumentation § 57.1 added on the verifier side, but across the wire,
so it is a protocol change rather than a message change.

Until then no premise can be proposed honestly. § 57.3's first version proposed
one anyway and was wrong about the code it named.

## 58. Reading the 122 from the registry cache (2026-09-11)

### 58.1 The sources were already on disk

§ 57.4 said sizing needed a producer protocol change. It does not, for a first
pass: `rust/target/registry-cache/v1` holds every package tarball the corpus
installed, content-addressed, 827 of them. The refusal names a path and a byte
span, so the subject expression can be read directly — no network, no wire
field.

**With one real limit.** The report records `installedVersions` for the probed
package and `solid-js` only, not for transitive dependencies, and most of these
forms are *in* transitive dependencies. Matching a tarball by name alone picks
an arbitrary version, and a version that differs by a byte shifts every offset.
So each extraction is checked against the recorded node kind — a
`SpreadAssignment` must start with `...`, an `ElementAccessExpression` must end
in `]`, and so on — and only sites that pass are counted.

| | claims |
| --- | --- |
| aligned and read | **68** |
| misaligned (version drift) | 51 |
| file absent from every cached tarball | 3 |

Everything below is over the 68. It is a sample, not the population, and it is
not a parse — the classification is a regex over the extracted text and a 140-byte
window before it.

### 58.2 The 122 are 75 code sites, and the subjects are simple

The 122 claims collapse to **75 distinct (package, file, span) sites**: the same
code refused in several packages that vendor it, and the two largest sites
account for 24 claims between them.

Subject shape, over the 68:

| claims | subject |
| --- | --- |
| 49 | a plain identifier |
| 16 | a property chain (`window.navigator.userAgentData`) |
| 2 | a call result |
| 1 | an element-access chain |

And the 49 plain identifiers, by what the window shows of their binding:

| claims | binding |
| --- | --- |
| 22 | no binding visible in the window |
| **17** | **a written binding** |
| 10 | parameter-shaped, no write seen |

So these are not exotic expressions. They are ordinary names, and the question
is what the name is bound to — which is a *binding* premise, not an
expression-walking one.

### 58.3 The most repeated site, and a candidate it suggests

The single most repeated shape is this, refused in four packages that each
vendor a copy:

~~~js
if (typeof b === "string") { b = stringStyleToObject(b); }
return { ...a, ...b };
~~~

`b` is a parameter, so it would root — except it is written, and
`parameterSubjectRootsLocked` admits only unwritten parameters. The written-binding
leg that would rescue it refuses on purpose, and says why in the code:

> Only a caller-provenance root propagates through a local binding. An own
> literal roots a direct reference alone (ADR 0044): what one of its properties
> *holds* is an arbitrary value, so `const item = table[key]` names nothing this
> premise can speak for.

That reasoning is about a property *read off* an own literal, and it is right
about that. It does not cover this shape. Here every value written to `b` is
either the caller's own argument or the result of `stringStyleToObject` — a
declaration in this artifact's own runtime source, which is exactly what
`localLiteralResultLocked` already premises for a subject that *is* the call.
The premise exists on both sides; it simply does not propagate through a
binding.

**The candidate, stated as a candidate:** let the local-literal-result premise
join through a written binding, when every value written to it is itself either
caller-rooted or a local-literal result. It is consistent with ADR 0044's stated
boundary, it reaches the most repeated site in the corpus, and it needs no new
observation — only a join the producer does not currently perform.

### 58.4 What this still does not establish

How many of the 122 it would reach. 54 of them could not be read at all, the 22
"no binding visible" are a window artifact rather than a finding, and nothing
here is a parse. A number needs the producer to report which leg it fell off,
which is still the clean route and still a protocol change.

Two claims in § 57 were wrong and are corrected above: § 57.3 (the producer does
attempt `own-literal`) and § 57.2's "not a written parameter" (17 of the 68 are
written bindings, and the most repeated site is a written parameter).

## 59. ADR 0090 landed, and it is small (2026-09-11)

§ 58.3's candidate — letting the local-literal-result premise join through a
written binding — **was not built, and cannot be as stated.**
`LocalLiteralResultPremise` names one call (`Call`, `Callee`, `Allocation`,
`Returns`). The motivating site's binding has two provenances, the caller's
argument and one call's result, and a premise naming only the call would tell
the verifier the subject is always that call's result. The wire has no joined
shape, so that candidate needs a protocol change of its own.

What the same reading *did* surface is a shape that needs no join at the wire
at all. Two of the thirteen written-binding sites were not assignments:

~~~js
export function makeRetrying(fetcher, options = {}) {
  const delay = options.delay;
~~~

`options` is a parameter with a default, which ADR 0034 excludes uniformly and
ADR 0043 relaxed only for a default *naming another rooted parameter*. A
data-only literal default is the other relaxation, and its two arms are each
already reviewed — the caller's argument by ADR 0034, the freshly created
literal by ADR 0044 — and exhaustive. ADR 0090 states it as
`parameter-default-literal`, spelled apart from `parameter-default` because
only one of its arms is the caller's.

Measured, against § 57's run:

| | before | after |
| --- | --- | --- |
| `property-access-unknown-accessor` claims | 122 | **118** |
| census-refused distinct claims | 397 | **395** |
| certified closures | 5,187 | **5,190** |
| rows moved | — | 3, all gaining |
| certified packages | 381 | 381 |

**That is a small gain for a handshake bump, and worth saying plainly.** It is
the first premise this arc chose from a reading of the corpus rather than from
a prediction, and the reading covered 68 of 122 claims (§ 58.1) — so a modest
result is what a two-of-thirteen incidence in an aligned sample should have
predicted. It regresses nothing.

The 118 that remain are undiagnosed, and § 57.4's prerequisite is unchanged:
the producer knows which leg each subject fell off and does not report it. That
is now the third time a premise has been sized by reading code rather than by
measuring, and the second time the answer was "smaller than it looked".

## 60. The instrument, built — and the 118 classified in one run (2026-09-11)

### 60.1 Three sections said this was the prerequisite; it is built now

§ 54.4, § 57.4 and § 59 each ended by naming the same missing thing: the
producer knows which leg a subject's rooting fell off and does not say it
across the wire. Handshake protocol 48 adds `subjectRootRefusal` on an
uncensused invoking form — stated only where `subjectRoot` is empty, over a
closed vocabulary whose default `unclassified-subject` means *no information*.

**It is a diagnostic and never a premise.** Nothing is admitted on its account;
`refuse_form` prints it and the corpus report carries it. The producer computes
it in `subjectRootRefusalLocked`, which re-walks beside `subjectRootLocked`
rather than being threaded through it — a diagnostic concern inside the premise
is a thing a later edit can make decide something. The cost of walking beside is
drift, and the one invariant that matters is pinned instead: an accessor form
states a root **or** a reason, never both and never neither.

### 60.2 All 118, on the first run

| claims | pkgs | leg |
| --- | --- | --- |
| 41 | 19 | `local-binding` |
| 36 | **8** | `module-binding` |
| **30** | **18** | `written-parameter` |
| 5 | 3 | `call-result` |
| 3 | 3 | *(not stated)* |
| 3 | 3 | `not-a-reference` |

Behaviour-neutral by measurement: 381 packages and 5,190 closures, every
withheld-reason count identical, **zero rows moved**.

### 60.3 What it says, and what § 58 got right

`written-parameter` is 30 claims across 18 packages — the `{ ...a, ...b }`
shape of § 58.3, now counted rather than sampled. § 58's archaeology found 17
written bindings in a 68-claim aligned sample and called it the largest
identifiable shape; the population says 30 written parameters plus a share of
the 41 local bindings. The sample was directionally right, at a quarter of the
effort this took and with none of the confidence.

The two larger legs are new information:

- **`local-binding`, 41 claims across 19 packages** — the widest. A binding no
  propagation rule roots, typically initialized from a call this build does not
  premise.
- **`module-binding`, 36 claims across 8 packages** — nearly as many claims from
  less than half the packages, so it is concentrated: a few modules read a
  module-scope value many times. ADR 0044 already roots a module-scope
  *data-only literal*; these are the ones that are something else.

### 60.4 The gap in the instrument

Three claims state no reason at all. The producer computes one only when
`accessorFormSubjectExpression` yields a subject, and these forms yield none —
so the vocabulary is complete for the forms it reaches and silent for the rest.
That is the honest direction (silence, not a guess) and it is 2.5% of the class.

## 61. ADR 0091, and the instrument earning its keep twice (2026-09-11)

§ 60.2 put `written-parameter` at 30 claims across 18 packages — the widest leg
with a premise already available, because ADR 0050 had made exactly this
argument for a *local* binding and left it standing on parameters. ADR 0091
lifts it: a written parameter roots at its own slot when every value assigned
to it is rooted at that same slot.

Measured against § 60's run:

| | before | after |
| --- | --- | --- |
| accessor-class claims | 118 | **114** |
| `written-parameter` | 30 | **26** |
| certified closures | 5,190 | **5,192** |
| every other leg | — | unchanged |
| rows moved | — | 1, gaining |

### 61.1 The negative result is the useful one

Four of thirty. **Twenty-six written parameters have a value the join cannot
call the caller's**, and § 58.3 already read what they are:

~~~js
if (typeof b === "string") { b = stringStyleToObject(b); }
~~~

`stringStyleToObject(b)` is a call into this artifact's own runtime source. The
premise for that value exists — `localLiteralResultLocked` — and it does not
reach here, because `LocalLiteralResultPremise` names one call and this binding
has two provenances. That is the same wire-shape obstacle § 59 recorded, now
with a number on it: **26 claims, not a guess.**

### 61.2 What two ADRs in a row have shown

ADR 0090 moved 4 claims, ADR 0091 moved 4 more. The accessor class has gone
122 → 114 across two handshake bumps. That is a modest return, and the honest
reading is not that the premises were wrong but that **the class was never one
problem.** Its legs are five different questions, and the two that were
answerable with existing machinery were the two smallest:

| claims | leg | what it needs |
| --- | --- | --- |
| 41 | `local-binding` | unexamined |
| 36 | `module-binding` | unexamined; concentrated in 8 packages |
| 26 | `written-parameter` | a joined local-literal-result premise — a wire shape |
| 5 | `call-result` | ADR 0048's boundary, deliberate |
| 3 | *(not stated)* | an instrument gap (§ 60.4) |

The instrument is what makes that table sayable. Before protocol 48 the same
114 claims read as one undifferentiated refusal, and the only way to choose was
to extract tarballs and run regexes over byte spans — which is how § 58 arrived
at a directionally-right answer it could not check. Two runs have now each
ended with a measured target list instead of a reading, and one of them ended
with a measured *dead end*, which is the more valuable of the two.

### 61.3 One hole this closed on the way

The join asks two different predicates about the same binding:
`parameterIsWrittenLocked` says it is written, and the source collector
enumerates the writes. If they ever disagree — written, but no source seen —
admitting would root a binding whose every write went unenumerated. An empty
source list therefore refuses rather than reading as "nothing is assigned", and
`writtenParameterDestructured` pins it: a destructuring assignment the
collector refuses, on a parameter the write predicate calls written.

## 62. The two unexamined legs, split — and a bug in the instrument (2026-09-11)

§ 61.2 left `local-binding` (41) and `module-binding` (36) as 77 of the 114
accessor-class claims with nobody having looked at either. Protocol 50 splits
them the way protocol 48 split the class itself: a binding leg now names which
of ADR 0044's conditions it failed — assigned somewhere, uninitialized,
initialized from a call, initialized from a literal that is not data-only —
separately per scope, and a parameter of a callable *other* than the censused
one is `nested-parameter` rather than a local binding.

Still diagnostic, still never a premise. Behaviour-neutral by measurement: 381
packages, 5,192 closures, zero rows moved.

### 62.1 The first run was wrong, and said so by omission

The refined run put `module-binding-uninitialized` at 35 claims and showed
**no `imported-binding` row at all** — zero imports across 114 claims, in
bundled code that reads `sharedConfig` and `isServer` from solid-js. That is
not a plausible measurement.

The cause is one line. `subjectRootRefusalLocked` asked
`canonicalSymbol(...).Flags & SymbolFlagsAlias`, and `canonicalSymbol` *walks
the alias chain to the original declaration* — so by the time the question was
asked the import had become whatever it resolves to, and the answer was always
no. A `declare const sharedConfig` in solid-js' typings then classified as a
module-scope binding with no initializer, which is true of the declaration and
says nothing about the code.

Asked of the raw symbol instead, nine claims move to `imported-binding`. The
same pass adds `ambient-declaration` for a binding declared outside the
artifact's own runtime source, because no premise in this family can reach one:
they all turn on what the artifact's own code assigned, and a typings file
assigns nothing. It does not fire on this corpus, which is itself worth
knowing — the remaining uninitialized module bindings are real code.

### 62.2 The corrected table

| claims | pkgs | leg |
| --- | --- | --- |
| 26 | 18 | `written-parameter` — needs a joined premise on the wire (§ 61.1) |
| **26** | **2** | `module-binding-uninitialized` |
| 22 | 11 | `nested-parameter` |
| 9 | 8 | `imported-binding` |
| 9 | 7 | `local-binding` |
| 6 | 3 | `local-binding-from-call` |
| 5 | 3 | `call-result` — ADR 0048's boundary, deliberate |
| 4 | 6 | `local-binding-written` |
| 3 | 3 | *(not stated)* — the § 60.4 instrument gap |
| 3 | 3 | `not-a-reference` |
| 1 | 1 | `module-binding` |

### 62.3 The striking one

**26 claims from two packages**, and 24 of them from `@kobalte/utils` alone.
A module-scope binding, declared with no initializer, in the artifact's own
runtime source, never assigned anywhere the producer can see — which is close
to a contradiction, since such a binding holds `undefined` and a property read
of it throws. Either the assignment is in a shape `symbolIsAssignedLocked` does
not count, or the declaration is not what it appears to be.

That is now the cheapest thing on the board: one package, one shape, 24 claims,
and the question is answerable by reading a single file rather than by
proposing a premise. It is *not* a premise candidate yet — it is a thing that
does not add up, and § 58 and § 62.1 are both reminders of what happens when a
number that does not add up gets built on.

## 63. The 26 that did not add up were `window` (2026-09-11)

§ 62.3 flagged `module-binding-uninitialized` — 26 claims, 2 packages, 24 of
them `@kobalte/utils` — as a thing that does not add up rather than a premise
candidate, because a module binding that is never assigned holds `undefined`
and a property read of it throws. Twenty-four of those in shipped code is close
to a contradiction.

Every one of them is the same expression. The byte spans the refusals name —
`@kobalte/utils/src/platform.ts:753..793`, `dist/index.js:4198..4238`, and the
two in `@kobalte/core/dist/i18n` — all read:

~~~ts
window.navigator.userAgentData?.platform
~~~

The subject root is `window`, declared in `lib.dom.d.ts` as
`declare var window: Window & typeof globalThis`. A module-scope variable
declaration with no initializer, assigned nowhere the producer can see. The
classification was accurate about the declaration and said nothing whatever
about the code.

### 63.1 One missing conjunct

`subjectRootRefusalLocked` files a declaration as ambient when it is outside the
artifact's own runtime source, and asked that as `!formIsRuntimeSourceFile`.
That predicate answers whether a file is in the **program** — and the program
loads the default libraries, so `lib.dom.d.ts` is in it. Every other caller of
the predicate pairs it with `IsDeclarationFile`:

~~~go
if sourceFile == nil || sourceFile.IsDeclarationFile || !p.formIsRuntimeSourceFile(sourceFile) {
~~~

The refusal classifier had only the second half. Adding the first moves all 26
to `ambient-declaration`, which is where § 62.1 put this family when it
introduced the leg and observed — correctly, and for the wrong reason — that it
"does not fire on this corpus".

### 63.2 Measured

| leg | § 62.2 | now |
| --- | --- | --- |
| `module-binding-uninitialized` | 26 (2 pkgs) | **0** |
| `ambient-declaration` | 0 | **26 (2 pkgs)** |

Certification cannot move: `subject_root_refusal` has exactly one consumer in
the Rust client, the message text inside `refuse_form` in `type_facts.rs`,
which is already on the refusing path. The ecosystem regression gate passed
against its certification thresholds on the same run.

The other legs' claim counts shift by a few against § 62.2's table, and that is
a difference between two ad-hoc queries rather than movement. § 62.2's package
column was itself inconsistent — it reports `local-binding-written` as 4 claims
across 6 packages, which cannot be true — so its counts are not a baseline. The
two legs above are the ones this run establishes, and it establishes them
against each other in a single query.

### 63.3 What this says about the instrument

Three of the arc's classifications have now been wrong, and all three the same
way: § 62.1's alias walk, this file's ambient conjunct, and § 57.3 before them.
Each was a predicate that answered a *nearby* question convincingly. None was
caught by a test, because until now **no test asserted a refusal reason at
all** — protocol 50 shipped an eleven-way classifier whose only check was that
a form states a root or a refusal but never both.

`TestSubjectRootRefusalNamesTheBlockingShape` closes that. It pins six shapes,
including the lib global that caused this, and it includes the genuine
uninitialized module binding so the correction cannot trade one wrong number
for another. It reproduced this bug from a four-line fixture in 0.5 s; the
measurement that surfaced it took a seven-minute ecosystem run and a day of not
believing it.

### 63.4 What is left

`ambient-declaration` is a dead end by construction, and that is the finding:
these 26 claims are **not reachable by any premise in this family**. No
assignment exists in the artifact's own code to observe, so ADR 0044 and its
descendants have nothing to look at. They are correctly withheld, and the
accessor class's addressable size drops from 114 to 88 with no work done.

The board after this:

| claims | pkgs | leg | status |
| --- | --- | --- | --- |
| 26 | 11 | `written-parameter` | § 61.1's negative result — needs a joined premise |
| 26 | 2 | `ambient-declaration` | **unreachable by construction** |
| 22 | 10 | `nested-parameter` | unexamined |
| 10 | 4 | `local-binding-from-call` | ADR 0044 boundary |
| 9 | 5 | `imported-binding` | unexamined |
| 9 | 4 | `local-binding` | unexamined |
| 5 | 3 | `call-result` | ADR 0048's boundary, deliberate |
| 4 | 3 | `local-binding-written` | ADR 0050 boundary |
| 3 | 3 | *(not stated)* | the § 60.4 instrument gap |
| 3 | 3 | `not-a-reference` | |
| 1 | 1 | `module-binding` | |

`nested-parameter` at 22 across 10 packages is now the largest unexamined leg.

## 64. A corpus recipe pass, what it measured, and two harness defects it found (2026-09-11)

§ 47.4 left one thing open: "the corpus-wide fraction. 53% is one artifact case
of one package, and this document has twice extrapolated a number from a sample
and been wrong." § 48 removed the accounting gaps that made such a pass
unreadable. This is the attempt, and it is a partial one — it returns a second
sample rather than the corpus number, and the reasons are worth more than the
number.

### 64.1 Why the backlog is worth sizing at all

Measured on the same run, per-node rows on both sides:

| domain | closed | withheld | rate | veto |
| --- | --- | --- | --- | --- |
| `creates` | 10,248 | 3,880 | 72.5% | synthesized |
| `returns` | 570 | 2,328 | 19.7% | synthesized |
| `reads` | 60 | 17,082 | **0.35%** | none |

**1,411 of 1,413 withheld `reads` claims are withheld for one reason —
`no recipe in corpus`.** That is 71% of the entire withheld backlog blocked on
hand-authoring rather than on any premise, ADR, or producer capability, and it
dwarfs everything §§ 51–63 worked on.

The same table answers a question this arc had not asked directly: **human-less
contracts already exist and are the overwhelming majority.** 10,248 of 10,878
certified closures are `creates`, closed by a veto synthesized from Type Facts
with nobody in the loop. What is not human-less is `reads`, and § 6 argues that
is a proof rather than a gap.

### 64.2 `returns` is the headroom nobody has looked at

`returns` has a synthesized veto and still closes 19.7%. 2,292 of its 2,328
withheld rows say `no recipe in corpus` anyway, because `synthesize` admits a
`returns` candidate only when its enumeration is **empty** — the reviewed
observation ("any call whose result is not undefined") would falsify a contract
that permits a value. The code says what that status is:

> Nonempty enumerations require a claim-addressed recipe **until their
> observation is reviewed here**.

A pending review, not an impossibility proof. It is the one place left where a
single piece of design work could move thousands of claims with no per-package
authorship, and this arc has never examined it.

### 64.3 Attribution: the tail of § 48.1

A recipe has to name the package it imports, and the benchmark's withheld rows
did not carry one. They span several packages: `@corvu-next/accordion`'s rows
include `isButton` and `afterPaint`, which occur **zero times anywhere in that
package** — they are `@corvu-next/utils`'.

`report_withheld_closures` does attach `node: {package, version, digest}`, but
only where a node identity exists, which is the dependency-graph lane. On that
lane 51,908 of 53,492 rows are attributed. **1,030 of 1,486 `reads` recipe gaps
(69%) can therefore be addressed; the other 456 cannot be**, and that residue is
§ 48.1's tail: some certification paths still report a withheld closure with no
node. Attributing them by probe id was rejected — the accordion case above is
exactly the shape that would corrupt.

### 64.4 The measurement

1,030 throwing scaffolds into a scratch corpus, then certify against it. Of the
three probes chosen for the second pass, **only one produced any classification
at all**:

| | claims | share |
| --- | --- | --- |
| `census refused` — no recipe can serve them | **102** | **59%** |
| scaffold threw — a human could finish these | **71** | 41% |

All 173 are `@solid-primitives/url`. `corvu` and `motion-solidjs` contributed
nothing: both fail on a `dependency-contract-obligation` before their gates run.
They failed **identically in the first pass**, so the scaffolds did not cause it
— but with recipes present they emit no withheld `reads` rows at all (170 and
239 before, zero after), and that difference is observed here without a
mechanism, deliberately.

So: 59% unserviceable, against § 47.2's 53% on a different package measured a
different way. Two independent samples agreeing that **half to three-fifths of
the reads backlog is work nobody should ever do**. It is still not § 47.4's
corpus fraction, and this section does not claim it is.

### 64.5 The cost, and why it recurs

Applying 41% serviceable to 1,486 claims gives ~610 recipes. The only measured
authoring data point is `d012597a` — five recipes, 215 lines, per-export
reasoning about which reactive sources a closure owns — so 1–3 h each, and
**600–1,800 hours, 4 to 11 person-months.**

It also recurs. Claim ids are content digests, so a release orphans its
recipes: `clamp` in `@kobalte/utils` carries three distinct claim ids and three
separate modules across two versions. The recipe *bodies* barely differ — the
0.9.2 and alpha `clamp` recipes are the same logic, one sample apart — so a
version bump needs **re-pointing, not re-reasoning**. Nothing automates that
re-point today, and the mistake it invites (a module and its manifest entry
disagreeing on `expectedEvent.marker`) makes an unmatchable gate *pass*, which
`ecosystem-probe-recipes.test.mjs` does not catch: it asserts only that the
marker is a non-empty string.

And a written recipe does not close a claim. `d012597a` wrote five and closed
none. The two packages that would have tested that here fail one layer deeper.

### 64.6 Two harness defects, both found by running it

**The pool crashed on cleanup.** `cli-worker-pool.mjs` tolerated `ESRCH` from
its process-group kill and rethrew everything else — but it killed from the
child's own `exit` handler, after the child was gone, where macOS answers
`EPERM`. The throw escaped an EventEmitter handler and killed a 9½-minute
corpus run with no report written. The exit-handler path is now best-effort; the
deadline kill still rethrows, because failing to signal a *live* group is real.

**The pool leaked whole process trees, which was worse.** Fixing the crash did
not fix this, and the difference was measured rather than assumed: a killed run
left **eight orphaned workers alive for 37 minutes**, two of their children
pinned above 200% CPU, and the run that replaced it was starved to the point of
looking like a hang. The worker already exited on stdin EOF — the signal that
arrives whether the runner exits, throws, or is killed — but it waited for the
in-flight request first, and a certification runs for minutes nobody will read.

It now exits immediately on EOF, and signals its own process group so the native
children go with it; the pool tells it, through
`SOLID_CHECKER_CLI_WORKER_GROUP_LEADER`, that `detached` made it the leader,
because Node exposes no `getpgid` for it to check. Verified by experiment rather
than by reading: a runner killed with a certification 51 s into its work left
**zero** workers and zero checker processes, where the same scenario previously
left eight of each. The pool's own test suite dropped from 32.2 s to 3.8 s — it
had been waiting on lingering workers too.

### 64.7 What this says to do

Not industrialise `reads`. 4–11 person-months per snapshot, recurring per
release, for a domain at 0.35%, where half the work is provably wasted and a
finished recipe moves the blocker rather than removing it. § 64.2's `returns`
observation is the better buy, and the re-point tooling in § 64.5 — matching
orphaned recipes by (package, export, artifact case, domain), plus the marker
agreement check nothing performs — is days of work that would remove the
treadmill and close a soundness hole at the same time.

## 65. Every `reads` recipe ever written is vacuous as a veto (2026-09-11)

§ 64.5 named a soundness hole the corpus README had already flagged and nothing
checked: a recipe module and its manifest entry must agree on the event, and a
disagreement fails **open**.

`event_matches` requires marker *and* class to be equal. A run that completes
without a matching event is a `CleanNonObservation` — "a complete, isolated,
deterministic, scenario-satisfying execution did not observe the contradiction
the recipe was written to provoke" — and that verdict **satisfies** the
mandatory gate. So a recipe mistyped in one character does not refuse and does
not warn. It silently stops being able to veto, and the closure certifies on the
census alone.

`ecosystem-probe-recipes.test.mjs` now checks the agreement statically. Writing
that check was three lines of work. Running it was the interesting part.

### 65.1 What it found

| recipes | can emit their expected event |
| --- | --- |
| 14 `creates` / `returns` | **all of them** |
| 5 `reads` | **none of them** |

Not one `reads` recipe in the corpus contains a code path that emits its
expected marker. `clamp` says so in its own comment — *"No emit is the point:
nothing contradicted the closure"* — and `access`'s manifest entry already said
it: *"the read it counts is the caller's under ADR 0034 and is deliberately not
emitted."* `arrayEquals` and `compare` are the pair `d012597a` kept precisely
because they can never fire, so that the census refusal underneath is not masked
by `no recipe in corpus`.

This is not unsound. A vacuous recipe cannot establish closure — that is the
implementation census's job (ADR 0008) — it can only fail to veto. But it means
the mandatory `reads` veto, on every claim anyone has ever written a recipe for,
is a gate that passes by construction.

### 65.2 Why the split is exactly where § 6 said it would be

§ 6 argued that an unenumerated read is a read of a source the export **owns**,
and that this is the half no synthesized veto can instrument. That was a design
claim. The five hand-written recipes are the experiment, and they agree with it:
a human sat down to write the observation and could not construct one either.
`access` gets closest — it installs its own getter and asserts the getter ran —
and then deliberately does not emit, because the read it can see is the
caller's.

So the boundary is not about synthesis. It is about `reads`.

### 65.3 What it does to § 64's arithmetic

§ 64.4 put 41% of the reads backlog at "a human could finish these". That number
is an upper bound in a way § 64 did not know: the five already finished by hand
are *all* in the can-never-veto class. Finishing the other 71 the way these five
were finished would produce 71 more gates that pass by construction.

The estimate in § 64.5 — 4 to 11 person-months, recurring per release — was
already an argument against industrialising `reads`. This makes it a stronger
one: the deliverable that money buys may be a corpus of vetoes that cannot veto.

### 65.4 The convention, and its two directions

A module that deliberately cannot emit now declares it with a coverage
limitation opening `NEVER EMITS:`, and all five `reads` recipes carry one
stating their own reason. The waiver is checked **both** ways: a module that
declares it and *does* emit fails too, because a stale declaration would waive
the marker check for a recipe whose marker later drifts.

It is prose rather than a manifest field on purpose — Rust mirrors the field set
with `deny_unknown_fields`, so a new key is a wire change, and the distinction
being recorded is a claim about the author's intent that only the author can
make.

### 65.5 What is still unchecked

Whether an emit is *reached*. A recipe whose emit sits behind a condition that
never holds at run time is the same failure with the same open verdict, and no
static check finds it — only a run that contradicts something does. That is the
residue, and it is named here rather than left implied.

## 66. The largest unexamined `creates` family is § 27's unpriced bill (2026-09-11)

With `reads` decided against (§§ 64–65), the question became which `creates`
family to work next. That needed the breakdown nobody had taken. Of 514 withheld
`creates` claims, by distinct claim rather than by row:

| claims | pkgs | family |
| --- | --- | --- |
| 132 | 26 | `property-access-unknown-accessor` — the accessor class of §§ 51–63 |
| **65** | **14** | **resolved callee is not a definition — unexamined** |
| 61 | 5 | veto did not complete (synthesized gate) |
| 44 | 10 | coercion form — unexamined |
| 40 | 10 | no function-like declaration node |
| 35 | 11 | named declaration refusal |
| 24 | 3 | domain-exhaustiveness |
| 21 | 7 | iteration-protocol |
| 20 | 1 | transcript at depth 0 |
| 18 | 7 | call through a binding |
| …13 smaller | | jsx-element, stdlib member, unresolved callee, instanceof, … |

Two things fall out of the table before any investigation. The accessor class
was the **largest** family all along, so §§ 51–63 chose their target correctly
even though the yield per ADR was three to five claims. And `returns`, which
§ 64.2 called the best remaining buy, is **29 claims** — that recommendation was
made on row counts, which over-count `returns` by 94× against 15× for `creates`.
Rows are not claims; § 56 normalized this once and it was forgotten here.

### 66.1 All 65 resolve to a `.d.ts`

| claims | declared in | callees |
| --- | --- | --- |
| 54 | `solid-js` [.d.ts] | createComponent, createMemo, createSignal, getOwner, onCleanup |
| 7 | `@solidjs/signals` [.d.ts] | NotReadyError, getOwner, onCleanup |
| 3 | `@solid-primitives/utils` [.d.ts] | withArrayCopy |
| 1 | `@solidjs/web` [.d.ts] | createComponent |

Every one. And the callees are core Solid vocabulary — a dependent package calls
`createSignal`, the declaration resolves to
`solid-js/types/reactive/signal.d.ts`, and the census refuses.

A first reading of this data was wrong and is worth recording: a greedy `.*` in
the grouping query captured the *call site* rather than the declaration, which
made the primitives look as though they were declared inside the consuming
`@solid-primitives/*` packages, and suggested a story about bundlers inlining
solid-js. Checking one package's dist falsified it — `@solid-primitives/memo`
imports `createSignal` from solid-js rather than inlining it — and reading a
single refusal in full gave the real shape.

### 66.2 Why the dialect-axiom tier declines, and why that is § 27

`census_dialect_axiom_for_callee` is the tier that would admit a call to a known
primitive. It requires the declaration's file to strip against a snapshot source
root whose `root.dependency` is true, and then to be `read` out of that
authenticated archive. The core runtime has neither: `module_closure.rs` clears
the frontier for `solid-js`, `@solidjs/signals` and `@solidjs/web` precisely
because they "have no package contract *by design*". No contract, no accepted
edge, no authenticated archive — so the axiom tier cannot reach them, and
`census_dependency_claim` cannot either.

§ 27's own comment anticipates exactly this, in the same breath as the exemption:

> every proposed closure is re-proved by the certifier's implementation census,
> which walks the archive's own implementation and **refuses any form it cannot
> census**.

So these 62 claims are not a defect. They are the **price** of § 27 — which
bought something much larger, since an opaque frontier for Solid "opened every
domain of every export of any package that imports Solid at all". The trade was
correct. What it was not, until now, is *measured*: **62 claims across 14
packages** is what the exemption costs at current corpus scale.

### 66.3 The open question, stated rather than answered

Recovering them means giving the core runtime an authenticated identity the
axiom tier can read — **without** giving it a package contract, which § 27
withholds deliberately and `GenerationScope::DialectDefiningPackage` withholds
again. That is a trust-model question (what authenticates the runtime, and under
whose authority a primitive's behaviour becomes an axiom), not a premise
question, and it is not decided here.

It is, however, the best-shaped target left in `creates`: one root cause, 62
claims, 14 packages, five callee names, and no per-package work — against the
accessor class's three-to-five claims per ADR.

### 66.4 The second-best, and it needs no trust-model argument

61 claims fail at the gate rather than the census, and **they have already
passed the census** — a synthesized veto exists and the only thing between them
and a verdict is a probe that cannot run. Two causes, two packages:

- **37** (`@kobalte/utils`): `Stripping types is currently unsupported for files
  under node_modules` — the package ships `.ts`, and Node refuses to strip types
  there;
- **16** (`@tanstack/query-core`): `export condition "@tanstack/custom-condition"
  is not a plain condition name, so it cannot be given to the pinned
  interpreter`.

Both are harness capability rather than semantics — packages that ship
TypeScript, and packages with custom export conditions, are general shapes. Both
nonetheless touch the pinned interpreter, which is part of the probe's
authenticity, so neither is plumbing: "transform first" and "pass an arbitrary
condition" are decisions about what is executed versus what ships. Whether
either is achievable within the pins is unestablished and should be checked
before it is planned.

## 67. 37 of § 66.4's gate failures are impossible, not pending (2026-09-11)

§ 66.4 listed the 61 gate-incomplete `creates` claims as the target needing no
trust-model argument, with 37 of them blocked by Node's refusal to strip types
under `node_modules`. Checking that before planning it, as § 66.4 said to:

Every one of the 37 is `@kobalte/utils/src/*.ts` — 62 rows on
`src/index.ts`, 8 on `src/number.ts`, 2 each on `src/get-scroll-parent.ts` and
`src/polygon.ts`. The package publishes `"./src/*": "./src/*"`, so that artifact
case's entry point *is* raw TypeScript. Its `.` entry resolves to
`dist/index.js` and certifies fine; this is the separate case that tests what
the `src/*` subpath does.

The pinned interpreter cannot load it, and no flag changes that. Measured
directly on Node v24.11.1 with a two-file reproduction:

~~~
Error [ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING]: Stripping types is
currently unsupported for files under node_modules
    at stripTypeScriptModuleTypes (node:internal/modules/typescript:183:11)
~~~

`--experimental-transform-types` throws the same error from the same line. The
refusal is unconditional in `node:internal/modules/typescript`.

So these are not pending work. Recovering them needs a loader that transforms
the bytes before Node sees them — which changes what the probe *executes* away
from what the package *ships*, and the whole point of the pinned interpreter is
that those are the same thing. That is a trust-model change with a poor trade:
a probe of transformed source proves something about the transform.

What is worth fixing is the accounting. They are recorded as
`veto did not complete`, which reads as addressable, and § 66.4 duly listed them
as the best-shaped target left. They are better described the way a closure
hazard is: an artifact case the pinned interpreter **cannot execute by
construction**, for which no recipe, premise or gate exists that would change
the answer. Until that is done, any count of "gate-incomplete" claims overstates
the work available by 37.

That leaves § 66.4's genuinely addressable half at **16 claims**
(`@tanstack/query-core`, a custom export condition the pinned interpreter
refuses), and moves the § 66.3 core-runtime question — 62 claims, one root
cause — further ahead of everything else in `creates`.

## 68. The coercion premise is 13 claims, measured before it was written (2026-09-11)

§ 66 put `coercion` at 44 claims across 10 packages and called it the best
unblocked semantic target: the largest family with no trust-model argument in
the way, and one with a precedent. `census_form_shape_reads_the_subject` already
admits two shapes on exactly this reasoning — ADR 0047's `instanceof`, whose
subject is the constructor whose `Symbol.hasInstance` may run, and ADR 0042's
iteration protocol, where "the code that runs is the caller's exactly as a
getter's is". A coercing operator applies ToPrimitive to its operands, reaching
`Symbol.toPrimitive`, `valueOf` and `toString`, so the same argument should
reach it.

With one difference that decides the premise: a coercion has **operands**, not a
receiver. One operand this program built is one object whose `valueOf` this
program owns, and the form is then this program's act however the others rooted.
So the premise can only admit a form where *every* operand is the caller's —
structurally ADR 0091's guard, and already the shape `coercionPremiseLocked`
uses for its own question.

Protocol 51 states it as a diagnostic: `coercionSubjectRoot`, the derivation
every operand agreed on, or `coercionSubjectRootRefusal`, why they did not.
Measured on 418 probes:

| claims | pkgs | leg |
| --- | --- | --- |
| **13** | **5** | every coerced operand rooted at `parameter` |
| 8 | 3 | `nested-parameter` |
| 7 | 3 | `written-parameter` |
| 4 | 2 | `local-binding-written` |
| 3 | 1 | `module-binding` |
| 2 | 1 | `local-binding` |
| 1 each | | `call-result`, `local-binding-from-call`, `local-binding-uninitialized`, `mixed-operand-roots` |

**Thirteen, not forty-four.** The family's size was never the premise's size, and
this is the first time in the arc that gap was measured *before* an ADR was
written rather than discovered after. It is still the largest measured premise
available — ADR 0090 moved 3 claims and ADR 0091 moved 2 — but it is a quarter
of what § 66 implied, and a plan built on 44 would have been wrong.

### 68.1 The instrument's own bug, caught before the run

The first version refused any operand that rooted at nothing, literals included.
`x + 1` is the commonest coercion there is, and that version would have reported
this whole family as `not-a-reference` — a number that says more about the
instrument than the corpus, which is exactly § 62.1's failure and § 58's before
it.

A provably primitive operand has nothing for ToPrimitive to reach, so it is now
skipped rather than refused, using `mayBeObjectTypedLocked` — the predicate the
classifier itself uses to decide whether to record the form at all. The
difference between those two readings is the difference between 13 and roughly
zero.

This is the first time an instrument bug in this family was caught before the
measurement rather than by disbelieving the measurement afterwards.

### 68.2 A test, this time first

Protocol 50 shipped an eleven-way classifier whose only assertion was that a
form states a root or a refusal and never both, and § 62.1 and § 63 each found it
wrong. `TestCoercionSubjectRootNamesWhatEveryOperandAgreedOn` pins six shapes
before the run, including the literal case above.

One of its six expectations was wrong, and the code was right:
`writtenParameterCoercion` was predicted to report `mixed-operand-roots` on the
theory that a written parameter roots at its assigned literal. ADR 0091 refuses
such a parameter outright, so it roots at nothing and the answer is
`written-parameter` — the more informative leg. The expectation was corrected,
not the classifier.

### 68.3 What it recommends

Write it. 13 claims is four to six times the arc's recent rate, the argument is
already made twice in the same function for `instanceof` and iteration, the
"every operand" guard has a precedent in ADR 0091, and the instrument that
sizes it is now committed. But it is 13, and this section says so in the
sentence a plan would quote.

The two legs beside it — `written-parameter` (7) and `nested-parameter` (8) —
are the same two legs that top the accessor class (§ 62.2), which suggests one
premise on each would pay twice. Neither is examined here.

## 69. What recovering the 62 actually requires — § 66.3 corrected (2026-09-12)

§ 66.3 said the 62 core-runtime claims need "an authenticated identity the axiom
tier can read, **without** a package contract", and called it a trust-model
question. That was half the answer and the less important half. Reading the tier
to its end, and the dialect audit behind it, gives two **independent** blockers,
and the second is the one that decides the work.

### 69.1 Blocker one: no source root, so the tier never reaches its table

`census_dialect_axiom_for_callee` strips the declaration's file against the
snapshot source roots and requires `root.dependency`. Those roots come from
`snapshot_source_roots`, which builds one per *certification plan* — the owner's
and its dependencies'. § 27 gives the core runtime no contract and no accepted
edge, so no plan exists for it, no root exists, and the tier returns `None`
before it consults anything. This is the blocker § 66.3 named.

### 69.2 Blocker two: the table is a **negative** one, and the rows are absent

This is what § 66.3 missed. The tier does not admit "a call to a known
primitive". It admits a call the dialect **denies** in that domain —
`primitive_performs_no_operation`, over `NEGATIVE_ROWS`, keyed
`(package, export, domain)` and gated on the audited archive identity.

Counting the Solid 2 rows: `@solidjs/signals` carries 16 `creates` denials
including `createSignal`, `createMemo`, `onCleanup` and `getOwner`;
`@solidjs/web` carries 3 (`clientOnly`, `httpHeader`, `httpStatus`); **`solid-js`
carries 9, and not one of them is any of our five callees.**

The audit says why, and it is not an oversight. `solid-js/types/index.d.ts:8`
re-**declares** `createSignal` from `./client/hydration.js` instead of
re-exporting `@solidjs/signals`', so a `solid-js` import does not inherit that
package's row. And for `createSignal` the withholding is a *condition* finding
recorded in the source:

> The browser bodies perform no `create` … but the `node`/`worker`/`deno`
> body's derived overload reaches `ctx.serialize(id, deferred.promise,
> deferStream)` … A `(package, export, domain)` row carries no condition, so the
> row must be withheld until the table is condition-aware.

So even with blocker one fixed, `createSignal` under `solid-js` recovers
nothing. Granting it would prove a false claim under server conditions.

### 69.3 The 62, by callee

| claims | callee, as resolved |
| --- | --- |
| 14 | `solid-js :: createSignal` |
| 6 | `solid-js :: createMemo` |
| 6 | `solid-js :: onCleanup` |
| 5 | `solid-js :: createComponent` |
| 2 | `solid-js :: getOwner` |
| 2 | `@solidjs/signals :: onCleanup` |
| 1 | `@solidjs/signals :: getOwner` |
| 2 | `@solidjs/web :: createComponent` |
| 6 | three non-core callees (`withArrayCopy`, `createServerPlugin`, `registerServerFunction`) |

`createSignal` is the largest single entry **and** the one that cannot be
granted without condition-aware rows. The three `@solidjs/signals` claims are the
opposite: their rows already exist, so blocker one alone is what withholds them.

### 69.4 Three stages, each decidable on its own

**Stage 1 — authenticate the core-runtime archives as snapshot-only roots.**
Give `solid-js`, `@solidjs/signals` and `@solidjs/web` a source root bound by
the `AuditedArchive` identity they already carry (name, version, integrity),
with no package contract, no accepted edge and no claims derived from them. This
does not reopen § 27: the exemption is about contracts and opaque frontiers,
and an archive whose bytes are authenticated establishes nothing by existing.
**Yield: 3 claims** — the `@solidjs/signals` rows that already exist. Small, and
that is the point: it proves the path end to end before anything is audited.

**Stage 2 — audit the four non-`createSignal` callees under their own package.**
`createMemo`, `onCleanup`, `getOwner` under `solid-js` and `createComponent`
under `solid-js`/`@solidjs/web`, each granted only if *every* audited condition
closes the domain empty — the rule `NEGATIVE_ROWS` already states. **Yield: up
to 19**, and less wherever a condition disagrees, which is exactly what the
audit would be for.

**Stage 3 — make the rows condition-aware.** A row gains a condition set; the
certifier binds the artifact case's conditions and grants only a row that covers
them. The audit names this as the blocking design, and it is the only route to
`createSignal`. **Yield: 14**, plus it retires the "one approximation, stated
rather than hidden" that § `NEGATIVE_ROWS` already carries about
`browser/production`.

### 69.5 What I would pick, and what is yours

Stage 1 is mechanical and I would do it without asking: it is plumbing, it
changes no claim's truth, and 3 claims is a cheap end-to-end proof.

Stage 2 is an **audit**, not a code change. Its cost is reading four exports'
published bytes across every captured condition and citing them, and its risk is
the § 7.3 shape — a body that differs by condition and quietly makes a row
false. It needs a reviewer who will say no.

Stage 3 is the real decision: a schema change to the negative authority, a
condition binding at the gate, and a re-reading of citations per condition. It
buys 14 claims here and removes a standing approximation — but it is the one
that deserves an explicit yes rather than being started because it is next.

§ 66.3 framed all of this as a trust-model question. Only stage 1 is, and it is
the cheap one.

## 70. Stage 1 checked before it was built, and the 62 is really 14 (2026-09-12)

§ 69 split the core-runtime claims into three stages and put stage 1's yield at
3, stage 2's at "up to 19" and stage 3's at 14. Reading the gates in order
before writing the plumbing — which is what stage 1 is — falsifies two of those
three numbers, and for a reason neither § 66 nor § 69 had looked at.

### 70.1 Solid 1.x has no negative authority at all

~~~rust
static NEGATIVE_AUTHORITY: DialectNegativeAuthority = DialectNegativeAuthority {
    archives: &[],
    rows: &[],
};
~~~

That is `solid_1x.rs`. `primitive_performs_no_operation` requires the snapshot's
audited archive to be **in** the dialect's authority, so for a Solid 1.x project
the axiom tier can never admit anything — no source root, no authentication and
no audit changes that, because there is nothing for the archive to be a member
of. The Solid 2 authority is the only one with archives, and its three are all
pinned at `2.0.0-rc.3`.

### 70.2 Which means most of the 62 are out of reach of all three stages

| family | claims | reachable by §69's stages |
| --- | --- | --- |
| **solid1** | **37** | **none** |
| solid2 | 20 | most |

And within solid2, by callee:

| claims | callee | stage |
| --- | --- | --- |
| 6 | `solid-js :: createSignal` | 3 — condition-aware rows |
| 3 | `solid-js :: createMemo` | 2 — audit |
| 2 | `@solidjs/web :: createComponent` | 2 — audit |
| 2 | `@solidjs/signals :: onCleanup` | **1 — the row already exists** |
| 1 | `@solidjs/signals :: getOwner` | **1** |
| 2 | `@solid-primitives/analytics :: createServerPlugin` | none — not a primitive of any dialect |

So the corrected yields are **stage 1: 3, stage 2: 5, stage 3: 6** — fourteen
claims, not sixty-two. § 69.4's "up to 19" and "14" were computed over the
combined corpus without noticing that more than half of it runs a dialect whose
authority is empty.

### 70.3 The 37 are a different piece of work, and it has never been scoped

Recovering the Solid 1.x claims means auditing Solid 1.x the way
`RC3_CORE_PRIMITIVES_AUDIT` audited 2.0.0-rc.3: pinning archives by name,
version, integrity and manifest digest, then establishing per export and per
domain that every condition closes empty, with citations. That is the largest
single thing this document has ever pointed at and it is not a premise, a
protocol or a gate — it is an audit of another major version's published bytes,
and it inherits every difficulty § 7.3 records, including the condition problem
that already withholds `createSignal` on 2.x.

It is also the only route to the largest block of withheld `creates` claims in
the corpus. Naming it is the point of this section; costing it is not attempted
here.

### 70.4 What this says about stage 1

Stage 1 recovers **3 claims**. It is still the cheapest end-to-end proof that
the authentication path works, and stages 2 and 3 both depend on it — neither
can grant a row to a snapshot the tier never reaches. But as a unit of its own
it buys three claims for a piece of certification plumbing, and it should be
started because stages 2 and 3 are actually intended, not because it is next.

That is a decision about whether ~14 claims justifies an audit and a schema
change, and it is the user's. The correction above is what it should be made
on: § 66.3 said 62, § 69 said 62 across three stages, and the measured answer is
14 plus an unscoped audit of Solid 1.x for the other 37.

## 71. The negative authority covers 28% of the corpus, and decays to nothing on every release (2026-09-12)

Building stage 1 meant establishing, first, that a source root is the *only*
thing standing between the axiom tier and the three claims whose rows already
exist. It is not, and finding out what else stands there is worth more than the
stage.

### 71.1 The audited pin and the installed corpus have already diverged

`audited_archive_for_snapshot` matches name, **version**, integrity and
`package.json` digest, field by field. The Solid 2 authority pins exactly one
version — `2.0.0-rc.3` — for all three archives. What the corpus installs:

| probe rows | solid-js |
| --- | --- |
| 117 | **2.0.0-rc.3** — the audited pin |
| 111 | 2.0.0-rc.0 |
| 17 | 2.0.0-beta.19 |
| 2 | 2.0.0-rc.2 |
| 168 | 1.9.14 (Solid 1.x — empty authority, § 70.1) |

So the negative authority can answer for **117 of 415 rows, 28%**. The other 72%
fail the identity gate before any row is consulted — 130 Solid 2 rows on an
unaudited prerelease, and every Solid 1.x row on a dialect with no archives at
all.

This is not a defect in the gate. Binding version and integrity exactly is what
makes a row a claim about *bytes* rather than about a name, and § 27's own
comment is explicit that a name-only predicate may not stop withholding. The
gate is right. The **pin** is the problem.

### 71.2 It is the recipe treadmill again, in another subsystem

§ 64.5 recorded that probe-recipe `claimId`s are content digests, so every
package release orphans its recipes, and that nothing automates the re-point.
The negative authority has the same shape and a worse blast radius: it is pinned
to one prerelease of one package, and **a single Solid release takes it from 28%
to 0%** until someone re-audits — archives re-pinned by integrity and manifest
digest, and every row's citations re-read against the new bytes, because a row
is only granted when every audited condition closes the domain empty.

Nobody had noticed, because the failure is silent in exactly the way § 65's was:
an unmatched identity produces a refusal, refusals are the normal state of a
withheld claim, and no gate reports "the authority answered for nothing today".

`RC3_CORE_PRIMITIVES_AUDIT` was hand work — 23 rows read out of audited contract
documents and five out of the archive's runtime bytes by hand. Repeating that on
every prerelease is not a plan.

### 71.3 What this does to §§ 69–70's stages

Stage 1 was costed at 3 claims (§ 70.2). Those three sit on
`@solid-primitives/lifecycle`, `/scheduled` and `/websocket`, each of which
appears twice in the corpus — once on rc.0 and once on rc.3 — so at most the
rc.3 halves could ever close, and `@solidjs/signals` is not even a tracked
install in those projects; it is reached transitively through solid-js, so its
own version follows.

Stage 1 therefore buys **at most three claims, on the half of the corpus that
happens to match a pin that will be stale at the next release**. It is a real
prerequisite for stages 2 and 3 and it is not wrong — but building certification
plumbing across the trust boundary for that is not a defensible unit of work,
and saying so is more useful than a commit that looks like progress.

### 71.4 What is worth doing instead

The leverage is not another row or another stage. It is that the authority's
coverage is 28% and structurally decaying, and there are three shapes of answer,
none of them scoped here:

- **Widen the pin.** Audit each prerelease the corpus actually installs. Honest,
  and it is the treadmill.
- **Derive the rows instead of auditing them.** The 23 of 28 rows already read
  out of `pkg/contracts/bundled/solid-v2/*.json` came from *contract documents* —
  documents this system generates. A row derived from a certified contract of
  the runtime itself, rather than transcribed by hand, would move with the
  version. It also raises ADR 0005 objection 5 squarely, which is why the tier
  already refuses when the certified artifact is itself audited.
- **Accept the coverage.** State that the axiom tier serves one audited release
  and that everything else is withheld, and stop treating the resulting refusals
  as a backlog.

The third is what the code does today. What it has not done is say so out loud
with a number attached, which is what this section is for.

## 72. The coverage is 28.7%, and now something says so every run (2026-09-12)

§ 71 ended by naming three shapes of answer and observing that the code takes
the third — serve one audited release, withhold everything else — without ever
saying so. § 65 ended in the same place from the other side: five vetoes that
cannot veto, and no reader of a run could tell. Both failures are the same
failure, and it is not a wrong answer. It is a *silent* one: an unmatched
archive identity and an unmatchable recipe gate both produce the refusal a
withheld claim already produces, so nothing in a green run distinguishes "the
authority answered for nothing today" from an ordinary day.

This section is what was built instead of stage 1.

### 72.1 The pins are published, and held to the tables

`rust/crates/solid-dialect/audited-archives.json` mirrors each dialect's
`AUDITED_ARCHIVES` and its negative row count, and
`audited_archives_json_mirrors_the_dialect_tables` fails until a re-audit that
changes a tuple updates it. The file exists because the question "is this
installed tree one the authority can answer about" has consumers outside Rust
and nothing published the answer. Both dialects appear, including the one that
audited nothing: an absent entry would read as "not a dialect" where § 70.1's
finding is "looked, and there is nothing to be a member of".

### 72.2 The join, and what it measures

`scripts/ecosystem-benchmark/lib/dialect-authority.mjs` joins those pins
against every probe row's `installedVersions`, keyed by the row's **own**
dialect — the identity gate admits an authority only when that authority's own
archive list carries the exact tuple, so counting a 1.x row as covered by a
Solid 2 pin would overstate reach the gate never grants. Measured on the pinned
corpus:

| | rows | covered |
| --- | --- | --- |
| solid-v1 | 168 | **0** — no archives, no rows (§ 70.1) |
| solid-v2 | 250 | **120** |
| corpus | 418 | **120 (28.7%)** |

with every installed version of an audited name ranked beside it, so the
versions the pin misses are named rather than implied: `solid-js@1.9.14` 168
rows, `@solidjs/web@2.0.0-rc.0` and `solid-js@2.0.0-rc.0` 111 each,
`2.0.0-beta.19` 17 each, `2.0.0-rc.2` two each. § 71.1 put it at 117 of 415
counting `solid-js` alone; 120 of 418 is the same fact counting all three
audited names and every row.

**It is an upper bound and never a yield**, and the report says so where the
number is. The join is on name and version; the gate also binds integrity and
the archive's own manifest digest, which a run's report does not record. And a
covered row is not an answered one — § 69.1's missing source root still sits in
front of the table, so the tier answers nothing on this corpus today. What the
number bounds is what any of §§ 69–70's stages could ever reach.

### 72.3 The floor

`minAuthorityCoveredRows: 120` in
`scripts/ecosystem-benchmark/certification-regression-thresholds.json`, checked
by `make ecosystem-regression`. A Solid release that moves the corpus off
`2.0.0-rc.3` now fails that gate and prints the unaudited versions it found,
which is the re-audit request stated as the thing it actually is. A report that
could not read the pins fails the same threshold rather than passing as
coverage — the same rule the baseline branch beside it already applies, because
silence about the authority cannot satisfy a claim about its reach.

The floor is a full-corpus number; a scoped run carries fewer rows and would
fail it, which is why it lives in the regression thresholds rather than in
`phase16-thresholds.json`.

### 72.4 The recipe half

The per-recipe agreement check landed in `43830a24` (§ 65). What it could not
say is how much of the corpus is in the can-never-veto state, so the corpus
test now pins the list: **5 of 19 recipes declare `NEVER EMITS:`**, all five
`reads`, and 14 can still contradict something. A sixth is now a diff rather
than a discovery, and a `creates` or `returns` recipe going silent — the case
that would matter — cannot happen quietly.

### 72.5 What is still silent

- **An emit that is never reached** (§ 65.5). Unchanged: no static check finds
  it, and only a run that contradicts something would.
- **A hand-edited pin file.** The Rust test binds the JSON to the tables, so
  drift fails; nothing binds the tables to the archives they were read against
  beyond the citation tests already in `solid_2.rs`.
- **`make verify` does not run the ecosystem gate.** The floor bites in
  `make ecosystem-regression` only, which is the same reach every other
  corpus-scale check here has, and the same one § 28.2 recorded.
- **The corpus manifest carries its own copy of the pin.**
  `scripts/ecosystem-benchmark/manifest.json` records
  `auditedSolid2: "2.0.0-rc.3"`, which happens to equal the negative
  authority's archives today and is a different claim — the version the corpus
  installs as its audited target, not the bytes the rows were read against.
  Nothing checks that the two agree, and nothing should until somebody decides
  they must.

### 72.6 The decay is not hypothetical, and the manifest is what defers it

Checked against the registry on 2026-09-12: `solid-js@next` is **2.0.0-rc.8**,
five prereleases past the audit, and `latest` is 1.9.15 against the corpus's
1.9.14. Coverage is still 120 only because
`scripts/ecosystem-benchmark/manifest.json` pins an exact version per probe and
was last generated on 2026-08-26 — a fresh `make ecosystem-regression` installs
rc.3 because the manifest says rc.3, not because rc.3 is current.

So the next `make ecosystem-discover` is what takes the authority's reach to
roughly nothing, and from now on that shows up as a failed floor naming
`solid-js@2.0.0-rc.8` rather than as an unchanged green run. That is the whole
point of the gate, and it also means the floor will fail on a manifest refresh
that is otherwise entirely correct. The right response there is § 71.4's
decision — re-audit, derive, or accept and lower the floor deliberately — not a
quiet edit of the number.

## 73. ADR 0092 measured: 83 closures, and the depth gate priced at 4 claims (2026-09-12)

§ 68.3 said "write it", with 13 in the sentence a plan would quote. It is
written, and the two things worth recording are the number it actually moved
and the restriction the fixture forced.

### 73.1 The restriction the fixture found

The premise as § 68 framed it — every ToPrimitive operand rooted at a
caller-provenance derivation — flipped two fixture cases nobody intended:
`helperSpreadCoercion` and `helperUntypedArgument`, the pair that pins
ADR 0038's helper-premise boundary. Both refuse *at the helper*, and inside that
helper the coercion's operands are the helper's own parameters.

ADR 0034 admits exactly that: "a parameter binding of the censused
declaration — the export itself, **or a `local-recursion` target frame**". So the
flip was consistent with the accessor rule. It is not consistent with the
coercion's own argument. At depth 0 a parameter holds what the external caller
passed, by construction. Inside a frame the value at that slot was supplied by a
call site *in this artifact*, which may have handed it an object this program
built — whose `valueOf` is then this program's code, which is the case the
premise is about rather than one it covers.

ADR 0092 is therefore gated to the censused export's own declaration. Whether
ADR 0034's frame case survives the same argument is a question this document
records and does not answer; it is measured nowhere.

### 73.2 Measured, control versus change, same machine

| | control | ADR 0092 |
| --- | --- | --- |
| certified closures | 5,192 | **5,275** |
| coercion refusals at the `parameter` leg | 171 details / **13 claims** | 67 details / **4 claims** |
| certified entrypoints | 654 | 654 |
| verified / complete / partial / refused | 381 / 324 / 57 / 18 | 381 / 324 / 57 / 18 |
| fixes / regressions vs the pin | 13 / 0 | 13 / 0 |

The control is the same tree with the consumer arm returning `None`, so the
producer, the protocol and every other commit are held fixed. Its 5,192 is
exactly ADR 0091's committed number, which also says the four commits since
moved no closure.

**+83 closures, and no row moved.** Nine of the thirteen claims had their
coercion form admitted and not one of them changed a row's status or coverage:
they refuse at their next blocker, which is §§ 42/49/69's lesson for the fourth
time. What the 83 buys is stronger documents on rows that already certified —
closures are per claim × artifact case, and § 56.1's roughly fivefold
over-count is the same factor that turns 9 claims into 83 closures.

**The remaining 4 are the depth gate's price**, and that is the useful half of
the number: the restriction in § 73.1 costs four claims, stated rather than
estimated. The rest of the family is unchanged — `written-parameter` 7,
`nested-parameter` 6, `local-binding-written` 4, `module-binding` 3,
`local-binding` 2, one `mixed-operand-roots`.

### 73.3 A measurement trap, paid for twice

The first regression run failed the gate with one certification regression
(`@kobalte/utils`) and a second row not certifying (`solid-js@1.9.14`). Both were
`infrastructure-failure: policy-2 certification attempt timed out`, at 626 s and
635 s against the 600 s budget, with `@kobalte/core` at 581.6 s beside them.
`pmset` said battery, Low Power Mode on — the 2× condition
docs/precision-backlog.md already records. A rerun at `--timeout 1200` returned
the control's numbers exactly.

Three rows clustering at the cap on a throttled machine is the signature, and
the lesson is the one already written down: judge a Kobalte timeout with
`pmset -g` in hand. The new half is that `make ecosystem-regression` hardcodes
`--timeout 600`, so on a throttled machine the gate reports a *semantic*
regression for a *wall-clock* cause. Running `run.mjs` directly with a larger
timeout is what separates them.

And one process note, mine: the first run's report was overwritten by the
second before the two were diffed, so the comparison had to detour through the
2026-09-06 pin. Copy a report aside before re-running one.

## 74. § 71.4's first option, priced (2026-09-12)

§ 71.4 named three answers to the authority's 28% coverage and costed none of
them. This section prices the first — **widen the pin** — for the versions the
corpus actually installs, which is the only variant that raises coverage now:
auditing `2.0.0-rc.8`, the current `next`, would cover **zero** corpus rows,
because every probe installs rc.3, rc.0, beta.19 or rc.2 under a manifest
generated 2026-08-26.

### 74.1 What it would buy

Coverage counts a row as reachable when it installs an audited archive at the
audited version, and the corpus installs `solid-js` and `@solidjs/web` together:

| add | rows gained | coverage |
| --- | --- | --- |
| rc.0 | +111 | 120 → 231 |
| beta.19 | +17 | → 248 |
| rc.2 | +2 | → 250 |

250 of 418 is every solid-v2 row; the remaining 168 are Solid 1.x, whose
authority is empty and which § 70.3 prices separately.

**`@solidjs/signals` is not on this path at all.** The one corpus row installing
it at rc.0 installs `solid-js@2.0.0-rc.0` as well, so it is already covered by
the two packages above — and that archive is where all 15
`AuditedCitation::Implementation` rows live, the hand readings of runtime bytes
that `RC3_CORE_PRIMITIVES_AUDIT` records. The expensive half of the table is the
half that buys nothing here.

### 74.2 What it would cost, measured rather than estimated

Per added version, the rows to re-establish are:

| package | exports | rows | documents |
| --- | --- | --- | --- |
| `solid-js` | 9 — `For`, `Loading`, `Match`, `Repeat`, `Show`, `affects`, `isPending`, `latest`, `refresh` | 15 | `solid-js.json` |
| `@solidjs/web` | 2 — `hydrate`, `render` | 8 | `solidjs-web.json`, `solidjs-web--web-node-server.json` |

**11 exports across 3 documents per version; 33 export-audits across 9 documents
for all three.** Each one must close its domain empty in *every* audited
condition, which is why `@solidjs/web` carries two artifact-case documents.

**There is no byte-identity shortcut, and that was worth checking first.** If an
export's implementing bytes were unchanged between rc.3 and the new version, its
row would carry over with a digest rather than a reading. Measured on the
published archives: `solid-js` rc.3 → rc.0 keeps 17 of 43 files identical, but
the runtime files are `dist/solid.js`, `dist/server.js` and `dist/dev.js` and
**all three changed**; the only identical runtime file is `dist/refresh.js`.
`@solidjs/web` is the same story — `dist/web.js`, `dist/server.js` and every
`frames/` and `serialization/` bundle changed, and the two identical runtime
files are `server-functions/dist/rich-args.js` and `storage/dist/storage.js`.
rc.2, one release before the audited pin, still changes 18 of 43 `solid-js`
files and 57 of 103 `@solidjs/web` files. Every audited export's bytes differ at
every version, so each row is a fresh reading.

### 74.3 The structural blocker, which is not an audit question

A row's `AuditedCitation::Summary` names a repo-relative document and a byte
range inside it, and those documents are the dialect's **bundled** contracts —
one set per dialect, pinned to one archive by their own `package` block, and
compiled into the analyzer through `include_bytes!`. There is nowhere to put a
second version's audited document today, and the row table is keyed
`(package, export, domain)` with **no version**, so simply adding an archive to
`archives` re-points every existing citation at bytes it was never read
against.

Two shapes of answer, and the choice belongs to whoever owns the dialect seam:

- **Version-keyed audited documents outside the runtime bundle**, cited by the
  rows and never loaded by the analyzer. Smallest blast radius; the bundle index
  stays one-per-dialect.
- **A version in the row key**, so `denies()` answers per archive rather than per
  name. Honest about what a row is, and it touches every row, the citation
  tests, and the tier.

### 74.4 What this section recommends

Not this option. 33 export-audits against wholly different bytes, plus a
structural change, buys 130 rows of *reachability* — and reachability is not
yield: § 69.1's missing source root still stands in front of the table, so the
tier answers nothing on any of those rows until stage 1 is built too. The same
work recurs at the next prerelease, which is § 64.5's treadmill with a bigger
bill.

§ 71.4's second option — derive the rows from the runtime's own certified
contracts, so they move with the version — is the one that would retire the
treadmill rather than paying it again, and it is the one nobody has costed. It
raises ADR 0005 objection 5 squarely and that is a design argument, not a
reading of bytes. The third option, which the code takes today, is now at least
stated with a number attached (§ 72).
