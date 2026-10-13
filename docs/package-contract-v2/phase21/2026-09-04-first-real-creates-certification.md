# The first recipe for a real row's `creates: []` claim, end to end

Date: 2026-09-04

## What this slice set out to do

Make one real ecosystem package's `creates: []` claim actually certified: pick
one of the 43 withheld closure candidates the corpus now plans, write its veto
recipe, and run the whole chain — census, gate, probe, authentication, receipt.

**It is not certified, and the reason is two named blockers rather than a gap in
the chain.** Everything between the generator's proposal and the probe gate now
runs on real published bytes for the first time: a real row derived a
`domain-exhaustiveness` demand, the implementation census ran against an
authenticated ecosystem artifact, and for one export it **proved** `creates: []`.
The refusal that remains is downstream of the census and structural.

## The candidate population, enumerated

Three rows carry `creates` candidates
(`docs/precision-backlog.md`, "Re-measured: the candidates arrive"). Measured
here by re-running each row with `--attempt-certification --keep-temp` and
reading `…certification-audit.json`'s `withheldClosures`:

| row | candidates | artifact cases they live on |
| --- | ---: | --- |
| `@kobalte/utils@0.9.2\|solid1\|only` | 33 | 12 cases, **every one a `./src/*.ts` file** |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | 7 | 3 cases, **every one a `./src/*.ts` file** |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | 3 | one case, `dist/index.js` |

The kobalte rows' shape is not an accident of sampling. Both versions publish
`"exports": { ".": {…}, "./src/*": "./src/*" }`, and the `.` case carries an
`unaccepted-external-dependency` frontier on `solid-js` that opens `creates`
before publication — the 2.0.0-alpha row *has* two `.` → `dist/index.js` cases
and no candidate survives on either. So every candidate a kobalte row can offer
lives on a TypeScript source entrypoint.

**That makes 40 of the 43 candidates unprobeable on the pinned interpreter, for
a reason nothing in a recipe can address.** The private probe workspace places
the authenticated snapshot at `<private>/node_modules/<package>/` — deliberately,
because that is what contains every `node_modules` rung of Node's resolver
(ADR 0006 § "Module resolution: the disposition table") — and Node refuses to
strip types from any file under a `node_modules` directory:

~~~
Error [ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING]: Stripping types is
currently unsupported for files under node_modules, for
"file://…/node_modules/@kobalte/utils/src/noop.ts"
~~~

Reproduced twice: once in a bare two-file harness on the pinned Node 24.11.1,
and once in a hand-built copy of the private workspace's exact layout
(`node_modules/@kobalte/utils/` beside a `recipes/` directory carrying an
`exports`-less `"type": "module"` scope), launched with the same
`--conditions` flags. Resolution succeeds in both; it is the **load** that
fails, so the worker's resolution echo would have been satisfied and the run
still dies.

## The claim chosen, and why

`@kobalte/utils@0.9.2`'s **`noop`**, on the `./src/noop.ts` artifact case:

~~~ts
/** A function that does nothing. */
export function noop() {
	return;
}
~~~

`claim:v1:sha256:1553467a94fb362adae87d8f2809233d759d5c71fc3334a45fdf3ddc5cb7097d`

No call, no loop, no `switch`, no `try`, no jump, no invoking form of any kind.
It is the one candidate in the whole population for which the implementation
census has *nothing to refuse*, so it isolates the workspace blocker from the
census blockers — which matters, because otherwise the type-stripping refusal
would have stayed a prediction. The three probeable candidates (i18n's) all
refuse inside the census, so no combination of them can measure the gate.

## The recipes

`scripts/ecosystem-benchmark/probe-recipes/` — chosen over
`benchmarks/ecosystem/probe-recipes/` because that directory holds the run's
*outputs*, which the phase-20/21 ledgers pin, and over `fixtures/` because
every corpus fixture there is scanned by `contract-corpus.mjs` and
`coverage.mjs` while these recipes address published packages the repository
does not own. The runner's other hand-authored inputs — `manifest.json`,
`sentinel.json`, `phase16-thresholds.json` — are its neighbours, and the runner
is the corpus's only consumer. The directory's README records the choice and
the corpus's real weakness: a `claimId` is a content digest, so any generator
change silently unaddresses a recipe and the candidate is *withheld* rather
than failing loudly.

`kobalte-utils-noop.mjs`, verbatim:

~~~js
import { noop } from "@kobalte/utils/src/noop.ts";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  const before = Object.keys(globalThis).length;

  const answered = noop();
  if (answered !== undefined) {
    throw new Error(`noop returned ${JSON.stringify(String(answered))}`);
  }

  if (Object.keys(globalThis).length !== before) {
    harness.emit({
      marker: "create-operation",
      kind: "call",
      phase: "enter"
    });
  }

  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
~~~

Three properties of every recipe in the corpus, each a rule rather than a
preference:

* **It invokes the export in the way that would surface a create.** For
  `scopedTranslator` and `chainedTranslator` that means calling the *returned*
  translator as well as the export, because the export's own body only builds
  arrows and the calls live inside them. A recipe that stopped at the wrapper
  would pass its veto while observing nothing.
* **It performs no create itself.** The only values handed in are plain arrows
  over module-local variables and plain object literals.
* **It hands the package neither `session` nor `harness`** (ADR 0006, "The
  worker's realm is the package's realm"). Nothing in the design can enforce
  this, so `scripts/ecosystem-probe-recipes.test.mjs` pins it by source
  inspection: comments stripped, `harness` may appear exactly once outside an
  `.emit` call — the parameter itself — and `session` not at all.

**What the veto can actually observe, stated plainly.** A version-1 `create`
registers a resource into a runtime that outlives the invocation. The worker's
realm has no DOM, so a `render`-style `register-delegation` could not succeed
there even if attempted, and the only such runtime reachable is the global
object. So each recipe counts `globalThis`'s own keys around the invocation and
emits the `create-operation` marker if that set changes. That is a narrow
tripwire, it is declared as one in every recipe's `coverageLimitations`, and it
is not what proves the claim — the census is. A recipe that observes nothing
proves nothing.

## Every gate, in order

Measured with `rust/target/debug/solid-checker-rust` built through
`make build-checker-debug` (so the Type Facts and probe pins are compiled in),
`bin/solid-typefacts` at source manifest `efc33e8d…` (build id `dev`), and the
pinned Node 24.11.1.

| # | gate | `noop` | i18n's three |
| --- | --- | --- | --- |
| 1 | generator's `CreatesProposalWalk` clears the export | passed (proposed) | passed (proposed) |
| 2 | closure hazard on the artifact case | passed (none) | passed (none) |
| 3 | candidate published in the document, rebuilt by `inspect_candidates` | passed | passed |
| 4 | recipe found in the supplied corpus by exact claim id | passed | passed |
| 5 | `recipe_gated` does **not** withhold it | passed, `withheldClosures` 0 | passed, 0 |
| 6 | `DomainExhaustiveness` demand derived | passed, 1 demand | passed, 3 demands |
| 7 | probe gate scheduled | passed | passed |
| 8 | implementation census decides the domain | **passed — `creates: []` proved** | **refused**, by name |
| 9 | probe workspace built, recipe imported, run completes | **refused** — gate did not complete | not reached |
| 10 | gate authenticates, receipt binds a nonempty `probe_gate_root` | not reached | not reached |

Gate 6 is the number that had never been nonzero on a real row:
`demandCountsByFamily` now carries `"domain-exhaustiveness": 1` for the kobalte
row and `3` for the i18n row, where the 418-row corpus report has never named
the family at all.

### `noop`: the census passes, the gate does not complete

~~~
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate
sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not
complete
~~~

`ProbeGateError::IncompleteGate` — the run errored, which is
`ProbeTargetVerdict::Incomplete` → `ProbeGateOutcomeKind::ErrorOrTimeout`. The
refusal message carries no launch cause, which is why the two independent
reproductions above were needed to name it: the recipe's `import` of
`@kobalte/utils/src/noop.ts` cannot evaluate under the pinned interpreter.

The important half is what precedes it. The refusal is at *case-set
finalization*, not at witness acquisition — so demand 6 was **satisfied**. The
implementation census read `noop`'s `ExportImplementationTranscript` from
authenticated published bytes, found every premise held (`census-total:0:0`,
nothing to disposition, no uncensused form, an empty proposal item set), and
proved the closure. That is the first time a real ecosystem package's
`creates: []` has survived the census.

### i18n's three: the census refuses, each by name

Measured one at a time with a single-recipe corpus, so each claim's own premise
is the one that fires:

| export | refusal |
| --- | --- |
| `scopedTranslator` | `creates census refuses an uncensused invoking form: coercion (TemplateExpression) at dist/index.js:3349..3367, reach reachable` |
| `flatten` | `creates census refuses an implementation transcript whose control-flow census is unsupported (iterationReachability) at depth 0 for dist/index.js:939..946` |
| `chainedTranslator` | the same `iterationReachability` refusal, at `dist/index.js:3397..3414` |

Both refusals are ADR 0008's own recorded limits, met on real code:

* `flatten` and `chainedTranslator` each iterate `Object.entries(dict)` with
  `for…of`. Neither contains a `break` or a `continue`, so **no row was
  withheld** — this is item 0's stated over-refusal ("over-refuses every export
  whose frame has a loop, a `switch`, or a `try` even where nothing was
  withheld"), and its fix is producer-side.
* `scopedTranslator`'s body is
  `return (path, ...args) => translator(\`${scope}.${path}\`, ...args);`. The
  template expression applies `ToString` to operands that are untyped in the
  shipped runtime bytes, which the producer records as a `coercion` invoking
  form, and item 2 refuses every uncensused form the `MayExecute` floor admits.
  Refusing is *correct* here rather than an over-refusal: a `toString` on a
  passed object is code this census never saw. Its rest-spread argument
  (`...args`) would have refused for a second reason had the template not fired
  first.

## The verdict, with numbers

* `exportsProven` for `@kobalte/utils@0.9.2|solid1|only`: **0 of 50 before, 0 of
  50 after.** For `@solid-primitives/i18n@2.2.1|solid1|only`: **0 of 9 before, 0
  of 9 after.**
* What moved instead: with no corpus both rows certify with `withheldClosures`
  33 and 3. With this corpus supplied both **refuse** — the kobalte row at
  case-set finalization on the incomplete gate, the i18n row at witness
  acquisition on `chainedTranslator`'s marker — and `withheldClosures` drops to
  0 because every candidate is now addressed. That is a worse row verdict and a
  better measurement, and it happens only under the opt-in flag.
* No `probe_gate_root` was ever nonempty on a real row, so no receipt moved.

## What it took, and whether synthesis is feasible

Writing the four recipes was the cheap part: each is twenty lines, took one read
of the published `dist/index.js` or `src/*.ts`, and needed exactly two facts
that are not obvious from the export's type — that the returned closure has to
be called for the body to run, and that the claim id must be read out of a
previous run's certification audit. A generator could produce that shape
mechanically for any export whose arguments are plain data or plain callbacks.

The expensive parts are the two blockers, and neither is a recipe problem:

1. **A `.ts` artifact case cannot be probed at all** while the workspace places
   the snapshot under `node_modules`. This is 40 of 43 candidates. It is not a
   small fix: the argument vector is deliberately `--conditions=` flags plus the
   worker path (`argv:worker-path-plus-requested-conditions-only` in the sandbox
   policy digest), no interpreter flag lifts the `node_modules` restriction, and
   moving the copy out of `node_modules` would dismantle the containment the
   whole disposition table rests on. It needs a decision — a pre-stripped
   private copy would substitute bytes the transaction authenticated, which is
   exactly what `retain_collision_free_source_packages` refuses elsewhere.
2. **Item 0's loop over-refusal is what stops the only probeable row.** Two of
   i18n's three candidates are refused for a marker left by a loop that withheld
   nothing. ADR 0008 already names the producer-side fix (emit a withheld row
   with `reach: unknown`, or as an uncensused form) and defers it. On this
   population it is worth two of three probeable claims.

So recipe *synthesis* is feasible and is not the next slice. The next slice is
whichever of those two blockers is cheaper, and until one falls the corpus has
no reachable certified `creates: []` on a real row.

## Open, exactly

* **`exportsProven` is still 0 corpus-wide.** No real row's closed claim domain
  certifies.
* **A `.ts` artifact case is unprobeable**, and therefore so is every candidate
  on both kobalte rows. Reproduced, not inferred.
* **`noop`'s census verdict is measured but not receipt-bound.** It passed
  demand 6 inside a transaction that then refused, so nothing persists it. A
  test cannot pin it either: reproducing it needs the network, an npm install,
  and a real launch.
* **Item 0 refuses `flatten` and `chainedTranslator` where nothing was
  withheld.** Producer-side fix pending, per ADR 0008.
* **`scopedTranslator`'s `coercion` refusal is correct and stays.** Its
  rest-spread argument is a second, independent refusal behind it.
* **Claim ids are fragile.** The four in `recipes.json` were read from this
  run's audits; any generator change unaddresses them silently, and the
  candidate is withheld rather than refusing. ADR 0006 Stage 3.
* **The gate's refusal message names no launch cause.** `IncompleteGate` carries
  only the gate id, so diagnosing a failed run means rebuilding the workspace by
  hand. Worth a typed cause, not taken here.

## Addendum, 2026-09-04: item 0 is discharged, and it did not unblock this row

The producer slice this document called "whichever of those two blockers is
cheaper" landed: `docs/typefacts/adr/0026-…` (amendment, handshake protocol 15)
and `docs/adr/0008-…` item 0, rewritten. The call census no longer drops a row
in a jump region — it states it with `reach: unknown` — and each control-flow
`unsupported` marker now carries a class, so a construct whose *reachability
lower bound* alone is unmodelled is admitted while one the producer cannot
account for still refuses.

**`flatten` and `chainedTranslator` pass the control-flow premise and refuse one
premise later.** Re-measured with the same single-recipe corpora:

| export | refusal after |
| --- | --- |
| `flatten` | uncensused invoking form `property-access-unknown-accessor (SpreadAssignment)` at `dist/index.js:979..986`, reach reachable |
| `chainedTranslator` | the same form at `dist/index.js:3471..3483` |
| `scopedTranslator` | unchanged: `coercion (TemplateExpression)` at `3349..3367` |

Both bodies open with `const flat_dict = { ...dict };` (respectively
`{ ...init_dict }`). An object spread reads every own enumerable property of a
value whose shape is not statically known, which ADR 0026 records as
`property-access-unknown-accessor` — a correct refusal, not a new
over-refusal — and their `for…of` is a second, independent `iteration-protocol`
refusal behind it. So neither export would have certified even with the spread
gone, and the table above's "Item 0's loop over-refusal is what stops the only
probeable row" was true of the *first* premise only.

**Row-level numbers are unchanged.** `@solid-primitives/i18n@2.2.1|solid1|only`:
`exportsProven` still 0 of 9, verdict `success`, `declinedClosures` still 2,
three `domain-exhaustiveness` demands, and with the full corpus supplied the row
still refuses at witness acquisition. `@kobalte/utils@0.9.2|solid1|only` still
refuses at case-set finalization on the same gate id
`sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc` — so
`noop`'s census verdict still holds and the `.ts` type-stripping blocker is
untouched.

**What that leaves as next.** Of the two blockers this document named, the
cheaper one is done and bought no certified row; the expensive one (40 candidates
on unprobeable `.ts` artifact cases) is unchanged. The binding constraint on the
probeable population is now the producer's accessor census over untyped
receivers in shipped JavaScript — an object spread and a `for…of`, both recorded
in `docs/precision-backlog.md` as the next measurement.
