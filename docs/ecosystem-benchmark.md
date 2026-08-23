# Ecosystem benchmark

`scripts/ecosystem-benchmark/` measures how the package-contract generator
behaves against the real Solid ecosystem as it exists on the npm registry
today: which packages generate a contract cleanly, which fail, and exactly
how they fail. It is a measurement and classification harness, not a
correctness gate. It never weakens fail-closed semantics, loosens a fixture,
or relaxes a rule to raise its own success rate; a package failing generation
is the finding, not a defect in the benchmark.

It is separate from `scripts/contract-corpus.mjs`. The contract corpus is a
bounded, checked-in torture set with expected outputs reviewed like
snapshots — it answers "did the generator's behavior on these exact bytes
change." The ecosystem benchmark answers a different question — "how does the
generator behave across the packages people actually install" — against
live, moving registry state, and it has no expected-output snapshots to pin
against. Neither replaces the other.

## What it measures, and what it deliberately does not

It measures, per discovered package/Solid-target pair: whether
`solid-checker contract generate` succeeds, and when it does not, which
failure class the stderr matches. It aggregates this across families, across
Solid 1.x versus Solid 2.x, and across a package's floor and head compatible
Solid releases.

It does not:

- treat an install or generation failure as evidence that the checker is
  wrong. A package that only ships a CJS entrypoint, or forwards a callback
  through an uncontracted scheduler, is *correctly* refused — the benchmark
  records that as a named failure class, not as a bug to work around.
- run any installed package's code. Generation is static; the runner installs
  packages only so the generator can resolve their published declarations and
  `package.json#exports` map.
- treat "unresolved" as "safe" or "unsafe." A benchmark result is a
  classification of what happened, not a certification of the package.
- select or substitute a Solid version the manifest did not select. Discovery
  picks compatible releases according to the rules below; the runner only
  ever installs what discovery already decided.

## The eight families

`lib/families.mjs` enumerates every ecosystem family the benchmark is
required to cover: `official-solid`, `kobalte`, `solid-primitives`, `corvu`,
`tanstack`, `solid-devtools`, `solid-recharts`, `motion-solidjs`. A manifest
missing rows or exclusions for a required family, or missing one of a
family's `minimumPackages` on either Solid target, fails manifest validation
— a family silently dropping out of coverage is a defect in discovery, not an
acceptable gap.

`kobalte` and `corvu` also have search-detected forks or lookalikes; anything
matching a family's search terms but not actually published under its org is
recorded as `status: "supplemental"` and excluded from that family's official
totals. `tanstack` additionally requires a declared Solid dependency, because
most of TanStack is not Solid-specific — a React, Vue, Svelte, or
framework-neutral TanStack package must never enter the corpus.

## Solid targets: the 1.x audited pin and the 2.x floor/head model

Solid 1.x has one target: `solid-js@1.9.14`, the exact version this
repository audits its bundled 1.x contract against (see
[package-contracts.md](package-contracts.md)). A package is compatible with
`solid1` when its declared `solid-js` range accepts that exact version;
there is no floor/head split because there is only one pinned version to
probe.

Solid 2.x has no single audited release yet — it is still moving through
prereleases — so the benchmark probes a *range* instead of a pin: for each
compatible package version, it selects the floor (`minSatisfying`) and head
(`maxSatisfying`) published Solid 2.x releases the package's declared range
accepts, across `solid-js`, `@solidjs/web`, and `@solidjs/signals` together.
When floor and head land on the same release for every runtime package, that
is a single `kind: "only"` probe rather than two identical ones — this is how
a beta-only package's exact compatibility window is preserved rather than
silently widened. Otherwise the benchmark runs both a `floor` and a `head`
probe and reports where they disagree (see "Report contents" below).

The floor is then raised, if it has to be, until the runtime packages in it
accept each other. Flooring each of `solid-js`, `@solidjs/web`, and
`@solidjs/signals` from its own compatible set can synthesize an environment
that has never existed: `@tanstack/solid-router@2.0.0-rc.1` pins
`@solidjs/web@^2.0.0-rc.1`, and that web release peers `solid-js ^2.0.0-rc.1`,
so a floor pairing `solid-js@2.0.0-rc.0` with web rc.1 is refused by npm before
the checker ever runs. The resulting `install-failure` would describe the
benchmark's own arithmetic rather than the package. A floor is only ever raised
within the declared compatible set, never substituted outside it; when floor
and head end up equal the row collapses to a single `only` probe, and when no
coherent combination exists the row keeps only the head, because there is no
distinct older environment left to measure.

The floor is anchored at `2.0.0-rc.0` rather than at whatever a declared range
formally accepts. The 2.x line spent a long time in `experimental` and `beta`,
and a package published this month can still carry a range whose formal lower
bound is an old beta while its own dependencies have moved on — installing that
beta then produces a peer conflict that describes nobody's supported window and
says nothing about the generator. `compatibleSolidVersions` in the manifest
still records the complete accepted set, so the range fact is preserved; only
the probe moves. The anchor raises a floor, it never invents one: a range that
accepts no `rc` keeps its own oldest accepted beta, for the same reason a
beta-only package is never probed against a newer release candidate.

A beta-only package must never be probed against a newer release candidate.
`^2.0.0-beta.17` accepting `2.0.0-rc.1` is a real semver fact (prereleases of
the same major line satisfy a caret range that starts on a prerelease), but
substituting the newer RC into a probe for a package that has only ever been
tested against betas would attribute the RC's behavior to a compatibility
window the package author never declared or verified. Selection always uses
the package's own declared range against the real published catalog, never a
newer version chosen because it seems close enough.

## Vocabulary

These four terms are kept distinct everywhere in the code, manifest, and
report, and this document does the same:

- **family** — one of the eight ecosystem families above.
- **row** — one package/Solid-target pair in the manifest (for example,
  `@solid-primitives/scheduled` against `solid2`).
- **package-Solid environment pair** — the manifest's `probes` entry within a
  row: one exact package version paired with one exact set of Solid runtime
  versions (`floor`, `head`, or `only`). A row can carry one or two of these.
- **runtime entrypoint** — a `package.json#exports` subpath the generator
  reports on for a probe (for example `./store`).
- **contract-generation invocation** — one `solid-checker contract generate`
  process run against one installed probe, producing one classified result.

A row is a selection decision; a probe is what actually gets installed and
run; an entrypoint is what generation reports on inside one probe; an
invocation is the one process execution that produces a result. Conflating
any two of these misdescribes what a benchmark number is counting.

## Discovery and execution

Discovery is the only network-enabled step. It reads the live npm registry,
selects rows and probes, and writes the manifest:

```sh
make ecosystem-discover
```

The manifest describes live registry state, so `discover.mjs --check` reporting
drift is normal rather than alarming: during the run that produced the first
checked-in report, three `@tanstack` packages published new alpha versions and
`--check` correctly refused the manifest as stale, naming both the version and
the integrity change for each. Chasing that continuously is pointless; the
contract is narrower and more useful than "the manifest is always current":
whenever a refresh moves a row, re-run the benchmark so the checked-in manifest
and report describe the same package versions. The report records the
`manifest.generatedAt` it was produced from, so a reader can always tell which
manifest a number came from. `--check` deliberately ignores `generatedAt` itself
-- a differing discovery timestamp is not drift, and treating it as drift would
make the check fail on every single invocation.

`--check` compares the complete serialized document, so it refuses a manifest
whose *probes* moved as readily as one whose package versions did — a new
runtime release shifting a head, or a change to selection policy itself, both
count as drift. The printed diff reports probe changes for the same reason it
reports exclusions and limitations: a diff that says "(no changes)" above a
"file is out of date" verdict tells the reviewer the opposite of the truth.

Discovery fails loudly rather than guessing: an unparsed semver range is
recorded in `unparsedRanges` and never treated as a match, a package with no
resolvable release is recorded as an exclusion with a reason, and every
registry gap is recorded in the manifest's `limitations` array. Review
`node scripts/ecosystem-benchmark/discover.mjs --print-diff` before trusting
a refreshed manifest — an add, removal, or integrity change should always be
inspected in context, especially an integrity change, which discovery always
surfaces rather than merging in silently.

Execution reads the manifest and never touches the network itself beyond the
`npm install` invocations it needs per probe:

```sh
make ecosystem-sentinel     # the pinned regression subset
make ecosystem-benchmark    # every row's every probe
```

Reports are named for the scope that produced them. Only an unfiltered run
writes the canonical `benchmarks/ecosystem/report.json` and `report.md`;
`--sentinel`, `--family`, and `--solid` each derive their own name
(`report-sentinel.json`, `report-family-kobalte.json`, and so on), so a subset
can never overwrite the corpus-wide artifact. An explicit `--json`/`--markdown`
still wins. Every report also records the scope it covered, and the Markdown
header states it: a partial run says so on its second line and names how many
probes it actually ran, because the `Manifest generated at` line above it
describes the corpus the run was *selected from* -- 417 probes even for a
23-probe sentinel. For the same reason `--baseline` refuses a report whose
scope differs from the current run and exits 2: a full run measured against a
sentinel baseline would report every probe the sentinel never ran as removed.

Both require `SOLID_CHECKER_NATIVE_BIN` and `SOLID_TYPEFACTS_BIN` to point at
real binaries; the runner exits 2 immediately otherwise. It must never
measure against the possibly-stale checked-in `bin/solid-checker-rust`. The
sentinel uses a fresh debug build to preserve its deliberate timeout-class
probe; the full corpus uses a fresh release build so its duration represents
shipped package generation. See `scripts/ecosystem-benchmark/README.md`.

The runner uses up to eight workers, bounded by the host's available CPU count,
and accepts `--concurrency N` for an explicit comparison. Reports retain
install and generation time separately for every probe and aggregate those
phases under the combined worker timings section.

## Per-probe timeout

The default is 300s, which the pinned sentinel set relies on: it deliberately
keeps a `timeout`-class probe so that classification path stays exercised, and
that probe would stop timing out under a longer budget.

The full corpus runs with `--timeout 600` instead. It analyzes every discovered
entrypoint (`@tanstack/charts` alone declares 113), so the larger ceiling keeps
a future wide release or a temporarily contended scheduled runner from being
misclassified as unsupported. Current release-mode probes complete well below
that ceiling; it is a guardrail, not expected runtime.

## Failure classes

Every contract-generation invocation is classified into exactly one of:

| Class | Meaning |
| --- | --- |
| `success` | Generation completed and emitted a **complete** contract: every entrypoint it reached is described. |
| `partial-success` | Generation exited 0 and emitted a contract that omits one or more refused entrypoints. |
| `unsupported-package-shape` | The package's manifest or exports map cannot be resolved into an entrypoint at all. |
| `no-esm-runtime-target` | No materialized ESM implementation target exists for a runtime entrypoint — typically a publishing mistake, such as an `exports` map naming a file the tarball does not contain. |
| `no-exported-surface` | The ESM target resolved and parsed, and exports nothing. A side-effect-only module has no reactive surface to describe; this is a well-formed package, not a broken one. |
| `cjs-only-entrypoint` | The entrypoint resolves to a CJS-only target, which contract generation does not support. |
| `conditional-export-incompatible` | Conditional-export branches cannot be ordered or resolved into complete variant summaries. |
| `incompatible-conditional-summaries` | Build targets disagree in a claim shape that cannot be represented as complete variants. |
| `unresolved-parameter-behavior` | A callback parameter's execution timing could not be resolved to an audited position. |
| `reactive-dispatch-unresolved` | A call or structured-return target could not be resolved to an exact declaration. |
| `reactive-source-uncaptured` | A reactive source read could not be attributed to a resolved symbol. |
| `dependency-contract-obligation` | Demand-driven recursive generation could not produce the exact dependency contract needed by an export-all barrel or unresolved subpath. |
| `package-contract-environment-dependent` | A dependency's contract (typically Solid's own) has environment-dependent variants and no runtime condition was selected, so applying one would be a guess. |
| `package-contract-export-missing` | A dependency's contract has no entry for an export the package imports. |
| `type-facts-failure` | The native Solid compiler facts / Type Facts session reported an error. |
| `checker-crash` | The checker process died by signal or panicked. |
| `timeout` | The per-probe generation invocation exceeded its timeout. |
| `install-failure` | `npm install` for the probe's exact versions failed. |
| `integrity-failure` | The installed versions or lockfile integrity did not match what the manifest recorded. |
| `unclassified` | The stderr matched none of the known signatures; the full raw output is retained rather than discarded. |

### Complete, partial, failed

A probe result carries one of three outcomes, and `success` means the
narrowest of them: a contract that describes every entrypoint the generator
reached.

`contract generate` refuses an entrypoint it cannot certify, omits it, and
still exits 0 — deliberately, so one unrepresentable subpath does not cost a
package its other twenty (see
[package-contracts.md](package-contracts.md)). The refusal is stated on
stdout (`; N entrypoint(s) refused and omitted`) and listed in the review
plan, and the benchmark reads that statement rather than comparing declared
against generated entrypoint counts — those two legitimately disagree on a
complete contract, since one wildcard subpath is one declared pattern and many
generated entrypoints.

Such a probe is recorded as `partial-success`: outcome `partial-success`,
`generatedEntrypoints` and `checklistItems` measured exactly as for a complete
contract, plus `refusedEntrypoints`. It is not a failure — usable output
exists — and it is not a success, because a consumer importing a refused
entrypoint gets an uncertifiable result. Family sections and totals report
`successCount`, `partialCount`, `failureCount`, and the refused-entrypoint sum
separately; `successRate` counts only complete contracts in its numerator and
every probe in its denominator. `--baseline` treats a complete contract that
became a partial one as a regression, since entrypoints were lost.

This split only ever makes the benchmark's own rate stricter, which is the
direction the harness is allowed to move it.

**When the partial-success split first landed (2026-08-22, superseded).** The
reports regenerated against this classification recorded **403 complete
contracts, 6 partial, 7 failures** of 416 probes, against the previous
**409 successes and 7 failures** on the same manifest. The failure set was
unchanged package-for-package and class-for-class
(3 `install-failure`, 2 `no-esm-runtime-target`, 1 `cjs-only-entrypoint`,
1 `no-exported-surface`), and every one of the 6 new partials was a former
`success` — `@kobalte/core` (1 refused of 69 declared-and-reached),
`@tanstack/charts` (2), `@tanstack/solid-pacer` (1), `@tanstack/solid-router`
(1), and `solid-js@2.0.0-rc.1` on both the floor and head probes (1 each).
`409 = 403 + 6` exactly, so the split moved probes only in the direction it was
designed to and the typed generation-refusal change moved none into a failure
class. The pinned sentinel subset moved the same way: 23 probes, 20 complete,
2 partial, 1 failure, and it started running against the same 305-row/416-probe
manifest as the full report rather than an older 417-probe one.

**Those numbers are superseded twice over and are kept only for the
class-by-class account above.** Two later runs moved probes out of the partial
class: `403 / 6 / 7` became `406 / 3 / 7` when the whole-summary unknown
collapse was fixed, and `406 / 3 / 7` is still the split today. **The
checked-in reports under `benchmarks/ecosystem/` are always the current
measurement state**; the figures they carry are stated once, under
"[Headline numbers](#headline-numbers-2026-08-23-fifth-measurement-state-release-binary-416-probes)",
and every historical figure in this document is marked as superseded where it
appears.

One thing the split did **not** explain. The Official Solid family under
Solid 1.x still reads "Declared entrypoints: 44 / Generated entrypoints: 28 /
Success: 6/6 (100%)" with **zero** refused entrypoints in the family, so that
gap is not refusals — `solid-js` alone declares 23 and generates 11, and
`@solidjs/image` declares 5 and generates 2, both cleanly. Declared-entrypoint
counts include export-map branches the generator does not emit a contract
entrypoint for at all, and the report still has no field that attributes the
difference. That remains unmeasured.

Classification matches the most specific known stderr marker first;
`unclassified` is the fallback of last resort, and it is the one class the
report is required to keep full raw output for, since a new marker showing up
there is itself a signal that classification needs to grow a case.

That is not hypothetical: the first full ecosystem run put 84 probes in
`unclassified`, which turned out to be only five distinct shapes. Two of them —
`PackageContractEnvironmentDependent` and `PackageContractExportMissing`,
together 83 of the 84 — became the two consumer-side contract classes above.
Both record the blocking dependency in `detail.module`, which is what lets the
report count them as shared blockers rather than as hundreds of unrelated
failures. Retaining full raw output is what made that diagnosis possible from
an already-completed run instead of requiring a re-run.

## Contract content: how much of an emitted contract is actually a claim

Everything above measures **generation reachability** — whether a contract was
emitted. It says nothing about what the emitted document contains, and the two
are very different questions. A package can generate a perfectly complete
contract in which every export's behavior is the schema's
`{"status": "unknown"}` sentinel: a successful generation and a worthless
proof. `lib/contract-content.mjs` measures the second question by opening each
emitted `solid-reactivity.json` and its sibling `.review.json` before the
probe's temporary directory is cleaned up.

### What is counted

Per probe that produced a contract (complete or partial):

- **entrypoints emitted** and **entrypoints refused** (from the review plan's
  own `refused-entrypoint` items, which name each refused subpath; a
  disagreement with the generator's stdout count is recorded, not resolved by
  preference).
- **exports**, counted **per export name, never per summary id**. A contract's
  `entrypoints[ep].exports` maps one summary to every name that shares it —
  `@kobalte/core@0.13.13` has 40 summaries covering 610 export names — and a
  consumer imports a name.
- **exports carrying an unknown**, broken down by the five claim domains the
  schema allows the sentinel on: `callbacks`, `reactiveReads`, `returns`,
  `ownerRequirements`, `asyncBehavior`. An export is counted once per (export,
  domain) whether the sentinel is on the default summary or inside any
  conditional variant.
- **exports fully proven** — no sentinel in any domain.
- **closure notes** — the review plan's `generation.entrypoints[*].notes`. Each
  one says a runtime-module closure could not be fully enumerated or hashed, so
  the contract is bound to fewer bytes than it describes (see
  "[The runtime-module closure is walked, not attested](precision-backlog.md)").
- **positive behavioral rows** by kind — callback execution rows, reactive-read
  rows, return trees, owner requirements, async behaviors. These are the rows a
  future probe step would have to actually drive to move a claim from
  `inferred` to `probed`.

Two derived shapes are tracked separately because a flat per-domain table
misreads them:

- **all five domains unknown at once** is not five gaps, it is one export the
  generator could say nothing about. It dominated the corpus on 2026-08-22 and
  was mostly an emitter defect rather than an analysis limit; the numbers below
  record both readings.
- **unknown only inside a conditional variant** would mean the default
  resolution is fully claimed and the uncertainty is confined to condition sets
  a given consumer may never select. In the measured corpus this is **zero** —
  every unknown reaches the default summary.

A probe is **fully proven** when it has no unknown claim, no export missing a
summary, no refused entrypoint, and no closure note. A **package** is fully
proven only when every one of its probes is: clean under Solid 1.x and full of
unknowns at a Solid 2 head is not a clean package.

An emitted contract that cannot be parsed is recorded as `measured: false` and
named in `unmeasuredProbes`, never as a row of zeroes — a hole reported as zero
unknowns is the one wrong answer this measurement could give.

### Headline numbers (2026-08-23, fifth measurement state, release binary, 416 probes)

Of the **409 probes that produced a contract**, covering 207 distinct packages.
This is the fifth measurement of the same 305-row / 416-probe manifest, and the
earlier four are kept beside it because **the numbers got worse again and that
is the result**. Nothing in the corpus, the analysis facts, or the harness
changed between any of them; what changed is how much the generator is willing
to certify.

The checked-in `benchmarks/ecosystem/report.{json,md}` are this state: the full
corpus from the release binary
`f0a80fc3240a72358d53fbe59b54d38b452f693f149d2b3cd7d73f4bf590cfb8` at a
600-second budget (94.495 s wall, against 94.675 s for the fourth state's
`8dde96e8…`). `report-sentinel.{json,md}` were **not** re-run and still describe
the third state; the sentinel figures quoted below are labelled accordingly.

| Figure | 2026-08-22 (first) | 2026-08-23 (second) | 2026-08-23 (third) | 2026-08-23 (fourth) | 2026-08-23 (fifth, current) |
| --- | --- | --- | --- | --- | --- |
| Probes fully proven | 300 / 409 (73.35%) | 304 / 409 (74.33%) | 288 / 409 (70.42%) | 229 / 409 (55.99%) | **205 / 409 (50.12%)** |
| Packages fully proven (every probe) | 126 / 207 (60.87%) | 128 / 207 (61.84%) | 111 / 207 (53.62%) | 91 / 207 (43.96%) | **86 / 207 (41.55%)** |
| Probes with at least one unknown claim | 102 | 99 | 116 | 177 | **201** |
| Probes with at least one refused entrypoint | 6 | 3 | 3 | 3 | **3** |
| Probes with at least one closure note | 7 | 7 | 7 | 7 | **7** |
| Exports proven | 5,415 / 8,113 (66.74%) | 6,520 / 8,320 (78.37%) | 6,095 / 8,358 (72.92%) | 5,477 / 8,358 (65.53%) | **5,410 / 8,358 (64.73%)** |
| Exports carrying an unknown | 2,698, of which 2,077 in all five domains | 1,800, of which 492 in all five | 2,263, of which 527 in all five | 2,881, of which 528 in all five | **2,948, of which 528 in all five** |
| Unknown claims, total | 11,013 | 4,898 | 5,903 | 6,672 | **6,776** |
| Entrypoints | 847 emitted, 7 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | **850 emitted, 4 refused** |
| Closure notes | 32 | 32 | 32 | 32 | **32** |
| Outcome classes | 403 / 6 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | **406 / 3 / 7** |

Unknown claims by domain — read together, not separately, since 528 of the
2,948 unknown exports appear in every column:

| Domain | 2026-08-22 | 2026-08-23 (second) | 2026-08-23 (third) | 2026-08-23 (fourth) | 2026-08-23 (fifth) |
| --- | --- | --- | --- | --- | --- |
| callbacks | 2,205 | 630 | 693 | 1,368 | **1,472** |
| reactiveReads | 2,577 | 1,657 | 2,019 | 2,065 | **2,065** |
| returns | 2,077 | 1,627 | 2,136 | 2,182 | **2,182** |
| ownerRequirements | 2,077 | 492 | 527 | 528 | **528** |
| asyncBehavior | 2,077 | 492 | 528 | 529 | **529** |

Positive behavioral rows a probe step would have to drive: 1,319 callback
executions, 1,202 return trees, 1,198 reactive reads, 542 owner requirements,
100 async behaviors — **4,361 rows**, against 4,812 in the fourth state, 5,005 in
the third, 5,545 in the second and 4,199 in the first.

**The fourth → fifth movement is the callbacks domain and nothing else.**
`reactiveReads`, `returns` and `ownerRequirements` are unchanged to the claim,
`asyncBehavior` is unchanged, the outcome classes are unchanged probe-for-probe,
and the entrypoint and closure-note figures are identical. What moved is the two
generator fixes in this change set that can move a content figure — a parameter
carrying two different `execution` values within one analyzed target, and the
cross-target merge unioning contradictory callback rows, both of which now open
`callbacks: {"status":"unknown"}` on the declaring export. It shows up as the
same near-identity the fourth state's retained-callback sentinel did: **exports
proven −67, `callbacks` unknowns +104**, the 37-claim difference being exports
that already carried an unknown in another domain and were therefore already
counted.

`callbackExecution` rows fell 1,764 → **1,319**, a −445 that is much larger than
the 104 exports involved, and that ratio is the shape of the fix rather than a
discrepancy: the sentinel is **per export**, so one contradicted parameter
discards every callback row of that export — including the parameters that never
disagreed. The width is deliberate and is recorded as unresolved in the precision
backlog ("[the contradiction sentinel is per export, which is wider than the
contradiction](precision-backlog.md)"). Owner-requirement rows fell 548 → 542 for
the same per-export reason, the only other row kind touched at all.

**24 probes lost fully-proven status and none gained it**, and the loss is
concentrated in Solid Primitives (176 → 152 fully proven), which is the family
whose small single-purpose exports most often invoke one parameter from two
sites. This is a loss of certified surface bought deliberately: the verification
measurement below is where the same two fixes are visible as a gain, and the
trade is stated there.

**No outcome class regressed.** 406 complete / 3 partial / 3 install failures /
2 `no-esm-runtime-target` / 1 `cjs-only-entrypoint` / 1 `no-exported-surface`,
probe-for-probe identical to the fourth state.

#### How the earlier states moved (history)

The four transitions below are kept because each records a cause that was
measured rather than assumed, and because together they are the record of a
generator that has been getting *less* willing to certify on every pass.

**The third → fourth movement is one change, and the arithmetic says so.** Two
generator fixes landed between the states — an exported class is `kind:
"function"` rather than `"value"`, and a callback parameter a function *retains*
rather than calls now opens `callbacks: {"status":"unknown"}` — and only the
second can move a content figure, because the first changes a claim's value and
not whether it is claimed. It shows up as an almost exact identity: **exports
proven −618, `callbacks` unknowns +675**, across 98 probes, with the 57-claim
difference being exports that already carried an unknown in another domain and
so were already counted. `callbackExecution` rows fell 1,981 → 1,764 by the same
mechanism: a proven row replaced by the sentinel is a row a probe no longer has
to drive because there is no longer a claim there to confirm.

Per probe, the movement is concentrated where retention is a coding style:
`@tanstack/solid-db@0.2.37` (−113 exports proven), `@tanstack/charts@0.14.0`
(−99), both `@solidjs/web@2.0.0-rc.1` probes (−40 each), `solid-js@1.9.14`
(−36), both `@solidjs/signals@2.0.0-rc.1` probes (−24 and −23). Every one of
those loses exactly as many proven exports as it gains `callbacks` sentinels.

**59 probes lost fully-proven status and none gained it.** That is the expected
shape for a fail-closed widening and the check that nothing moved the other way:
an export whose contract said "invokes no caller-supplied function" while the
package retains the callback and calls it later was a *certified negative that
was false*, and every consumer of that claim was being told something the
package contradicts on every use. The account is in the precision backlog
("[Generated contracts contradicted by the runtime
probe](precision-backlog.md)"). The verification measurement below is where the
same fix is visible as a gain rather than a loss.

**No outcome class regressed.** 406 complete / 3 partial / 3 install failures /
2 `no-esm-runtime-target` / 1 `cjs-only-entrypoint` / 1 `no-exported-surface`,
probe-for-probe identical to the third state.

**Why the second state's numbers were too good.** They were measured after
contract *emission* stopped erasing claims it had already proven — an
unresolved dispatch used to mark all five domains of every export of the
entrypoint, and now marks `reactiveReads` and `returns` on exactly the exports
that can reach it — and **before** two rounds of soundness fixes to the
attribution ladder that followed. Those fixes found that the ladder's
fail-closed guarantees were not guarantees: six shapes published an export
whose behavior depends on an unresolved obligation with the affected domain
simply *omitted*, which schema v1 reads as a certified negative. Every one of
them made "exports proven" larger by certifying something the analyzer had not
proven. The improvement recorded in the second state is real in direction; part
of its magnitude was that inflation, and this state is the one to compare
future work against.

**Where the movement comes from.** The second → third difference was measured
by re-running the full corpus twice against the current binary, once with and
once without the contract-merge change, so the two causes are separated rather
than assumed:

| Cause | Probes fully proven | Exports proven | Unknown claims | Probes touched |
| --- | --- | --- | --- | --- |
| second state | 304 | 6,520 / 8,320 | 4,898 | — |
| engine soundness rounds | 289 | 6,204 / 8,358 | 5,794 | 48 |
| conditional-merge one-sided fix | 288 | 6,095 / 8,358 | 5,903 | 8 |

- **The engine soundness rounds cost 15 fully-proven probes and 316 proven
  exports**, across 48 probes. Two changes landed in the same binary and the
  benchmark cannot separate them from outside without rebuilding at an
  intermediate revision, which this pass did not do: the six under-marking
  fixes in the attribution ladder, and the `.d.ts` fail-closed widening — a
  declaration file beside an internal runtime module makes every importer bind
  to the declaration, so the implementation's caller edges vanish and the
  enumeration now reports itself incomplete instead of publishing the exports
  as certified. Both push the same way, so the aggregate is their sum with
  nothing cancelling. The widening is expected to dominate, because it applies
  to the shape almost every published package has and it widens to *every*
  export of an affected entrypoint. Both accounts are in the precision backlog
  ("[Closed 2026-08-23: under-marking in the attribution
  ladder](precision-backlog.md)" and "[Closed 2026-08-23: a declaration sibling
  no longer certifies what it hid](precision-backlog.md)"). Movement was not
  uniformly downward — `@solid-primitives/range` and `@tanstack/solid-table`
  gained a fully-proven probe, and `@solid-devtools/shared` gained 38 exports —
  which is what a change to *which modules are enumerated* looks like rather
  than a blanket widening.
- **The conditional-merge one-sided fix costs 109 proven exports**, exactly:
  108 `returns` sentinels and 1 `asyncBehavior`, across 8 probes —
  `solid-js@1.9.14` (−24), both `solid-js@2.0.0-rc.1` probes (−14 each), both
  `@solidjs/web@2.0.0-rc.1` probes (−22 each), `@kobalte/core@0.13.13` (−11),
  `@tanstack/solid-router@2.0.0-rc.1` (−1) and
  `@solid-primitives/visibility-observer@2.0.1` (−1, the only probe that lost
  its fully-proven status to this cause). The merge used to hand the
  environment-unaware base one branch's proven `returns` when the other branch
  proved *none* — and in a proven summary an absence is a certified negative,
  not an absence of knowledge, so the base was false in that environment. The
  exact per-branch claims survive as `variants`, so an environment-aware
  consumer loses nothing.

No outcome class regressed across that transition either: 406 complete /
3 partial / 7 failures in both the second and the third state,
package-for-package identical. Every export that moved moved from "certified"
to "unknown", which is the only direction a soundness fix is allowed to move
one — and the same is true of the third → fourth transition above.

The pinned sentinel subset (23 probes, debug binary, default 300-second budget)
tracked the second → third transition the same way: 22 complete / 0 partial /
1 failure in both states, 7 probes fully proven in both, with exports proven
falling 758 → **643** of 2,185 and unknown claims rising 4,168 → **4,370**.
`report-sentinel.{json,md}` have not been re-run since, so those figures still
describe the third state; the retained-callback sentinel and the two
contradiction sentinels are expected to move them the same way they moved the
full corpus.

### Per family

2026-08-23 fifth state, matching the headline table above; the fourth state's
column is kept beside it because the whole movement is in one family:

| Family | Contracts | Fully proven (fourth → fifth) | Exports proven (fourth → fifth) | Unknown claims (fourth → fifth) |
| --- | --- | --- | --- | --- |
| Official Solid | 23 | 3 → **3** (13.04%) | 1120 → **1114** / 1546 | 585 → **601** |
| Kobalte | 4 | 0 → **0** | 364 → **363** / 1206 | 2,266 → **2,269** |
| Solid Primitives | 288 | 176 → **152** (52.78%) | 1684 → **1638** / 2038 | 642 → **693** |
| Corvu | 28 | 23 → **23** (82.14%) | 229 → **229** / 266 | 74 → **74** |
| TanStack | 50 | 24 → **24** (48%) | 1575 → **1561** / 2124 | 990 → **1,024** |
| Solid Devtools | 10 | 3 → **3** (30%) | 206 → **206** / 233 | 60 → **60** |
| Solid Recharts | 3 | 0 → **0** | 16 → **16** / 327 | 639 → **639** |
| Motion for Solid | 3 | 0 → **0** | 283 → **283** / 618 | 1,416 → **1,416** |

Only Solid Primitives lost a fully-proven probe — all 24 of the corpus's losses
are in it — while Official Solid, Kobalte and TanStack lost exports without
losing a probe that was already clean. Corvu, Solid Devtools, Solid Recharts and
Motion for Solid are unchanged export-for-export, which keeps them the useful
controls: neither sentinel reaches them. *Why* — whether they never invoke one
parameter twice, or invoke it twice with the same schedule — is not established
by this measurement; it is a per-package question the report has no field for.

**Solid Primitives is still the clean end of the ecosystem** and it is also the
largest family: 288 of the corpus's 409 contracts, zero refusals and zero
closure notes. It is also where every loss lands, in every round: 230 → 217 fully
proven to the soundness rounds, 217 → 176 to the retained-callback sentinel, and
176 → **152** here. The small-single-purpose-package shape is what the generator
handles best *and* the shape most likely to invoke one parameter from two call
sites, which is exactly what the new sentinel refuses to average.

**The remaining unknowns still concentrate in two packages, and they are still
not one summary shape.** `@kobalte/core@2.0.0-alpha.0` and
`motion-solidjs@0.7.0-beta.4` are roughly half the corpus total between them.
Their 1.x halves report a dominant cause of `reactiveReads` and `returns`
rather than `all-domains`, which is the shape the collapse fix was for: the
obligation is real, and it costs the two domains it actually invalidates
instead of five.

**TanStack's unknowns were never its options-object callback pattern.** The
second state had them nearly gone — 98.21% of exports proven, 111 unknown claims
— and the reading that 318 of its 322 unknown exports had been the all-five
whole-summary shape was correct: it was measuring the attribution defect, not
TanStack. The three fail-closed rounds since have taken it to **1,561 / 2,124
(73.5%) proven and 1,024 unknown claims**, all of it retained or
multiply-scheduled callbacks rather than the options-object pattern. Both
`@tanstack/solid-query` majors still
declare a non-standard `"@tanstack/custom-condition": "./src/index.ts"` branch
pointing at TypeScript source; that remains the only structural oddity in the
family.

### Caveats, stated because these numbers are easy to over-read

- **This measures the content of a generated draft, not consumer findings.** An
  unknown claim becomes a finding only when a consumer actually touches that
  surface. A package with 452 unknown exports costs a project nothing if it
  imports two proven ones. The corpus has no demand model, so nothing here
  predicts how often a real project would hit an uncertifiable result — that
  would need the same measurement driven from real import sites.
- **"Proven" here means "claimed", not "verified".** Every claim counted as
  proven is `inferred` evidence sitting below the SC9005 trust ceiling, awaiting
  review. See "[Open: generation success is not contract correctness](precision-backlog.md)":
  a contract asserting that `map` never invokes its callback counts as fully
  proven by this measurement and is false.
- **Probe drivability is not measured.** The behavioral-row counts say how many
  positive rows exist, not how many a probe harness could actually execute — no
  attempt was made to drive any of them.
- **A closure note blocks byte-attestation regardless of how clean the claims
  are.** 7 probes and 32 notes: those contracts describe bytes nobody
  enumerated, so a machine-verification scheme cannot bind them to an artifact
  at all. This is now the largest remaining blocker, and it did not move.
- **An unknown claim still does not say why it is unknown, in the contract.**
  Schema v1 cannot carry that. The reason — which obligation, where, and how
  emission decided the claim belonged to that export — is on the matching
  `unknown-sentinel` item of `<contract>.review.json`, under `because`. Reading
  the benchmark's unknown counts without that sidecar tells you how much is
  unproven and nothing about what would close it.
- **Per-probe, not per-package, unless the row says otherwise.** A package with
  a Solid 1.x and two Solid 2.x probes contributes three rows to every probe
  figure above.

## Machine verification across the corpus

Everything above stops at a generated draft. This section measures the whole
RFC 0002 pipeline instead — `contract generate` → `contract probe --write` →
`contract verify` — and answers the question a consumer actually asks: **how
many real ecosystem packages machine-verify end to end?** It is RFC 0002's
unresolved question 1, measured at corpus scale rather than argued.

It also closes the "**probe drivability is not measured**" caveat above. It is
now measured, and the answer is in "Drivability" below.

> **This measurement executes package code.** `contract probe` imports and runs
> each installed package, and its dependencies, in child processes — that is
> exactly why it is a separate command from `contract generate`, which imports
> nothing. The harness keeps every install and every execution inside temporary
> directories under its own state directory, runs `npm install` with
> `--ignore-scripts` so no package lifecycle script ever executes, and bounds
> every probe with both a per-condition-mode child timeout and a whole-phase
> wall budget. Run it where you would run those packages' own test suites.

### Method

`scripts/ecosystem-benchmark/verify-corpus.mjs` is a standalone harness. It
reads the same pinned manifest and reuses `lib/install.mjs`, `lib/classify.mjs`
and `lib/families.mjs`, but it is deliberately **not** `run.mjs`: that harness's
checked-in reports measure contract *generation*, and folding a verification
measurement into them would change what its numbers mean.

Per manifest probe row it installs the package into a throwaway project, along
with the Solid runtime the manifest row pins and the non-optional peers the
installed artifact itself declares, generates the contract outside the install
tree, probes it with discovery **on** (`contract verify` refuses a
`--no-discovery` report outright, so turning it off would measure nothing),
writes evidence, and verifies. Every completed row is appended to a journal, so
an interrupted run resumes rather than redoing work, and `--aggregate-only`
rebuilds the reports from an existing journal.

```sh
SOLID_CHECKER_NATIVE_BIN=... SOLID_TYPEFACTS_BIN=... \
  node scripts/ecosystem-benchmark/verify-corpus.mjs --state-dir /path/to/scratch
```

Reports: `benchmarks/ecosystem/verification-report.json` and
`verification-report.md`. They never overwrite `report.json`/`report.md`.

Three properties of the environment are configurable *because they change what
the numbers are a measurement of*, and each is recorded in the report's
`budgets` block:

- **The import-environment shim** (`--no-environment-shim` disables it). The
  probe worker defines a minimal inert browser surface — `window`, `document`,
  `navigator` and thirteen more — before it imports anything, in the `client`,
  `development` and `production` sessions only. Server sessions are never
  shimmed. See [package-contracts.md](package-contracts.md#probing-a-generated-contract)
  for the full premise; the short version is that a claim observed under the
  shim is a **weaker** observation than one observed in a browser, and the
  probe report and verify sidecar both name the globals that were faked.
- **Peer-complete installs** (`--no-peer-install` disables it). The row's pinned
  Solid runtime is completed with `@solidjs/web` where a Solid 2 row pinned only
  `solid-js`, and the artifact's own declared non-optional peers are installed
  in a second npm invocation so no peer range can take part in resolving a
  pinned version.
- **A per-row probe budget** (`--probe-budget <MS>` pins it flat instead). The
  whole-phase wall budget is 90 s + 500 ms per planned claim, capped at 900 s,
  so a wide-surface package gets proportional time instead of a budget sized for
  the median one.

**A timeout is never a verification result.** A row whose probe exceeded the
wall budget is its own outcome class and is counted as neither verified nor
refused. So is a row for which no Solid runtime can honestly be chosen.

### Measured state (2026-08-23, execution-kind pass, full corpus, 416 probe rows)

Binaries were **copied out of the repository before the run and used from the
copies**, so a concurrent rebuild could not change the engine mid-measurement.
The hash is the identity these numbers belong to:

- native `solid-checker-rust`
  `f0a80fc3240a72358d53fbe59b54d38b452f693f149d2b3cd7d73f4bf590cfb8`
  (14,602,048 bytes, source `rust/target/release/solid-checker-rust`)
- `solid-typefacts`
  `2bbdef833749ed8c9fdda60ed9245b54baeaa9ceb98b1a880853a2c90ac56f2d`
  (28,389,218 bytes, source `bin/solid-typefacts`)

Budgets: install 240 s, generate 120 s, probe 20 s per condition mode and
90 s + 500 ms per planned claim (cap 900 s) for the whole phase, verify 90 s;
concurrency 6. No subsetting — every one of the manifest's 416 probe rows ran.
Wall clock 7 m 10 s.

**This supersedes the earlier 2026-08-23 state**, which supersedes 2026-08-22;
both are kept as labelled columns because the movement between them is the
result. The middle column is the state the previous change set left behind and
the one the checked-in reports carried until this run:

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (execution kinds, current) |
| --- | --- | --- | --- |
| Probe rows run | 416 | 416 | 416 |
| Reached a generated contract | 409/416 (98.32%) | 409/416 (98.32%) | 409/416 (98.32%) |
| **Reached `verified`** | **194/416 (46.63%)** | **222/416 (53.37%)** | **261/416 (62.74%)** |
| Reached `verified`, of the rows that produced a contract | 194/409 (47.43%) | 222/409 (54.28%) | 261/409 (63.81%) |
| Refused by `contract verify` | 210/416 (50.48%) | 185/416 (44.47%) | 146/416 (35.10%) |
| Claims planned | 11,444 | 13,206 | 12,944 |
| Claims driven | 6,039 (52.77%) | 7,809 (59.13%) | 7,647 (59.08%) |
| Claims that passed | 5,686 | 7,591 | 7,584 |
| Claims that failed | 353 | 218 | 63 |
| — of which a wrong `callbacks[].execution` | not measured | 159 | **10** |
| Incompleteness findings | 1,091 | 1,080 | 734 |
| Exports certified by a verified contract | 449 | 752 | 1,018 |
| Verified rows carrying a probed behavioral row | 6 | 15 | **3** |
| Probed behavioral row markers kept | 12 | 25 | **3** |
| Probe timeouts | 3 | 0 | 0 |
| Never reached verification | 3 install, 4 generation, 2 probe errors, 3 timeouts | 3 install, 4 generation, 2 no-runtime, 0 timeouts | 3 install, 4 generation, 2 no-runtime, 0 timeouts |

Solid 1.x verifies at 98/168 (58.33%) and Solid 2.x at 163/248 (65.73%), against
49.40% and 56.05% in the previous state and 41.67% and 50.00% in the first.

**Read the two bold rows together or not at all.** The rate rose by 39 rows while
the probed behavioral evidence a verified contract carries fell by 12 rows and 22
markers, to three rows and three markers in the whole corpus. Both are the same
fix. Every one of the 12 rows that lost its markers converts the affected domain
with the same recorded reason — *"probed row evidence does not cover every mode
the claim is stated for (client, server, development, production); narrowing the
stated modes would claim semantics for an environment nobody observed"* — so what
those markers had been resting on is now visible: an observation in the `server`
mode, where both audited Solid releases resolve `node` to a build that re-runs
nothing (1.9.14's `dist/server.js` has an empty `createEffect`; 2.0.0-rc's
`flush()` is a no-op). In such a runtime `inline`, `tracked` and `deferred` are
indistinguishable, so a *matching* observation was never evidence. Those markers
were unearned. Losing them is the point; the residue — **3 of 261 verified rows
carry any observed behavior at all**, against 15 of 222 — is the honest floor
this measurement now reports, and it is worse than the one the previous state
advertised.

#### The staged decomposition (2026-08-23)

The change set has two independent halves and folding them into one number would
make it impossible to say which mattered, so the corpus was run **twice** more
against snapshotted binaries, one half at a time. Each stage is a full 416-row
run and each attribution is a per-row set difference between consecutive runs,
not a classification of deltas. Stage 1's engine is a release build of
`origin/main` (95270bee) in a separate worktree with only the three probe-side
files overlaid, so its generator is the previous state's exactly:

| Stage | Verified | Δ | Failing claims | of which execution-kind | What changed |
| --- | --- | --- | --- | --- | --- |
| baseline (previous state) | 222/416 (53.37%) | — | 218 | 159 | — |
| + probe-side fixes | 243/416 (58.41%) | **+21 / −0** | 106 | 47 | control interval, first-run/transitive-subscription withdrawal, inert-runtime withdrawal |
| + generator-side fixes | **261/416 (62.74%)** | **+18 / −0** | 63 | 10 | clearing-wrapper vocabulary, tracked-callback schedule, both contradiction sentinels |

- **The probe-side half is +21 rows and no losses**, and it is a withdrawal, not
  a discovery: it removes 112 failing claims by declining to classify
  observations whose counters name no execution mode. 124 claims moved into three
  new `undriven` buckets — 57 `runtime re-runs nothing in this mode`, 51 `callback
  ran more often than the call site`, 16 `callback re-ran with nothing written` —
  while the pre-existing `callback ownership ambiguous in the driver's read scope`
  bucket fell 23 → 5. That last movement is a count, not an explanation: the
  report has no field attributing a claim's bucket in one run to its bucket in
  another. Corpus-wide `probe-failed` as a root cause falls 75 → 46 rows.

  Two of the six new withdrawal reasons **never fired**: `the callback had not run
  by the time of the write` and `the observation reports no count for the settle
  interval` are zero across all 416 rows in both stages. The second is fail-closed
  on a malformed observation and zero is the expected reading. The first is not:
  it exists for a callback whose only run lands in the write interval, and the
  control settle the same change adds appears to absorb that shape entirely — a
  late first run now lands in the control interval and reads `deferred`. Zero is
  what was measured; that the control interval is *why* is not established by this
  measurement.
- **The generator-side half is +18 rows and no losses**, and it is the opposite
  kind of change: the contract now says something different rather than the probe
  saying less. Failing claims fall 106 → 63 and the execution-kind class falls
  47 → 10. The claim plan itself shrinks, 13,206 → 12,944, because a contradicted
  parameter's rows are replaced by one sentinel — which is the same −445
  `callbackExecution` rows the content measurement above records as a loss.
- **Neither half lost a verified row.** 21 then 18 rows moved `refused →
  verified` and none moved the other way, and no outcome class other than
  verified/refused changed at all (3 install failures, 4 generation failures, 2
  `no-runtime`, 0 timeouts in all three states).
- **One class of finding was lost rather than fixed, and it is not a callbacks
  finding.** All six `returns: claimed accessor, observed array` failures
  disappear in the generator stage, and none of them was corrected: they are
  `@solid-primitives/utils`'s `createHydratableSignal` and `createHydrateSignal`
  across three rows, both of which really do return a tuple against a contract
  that says `accessor`. They are now **undriven**, with the reason *"no plantable
  reactive source: proving the returned value is an accessor needs a signal read
  inside a callback the contract states, and this export states none."* The
  callbacks sentinel is what removed the callback the returns probe was planting
  through. Nothing false is published — verification converts the `returns`
  domain to unknown, so the wrong `accessor` claim does not survive into the
  verified contract — but the *defect* is now invisible to this measurement, and
  three of the generator stage's eighteen gains are rows that verified partly
  because of it. The coupling is a fact about the driver rather than about these
  packages — a `returns: accessor` claim is driven *through* a stated callback —
  and it is the only cross-domain coupling this measurement observed. Recorded in
  the precision backlog.
- **`kind-observed` is now the largest single root cause** — 77 rows, against 71
  and 82 — because it is the one blocker neither half touches: `kind` has no
  unknown sentinel, so a mode with no observation refuses regardless of how
  honest the callbacks claims have become. The 53 `kind: claimed value, observed
  function` failures are unchanged to the claim across all three states.

#### Where the earlier +28 came from, decomposed by cause (2026-08-22 → 2026-08-23)

The engine and the probe environment both changed, and folding the two into one
number would make it impossible to say which mattered. So the corpus was run
**four times** against the same two snapshotted binaries, turning one group of
changes on at a time. Each step is a full 416-row run, not a sample, and the
attribution below is a per-row set difference between consecutive runs rather
than a classification of deltas:

| State | Verified | Δ | What changed |
| --- | --- | --- | --- |
| 2026-08-22 baseline | 194 | — | — |
| + engine fixes | 214 | **+20 / −0** | class kind encoding; retained-callback sentinels |
| + probe-worker abort guard | 217 | **+3 / −0** | an asynchronous package throw no longer costs the whole mode |
| + import shim, peer-complete install, scaled budget | **222** | **+12 / −7** | the environment half |

- **The engine fixes are pure gain: +20 rows, nothing lost.** Both act on the
  same failure — a contract that says something the package contradicts. The
  class fix removes 49 `kind: claimed value, observed function` failures
  outright; the retained-callback sentinel replaces a false certified negative
  with an honest unknown, so the claim the probe used to contradict is no longer
  made. Corpus-wide, failing claims fall 353 → 185. This is the same change that
  costs 618 proven exports in the content measurement above: it buys
  verification rate with certified surface, and that trade is the point of the
  fix rather than a side effect of it.
- **The abort guard is +3 and nothing lost.** Package code the probe set running
  — a deferred callback, a rejected promise — throws outside every `try` the
  worker has. The process used to die with status 1 and an empty stdout, so the
  parent had *no* results for that mode and did not restart. It now answers with
  what it observed and the abort reason, and the parent restarts for the
  remainder. `@solid-primitives/autofocus`, `@solid-primitives/clipboard` and
  `@tanstack/solid-devtools` verify because of it.
- **The environment half is +12 / −7, a net +1 on the headline** — and that
  small net is the honest result rather than a disappointment. What it actually
  bought is **observation**: claims driven rise 6,257 → 7,809 (+1,552), rows
  with an entrypoint import throw fall 55 → 34, exports certified rise 672 →
  752, and the three probe timeouts go to zero. More observation surfaces more
  contradictions as well as more confirmations, and a single contradiction
  refuses a whole contract — so the rate barely moves while the evidence under
  it roughly doubles. `probe-failed` as a root cause rises 65 → 75 in exactly
  that way, while `kind-observed` falls 82 → 71.

**The seven rows the environment half lost, each investigated.** None is a
false alarm produced by the harness; all seven are things the bare-Node
environment was hiding, though two of them are hidden *by* the fake DOM in turn:

| Row(s) | Now refuses on | Reading |
| --- | --- | --- |
| `@solid-primitives/fullscreen` (both Solid 2 probes) | `probe-failed`: `.:createFullscreen callbacks[0]=tracked`, observed `inline` | A real disagreement. The export was unreachable before because the module needs a DOM to import. |
| `@solid-primitives/resize-observer` (both Solid 2 probes) | `incompleteness`: `createResizeObserver` invoked the parameter-0 callback (observed `tracked`) and the contract states no such claim | A real negative-claim falsification, reachable only once `ResizeObserver` exists. |
| `@solid-primitives/pagination@0.5.2` | `probe-failed`: `.:createInfiniteScroll callbacks[0]=deferred`, observed `inline` | **Possibly the shim's doing.** The fake `IntersectionObserver` never fires, so a callback that a browser would run on intersection ran only at setup. Candidate for the driver's existing "the driver's own scaffolding could explain this" undriven rule. |
| `@solid-primitives/interaction` (both Solid 2 probes) | `kind-observed` | Not the shim, and not the engine: the package reads `el.ownerDocument` on the element the *caller* passes, and the driver synthesizes `{}` there. The shim only let execution get far enough to reach the limit. |

Five of the seven are findings the measurement should want. Two —
`pagination`, and arguably the `resize-observer` pair — sit on the line where an
inert fake changes an answer, which is precisely why every probe report and
verify sidecar records the globals that were faked instead of leaving the
reader to assume a browser.

### Per family

2026-08-23 execution-kind pass, with the previous state's verified count beside
each row:

| Family | Rows | Contracts | Verified (previous → current) | Refused | Dominant root cause |
| --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 23 | 7 → 7 (30.43%) | 14 | `kind-observed` 7, `probe-failed` 3, `incompleteness` 2, `closure-note` 2 |
| Kobalte | 6 | 4 | 1 → 1 (16.67%) | 3 | `incompleteness` 2, `probe-failed` 1 |
| Solid Primitives | 289 | 288 | 193 → **221** (76.47%) | 67 | `kind-observed` 28, `incompleteness` 27, `probe-failed` 12 |
| Corvu | 28 | 28 | 7 → **16** (57.14%) | 12 | `kind-observed` 9, `incompleteness` 3 |
| TanStack | 52 | 50 | 11 → **13** (25.00%) | 37 | `kind-observed` 23, `probe-failed` 10, `incompleteness` 4 |
| Solid Devtools | 12 | 10 | 2 → 2 (16.67%) | 8 | `kind-observed` 7, `probe-failed` 1 |
| Solid Recharts | 3 | 3 | 0 → 0 (0%) | 3 | `kind-observed` 3 |
| Motion for Solid | 3 | 3 | 1 → 1 (33.33%) | 2 | `incompleteness` 2 |

Corvu moves the most in proportion — 7/28 to 16/28, every one of the nine in the
probe-side stage — and Solid Primitives moves the most in absolute terms, 193 to
221 across both stages. The four families that did not move are held by
`kind-observed`, which neither half of this change set touches: across Official
Solid, Solid Devtools and Solid Recharts it is the root cause of 17 of their 25
refusals, and `probe-failed` has disappeared from Corvu and Solid Recharts
entirely.

### Why verification refuses

`contract verify` raises every blocker it finds, so a row can carry several. The
row counts are the number of refused rows raising each blocker at least once:

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines | Previous state (rows / lines) |
| --- | --- | --- | --- |
| `kind-observed` | 106 | 357 | 107 / 358 |
| `probe-report-includes-evidence-write` | 63 | 63 | 108 / 108 |
| `incompleteness` | 45 | 734 | 59 / 1,080 |
| `probe-failed` | 27 | 63 | 75 / 218 |
| `closure-note` | 7 | 32 | 7 / 32 |

`probe-report-includes-evidence-write` is a **consequence, not a cause**:
`contract probe --write` declines to write evidence once a probe failed or
reported an incompleteness, so verification then sees passing claims that never
reached the contract. Attributed to one root cause per row instead:

| Root cause | Refused rows | Previous state | 2026-08-22 |
| --- | --- | --- | --- |
| `kind-observed` | 77 | 71 | 82 |
| `incompleteness` | 40 | 37 | 42 |
| `probe-failed` | 27 | 75 | 84 |
| `closure-note` | 2 | 2 | 2 |

**No row refused with the evidence-write blocker standing alone**, which is the
check that the consequence really is one. `probe-failed` has gone from the
largest root cause to the smallest but one, and `kind-observed` is now the
binding constraint on the corpus's rate.

Three blockers, in plain terms:

- **`probe-failed` (27 rows, was 75)** — the package does not behave the way the
  contract says. Real disagreements, and the most valuable output of the whole
  measurement. **63 failing claims in all, against 218**, grouped by shape rather
  than printed as sentences: `kind: claimed value, observed function` 53,
  `callbacks[n]: claimed deferred, observed tracked` 3, `callbacks[n]: claimed
  inline, observed tracked` 3, `callbacks[n]: claimed tracked, observed inline` 3,
  `callbacks[n]: claimed deferred, observed inline` 1. **Wrong execution kind is
  no longer the dominant visible defect class**: it was 159 of 218 and is now
  **10 of 63**, while the 53 `value`/`function` failures are unchanged to the
  claim and are now 84% of everything left. Every one of the 10 is named
  individually in the report with its export, claim, observed value and the modes
  it failed in, because "deferred in server only" and "deferred everywhere" are
  different findings.
- **`kind-observed` (77 rows as root cause)** — `kind` is the one claim schema
  v1 has no unknown sentinel for, so verification requires a passing `kind`
  observation in *every* mode an export is stated for. This was 82 rows and the
  single largest cause; the environment work is what moved it, by making the
  module importable in the mode where the observation was missing.
- **`incompleteness` (40 rows)** — discovery planted a callback where the
  contract states none, and the package invoked it. A negative claim a probe
  falsified is wrong, not incomplete, so this refuses rather than converting. The
  *findings* fell hard, 1,080 → 734 blocker lines, because a discovery finding is
  now withdrawn when the runtime it was observed in re-runs nothing — the same
  reason a claim is. The refused-row count rose anyway, 37 → 40, because rows
  whose other blockers cleared are now attributed here.

### Drivability

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (current) |
| --- | --- | --- | --- |
| Claims planned across every probed contract | 11,444 | 13,206 | 12,944 |
| Driven | 6,039 (52.77%) | 7,809 (59.13%) | 7,647 (59.08%) |
| Passed | 5,686 | 7,591 | 7,584 |
| Failed | 353 | 218 | 63 |
| Undriven | 5,405 (47.23%) | 5,397 (40.87%) | 5,297 (40.92%) |
| Incompleteness findings | 1,091 | 1,080 | 734 |

The planned total rose in the previous state because `@solidjs/web` being
installed changes which dependency contracts `contract generate` can resolve, so
the contracts themselves were larger. It **falls** here, 13,206 → 12,944, for the
opposite reason: the contradiction sentinels replace a parameter's rows with one
unknown, so there is less to plan.

Passed claims fall by only 7 corpus-wide and that flatness hides real movement:
per row, 20 rows lost 52 passing claims and 18 gained 45. The rows that lost them
are mostly the rows that *became verified* — `@solid-primitives/timer@1.4.5-next.1`
verifies with 7 passing claims where it previously refused with 13 — which is the
shape a withdrawal is supposed to have. The gains are the mirror image: a claim
whose only failing observation was withdrawn is confirmed by the modes that
remain, so it moves from `failed` to `passed` rather than to `undriven`.

The undriven half still splits into two very different things. **2,818 claims
have no probe form at all** — `reactiveReads` 1,354, `ownerRequirements` 565,
parameter identity 421, nested return leaves 257, `asyncBehavior` 100, no
unambiguous summary for the mode 85, callback arguments 13, store paths 23 — and
no probe harness will ever reach them; they are static claims or claims schema v1
has no evidence slot for. The rest is environment and attribution: 637 claims
lost to an entrypoint that **threw on import**, 335 to a synthesized call that
threw, 222 to a synthesized call that never invoked the callback, 213 to no
plantable reactive source, 91 to a session that wrote no readable report, 56 to a
per-mode timeout — plus the three new withdrawal buckets, **49 to a runtime that
re-runs nothing, 25 to a callback that ran more often than the call site, and 6 to
a callback that re-ran with nothing written**. Those 80 are the claims this change
set stopped answering, and they were the ones being answered by construction.

34 rows still had at least one entrypoint import throw — the same 34 in all three
states, because nothing in this change set touches the probe environment. Two
rows of the table below move anyway, and only because the claim plan shrank
underneath them: `Cannot read properties of null (reading '_depth')` 54 → 50 and
`ERR_MODULE_NOT_FOUND` for `@solid-primitives/utils` 94 → 84. The counts are
claims lost to a throw, so a contract with fewer callback claims loses fewer of
them to the same unchanged throw. Two of the
previous top three causes are gone entirely: `ReferenceError: window is not
defined` (432 claims) and `ERR_MODULE_NOT_FOUND` for `@solidjs/web` (248) no
longer appear. What is left is dominated by things no install policy or shim can
supply:

| Import failure | Claims left undriven | Reading |
| --- | --- | --- |
| `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING` | 227 | an export map pointing at TypeScript source under `node_modules` |
| `ERR_MODULE_NOT_FOUND` for `@solid-primitives/utils` | 94 | an **undeclared** dependency — the package imports it and declares it nowhere |
| `ERR_PACKAGE_PATH_NOT_EXPORTED` for `./web` | 81 | the subpath the package imports is not in its own export map |
| `[solid-devtools]: Debugger hasn't found the exposed Solid Devtools API` | 66 | the package refuses to load outside its own runtime |
| `ERR_MODULE_NOT_FOUND` for `server-only` | 60 | undeclared again |
| `Cannot read properties of null (reading '_depth')` | 54 | `@solidjs/router` reaching for a Solid owner at module scope |
| `ERR_UNKNOWN_FILE_EXTENSION` for `.jsx` | 27 | uncompiled JSX shipped to npm |

The distinction that matters here: **a missing *peer* is the harness's gap and
is now closed; a missing *undeclared* import is the package's.** Completing an
undeclared import would mean this harness choosing a version the package never
named, so it does not.

### The probe environment, recorded rather than assumed

404 of the 416 rows had at least one session that faked at least one global
(393 in `client`, 404 in `development`, 393 in `production`; `server` never).
The same fifteen names were faked on every one of those rows, because Node
provides none of them and the shim is a fixed list: `window`, `document`,
`self`, `location`, `screen`, `history`, `localStorage`, `sessionStorage`,
`matchMedia`, `requestAnimationFrame`, `cancelAnimationFrame`,
`getComputedStyle`, `MutationObserver`, `ResizeObserver`,
`IntersectionObserver`. `navigator` is real in modern Node and was left alone.

**Every claim this corpus verified in a client mode was observed against that
fake.** The verify sidecar of each row records it, so a consumer reading a
`verified` contract can tell. Where it could matter it did: see the
`pagination` and `resize-observer` rows above.

Worker processes: 20,070 started, of which 18,487 were restarts after a probe
threw, and 75 sessions died (20,367 / 18,784 / 78 previously). A restart is not a
failure — it is the only way to un-halt a Solid 2.0 development runtime — but the
count was previously invisible except as an unexplained probe duration, and it is
the shape behind every slow row. `@kobalte/core@0.13.13` alone accounts for
hundreds. Each session now also carries the **capability** of the runtime that
drove it, `{"reruns": true|false}` per mode, so a mode's withdrawals are visible
as measured rather than reconstructed from a pile of per-claim reasons. Across
the corpus that answer is completely uniform: `server` answered `false` in all
393 rows that ran a server session, `client` and `production` answered `true` in
all 393 and `development` in all 404, and **no session was left unmeasured** —
there is not one `null` in the corpus.

Install environment: 53 Solid 2 rows were given the `@solidjs/web` half of the
runtime they pinned only half of, 27 rows had a peer install (37 peer packages
in total), and 4 rows' peer installs failed or moved a pin and were reverted to
the pinned-only tree.

### What a verified contract actually certifies

This is still the number that should govern how the 62.74% is read, and it is
the one place where this measurement is **worse** than the state it supersedes:

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (current) |
| --- | --- | --- | --- |
| Claim domains converted to unknown | 379 | 595 (`returns` 316, `callbacks` 267, `asyncBehavior` 12) | 739 (`returns` 407, `callbacks` 320, `asyncBehavior` 12) |
| Exports carrying an unknown in the verified rows, at generation | 150/880 (17.05%) | 797/1,885 (42.28%) | 939/2,434 (38.58%) |
| Exports carrying an unknown in the verified rows, after verification | 431/880 (48.98%) | 1,133/1,885 (60.11%) | 1,416/2,434 (58.18%) |
| Verified rows carrying at least one **probed behavioral row** | 6/194 (3.09%) | 15/222 (6.76%) | **3/261 (1.15%)** |
| Probed behavioral row markers kept across the whole corpus | 12 | 25 | **3** |
| Inferred row markers dropped by verification | 1,118 | 2,292 | 2,955 |
| Probed markers discarded as unwitnessed by this run's report | 11 | 29 | 105 |

**98.85% of verified contracts now certify no observed behavior at all**, against
93% before. The three surviving rows are `@tanstack/solid-query@5.101.4` and both
`@tanstack/solid-query@6.0.0-rc.0` probes, one marker each. The 12 rows and 22
markers that went are the ones whose evidence did not cover the `server` mode
once that mode's observations were withdrawn, and their `callbacks` domain is now
an honest unknown instead — which is why `callbacks` conversions rise 267 → 320
in the same run.

A verified contract in this engine state is overwhelmingly `kind` observations,
negative claims, and unknown sentinels, and this measurement says so with a
smaller number than the last one did. The gain is entirely in coverage: 261 rows
instead of 222, and 1,018 exports certified instead of 752.

### The composite a consumer feels

Of all 9,015 exports the corpus's generated contracts describe:

| State | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (current) |
| --- | --- | --- | --- |
| (a) certified by a verified contract | 449 (4.98%) | 752 (8.34%) | 1,018 (11.29%) |
| (b) honest unknown inside a verified contract | 431 (4.78%) | 1,133 (12.57%) | 1,416 (15.71%) |
| (c) inside a contract that never reached `verified` | 8,135 (90.24%) | 7,130 (79.09%) | 6,581 (73.00%) |

(c) is every export of a contract that was generated and then refused, timed
out, or errored. Rows whose install or generation failed describe no exports and
are in none of the three states. (a) and (b) together are now **27.00%**, from
20.91% and 9.76% — a movement in *coverage*, not in how much any one contract
claims, and the "certifies no observed behavior" figure above is the reason to
keep reading it that way.

### What it costs

| Phase | Rows | Median | p90 | Max |
| --- | --- | --- | --- | --- |
| `npm install` | 416 | 697 ms | 1,556 ms | 15,934 ms |
| `contract generate` | 413 | 110 ms | 573 ms | 15,611 ms |
| `contract probe` | 407 | 664 ms | 3,389 ms | 203,034 ms |
| `contract verify` | 407 | 48 ms | 56 ms | 121 ms |
| generate + probe + verify | 413 | **908 ms** | **4,077 ms** | 218,698 ms |
| whole row, install included | 416 | 1,652 ms | 5,752 ms | 221,203 ms |

Under a second at the median for the checker's own three phases, unchanged across
all three states. The maximum is the four wide-surface rows the scaled budget
rescued in the previous state — `@kobalte/core@0.13.13`,
`@tanstack/solid-table@9.1.2`, `motion-solidjs@0.7.0-beta.4`, `@kobalte/utils` —
still completing rather than timing out. The whole 416-row corpus took 7 m 10 s of
wall clock at concurrency 6, against 7 m 11 s and 6 m 30 s; the extra control
settle per callback observation is not visible in the total.

### Caveats

- **`verified` is not `reviewed`.** It certifies what a machine observed or
  statically proved and converts everything else to the unknown sentinel. It is
  a weaker claim than the human tier and a stronger one than the `inferred`
  draft everything earlier in this document measures.
- **Client-mode observations were made against a fake DOM.** The probe worker
  defines a minimal inert browser surface so that an import-time `window` read
  does not cost a whole entrypoint. What is then observed is the package's
  behavior *given that fake*, which is not the same fact as its behavior in a
  browser — an inert `IntersectionObserver` never fires, an inert `matchMedia`
  never matches. Every probe report and verify sidecar names the globals it
  faked; `server` sessions fake nothing.
- **A `typeof window` guard never threw**, so for modules that branch that way
  the shim *redirects* rather than rescues: a package that took its server path
  in every earlier measurement now takes its browser path.
- **The install is peer-complete, not project-complete.** A package that imports
  something it declares nowhere still fails to import, and that is a fact about
  the package rather than about this harness.
- **A timeout is never a verification result**, and neither is a row with no
  honestly-choosable Solid runtime. The two `@solidjs/signals` rows are recorded
  `no-runtime`: the manifest pins no `solid-js` beside them, `@solidjs/signals`
  *is* the reactive core, and pairing one in would be this harness auditing a
  combination the corpus deliberately did not.
- **Per probe row, not per package**, exactly as everywhere else in this
  document.
- **A callback claim driven by a server *build* is not verifiable in this corpus,
  in either direction.** Both audited Solid releases resolve the `node` condition
  to a build that re-runs nothing, and every session that measured one answered
  so (393 of 393), so a callback observation made in it is withdrawn whichever way
  it came out — 49 claims outright, and more as the withdrawn half of a
  multi-mode claim. The withdrawal is per *runtime*, not per mode: a `server`
  session probing `solid-js/jsx-dev-runtime` drives `dist/solid.js`, which
  re-runs normally, and those observations still count — four of the ten
  remaining execution-kind failures are exactly that. Reaching a server *build's*
  schedule needs a probe that drives its own scheduling, which does not exist.
- **This measurement executed package code.** Nothing here is a safety claim
  about any package.

## Exit-code contract

Benchmark mode (`run.mjs` without `--thresholds`) exits 0 whenever the
benchmark infrastructure itself completed, regardless of how many individual
packages failed generation — a package failing is the measurement, not an
infrastructure fault. It exits 2 only for an infrastructure failure: an
invalid manifest, a missing checker or Type Facts binary, a corrupted
install, a version or integrity mismatch, a harness crash, an unwritable
report, or a `--baseline` covering a different scope than the run. `--thresholds` mode is opt-in and exits 1 only when a supplied
threshold regresses; it is not enabled by default and is not a blocking
pull-request check today; enabling it is a deliberate, separate decision once
a baseline exists worth holding the ecosystem to.

## CI split

`.github/workflows/ecosystem-benchmark.yml` splits the same harness into two
jobs on purpose:

- **`sentinel`** runs on every pull request and every push to `main` that
  touches the analysis engine, the package-contract generator, or the
  benchmark harness itself. It runs only the pinned probe subset in
  `sentinel.json`, so it consults no registry metadata — but it still
  installs each of those probes' exact versions, and therefore still needs
  network access. What makes it cheap enough for the review loop is the size
  of the subset, not an absence of installs; it never installs or analyzes
  the full discovered corpus.
- **`full-corpus`** runs only on `workflow_dispatch` or a weekly schedule,
  never on a pull request or push. It refreshes-checks the manifest against
  the live registry (`discover.mjs --check`, which needs network access) and
  then runs every row's every probe, with a generous timeout because
  installing and analyzing the whole ecosystem is long-running by design.

Both jobs build the debug engine binary fresh in the job rather than trusting
a checked-in artifact, for the same reason the Makefile targets do (see
`scripts/ecosystem-benchmark/README.md`).
