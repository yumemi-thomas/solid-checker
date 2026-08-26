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
`bun scripts/ecosystem-benchmark/discover.mjs --print-diff` before trusting
a refreshed manifest — an add, removal, or integrity change should always be
inspected in context, especially an integrity change, which discovery always
surfaces rather than merging in silently.

Execution reads the manifest and never touches the network itself beyond the
Bun install invocations it needs per probe:

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
| `install-failure` | `bun install` for the probe's exact versions failed. |
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
collapse was fixed, and `406 / 3 / 7` held for four measurement states until the
2026-08-24 `kind` refusals made it `387 / 11 / 18`, which the attested-closure
state then held probe-for-probe. **The
checked-in reports under `benchmarks/ecosystem/` are always the current
measurement state**; the figures they carry are stated once, under
"[Headline numbers](#headline-numbers-2026-08-24-eighth-measurement-state-release-binary-416-probes)",
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

**As of 2026-08-24 it holds eleven real packages**, every one of them a package
whose *every* entrypoint was refused because an export's runtime `kind` could not
be proved (`has no certifiable runtime entrypoint`). A **partial** refusal is
classified from the generator's own "N entrypoint(s) refused and omitted" note and
lands in `partial-success` regardless of the reason text; only an all-refused
package reaches the fallback. Growing a class for it is a deliberate taxonomy
decision this measurement pass did not make on the classifier's behalf — and
`unclassified` retaining full stderr is exactly what made the eleven diagnosable
without a re-run.

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
  one says the module record could not be established: a module the analyzing
  program opened that the generator's seed never named, one it opened that the
  record's scope excludes, bytes it could not hash, or — defensively, and never
  yet observed — no attestation at all. The contract is bound
  to fewer bytes than it describes (see "[The runtime-module closure is walked,
  not attested](precision-backlog.md)", whose closing section records what
  attestation moved).
- **attested closure notes** — `generation.entrypoints[*].runtimeNotes`, counted
  separately on purpose. There the record *is* the analyzing program's own file
  list and complete for what it read; what is unbounded is the runtime — a
  non-literal dynamic `import()`, or an unselected conditional `imports` branch
  whose targets are real modules on disk, may load a module the analysis never
  read. Both kinds refuse promotion, so `fullyProven` reads their sum — but
  merging the counts would make attestation's effect on the corpus unmeasurable.
  The refusal classifier keeps them apart for the same reason: a row whose only
  blocker is this one has root cause `attested-closure-note`, not `closure-note`
  and not `unclassified-refusal`, and `verify-corpus.test.mjs` holds every blocker
  kind `contract verify` can raise to being nameable here.
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
summary, no refused entrypoint, and no closure note of either kind. A **package** is fully
proven only when every one of its probes is: clean under Solid 1.x and full of
unknowns at a Solid 2 head is not a clean package.

An emitted contract that cannot be parsed is recorded as `measured: false` and
named in `unmeasuredProbes`, never as a row of zeroes — a hole reported as zero
unknowns is the one wrong answer this measurement could give.

### Headline numbers (2026-08-24, eighth measurement state, release binary, 416 probes)

**The eighth state is the attested module closure, and it moves the closure-note
figures and nothing else.** Every other number in the table below is identical to
the seventh state to the claim — probes measured, probes and packages fully
proven, exports total and proven, unknown exports and unknown claims in all five
domains, entrypoints emitted and refused, every behavioral-row count, and the
outcome classes probe-for-probe. What moved: closure notes **31 → 5**, the new
`attestedRuntimeNotes` counter reads **17**, and probes carrying a closure note
fall **7 → 2**. `fullyProven` reads the sum of the two note fields, so 22 notes
across 6 probes still block promotion; **no probe gained fully-proven status and
none lost it** (125 in both states). That is the honest reading of a
record-honesty change: it made 6 probes' notes *say something different and
truer*, and it certified nothing new.

The nine notes that disappeared are enumerated under "[What the attested record
answered, note by note](#what-the-attested-record-answered-note-by-note)" below,
because a note dropping is the one movement here that could hide a false
certification, and each one has to be attributable.

Of the **398 probes that produced a contract**, covering 202 distinct packages —
and that denominator moving is itself the story: **eleven probe rows across nine
packages** that generated a complete contract in every earlier state now generate
none, because *every* entrypoint they declare publishes an export `kind` the
analyzer cannot prove and generation refuses it. Five of those nine packages
(`@solid-devtools/locator`, `@solid-primitives/cookies-store`,
`@solid-primitives/platform`, `@tanstack/ai-devtools-core`,
`@tanstack/solid-hotkeys-devtools`) leave the corpus's measured set entirely; the
other four (`@kobalte/utils`, `@solid-primitives/analytics`,
`@solid-primitives/audio`, `@solid-primitives/intersection-observer`) survive only
through a probe row on their other major. That denominator is unchanged in the
eighth state. This is the eighth measurement of the same 305-row / 416-probe
manifest, and the earlier seven are kept beside it because through the seventh
**the numbers got worse on every pass and that is the result**. Nothing in the
corpus, the analysis facts, or the harness changed between any of the first seven;
what changed is how much the generator is willing to certify. The eighth is the
first that changes what a *record* says rather than what a claim says.

The checked-in `benchmarks/ecosystem/report.{json,md}` are the eighth state: the
full corpus from the release binary
`0356938638a2d6594452dd574dcff4a82332b2f0a4d58abed21b578a759a3588` at a
600-second budget (97.054 s wall, against 100.820 s for the seventh state's
`ddb0ecd8…` and 94.955 s for the sixth's `068b04bb…`). The engine binary is a new
one — `--emit-module-inventory` is a new flag on it — but it adds an opt-in read
of an already-built program and changes no analysis fact, which is what the
identical claim figures below measure rather than assume.
`report-sentinel.{json,md}` were **not** re-run and still describe the third
state; the sentinel figures quoted below are labelled accordingly.

| Figure | 2026-08-22 (first) | 2026-08-23 (second) | 2026-08-23 (third) | 2026-08-23 (fourth) | 2026-08-23 (fifth) | 2026-08-23 (sixth) | 2026-08-24 (seventh) | 2026-08-24 (eighth, current) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Probes that produced a contract | 409 | 409 | 409 | 409 | 409 | 409 | 398 | **398** |
| Probes fully proven | 300 / 409 (73.35%) | 304 / 409 (74.33%) | 288 / 409 (70.42%) | 229 / 409 (55.99%) | 205 / 409 (50.12%) | 205 / 409 (50.12%) | 125 / 398 (31.41%) | **125 / 398 (31.41%)** |
| Packages fully proven (every probe) | 126 / 207 (60.87%) | 128 / 207 (61.84%) | 111 / 207 (53.62%) | 91 / 207 (43.96%) | 86 / 207 (41.55%) | 86 / 207 (41.55%) | 44 / 202 (21.78%) | **44 / 202 (21.78%)** |
| Probes with at least one unknown claim | 102 | 99 | 116 | 177 | 201 | 201 | 269 | **269** |
| Probes with at least one refused entrypoint | 6 | 3 | 3 | 3 | 3 | 3 | 11 | **11** |
| Probes with at least one closure note | 7 | 7 | 7 | 7 | 7 | 7 | 7 | **2** |
| Exports proven | 5,415 / 8,113 (66.74%) | 6,520 / 8,320 (78.37%) | 6,095 / 8,358 (72.92%) | 5,477 / 8,358 (65.53%) | 5,410 / 8,358 (64.73%) | 5,417 / 8,358 (64.81%) | 4,450 / 8,082 (55.06%) | **4,450 / 8,082 (55.06%)** |
| Exports carrying an unknown | 2,698, of which 2,077 in all five domains | 1,800, of which 492 in all five | 2,263, of which 527 in all five | 2,881, of which 528 in all five | 2,948, of which 528 in all five | 2,941, of which 528 in all five | 3,632, of which 528 in all five | **3,632, of which 528 in all five** |
| Unknown claims, total | 11,013 | 4,898 | 5,903 | 6,672 | 6,776 | 6,762 | 7,622 | **7,622** |
| Entrypoints | 847 emitted, 7 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | 850 emitted, 4 refused | 829 emitted, 13 refused | **829 emitted, 13 refused** |
| Closure notes | 32 | 32 | 32 | 32 | 32 | 32 | 31 | **5** |
| Attested closure notes | not measured | not measured | not measured | not measured | not measured | not measured | not measured | **17** |
| Outcome classes | 403 / 6 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | 406 / 3 / 7 | 387 / 11 / 18 | **387 / 11 / 18** |

"Attested closure notes" reads `not measured` rather than `0` for the first seven
states because the field did not exist in them: their generators emitted no
`runtimeNotes`, so every note of either shape was counted in the row above. The
eighth state's 5 + 17 = 22 is not comparable to 31 as a subtraction of one number
from another; the note-by-note account below is what makes the 31 → 22 movement
readable.

Unknown claims by domain — read together, not separately, since 528 of the
3,632 unknown exports appear in every column. The eighth state is identical to
the seventh in every domain, to the claim:

| Domain | 2026-08-22 | 2026-08-23 (second) | 2026-08-23 (third) | 2026-08-23 (fourth) | 2026-08-23 (fifth) | 2026-08-23 (sixth) | 2026-08-24 (seventh) | 2026-08-24 (eighth) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| callbacks | 2,205 | 630 | 693 | 1,368 | 1,472 | 1,472 | 2,337 | **2,337** |
| reactiveReads | 2,577 | 1,657 | 2,019 | 2,065 | 2,065 | 2,058 | 2,060 | **2,060** |
| returns | 2,077 | 1,627 | 2,136 | 2,182 | 2,182 | 2,175 | 2,168 | **2,168** |
| ownerRequirements | 2,077 | 492 | 527 | 528 | 528 | 528 | 528 | **528** |
| asyncBehavior | 2,077 | 492 | 528 | 529 | 529 | 529 | 529 | **529** |

Positive behavioral rows a probe step would have to drive: 1,247 callback
executions, 1,165 return trees, 1,160 reactive reads, 533 owner requirements,
100 async behaviors — **4,205 rows**, unchanged from the seventh state, against
4,365 in the sixth, 4,361 in the fifth, 4,812 in the fourth, 5,005 in the third,
5,545 in the second and 4,199 in the first.

#### What the attested record answered, note by note

Seven probes carried a closure note in the seventh state, and all seven are named
here with their before and after, because the total is small enough to enumerate
completely and a note dropping is the only movement in this state that could hide
a false certification. **None of the seven changed its fully-proven status, its
refused-entrypoint count, its export total or any unknown claim** — the middle
three columns are what the record says, and the right-hand column is the reason
nothing else could move:

| Probe | Notes before | Notes after | Attested runtime notes | Fully proven |
| --- | --- | --- | --- | --- |
| `@solidjs/start@2.0.3` (1.x) | 19 | **4** | **7** | false → false |
| `@solidjs/web@2.0.0-rc.1` (2.x floor) | 4 | **0** | **4** | false → false |
| `@solidjs/web@2.0.0-rc.1` (2.x head) | 4 | **0** | **4** | false → false |
| `@solidjs/vite-plugin@3.0.0-next.31` (2.x floor) | 1 | **0** | **1** | false → false |
| `@solidjs/vite-plugin@3.0.0-next.31` (2.x head) | 1 | **0** | **1** | false → false |
| `@tanstack/charts@0.14.0` (1.x) | 1 | **1** | 0 | false → false |
| `@tanstack/form-devtools@1.0.0-alpha.2` (1.x) | 1 | **0** | 0 | false → false |

Three mechanisms, and every one of the 31 notes lands in exactly one of them:

- **17 notes reclassified to `runtimeNotes`** — `@solidjs/start` 7,
  `@solidjs/web` 8 across its two probes, `@solidjs/vite-plugin` 2 across its
  two. **Every one of the 17 is the non-literal dynamic `import()` shape**; the
  other shape that reaches `runtimeNotes` — an unselected conditional `imports`
  branch whose targets exist on disk — is pinned by
  `fixtures/package-contracts/conditional-imports-side-effect` and is exercised
  by no package in this corpus. The record is now attested and complete for what
  the analysis read; what stays unbounded is the runtime. They still block
  promotion, which is why no probe gained fully-proven status from a
  reclassification.
- **9 notes answered and dropped** — 8 on `@solidjs/start` and 1 on
  `@tanstack/form-devtools`, every one of them an asset import (`./styles.css`,
  `./style.css`) that names no runtime module inside the package. The compiler
  resolved nothing for the specifier either, and the walk found no runtime target
  on disk a runtime could select instead, so the record *is* complete and the walk
  was reporting a gap that does not exist. `@tanstack/form-devtools` is the one
  probe whose whole closure blocker disappeared; it is still refused and still not
  fully proven, because it has a refused entrypoint of its own.
- **5 notes retained and made more precise** — 4 on `@solidjs/start`
  (`../../../package.json`, once on each of `./client`, `./client/spa`,
  `./server`, `./server/spa`) and 1 on `@tanstack/charts` (`./Chart.svelte`), each
  restated with the file the analyzing program actually resolved the specifier to
  (`package.json`, and `dist/svelte/Chart.svelte.d.ts`). These are the notes the
  design predicted would disappear and did not: the specifier resolves for real,
  so the walk was right and attestation names the module it missed.

17 + 9 + 5 = 31, and 17 + 5 = 22 is the note total after. The distribution by
family: Official Solid 29 → 4 notes + 17 attested notes, TanStack 2 → 1 + 0, and
every other family had none before and has none now.

**The record itself roughly doubled, and that is the declaration files.** The
corpus report has no module-count field, so this was measured directly on
`@solidjs/start@2.0.3` outside the harness, both engines against one install:
the module sum over its 12 entrypoints goes **215 → 428** (distinct modules
**96 → 184**), and **209 of the 428** are declaration files, which the walk never
seeded and the analyzing program demonstrably opened. That standalone run
reproduces the corpus row's note movement exactly — 19 notes → 4 notes + 7
attested notes, the 8 dropped ones being the 8 `./styles.css` notes and the 4
retained ones the `../../../package.json` note on each of four entrypoints — which
is the check that the two measurements are of the same thing.

#### The seventh state's movement — the `kind` claim (history)

**The sixth → seventh movement is the `kind` claim and nothing else, and the
arithmetic closes exactly.** Exports proven fall 967, and that number is the sum
of two measured movements: **276 exports stopped existing** (the corpus's export
total falls 8,358 → 8,082, all of it exports of entrypoints generation now
refuses) and **exports carrying an unknown rise by 691** (2,941 → 3,632).
967 = 276 + 691 — an identity rather than an estimate, since every export in this
corpus either carries an unknown or is proven (`exportsWithoutSummary` is zero in
both states). `callbacks` unknowns rise 865 against those 691 exports, the
174-claim difference being exports that
already carried an unknown in another domain and were therefore already counted.
`ownerRequirements` and `asyncBehavior` are unchanged to the claim; `reactiveReads`
(+2) and `returns` (−7) move only where a refused entrypoint took claims with it.

The mechanism is one substitution, applied 691 times. An export the generator
used to publish as `kind: "value"` carries **no claim domains at all** — schema
v1 bars a `value` summary from carrying even an unknown — so it counted as fully
proven while asserting the maximal certified negative: reads nothing reactive,
returns nothing reactive, invokes no caller-supplied callback, requires no owner.
Where the analyzer can now prove the export is a function it is republished as
`kind: "function"` with `callbacks: {"status":"unknown"}`, and where it can prove
neither the entrypoint is refused. Both directions cost certified surface; the
first replaces a false negative with an honest gap, the second removes the claim
rather than guessing it. The verification measurement below is where the same
change is visible as a gain, and the trade is stated there.

**Fully-proven probes fall 205 → 125, and none gained the status.** That −80
splits: **73 probes still measured lost it**, and **7 more were fully proven and
stopped being measured at all** because their package now generates nothing
(`@solid-devtools/locator`, `@solid-primitives/analytics`,
`@tanstack/ai-devtools-core`, `@tanstack/solid-hotkeys-devtools`, and
`@solid-primitives/platform` on all three of its probe rows). Unlike the fifth
state the loss is spread across every family that has any: Corvu 23 → 3, TanStack
24 → 7, Solid Primitives 152 → 112, Official Solid 3 → 1, Solid Devtools 3 → 2.
Corvu is the sharpest reading, because it was the *control* in two earlier
rounds — neither contradiction sentinel reached it — and it is now down to three
clean probes out of 28. Each loss is small and the same shape: one to three
exports per package (twelve for the umbrella `corvu@0.7.2`) whose `kind` was
`value` and is now `function` with an unknown `callbacks`. One clean probe is
enough to lose, which is why a family that lost 41 exports lost 20 probes.

**21 entrypoints are newly refused, across 19 probe rows and 17 distinct
packages, and 11 of those rows lost every entrypoint they had.** The 11
all-refused rows cover the nine packages named above — `@solid-primitives/platform`
contributes three, one per probe — at these exact pins: `@kobalte/utils@0.9.2`,
`@solid-devtools/locator@0.16.7`, `@solid-primitives/analytics@0.2.1`,
`@solid-primitives/audio@1.4.5`, `@solid-primitives/cookies-store@1.1.11`,
`@solid-primitives/intersection-observer@2.2.5`, `@solid-primitives/platform`
(all three probe rows), `@tanstack/ai-devtools-core@0.5.6` and
`@tanstack/solid-hotkeys-devtools@0.7.0`. Nine further entrypoints are refused
inside a contract that still emits: `solid-js@1.9.14`'s **`./web`**,
`@solid-devtools/debugger`'s `./chunk-G2GTP2NP` and `./types`,
`@solid-devtools/ui`'s `./theme`, `solid-devtools`'s `./vite`,
`@tanstack/form-devtools`'s `./production`, and the `.` entrypoint of
`@tanstack/hotkeys-devtools`, `@tanstack/table-devtools` and
`@tanstack/solid-table-devtools`.

**Most of those refusals are a fact about the analyzed `.js`, not about the
package**, and the shape is uniform enough to name. Five of the nine all-refused
packages refuse on a **downleveled TypeScript enum** — `@kobalte/utils`'s
`EventKey`, `@solid-primitives/audio`'s `AudioState`,
`@solid-primitives/intersection-observer`'s `DirectionX`,
`@solid-primitives/analytics`'s `EventType`, `@solid-primitives/cookies-store`'s
`CookieSitePolicy` — each published as `var E; (function (E) { … })(E || {});`
with `export declare enum E` in the sibling `.d.ts`. `@solid-primitives/platform`
refuses on `isBrave`, computed from `navigator.brave` and declared
`export declare const isBrave: boolean`. `solid-js`'s `./web` refuses on
`Aliases`, a `const Aliases = Object.assign(Object.create(null), { … })` whose
declaration says `Record<string, string>`. In every one of those the export is
provably not a function *and the package's own published typing says so*, while
the analyzed implementation leaves `Callability::Unknown`. Every one of the 21
refusal reasons was read back from a fresh generation's review plan rather than
inferred: **17 report `whose runtime kind no closed type answers (Unknown)` and 4
report `which destructures a member of another value`**, and only 6 of the 21 name
a class-shaped export at all. This is measured, not argued, and it is recorded in
the precision backlog as the cost of the refusal path rather than as a defect in
these packages.

**No outcome class regressed in the sense that matters**: nothing moved from a
contract to a *failure* for any reason other than the refusal above, and no probe
lost a contract to an install, parse, or shape problem. 387 complete / 11 partial
/ 3 install failures / 2 `no-esm-runtime-target` / 1 `cjs-only-entrypoint` /
1 `no-exported-surface` / **11 `unclassified`**, the last being the eleven
all-refused packages: `classify.mjs` has no marker for a package whose every
entrypoint is refused, so it retains their full stderr in the fallback bucket
exactly as that bucket exists to do.

**The render-edge change of the sixth state survives this one intact, and is
visible in it.** This engine is the sixth state's plus the `kind` proof, and the
two overlap on exactly one export of one probe. Measured against the same change
set without the render edges — a run of the `kind` proof alone
(`34e97be60c60291debbae66239082cd1e252ff53831f7f1eb977647207f31aec`) — the render
edges are worth **exports proven +6, exports carrying an unknown −6, unknown
claims −14** (`reactiveReads` −7, `returns` −7) and **reactive-read rows +4**,
moving the same two probes they moved on their own and no others. The +6 is one
smaller than the +7 they were worth against the fifth state, and the missing
export is the overlap: of the seven `@tanstack/ai-solid-ui@0.7.18` exports the
render edge frees, one now carries a `callbacks` unknown the `kind` proof opened,
so it is not fully proven either way. No probe's outcome, and no other family's
figures, differ from either change measured alone.

#### How the earlier states moved (history)

The five transitions below are kept because each records a cause that was
measured rather than assumed, and because the first four together are the record
of a generator that got *less* willing to certify on every pass; the fifth is the
only one that moved the other way.

**The fifth → sixth movement is one probe, and it is a gain.** A rendered
component became a call-graph edge, so a private helper whose only caller is
`<Helper/>` no longer looks like an escaped function value and attribution no
longer widens to every export of the entrypoint (recorded in the precision
backlog as "[a rendered component is a call, not an
escape](precision-backlog.md)"). Corpus-wide that was **exports proven +7,
exports carrying an unknown −7, unknown claims −14** (`reactiveReads` −7,
`returns` −7), **reactive-read rows +4**, and nothing else: `callbacks`,
`ownerRequirements` and `asyncBehavior` unchanged to the claim, no probe
gained or lost fully-proven status, no package did, the entrypoint and
closure-note figures identical, and the outcome classes identical
probe-for-probe.

Two probes' review plans moved and no others, and both moved for a render edge
that resolved:

- **`@tanstack/ai-solid-ui@0.7.18` (Solid 1.x)** was the whole content delta.
  `MessagePart` is a private component in `src/chat-message.tsx` whose only
  caller is `<MessagePart …/>` inside `ChatMessage`; it holds a
  `ReactiveDispatchUnresolved` obligation (`computed-call-target-unresolved`, the
  `props.toolsRenderer[props.part.name]?.(toolProps)` dispatch). That obligation's
  attribution mechanism moves `fallback-all` → `reachability`, and the reach it
  enumerates is `MessagePart` → `ChatMessage` → `ChatMessages` — the second link
  is a second render edge, `<ChatMessage message={message} />` in
  `src/chat-messages.tsx`. The 18 unknown sentinels (9 exports × `reactiveReads`
  and `returns`) became **4** on the two exports that actually reach it, and
  `Chat`, `ChatInput`, `TextPart`, `ThinkingPart`, `ToolApproval`, `useChat` and
  `useChatContext` were proven instead. In the seventh state the same narrowing
  holds and one of those seven — the export whose `kind` the seventh state cannot
  prove — carries a `callbacks` unknown instead, which is why the render edge is
  worth six proven exports here rather than seven.
- **`@solid-primitives/start@0.0.4` (Solid 1.x)** moved no content figure and is
  the more interesting one: three `solid-start` obligations — two in
  `root/InlineStyles.tsx`, one in `root/Links.tsx` — stopped being attributed to
  this package's exports at all. Both components are rendered exactly once,
  `<InlineStyles />` in `root/Scripts.tsx` and `<Links />` in `root/Document.tsx`,
  so the enumeration is complete *and empty*: no export of the entrypoint
  reaches them. Its four sentinels stay, because other obligations in
  `solid-start/api/index.ts` and `solid-start/islands/server-router.tsx` still
  answer `fallback-all`; what changed is that the three render-reached ones are
  now disclosed as `artifact-binding` notes (5 → 8) rather than marking exports
  they cannot reach.

Both spellings of the ladder's answer moved in the same direction — narrower, or
empty — and no export was certified anywhere that a resolved render edge does not
stand behind.

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

**The eighth state is identical to the seventh in every column of this table, for
every family**, so the table still reads sixth → seventh. The eighth state's only
per-family movement is in the closure-note counts, which this table does not
carry: Official Solid 29 notes → 4 notes + 17 attested notes, TanStack 2 → 1 + 0,
and no other family had one in either state.

2026-08-24 seventh state, matching the headline table above; the sixth state's
column is kept beside it because for the first time every family with anything
to lose lost some of it, and the fourth → fifth column below it because that one
was concentrated in a single family:

| Family | Contracts (sixth → seventh) | Fully proven (sixth → seventh) | Exports proven (sixth → seventh) | Unknown claims (sixth → seventh) |
| --- | --- | --- | --- | --- |
| Official Solid | 23 → **23** | 3 → **1** (4.35%) | 1114 → **906** / 1546 → **1470** | 601 → **740** |
| Kobalte | 4 → **3** | 0 → **0** | 363 → **252** / 1206 → **1147** | 2,269 → **2,376** |
| Solid Primitives | 288 → **281** | 152 → **112** (39.86%) | 1638 → **1453** / 2038 → **1949** | 693 → **790** |
| Corvu | 28 → **28** | 23 → **3** (10.71%) | 229 → **188** / 266 | 74 → **121** |
| TanStack | 50 → **48** | 24 → **7** (14.58%) | 1568 → **1402** / 2124 → **2121** | 1,010 → **1,237** |
| Solid Devtools | 10 → **9** | 3 → **2** (22.22%) | 206 → **149** / 233 → **184** | 60 → **66** |
| Solid Recharts | 3 → **3** | 0 → **0** | 16 → **16** / 327 | 639 → **644** |
| Motion for Solid | 3 → **3** | 0 → **0** | 283 → **84** / 618 | 1,416 → **1,648** |

**Corvu is the reading that matters, because it was the control.** Neither
contradiction sentinel nor the retained-callback sentinel reached it in any
earlier round, and it went 23 → **3** fully proven here on 28 unchanged
contracts, losing 41 exports and gaining 47 unknown claims. Every one of those
losses is a single export per package whose `kind` was `value` and is now
`function` with an unknown `callbacks`, which is why the family that had nothing
to lose to the previous two rounds has almost nothing left to lose to this one.

**TanStack is the one family the render-edge half of this engine still shows in.**
Its 1,402 proven exports are six more than the `kind` proof alone produces
(1,396) and its 1,237 unknown claims fourteen fewer than that run's 1,251, all of
it `@tanstack/ai-solid-ui@0.7.18`; every other family's figures are identical
whether or not the render edges are present.

The sixth state's own movement, kept because it was one probe in one family:
Official Solid, Kobalte, Solid Primitives, Corvu, Solid Devtools, Solid Recharts
and Motion for Solid were unchanged export-for-export and claim-for-claim across
fifth → sixth, and TanStack moved by exactly the seven exports of
`@tanstack/ai-solid-ui` (1,561 → 1,568 proven, 1,024 → 1,010 unknown claims). The
rendered-only private helper is a component-library idiom, and this corpus's
largest family ships hooks and primitives instead — a fact about the corpus, not
a bound on the fix.

The fourth → fifth state, kept because its movement was in the other large
family:

| Family | Fully proven (fourth → fifth) | Exports proven (fourth → fifth) | Unknown claims (fourth → fifth) |
| --- | --- | --- | --- |
| Official Solid | 3 → 3 | 1120 → 1114 | 585 → 601 |
| Kobalte | 0 → 0 | 364 → 363 | 2,266 → 2,269 |
| Solid Primitives | 176 → 152 | 1684 → 1638 | 642 → 693 |
| Corvu | 23 → 23 | 229 → 229 | 74 → 74 |
| TanStack | 24 → 24 | 1575 → 1561 | 990 → 1,024 |
| Solid Devtools | 3 → 3 | 206 → 206 | 60 → 60 |
| Solid Recharts | 0 → 0 | 16 → 16 | 639 → 639 |
| Motion for Solid | 0 → 0 | 283 → 283 | 1,416 → 1,416 |

Only Solid Primitives lost a fully-proven probe there — all 24 of that state's
losses are in it — while Official Solid, Kobalte and TanStack lost exports
without losing a probe that was already clean. Corvu, Solid Devtools, Solid
Recharts and Motion for Solid were unchanged export-for-export, which keeps them
the useful controls: neither sentinel reaches them. *Why* — whether they never
invoke one parameter twice, or invoke it twice with the same schedule — is not
established by this measurement; it is a per-package question the report has no
field for.

**Solid Primitives is still the clean end of the ecosystem**, and it is still
where the most absolute loss lands: 230 → 217 fully proven to the soundness
rounds, 217 → 176 to the retained-callback sentinel, 176 → 152 to the
contradiction sentinels, and 152 → **112** here, on 281 contracts rather than 288
because five packages across seven probe rows (`analytics`, `audio`,
`cookies-store`, `intersection-observer`, and `platform` on all three of its rows)
now generate nothing at all. It is also the family that supplies most of the downleveled-enum
refusals named above. The
small-single-purpose-package shape is what the generator handles best *and* the
shape most likely to invoke one parameter from two call sites, which is exactly
what the contradiction sentinel refuses to average.

**Motion for Solid moved 199 exports on one probe and none on its twin**, which
looks like an inconsistency and is not: `motion-solidjs@0.7.0-beta.4`'s *floor*
probe already carried 249 all-domains-unknown exports and had only 12 proven left
to lose, while its *head* probe had 257 proven — 199 of them `kind: "value"`. The
floor/head split is a real difference in what each pinned runtime lets the
generator resolve, not a measurement artifact, and it is why this document reports
per probe rather than per package.

**The remaining unknowns still concentrate in the same two packages.**
`@kobalte/core@2.0.0-alpha.0` and `motion-solidjs@0.7.0-beta.4` are still roughly
half the corpus total between them, with the same reading as before: their 1.x
halves report a dominant cause of `reactiveReads` and `returns` rather than
`all-domains`, so the obligation costs the two domains it actually invalidates
instead of five.

**TanStack's unknowns were never its options-object callback pattern.** The
second state had them nearly gone — 98.21% of exports proven, 111 unknown claims
— and the reading that 318 of its 322 unknown exports had been the all-five
whole-summary shape was correct: it was measuring the attribution defect, not
TanStack. The four fail-closed rounds since have taken it to **1,402 / 2,121
(66.1%) proven and 1,237 unknown claims**, all of it retained callbacks,
multiply-scheduled callbacks, and now unprovable export kinds, rather than the
options-object pattern. Both
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
  are, and so does an attested closure note.** 6 probes and 22 notes — 5 closure
  notes on 2 probes, 17 attested closure notes on 5 — down from 7 probes and 31
  notes before attestation. The 5 say the record does not name every module the
  summaries came from; the 17 say it does and the runtime may still load
  something outside it. Both are unbindable to an artifact by a
  machine-verification scheme, and `fullyProven` reads their sum for that reason.
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
> directories under its own state directory, runs `bun install` with
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
  bun scripts/ecosystem-benchmark/verify-corpus.mjs --state-dir /path/to/scratch
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

### Measured state (2026-08-26, exact declaration/runtime attribution, full corpus, 416 probe rows)

This run carries the generator's exact successful static runtime edges across
the native boundary, then joins a declaration-bound import to the same named
export of that exact package-local runtime target. It does not pair `.d.ts` and
implementation files by filename. Missing compiler entities, namespace
bindings, incomplete module facts, external targets, and conflicting joins add
no redirect and retain the existing fail-closed attribution.

The user-requested small set ran before the corpus: both `@solidjs/web` Solid 2
rows verified, and `@tanstack/ai-solid-ui@0.7.18` remained refused because Node
cannot execute its published TypeScript entrypoint under `node_modules`. The
focused `declaration-sibling-reach` fixture supplies the actual precision
proof: `forwarded` alone keeps the reached unknown claims, `Isolated` regains
its independent summary, and the direct-entrypoint control stays exact.

The raw no-subsetting, concurrency-6 authority result is the checked-in
`verification-report.{json,md}`. It used native SHA-256
`1ae57d08854302148fd7613a4c628c52e569cdd80c4067019b89856eea4c4a83`
and the unchanged Type Facts SHA-256
`31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`:

| Figure | Previous authority run | This run | Movement |
| --- | ---: | ---: | ---: |
| Reached `verified` | 286/416 | **284/416** | -2 raw |
| Refused by `contract verify` | 110 | **109** | -1 raw |
| Install failures | 3 | **6** | +3 raw |
| Generation failures | 15 | **15** | 0 |
| Claims total / driven / passed | 11,935 / 8,333 / 8,325 | **11,969 / 8,327 / 8,319** | +34 / -6 / -6 raw |
| Failed claims | 8 | **8** | 0 |
| Undriven claims | 3,602 | **3,642** | +40 raw |
| Conversions (return conversions) | 802 (348) | **819 (365)** | +17 (+17) |
| Probed rows retained | 83 | **83** | 0 |
| Rows with probed evidence | 20 | **20** | 0 |

The outcome movement is registry/install environment movement, not a semantic
outcome change. Three rows that previously reached the checker now fail npm
installation: `@tanstack/solid-query-devtools@5.101.4`,
`@tanstack/solid-query-persist-client@5.101.4`, and
`@tanstack/solid-router-ssr-query@1.167.2-pre.0`. Across every row that reached
both binaries, no outcome changed. Substituting only those three previous
install outcomes yields the same **286 verified / 110 refused** split as the
preceding authority state.

Across the 393 rows with claim summaries in both runs, 19 changed
structurally: **claims +47, driven +2, passed +2, undriven +45, failed 0,
incompleteness 0**. Generated unknown-bearing exports rose by 55 and promoted
unknown-bearing exports by 53. That is primarily a soundness gain, not a
headline coverage gain: restoring real call edges exposes obligations and
return shapes the declaration split previously hid. The exact bridge also
narrows where the graph proves it can—`@solid-primitives/range@0.2.5` gained one
driven, passing claim and reduced its generated unknown-bearing exports by two.
No contradiction was suppressed or converted into fake evidence.

The report's 1,113 `reactiveReads` and 465 `ownerRequirements` rows are
**family-A compiler proofs retained by verification**. They are labelled
undriven only because there is no independent generic runtime probe for those
static claims; they are not claims verification discarded. Genuine remaining
probe/schema gaps include 270 nested return leaves, 23 store paths, 13 callback
argument descriptors, 96 async claims, 235 accessor returns with no plantable
reactive source, and executions lost to import throws, synthesized-call throws,
timeouts, or package process aborts.

**This supersedes the export-scoped open-load authority state below.**

### Measured state (2026-08-26, export-scoped open loads and isolated entrypoint imports, full corpus, 416 probe rows)

This run keeps the same stable native and Type Facts binaries as the preceding
relational-probe measurement and changes only the Node generator/probe layer.
An open dynamic import in a flat entry module is attributed through exact local
function references to explicit exports. Those exports are omitted; ambiguity
retains the entrypoint-wide blocker. Probe child lifetimes are also partitioned
by exact package specifier, so one entrypoint's failed or partially initialized
module cannot affect another entrypoint in the same mode.

The raw no-subsetting, concurrency-6 result is the checked-in
`verification-report.{json,md}`:

| Figure | Previous authority run | This run | Movement |
| --- | ---: | ---: | ---: |
| Reached `verified` | 281/416 | **286/416** | +5 |
| Refused by `contract verify` | 115 | **110** | -5 |
| Generation failures | 15 | **15** | 0 |
| Claims total / driven / passed | 11,941 / 7,959 / 7,951 | **11,935 / 8,333 / 8,325** | -6 / +374 / +374 |
| Failed claims | 8 | **8** | 0 |
| Undriven claims | 3,982 | **3,602** | -380 |
| Conversions (return conversions) | 699 (320) | **802 (348)** | +103 (+28) |
| Probed rows retained | 69 | **83** | +14 |
| Rows with probed evidence | 18 | **20** | +2 |

Both `@solidjs/web@2.0.0-rc.1` floor/head rows now verify. The open runtime
`entryUrl` remains open: `hydrate` is omitted from the root,
`./jsx-runtime`, and `./jsx-dev-runtime` contracts, while `ssrGroup` and every
export proven unable to reach the load retain their summaries. The eight
remaining failed claims are unchanged; no contradiction was hidden or demoted.

Entrypoint worker isolation also moved `@tanstack/solid-start@1.168.46` and
`@solid-devtools/shared@0.20.0` from refused to verified. It reduced aggregate
package-code session-abort withdrawals from 386 to 41. Import throws rose from
585 to 601 because later independent entrypoints are now actually attempted;
that is more complete attribution, not a regression. The main static gaps are
unchanged: 254 nested return leaves, 23 store paths, 13 callback-argument rows,
92 async claims, 1,107 reactive reads, and 465 owner requirements remain
undriven.

The raw run includes one environment timeout:
`@tanstack/ai-devtools-core@0.5.6` exceeded the fixed 120-second generation
limit. A no-contention rerun with identical inputs verified in 116.6 seconds.
Replacing only that timing outcome gives the environment-controlled
interpretation: **287 verified, 109 refused, and 14 generation failures**. The
checked-in report remains the unmodified authority result, 286/110/15.

No callback-argument or nested-leaf probe was added from a `typeof` sighting.
The current generated rows do not name a source and mutation path with which a
generic probe could establish reactivity or store behavior, so they remain
family C rather than acquiring fake evidence.

**This supersedes the 2026-08-25 relational-probe authority state below.**

### Measured state (2026-08-25, generic relational probes and entrypoint isolation, full corpus, 416 probe rows)

This run adds strict-identity probes for `returns=argument[N]`,
`returns=callback-result[N]`, and `returns=callback-result-function[N]`, repairs
the relational-return generator paths those probes falsified, and keeps a
conditional entrypoint's summaries from being merged into another entrypoint
merely because they share one runtime target. Stable copies were used for all
416 rows:

- native `solid-checker-rust`
  `da63bebcad37215615392ca8f7ae03cefccc70ee2d9fb185470d62d3c85648e5`
  (73,475,744 bytes)
- `solid-typefacts`
  `31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`
  (28,390,098 bytes)

The raw no-subsetting, concurrency-6 result is the checked-in
`verification-report.{json,md}`:

| Figure | Before relational probes | This run | Movement |
| --- | ---: | ---: | ---: |
| Reached `verified` | 282/416 | **281/416** | -1 raw |
| Refused by `contract verify` | 115 | **115** | 0 raw |
| Generation failures | 14 | **15** | +1 raw |
| Claims total / driven / passed | 12,470 / 7,827 / 7,815 | **11,941 / 7,959 / 7,951** | -529 / +132 / +136 |
| Failed claims | 12 | **8** | -4 |
| Undriven claims | 4,643 | **3,982** | -661 |
| Return conversions | 450 | **320** | -130 |
| Probed rows retained | 3 | **69** | +66 |
| Rows with probed evidence | 3 | **18** | +15 |

The raw outcome movement is timing noise, not a semantic regression. Two rows
that the earlier run verified were the only outcome differences:
`@tanstack/ai-devtools-core@0.5.6` hit the fixed 120-second generation timeout,
and `@tanstack/solid-table@9.1.2` lost its client observations when three probe
children hit their 20-second per-mode timeout. Rerunning exactly those two rows
with concurrency 1 and otherwise identical inputs verified both (116.7 s and
192.4 s respectively). Substituting those environment-controlled outcomes gives
**283 verified, 114 refused, 14 generation failures**: the one-row semantic gain
measured by the corrected composite, without hiding what the authoritative
concurrency-6 run actually observed.

The main coverage result is not the outcome count. The parameter-identity gap
is **400 undriven claims to zero**. Strict identity drove 132 more claims even
though the generator now emits 529 fewer total claims: false relational
summaries and cross-entrypoint duplicates disappeared rather than being made
easier to pass. The four `@solidjs/web` `ssrGroup` contradictions on
`./jsx-runtime` and `./jsx-dev-runtime` are gone. The root server variants keep
their proven `argument[0]` relation; the web/dev entrypoints keep their actual
void return.

The finite-dynamic-import follow-up recognizes nested literal conditionals and
inline literal lookup tables with finite literal selectors. Focused generation
tests prove that every enumerated target enters the attested module set, while
an open branch or key retains `runtimeNotes`. It changes **zero** corpus rows:
all five rows carrying the corpus's 17 dynamic-import notes (`@solidjs/start`,
both `@solidjs/vite-plugin` probes, and both `@solidjs/web` probes) remain
unbounded under the exact syntax rule. In particular, `@solidjs/web` imports a
runtime `entryUrl`; it is not a finite map the generator may guess closed.

This state still does not probe nested return leaves, store paths, callback
arguments, async behavior, reactive reads, or owner requirements generically.
The raw undriven table records 254 nested-return leaves, 23 store paths, 13
callback-argument claims, 92 async claims, 1,107 reactive-read claims, and 465
owner-requirement claims. Import throws, synthesized throws, and package-code
session failures remain observations of an environment that could not drive a
claim, not evidence for or against it.

**This supersedes the Phase A constructability state below.** That section
remains as the before-state history.

### Measured state (2026-08-25, Phase A constructability closure, full corpus, 416 probe rows)

This is the first full-corpus verification run after the span-exact
`constructability` fact, its consumer wiring, and the schema-15
`UntypedCallable` follow-up landed on the measurement branch. Both binaries were
copied out of the repository before the run, so one revision produced all 416
answers:

- native `solid-checker-rust`
  `b1c027420756dcd377d6928ed650155da4546befa9c844da3030d0a33001c2bd`
  (14,834,288 bytes, source revision `77067725`)
- `solid-typefacts`
  `31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`
  (28,390,098 bytes, producer revision `19671a889b273da135024722c5957b8979e43fbb`)

Budgets and corpus are unchanged from the state below: install 240 s, generate
120 s, probe 20 s per condition mode and 90 s + 500 ms per planned claim (cap
900 s) for the whole phase, verify 90 s, concurrency 6, no subsetting. Wall
clock was 7 m 04 s. The checked-in
`benchmarks/ecosystem/verification-report.{json,md}` is the complete report.

| Figure | 2026-08-24 attested closure | 2026-08-25 constructability | Movement |
| --- | ---: | ---: | ---: |
| Reached a generated contract | 398/416 | **399/416** | +1 |
| Reached `verified` | 275/416 (66.11%) | **281/416 (67.55%)** | +6 / -0 |
| Refused by `contract verify` | 121/416 | **116/416** | -5 |
| Generation failures | 15 | **14** | -1 |
| Claims planned / driven / passed | 12,509 / 7,504 / 7,480 | **12,525 / 7,559 / 7,548** | +16 / +55 / +68 |
| Failing claims | 24 | **11** | -13 |
| Contradicted `kind` claims | 13 | **0** | -13 |
| Exports certified by a verified contract | 929 | **937** | +8 |

Exactly six rows changed outcome and none regressed. Five moved `refused ->
verified`: `@tanstack/devtools-a11y@0.2.2`,
`@tanstack/form-devtools@1.0.0-alpha.2`,
`@tanstack/hotkeys-devtools@0.9.0`, `@tanstack/pacer-devtools@1.4.0`, and
`@tanstack/table-devtools@9.2.0`. Together they carried the seven remaining
`*DevtoolsCore` claims that said `value` while every mode observed `function`.
All seven now pass as `function`.

The sixth row, `@tanstack/ai-devtools-core@0.5.6`, moved
`generate-failure -> verified`: both of its exports now generate, both planned
claims are driven, and both pass. This is the corpus-visible recovery from the
schema-15 signature-less `Function` answer.

The other six corrected claims are `@solidjs/web@2.0.0-rc.1`'s
`ResponseEnvelope` on `.`, `./jsx-runtime`, and `./jsx-dev-runtime` in both
Solid 2 probes. Both rows remain refused, but only on their independently
documented attested module-closure note: their root cause moved
`probe-failed -> attested-closure-note`, and their 3/3 failed claims became 3/3
passing claims. That distinction is why the headline gains five refused rows,
not seven, even though all thirteen wrong-kind claims are gone.

The destructuring recovery moved content without changing a row outcome:
`@solid-devtools/ui@0.10.3` was verified before and remains verified, but now
emits 13 exports rather than 2, drives and passes 28 rather than 17 claims, and
keeps the same explicit refusals for `.` and `./icons`. General
`Callability::Unknown` shapes remain fail-closed; in particular
`solid-js@1.9.14`'s `./web` entrypoint is still refused.

**This supersedes the 2026-08-24 attested-module-closure state below.** That
state remains intact as the before column and as the history of the earlier
probe-environment, execution-kind, export-kind, and closure measurements.

### Measured state (2026-08-24, attested module closure, full corpus, 416 probe rows)

Binaries were **copied out of the repository before the run and used from the
copies**, so a concurrent rebuild could not change the engine mid-measurement.
The hash is the identity these numbers belong to, and it is a **new** one: the
engine gained `--emit-module-inventory`, an opt-in read of an already-built
program that a generation run asks for and an ordinary analysis does not. It
changes no analysis fact, and the identical claim figures below are the
measurement of that rather than the assumption:

- native `solid-checker-rust`
  `0356938638a2d6594452dd574dcff4a82332b2f0a4d58abed21b578a759a3588`
  (14,654,736 bytes, source `rust/target/release/solid-checker-rust`)
- `solid-typefacts`
  `2bbdef833749ed8c9fdda60ed9245b54baeaa9ceb98b1a880853a2c90ac56f2d`
  (28,389,218 bytes, source `bin/solid-typefacts`, unchanged)

Budgets: install 240 s, generate 120 s, probe 20 s per condition mode and
90 s + 500 ms per planned claim (cap 900 s) for the whole phase, verify 90 s;
concurrency 6. No subsetting — every one of the manifest's 416 probe rows ran.
Wall clock 6 m 24 s, against 7 m 08 s for the A9 stage 1 state.

**This is a record-honesty state, not a rate state, and the rate says so.** 275
verified and 121 refused, identical to A9 stage 1 to the row: **no row changed
outcome in either direction**, and the four never-verified classes are
probe-for-probe identical (3 install, 15 generation, 2 `no-runtime`, 0 timeouts).
Claims planned, driven, passed and failed are unchanged to the claim
(12,509 / 7,504 / 7,480 / 24), as are the 13 contradicted `kind` claims, the 30
verification-refused entrypoints across 8 rows, the 3 probed behavioral rows, the
822 conversions, and every export figure including the 929 certified by a verified
contract. What moved is what a refusal *says*: 7 rows' closure blockers were
re-stated or removed, and the refusal classifier grew the class that keeps them
nameable.

**This supersedes the 2026-08-24 A9 stage 1 state**, which supersedes the
export-kind proof one, which supersedes the render-edge one, which supersedes the
execution-kind one, which supersedes the probe-environment one, which supersedes
2026-08-22; all are kept as labelled columns because the movement between them is
the result:

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (execution kinds + render edges) | 2026-08-24 (export-kind proof) | 2026-08-24 (A9 stage 1) | 2026-08-24 (attested closure, current) |
| --- | --- | --- | --- | --- | --- | --- |
| Probe rows run | 416 | 416 | 416 | 416 | 416 | 416 |
| Reached a generated contract | 409/416 (98.32%) | 409/416 (98.32%) | 409/416 (98.32%) | 398/416 (95.67%) | 398/416 (95.67%) | **398/416 (95.67%)** |
| **Reached `verified`** | **194/416 (46.63%)** | **222/416 (53.37%)** | **261/416 (62.74%)** | **267/416 (64.18%)** | **275/416 (66.11%)** | **275/416 (66.11%)** |
| Reached `verified`, of the rows that produced a contract | 194/409 (47.43%) | 222/409 (54.28%) | 261/409 (63.81%) | 267/398 (67.09%) | 275/398 (69.10%) | **275/398 (69.10%)** |
| Refused by `contract verify` | 210/416 (50.48%) | 185/416 (44.47%) | 146/416 (35.10%) | 129/416 (31.01%) | 121/416 (29.09%) | **121/416 (29.09%)** |
| Claims planned | 11,444 | 13,206 | 12,948 | 12,509 | 12,509 | **12,509** |
| Claims driven | 6,039 (52.77%) | 7,809 (59.13%) | 7,647 (59.06%) | 7,504 (59.99%) | 7,504 (59.99%) | **7,504 (59.99%)** |
| Claims that passed | 5,686 | 7,591 | 7,584 | 7,480 | 7,480 | **7,480** |
| Claims that failed | 353 | 218 | 63 | 24 | 24 | **24** |
| — of which a wrong `callbacks[].execution` | not measured | 159 | 10 | 11 | 11 | **11** |
| — of which a wrong `kind` | 53 | 53 | 53 | 13 | 13 | **13** |
| Incompleteness findings | 1,091 | 1,080 | 734 | 594 | 594 | **592** |
| Exports certified by a verified contract | 449 | 752 | 1,018 | 890 | 929 | **929** |
| Entrypoints refused by *verification* inside a promoted document | 0 | 0 | 0 | 0 | 30, across 8 rows | **30, across 8 rows** |
| Verified rows carrying a probed behavioral row | 6 | 15 | 3 | 3 | 3 | **3** |
| Probed behavioral row markers kept | 12 | 25 | 3 | 3 | 3 | **3** |
| Probe timeouts | 3 | 0 | 0 | 0 | 0 | **0** |
| Never reached verification | 3 install, 4 generation, 2 probe errors, 3 timeouts | 3 install, 4 generation, 2 no-runtime, 0 timeouts | 3 install, 4 generation, 2 no-runtime, 0 timeouts | 3 install, 15 generation, 2 no-runtime, 0 timeouts | 3 install, 15 generation, 2 no-runtime, 0 timeouts | **3 install, 15 generation, 2 no-runtime, 0 timeouts** |

Solid 1.x verifies at 105/168 (62.50%) and Solid 2.x at 170/248 (68.55%),
unchanged from A9 stage 1, against 58.93% and 67.74% before that, 58.33% and
65.73% before that, 49.40% and 56.05% before that, and 41.67% and 50.00% in the
first.

**Incompleteness findings 594 → 592 is the one figure that moved, and it is a
probe-timing artifact, not this change set.** `@tanstack/solid-table@9.1.2`'s
*client*-mode probe session hit the 20-second per-condition-mode timeout in this
run and did not in the previous one — its whole-phase probe finished well inside
budget (161 s of 386.5 s), so nothing timed out at the row level. The dead client
session cost that row 2 client-mode incompleteness findings and left 306
`kind` obligations unobserved in that mode, which is the whole of five further
differences: `kindGaps` rows 84 → 85 and pairs 6,962 → 7,268 with a new
`probe session hit the per-mode timeout` reason at 306 (a classifier case that
already existed and had never fired), `kind-observed` blocker rows 68 → 69 and
lines 160 → 165, three undriven claims reclassified into the same new bucket
(total undriven unchanged at 5,005), and session counts 17,336 → 16,822 started /
15,796 → 15,282 restarts / 63 → 64 failed. **The row's outcome and root cause did
not move** — refused, root cause `incompleteness`, in both states — and this
change set touches neither the probe worker nor the analysis, so the artifact is
recorded rather than attributed. A re-run on an unloaded host would be expected to
restore 594.

#### The A9 stage 1 movement (history)

**Every claim figure in that column is unchanged to the claim, and that is the
first thing to read.** Claims planned, driven, passed, failed, undriven and the
incompleteness findings are identical to the previous state — 12,509 / 7,504 /
7,480 / 24 / 5,005 / 594 — as are the session counts (17,336 started, 15,796
restarts, 63 failed) and the whole probe environment. Nothing was observed
differently. What changed is what verification does with the same observations:
where an unobserved `kind` used to refuse the document, it now refuses the
*entrypoint*, and the document only when nothing would certify anything.

**Eight rows moved `refused → verified`, nothing moved the other way, and no
other outcome class moved at all.** 3 install failures, 15 generation failures,
2 `no-runtime` and 0 timeouts, probe-for-probe identical. The measured payoff is
therefore **+8 of the design-time upper bound of 10** — the bound A9 stage 1
predicted against the 77-row state, and the shortfall is the correlated
carve-out the amendment named rather than a surprise: see "The eight, and the two
the carve-outs kept" below.

#### The eight, and the two the carve-outs kept (A9 stage 1)

This subsection and the one above it describe the A9 stage 1 movement and its
figures, which the attested-closure state inherits unchanged except where the
closure-note attribution split; the current blocker and root-cause tables are
under "[Why verification refuses](#why-verification-refuses)".

Every one of the eight is a **multi-entrypoint package with at least one
entrypoint the probe could import**, and the mechanism was read back from each
row's own refusal list rather than inferred. The `kind`-refused entrypoints and
the promoted document's export count are both recorded per row, so the two
numbers together say the document is smaller than the draft:

| Row | Draft entrypoints | Verification refused | Promoted exports (of draft) | What survived |
| --- | --- | --- | --- | --- |
| `@solidjs/image@0.1.0` (1.x) | 2 | 1 (`.`, on `SolidImage` in all four modes) | 1 / 2 | the non-`.` subpath |
| `@solid-devtools/debugger@0.28.1` (1.x) | 4 | 3 (`.`, `./bundled`, `./index`, 19 exports each) | 5 / 62 | one subpath of four |
| `@solid-devtools/ui@0.10.3` (1.x) | 3 | 2 (`.` 12 exports, `./icons` 3, all `server`-only gaps) | 2 / 17 | one subpath of three |
| `@tanstack/devtools-ui@0.7.1` (1.x) | 3 | 2 (`.` 25 exports, `./icons` 21, `server`-only) | 8 / 54 | one subpath of three |
| `@tanstack/devtools-utils@0.7.0` (1.x) | 7 | 4 (`./preact`, `./react`, `./svelte`, `./vue`, 2 exports each, all four modes) | 7 / 15 | the three non-framework subpaths |
| `@tanstack/solid-start@2.0.0-rc.1` (2.x floor) | 10 | 7 (incl. `.` 36 exports and `./server` 39) | 3 / 91 | three subpaths of ten |
| `@tanstack/solid-start@2.0.0-rc.1` (2.x head) | 10 | 7 (same set) | 3 / 91 | three subpaths of ten |
| `corvu@0.7.2` (1.x) | 9 | 4 (`./otp-field`, `./popover`, `./resizable`, `./tooltip`, `server`-only) | 43 / 74 | five subpaths of nine |

**A9's own warning about what this buys is confirmed, not softened.** The
amendment predicted that "the surviving half is the less behavioral half,"
because by construction the entrypoints that survive are the ones the probe
*could* import. Measured: the eight rows contribute **zero** probed behavioral
rows — `probedRowsKept` stays at 3 corpus-wide, the same three
`@tanstack/solid-query` rows as the previous four states — and only one of the
eight (`corvu@0.7.2`, 11) contributes any conversion at all. Six of the eight
promote a document with **fewer than nine exports**. These are `kind`-only
negatives on narrow subpaths, which is exactly what the amendment said the count
would be worth and why it argued the durable value is consistency rather than the
number.

**Ten rows had a survivor; eight of them converted, and the two that did not are
the correlated carve-out.** Of the 74 rows root-caused `kind-observed` in the
previous state, 18 were multi-entrypoint, and they split three ways:

- **8 converted** — every entrypoint that was not `kind`-blocked survived, and no
  document-wide blocker stood beside it.
- **8 had no survivor at all** — every entrypoint they declare was `kind`-blocked,
  so the document still certifies nothing and still refuses:
  `@solid-devtools/shared@0.20.0`, `@tanstack/solid-pacer-devtools@0.14.0`,
  `@tanstack/solid-router` on both majors, `@tanstack/solid-start-client` on all
  three of its rows, and `@tanstack/solid-start@1.168.46`.
- **2 had a survivor and kept refusing anyway**, on a **closure note**:
  `@solidjs/start@2.0.3` (12 entrypoints, 28 kind blocker lines) and
  `@tanstack/charts@0.14.0` (110 entrypoints, 91). Their root cause moves
  `kind-observed` → `closure-note`, which is the whole of the `closure-note`
  root-cause movement 2 → **4**.

That second carve-out is the one A9 said would subtract from the 10
*correlated* with the population — "a package with dozens of entrypoints is
exactly the kind most likely to also carry a closure note" — and it landed on
precisely the two widest rows in the population, including the 91-blocker-line
row the amendment named in advance. The prediction was not merely right in
direction; it identified the two rows.

**`kind-observed` as a root cause falls 74 → 64, and the arithmetic is
closed**: 8 rows verified and 2 were reattributed to `closure-note`.
`incompleteness` (38) and `probe-failed` (15) are unchanged, as are all 14
baseline rows where `kind-observed` was a *co*-blocker rather than the root
cause — every one still refuses, because each has an independent blocker that
stage 1 deliberately leaves document-wide.

**Read the blocker *rows*, not the blocker *lines*.** `kind-observed` lines fall
322 → 160 and that number attributes to nothing on its own, exactly as the
amendment warned: a refused document now raises one summary line *plus* one line
per refused entrypoint, while a row leaving the refused set removes all of its
lines. The row count (88 → 68) is the readable figure.

#### The previous state (2026-08-24, export-kind proof) — history

**That state was the first whose headline gain was smaller than its gross
movement, and the difference was the point.** Thirteen rows moved
`refused → verified`, seven moved `verified → generate-failure`, and the net is
+6. Nothing moved `verified → refused`. Both directions are the same change:
generation stopped publishing a `kind: "value"` summary it could not prove, which
either replaces the contradicted claim with a provable `kind: "function"` plus
`callbacks: {"status":"unknown"}` — and those rows now verify — or refuses the
entrypoint, and where that was a package's only entrypoint the package generates
nothing and the row leaves verification entirely.

**Every one of the thirteen gains has a corrected-kind or refusal mechanism**,
checked row by row rather than assumed:
`@solid-primitives/map`, `@solid-primitives/set` and `@solid-primitives/trigger`
(both Solid 2 probes each) verify because `ReactiveMap`/`ReactiveWeakMap`,
`ReactiveSet`/`ReactiveWeakSet` and `TriggerCache` are `function` and the probe
observes a function; `@tanstack/solid-pacer@0.22.0`, `@tanstack/devtools@0.14.2`
the same way for their bundled classes;
`@tanstack/solid-table-devtools@9.2.0` because the `.` entrypoint carrying its
wrong `TableDevtoolsPanel` claim is now refused and its other entrypoint verifies
without it; `@corvu-next/focus-trap@0.1.5` because its one export's certified
negative became an honest `callbacks` unknown, which is exactly the claim its
`incompleteness` blocker had falsified; and
`@tanstack/solid-query-devtools@5.101.4`, `solid-devtools@0.34.5` and
`solid-recharts@1.0.1` because their blocker was `kind-observed` — no passing
`kind` observation in some mode — and a `function` kind is observable where a
`value` kind was not.

**The seven losses are all whole-package generation failures**, and they were
verified rows before: `@solid-primitives/analytics@0.2.1`,
`@solid-primitives/audio@1.4.5`, `@solid-primitives/cookies-store@1.1.11`,
`@solid-primitives/intersection-observer@2.2.5`, and `@solid-primitives/platform`
on all three of its probe rows. Four more rows moved into the same class from
`refused` (`@kobalte/utils@0.9.2`, `@solid-devtools/locator@0.16.7`,
`@tanstack/ai-devtools-core@0.5.6`,
`@tanstack/solid-hotkeys-devtools@0.7.0`), for 11 new generation failures in
total. The content measurement above names what each one refuses on and why most
of them are a limit of the analyzed `.js` rather than a fact about the package.

**Wrong `kind` is no longer the dominant visible defect class.** It was 53 of
every failing-claim total from 2026-08-22 through the render-edge state,
unchanged to the claim across four engine revisions, and it is now **13 of 24**.
Reconciled against the previous state's 53 individually: **25 corrected** to
`function` and observed as such (`@tanstack/solid-pacer` 10, `@kobalte/core@0.13.13`
4, `@solid-primitives/map` 4, `@solid-primitives/set` 4,
`@solid-primitives/trigger` 2, `@tanstack/devtools` 1), **15 withdrawn** because
their entrypoint is refused (`@solid-devtools/locator` 8,
`@tanstack/ai-devtools-core` 2, `@tanstack/solid-hotkeys-devtools` 1, the `.`
entrypoint of `@tanstack/table-devtools`, `@tanstack/hotkeys-devtools` and
`@tanstack/solid-table-devtools`, and `@tanstack/form-devtools`'s
`./production`), and **13 still wrong** — `@solidjs/web@2.0.0-rc.1`'s
`ResponseEnvelope` on `.`, `./jsx-runtime` and `./jsx-dev-runtime` across both
probes (6), and `*DevtoolsCore` in `@tanstack/devtools-a11y` (2),
`@tanstack/pacer-devtools` (2), `@tanstack/form-devtools` (1),
`@tanstack/hotkeys-devtools` (1) and `@tanstack/table-devtools` (1). 25 + 15 + 13
= 53. The residue is one family — a binding whose *type* is a class reached only
through a value expression, either a bundler's `/* @__PURE__ */ (() => { class …
})()` or a cross-package tuple element — and closing it needs a constructability
fact from the Type Facts producer, not more syntax chasing. The precision backlog
carries that account.

**One failing claim was gained, not lost, and it is a real finding.**
`@solidjs/testing-library@0.8.10` moved from `kind-observed` to `probe-failed`:
48 of its 83 exports were `kind: "value"` with no observable reading, its claims
driven went 4 → 87 of the same 106 planned, and one of the newly driven claims
disagrees — `testEffect callbacks[0]=deferred`, observed `inline` in client,
development and production. That is the shape this whole change is for: a
certified negative was hiding a claim nobody could check.

**The render edges of the previous state changed nothing here except the claim
plan, and this run confirms it against a third engine.** Measured against the same
`kind` proof without the render edges
(`34e97be60c60291debbae66239082cd1e252ff53831f7f1eb977647207f31aec`, 12,505
claims planned), the merged engine reproduces that run **row for row**: the same
267 verified and 129 refused rows — *the same rows* — the same 24 failing claims
in the same five shapes, the same four root-cause counts, the same 811
conversions, 890 exports certified, 3 probed behavioral rows, 17,336 worker
sessions, and the same three install / fifteen generation / two `no-runtime`
pre-contract outcomes. The one movement is **claims planned 12,505 → 12,509**,
`@tanstack/ai-solid-ui@0.7.18` regaining four `reactiveReads` rows, all four
`no probe form: reactiveReads` — static claims no probe harness can drive — so
driven, passed and failed are unchanged to the claim.

**Read the following two rows together or not at all.** In the render-edge state
the rate rose by 39 rows while
the probed behavioral evidence a verified contract carries fell by 12 rows and 22
markers, to three rows and three markers in the whole corpus. Both were the same
fix. Every one of the 12 rows that lost its markers converts the affected domain
with the same recorded reason — *"probed row evidence does not cover every mode
the claim is stated for (client, server, development, production); narrowing the
stated modes would claim semantics for an environment nobody observed"* — so what
those markers had been resting on is now visible: an observation in the `server`
mode, where both audited Solid releases resolve `node` to a build that re-runs
nothing (1.9.14's `dist/server.js` has an empty `createEffect`; 2.0.0-rc's
`flush()` is a no-op). In such a runtime `inline`, `tracked` and `deferred` are
indistinguishable, so a *matching* observation was never evidence. Those markers
were unearned. Losing them is the point; the residue — **3 of 275 verified rows
carry any observed behavior at all**, against 3 of 267, 3 of 261 and 15 of 222 —
is the honest floor this measurement reports, and it is worse than the one the
probe-environment state advertised. Neither that change set nor this one moved it
in either direction: the same three rows keep the same three markers
(`@tanstack/solid-query@5.101.4` and both `@tanstack/solid-query@6.0.0-rc.0`
probes), so the ratio fell only because the denominator rose — twice.

**That floor is the answer to the question A9 said would decide whether this
line of work matters.** The amendment asked to publish, once, how many of the
newly verified contracts carry any probed behavioral row. Measured: **zero of
eight.** It stays at 3 of 275 corpus-wide. So the honest conclusion is the one
RFC 0002 unresolved question 1 stated as its own risk: machine verification is
certifying negatives and `typeof`, and the human tier is still the useful one for
positives.

#### The staged decomposition (2026-08-23)

Superseded by the 2026-08-24 state above; kept because it is the only place the
execution-kind change set's two halves are separated by measurement rather than
by argument, and because its stage-2 engine is this state's baseline.

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

2026-08-24 attested-closure pass. **The verified/refused split is identical to A9
stage 1 in every family**, so the "prev → now" columns still show the export-kind
proof state's counts against stage 1's; contract counts are unchanged throughout,
because generation did not move in either state. The only column the attested
closure touched is the root causes, and only in Official Solid:

| Family | Rows | Contracts | Verified (export-kind → now) | Refused (export-kind → now) | Root causes now | Entrypoints verification refused |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 23 | 7 → **8** (34.78%) | 14 → **13** | `kind-observed` 4, `probe-failed` 4, `attested-closure-note` 2, `incompleteness` 2, `closure-note` 1 | 1 |
| Kobalte | 6 | 3 | 1 → 1 (16.67%) | 2 → 2 | `incompleteness` 2 | 0 |
| Solid Primitives | 289 | 281 | 220 → 220 (76.12%) | 61 → 61 | `kind-observed` 28, `incompleteness` 27, `probe-failed` 6 | 0 |
| Corvu | 28 | 28 | 17 → **18** (64.29%) | 11 → **10** | `kind-observed` 8, `incompleteness` 2 | 4 |
| TanStack | 52 | 48 | 17 → **21** (40.38%) | 31 → **27** | `kind-observed` 18, `probe-failed` 5, `incompleteness` 3, `closure-note` 1 | 20 |
| Solid Devtools | 12 | 9 | 3 → **5** (41.67%) | 6 → **4** | `kind-observed` 4 | 5 |
| Solid Recharts | 3 | 3 | 1 → 1 (33.33%) | 2 → 2 | `kind-observed` 2 | 0 |
| Motion for Solid | 3 | 3 | 1 → 1 (33.33%) | 2 → 2 | `incompleteness` 2 | 0 |

Official Solid's `closure-note` 3 became `attested-closure-note` 2 +
`closure-note` 1 — both `@solidjs/vite-plugin` probes moved, `@solidjs/start@2.0.3`
did not — and TanStack's one `closure-note` (`@tanstack/charts@0.14.0`) stayed put.
No family's verified or refused count moved by a row.

**Four families gain and none loses**, which is what a change that only ever
*removes* entrypoints from an otherwise-promotable document should look like.
TanStack moves the most for the second state running — 17/52 to 21/52, and it
also absorbs 20 of the corpus's 30 verification-refused entrypoints, because the
per-framework and per-plugin subpath is a TanStack idiom. Solid Devtools nearly
doubles, 3 → 5 of 9 contracts.

**Solid Primitives does not move at all, and that is the corpus's own shape
rather than a limitation.** It is the largest family, the cleanest, and 220 of
its 281 contracts already verified — but its 28 remaining `kind-observed`
refusals are single-entrypoint primitives, so there is no second entrypoint for
stage 1 to keep. Nothing in the family gains an entrypoint refusal: 0 of 30. The
small-single-purpose-package shape that makes it the best-verifying family is
exactly the shape per-entrypoint granularity cannot help.

### Why verification refuses

`contract verify` raises every blocker it finds, so a row can carry several. The
row counts are the number of refused rows raising each blocker at least once:

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines | A9 stage 1 (rows / lines) | Export-kind state (rows / lines) |
| --- | --- | --- | --- | --- |
| `kind-observed` | **69** | **165** | 68 / 160 | 88 / 322 |
| `probe-report-includes-evidence-write` | 50 | 50 | 50 / 50 | 50 / 50 |
| `incompleteness` | 40 | **592** | 40 / 594 | 40 / 594 |
| `probe-failed` | 15 | 24 | 15 / 24 | 15 / 24 |
| `attested-closure-note` | **5** | **17** | did not exist | did not exist |
| `closure-note` | **2** | **5** | 7 / 31 | 7 / 31 |

`attested-closure-note` is a new blocker kind, not a renaming: `closure-note` says
the record does not establish which bytes the summaries came from, and
`attested-closure-note` says it does and the runtime may still load a module
outside it. They are separate `BLOCKERS` entries, separate classifier classes and
separate root causes, because merging them would make attestation's effect on this
corpus unmeasurable — and because a blocker this harness cannot name is counted as
an `unclassified-refusal`, the one number [amendment
A9](rfcs/0002-a9-kind-has-no-unknown-form.md)'s stage 2 gate reads. **Measured:
zero `unclassified-refusal` rows**, and the six classes above are the complete set
of classes any refused row raised.

`kind-observed` at 69 / 165 and `incompleteness` at 592 lines are the probe-timing
artifact described in the measured state above (`@tanstack/solid-table@9.1.2`'s
client-mode session death), not a movement of this change set.

`probe-report-includes-evidence-write` is a **consequence, not a cause**:
`contract probe --write` declines to write evidence once a probe failed or
reported an incompleteness, so verification then sees passing claims that never
reached the contract. Attributed to one root cause per row instead:

| Root cause | Refused rows | A9 stage 1 | 2026-08-24 (export-kind) | 2026-08-23 (probe env) | 2026-08-22 |
| --- | --- | --- | --- | --- | --- |
| `kind-observed` | **64** | 64 | 77 | 71 | 82 |
| `incompleteness` | 38 | 38 | 40 | 37 | 42 |
| `probe-failed` | 15 | 15 | 27 | 75 | 84 |
| `closure-note` | **2** | 4 | 2 | 2 | 2 |
| `attested-closure-note` | **2** | did not exist | did not exist | did not exist | did not exist |

**No row refused with the evidence-write blocker standing alone**, which is the
check that the consequence really is one. `probe-failed` holds at **15** — the
smallest root cause by a wide margin, 84 → 75 → 27 → 15 → 15 → 15 across six
states — and `kind-observed` holds at **64**, still the binding constraint on the
corpus's rate.

**`closure-note` 4 → 2 + 2 is a split, not a reduction, and no row left the
refused set.** The four rows A9 stage 1 root-caused `closure-note` are
`@solidjs/start@2.0.3`, `@tanstack/charts@0.14.0` and both
`@solidjs/vite-plugin@3.0.0-next.31` probes. The two `vite-plugin` probes'
sole blocker is a non-literal dynamic `import()`, which is now
`attested-closure-note`; `@solidjs/start` keeps `closure-note` as its worst
blocker because 4 of its 11 remaining notes are still unattested-record notes, and
`@tanstack/charts` keeps its one. Three further rows had a closure blocker beside a
worse one and were reclassified without changing root cause — both
`@solidjs/web@2.0.0-rc.1` probes (root cause `probe-failed`) reclassified to
`attested-closure-note`, and `@tanstack/form-devtools@1.0.0-alpha.2` (root cause
`probe-failed`) lost its closure blocker entirely, going from 4 blockers to 3.

**No row was promoted by a note being answered**, which is the check this state
exists to pass. `@tanstack/form-devtools` is the only row that lost a closure
blocker outright, and it is still refused on `kind-observed` and `probe-failed`.

Three blockers, in plain terms:

- **`probe-failed` (15 rows, was 27)** — the package does not behave the way the
  contract says. Real disagreements, and the most valuable output of the whole
  measurement. **24 failing claims in all, against 63**, grouped by shape rather
  than printed as sentences: `kind: claimed value, observed function` 13,
  `callbacks[n]: claimed inline, observed tracked` 3, `callbacks[n]: claimed
  deferred, observed tracked` 3, `callbacks[n]: claimed tracked, observed inline`
  3, `callbacks[n]: claimed deferred, observed inline` 2. **Neither class
  dominates any more**: wrong `kind` was 53 in every state through 2026-08-23 and
  is now 13, wrong execution kind was 159 then 10 and is now 11. Every one of the
  24 is named individually in the report with its export, claim, observed value
  and the modes it failed in, because "deferred in server only" and "deferred
  everywhere" are different findings.
- **`kind-observed` (64 rows as root cause)** — `kind` is the one claim schema
  v1 has no unknown sentinel for, so verification requires a passing `kind`
  observation in every mode an export is stated for, and since
  [RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md) stage 1 it
  refuses the *entrypoint* rather than the document, which is refused only when
  no entrypoint would certify anything. It has been the largest cause in all
  six states (82 → 71 → 77 → 74 → 64 → **64**). A9 stage 1 was the first change
  set to move it for the reason the rule is about — 8 rows verified on a surviving
  entrypoint and 2 were reattributed to `closure-note` — and this state moves it
  not at all. It is still the binding constraint, and A9's own condition holds: it
  fell but **not to zero**, which is what would have meant gaps were being
  narrowed away rather than scoped.
- **`incompleteness` (38 rows)** — discovery planted a callback where the
  contract states none, and the package invoked it. A negative claim a probe
  falsified is wrong, not incomplete, so this refuses rather than converting. 592
  blocker lines across 38 rows, the −2 against A9 stage 1 being the client-mode
  session death described above rather than anything this change set did. An
  incompleteness finding stays document-wide, so a row carrying one cannot be
  rescued by scoping a `kind` refusal, and none was.

#### Why a `kind` observation is missing — the stage-2 gate, measured

This is the number [RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md)
stage 2 is gated on, and the reason stage 0 exists. Stage 2 would exclude, from
the modes a `kind` claim must be observed in, exactly those where the probe
**observed the export to be absent** (`export-missing`: the namespace loaded and
the binding was not in it) — an observation that the export does not exist there,
not a gap. Every other non-observation must keep blocking.

Across the corpus, 85 rows carry at least one gapped stated `kind` mode, over
3,083 `kind` obligations and **7,268 (claim, mode) pairs**:

| Why the mode produced no passing `kind` observation | (claim, mode) pairs | Share | Gap or absence |
| --- | --- | --- | --- |
| entrypoint import threw | 3,878 | 53.36% | gap |
| probe session aborted by package code | 2,629 | 36.17% | gap |
| probe session wrote no report | 328 | 4.51% | gap |
| probe session hit the per-mode timeout | 306 | 4.21% | gap |
| no unambiguous summary resolves in the mode (no `kind` claim exists) | 82 | 1.13% | gap |
| **`export-missing` in this mode** | **45** | **0.62%** | **absence** |

By mode: `server` 2,525, `client` 1,752, `production` 1,499, `development` 1,492
— so the gaps are not a server-artifact story either; they are spread across all
four modes because an entrypoint that will not import fails in each of them. The
`per-mode timeout` row and the 306 extra `client` pairs are one row's client
session death in this particular run (see the measured state above); the A9 stage
1 state read 84 rows / 2,777 obligations / 6,962 pairs with that reason at zero,
and every other cell of this table is identical to it.

**The verdict: stage 2 is a measured no-op, and must not be built.** The gate
condition A9 wrote for itself was *"if stage 0 shows the server-only gaps are
session deaths rather than absences, stage 2 buys nothing and must not be
built."* Measured, three ways, each stricter than the last:

1. **0.62% of gap pairs are observations of absence.** 45 of 7,268. The other
   99.38% are import throws, session deaths, per-mode timeouts, and unreadable
   reports.
2. **3 rows of 85 carry any `export-missing` gap at all**, and in *none* of the
   three is it the only gap reason — each is mixed with gaps that keep blocking,
   so excluding the absences would not promote any of them. All three are
   `solid-js@1.9.14` and both `@solidjs/web@2.0.0-rc.1` probes, and all 45 pairs
   are in the Official Solid family.
3. **All three are root-caused `probe-failed`, not `kind-observed`.** They carry a
   contradicted claim, which is an independent blocker stage 2 may never absorb.
   So they are outside stage 2's addressable population by definition.

And against the population stage 2 exists to serve — the **64 rows root-caused
`kind-observed`** — the answer is exact: **zero** of them carry a single
`export-missing` gap pair. Their 2,956 gap pairs are 2,226 import throws, 401
session aborts, 328 unreadable reports, and 1 unresolvable summary. The
addressable share is not small; it is empty.

A9's design-time bound for stage 2 was "at most 43 rows, and the report cannot
tell us how much of the 43 is real." It is now measured, and the answer is **0
rows**. The amendment's stated risk — that implementing stage 2 blind would
"narrow on gaps" — is precisely what it would do, since there are almost no
absences to narrow on. Stage 2 should be closed as measured-worthless rather than
left open as pending work.

#### `kind` claims the probe contradicted, counted separately

A mode whose observation **exists and disagreed**. A9 requires these to live in
their own object and never share a number with a gap, because absorbing a
contradicted `kind` is exactly what the amendment forbids. Measured: **7 rows, 13
claims, 52 (claim, mode) pairs** — 13 in each of the four modes, so every one of
them is contradicted in every mode it is stated for.

**That 13 is the same 13 as the failing-claim total's `kind: claimed value,
observed function` count**, which is the check that the separation is real: no
contradiction leaked into the gap tables, and no gap was counted as a
contradiction. The rows are `@solidjs/web@2.0.0-rc.1` (both probes, 3 claims
each), `@tanstack/devtools-a11y@0.2.2` (2), `@tanstack/pacer-devtools@1.4.0` (2),
`@tanstack/form-devtools@1.0.0-alpha.2` (1),
`@tanstack/hotkeys-devtools@0.9.0` (1) and `@tanstack/table-devtools@9.2.0` (1)
— the same residue the export-kind proof state named, still needing a
constructability fact from the Type Facts producer rather than more syntax
chasing.

### Drivability

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (execution kinds + render edges) | 2026-08-24 (current) |
| --- | --- | --- | --- | --- |
| Claims planned across every probed contract | 11,444 | 13,206 | 12,948 | **12,509** |
| Driven | 6,039 (52.77%) | 7,809 (59.13%) | 7,647 (59.06%) | **7,504 (59.99%)** |
| Passed | 5,686 | 7,591 | 7,584 | **7,480** |
| Failed | 353 | 218 | 63 | **24** |
| Undriven | 5,405 (47.23%) | 5,397 (40.87%) | 5,301 (40.94%) | **5,005 (40.01%)** |
| Incompleteness findings | 1,091 | 1,080 | 734 | **592** |

**Neither the A9 stage-1 state nor the attested-closure one appears as a column
here, deliberately: every one of these figures is unchanged to the claim across
all three.** Both change sets are verifier- and record-side, so the claim plan,
what was driven, what passed, what failed and the undriven total are identical to
the export-kind proof column — 12,509 / 7,504 / 7,480 / 24 / 5,005. Only *what
verification did with the results* moved. Two things in this table's neighbourhood
did change: the undriven distribution renamed a bucket in stage 0 (see "The
`other` bucket, resolved" below), and incompleteness findings read 592 rather than
594 because one row's client-mode probe session died on this run's host — a
timing artifact recorded in the measured state above, not a movement of either
change set.

The planned total rose in the probe-environment state because `@solidjs/web` being
installed changes which dependency contracts `contract generate` can resolve, so
the contracts themselves were larger. It has fallen twice since for the opposite
reason: the contradiction sentinels replaced a parameter's rows with one unknown,
and now eleven whole packages and nine further entrypoints plan nothing at all.
**Driven as a share of planned nevertheless rose**, 59.06% → 59.99%, which is the
shape to read: the plan shrank by 439 claims while the driven count shrank by only
143, because the claims that left were unplantable `value`-summary claims and the
ones that arrived are `function` claims a probe can actually observe.

Passed claims fall by 104 and failed by 39 — and the largest single per-row
movement is upward: `@solidjs/testing-library@0.8.10` alone goes from 4 driven of
106 to 87 driven, 86 of them passing. That row still refuses, on the one claim
that disagrees, which is exactly how a measurement that surfaces a defect is
supposed to look.

The undriven half still splits into two very different things. **2,743 claims
have no probe form at all** — `reactiveReads` 1,314, `ownerRequirements` 556,
parameter identity 398, nested return leaves 257, `asyncBehavior` 100, no
unambiguous summary for the mode 82, callback arguments 13, store paths 23 — and
no probe harness will ever reach them; they are static claims or claims schema v1
has no evidence slot for. The rest is environment and attribution: 634 claims
lost to an entrypoint that **threw on import**, 334 to a synthesized call that
threw, 227 to a synthesized call that never invoked the callback, 212 to no
plantable reactive source, 91 to a session that wrote no readable report, and the
three withdrawal buckets the previous state added — **49 to a runtime that
re-runs nothing, 25 to a callback that ran more often than the call site, and 6 to
a callback that re-ran with nothing written**. **`probe session hit the per-mode
timeout` went 56 → 0 in the export-kind state and reads 3 here**, and neither
movement is semantic: the export-kind state emptied it because the wide-surface
rows whose modes were running out of time plan fewer claims now, and the 3 are one
row's client session dying on this particular loaded host (see the measured state
above), which also accounts for the −2 and −1 on the two synthesized-call buckets.
The A9 stage 1 state read 336 / 228 / 0.

#### The `other` bucket, resolved — A9 stage 0

The remaining **671** claims were an unnamed `other` bucket in every state up to
and including the export-kind proof one, and it was the second-largest bucket in
the corpus. A9 stage 0 gave every reason the probe pipeline can emit a named
bucket, and the measurement is unambiguous: **`other` is 671 → 0, and all 671 are
`probe session aborted by package code`** — one class, a session-death class,
which is a gap and keeps blocking.

Two properties make that a measurement rather than a relabelling. The **total is
unchanged at 5,005** and every other bucket held to the claim when stage 0 landed,
so nothing moved *between* buckets — only out of the unnamed one. (In the
attested-closure run three claims moved into `probe session hit the per-mode
timeout` from the two synthesized-call buckets, for the host-timing reason
recorded in the measured state; the total is still 5,005 and `probe session
aborted by package code` is still 671.) And it reproduces from the
**previous state's own journal**: re-bucketing the 416-row baseline journal with
the new rules gives the identical 671, so the classification is a property of the
rules rather than of this run.

**One prediction in the amendment needed correcting, and it is a magnitude, not a
direction.** A9 recorded this bucket as **834** claims and predicted `834 → 0`.
That 834 was measured against an *earlier* journal, before the export-kind proof
reduced the claim plan; against the state this run actually baselines on the
bucket was already 671, and 671 is what resolved. The shape of the finding — one
class, all of it package code aborting the probe session — is exactly as
predicted. `UNDRIVABLE.owner`, `export is not callable` and the two
`returns`-distinguisher reasons account for **zero** claims in it, as stage 0
also predicted.

33 rows still had at least one entrypoint import throw, against the same 34 in the
three earlier states; the one that went is `@solid-primitives/platform`, whose
package no longer generates a contract to probe. Nothing in this change set
touches the probe environment, so the table below moves only where the claim plan
moved underneath it: `Cannot read properties of null (reading '_depth')` holds at
50 and `ERR_MODULE_NOT_FOUND` for `@solid-primitives/utils` at 84. The counts are
claims lost to a throw, so a contract with fewer claims loses fewer of
them to the same unchanged throw. Two of the
probe-environment state's top three causes are gone entirely: `ReferenceError:
window is not defined` (432 claims) and `ERR_MODULE_NOT_FOUND` for `@solidjs/web`
(248) no longer appear. What is left is dominated by things no install policy or
shim can supply:

| Import failure | Claims left undriven | Reading |
| --- | --- | --- |
| `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING` | 227 | an export map pointing at TypeScript source under `node_modules` |
| `ERR_MODULE_NOT_FOUND` for `@solid-primitives/utils` | 84 | an **undeclared** dependency — the package imports it and declares it nowhere |
| `ERR_PACKAGE_PATH_NOT_EXPORTED` for `./web` | 81 | the subpath the package imports is not in its own export map |
| `[solid-devtools]: Debugger hasn't found the exposed Solid Devtools API` | 66 | the package refuses to load outside its own runtime |
| `ERR_MODULE_NOT_FOUND` for `server-only` | 60 | undeclared again |
| `Cannot read properties of null (reading '_depth')` | 50 | `@solidjs/router` reaching for a Solid owner at module scope |
| `ERR_UNKNOWN_FILE_EXTENSION` for `.jsx` | 27 | uncompiled JSX shipped to npm |

The distinction that matters here: **a missing *peer* is the harness's gap and
is now closed; a missing *undeclared* import is the package's.** Completing an
undeclared import would mean this harness choosing a version the package never
named, so it does not.

### The probe environment, recorded rather than assumed

393 of the 416 rows had at least one session that faked at least one global
(382 in `client`, 393 in `development`, 382 in `production`; `server` never) —
eleven fewer than the previous state's 404, exactly the eleven packages that no
longer generate a contract to probe.
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

Worker processes: 17,336 started, of which 15,796 were restarts after a probe
threw, and 63 sessions died (20,070 / 18,487 / 75 previously, and 20,367 /
18,784 / 78 before that). A restart is not a
failure — it is the only way to un-halt a Solid 2.0 development runtime — but the
count was previously invisible except as an unexplained probe duration, and it is
the shape behind every slow row. `@kobalte/core@0.13.13` alone accounts for
hundreds. The 2,734-process drop is the smaller claim plan: fewer claims to drive
is fewer probe bodies to restart around. Each session also carries the
**capability** of the runtime that drove it, `{"reruns": true|false}` per mode, so
a mode's withdrawals are visible as measured rather than reconstructed from a pile
of per-claim reasons. Across
the corpus that answer is completely uniform: `server` answered `false` in all
383 rows that ran a server session, `client` and `production` answered `true` in
all 382 and `development` in all 393, and **no session was left unmeasured** —
there is not one `null` in the corpus.

Install environment: 53 Solid 2 rows were given the `@solidjs/web` half of the
runtime they pinned only half of, 27 rows had a peer install (37 peer packages
in total), and 4 rows' peer installs failed or moved a pin and were reverted to
the pinned-only tree.

**Every figure in this section is identical in the A9 stage-1 state**, to the row
and to the session: 393 rows shimmed with the same fifteen names, the same
382/393/382 mode split, 17,336 worker processes with 15,796 restarts and 63
deaths, the same install-environment counts. The change set does not touch the
probe environment, and the measurement confirms it did not.

### What a verified contract actually certifies

This is still the number that should govern how the 66.11% is read, and it is
still the one place where more coverage buys less certification per row. The
attested-closure state is identical to A9 stage 1 in every cell of this table, so
the two share a column:

| Figure | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (execution kinds + render edges) | 2026-08-24 (export-kind proof) | 2026-08-24 (A9 stage 1, and attested closure, current) |
| --- | --- | --- | --- | --- | --- |
| Claim domains converted to unknown | 379 | 595 (`returns` 316, `callbacks` 267, `asyncBehavior` 12) | 739 (`returns` 407, `callbacks` 320, `asyncBehavior` 12) | 811 (`returns` 443, `callbacks` 356, `asyncBehavior` 12) | **822 (`returns` 448, `callbacks` 362, `asyncBehavior` 12)** |
| Exports carrying an unknown in the verified rows, at generation | 150/880 (17.05%) | 797/1,885 (42.28%) | 939/2,434 (38.58%) | 1,204/2,608 (46.17%) | **1,293/3,014 (42.90%)** |
| Exports carrying an unknown in the verified rows, after verification | 431/880 (48.98%) | 1,133/1,885 (60.11%) | 1,416/2,434 (58.18%) | 1,718/2,608 (65.87%) | **1,751/3,014 (58.10%)** |
| Verified rows carrying at least one **probed behavioral row** | 6/194 (3.09%) | 15/222 (6.76%) | 3/261 (1.15%) | 3/267 (1.12%) | **3/275 (1.09%)** |
| Probed behavioral row markers kept across the whole corpus | 12 | 25 | 3 | 3 | **3** |
| Inferred row markers dropped by verification | 1,118 | 2,292 | 2,955 | 3,115 | **3,200** |
| Probed markers discarded as unwitnessed by this run's report | 11 | 29 | 105 | 125 | **129** |

**98.91% of verified contracts certify no observed behavior at all.** The three
surviving rows are the same three as the previous two states —
`@tanstack/solid-query@5.101.4` and both `@tanstack/solid-query@6.0.0-rc.0`
probes, one marker each — so neither change set touched the observed-behavior
floor in either direction; the ratio moved only because the denominator did.

**Exports certified by a verified contract rise 890 → 929, and this time the rise
is real rather than a population effect.** The previous state's reading — more
rows verifying while each certifies less — still holds as history; what is new is
that the +39 arrives while **334 exports were dropped from promoted documents**
with their refused entrypoints. Those 334 are counted in the composite below as
their own state and stay inside its denominator, so the certified share cannot
have risen by unobservable exports leaving the population. The generation-side
figures rise too — the verified rows' draft export total goes 2,608 → 3,014, since
eight more rows' drafts are now counted — and the denominators in the two
"carrying an unknown" rows move with it, which is why those two percentages fall
while their absolute counts rise.

**The 11 new conversions are one row.** `corvu@0.7.2` contributes all of them
(`returns` +5, `callbacks` +6); the other seven newly verified rows convert
nothing, because what survived their promotion states almost nothing to convert.

### The composite a consumer feels

Of all 8,696 exports the corpus's generated contracts describe — unchanged from
the previous state, because generation did not move:

| State | 2026-08-22 | 2026-08-23 (probe environment) | 2026-08-23 (execution kinds + render edges) | 2026-08-24 (export-kind proof) | 2026-08-24 (A9 stage 1, and attested closure, current) |
| --- | --- | --- | --- | --- | --- |
| (a) certified by a verified contract | 449 (4.98%) | 752 (8.34%) | 1,018 (11.29%) | 890 (10.23%) | **929 (10.68%)** |
| (b) honest unknown inside a verified contract | 431 (4.78%) | 1,133 (12.57%) | 1,416 (15.71%) | 1,718 (19.76%) | **1,751 (20.14%)** |
| (c) dropped from a verified contract with its refused entrypoint | 0 | 0 | 0 | 0 | **334 (3.84%)** |
| (d) inside a contract that never reached `verified` | 8,135 (90.24%) | 7,130 (79.09%) | 6,581 (73.00%) | 6,088 (70.01%) | **5,682 (65.34%)** |

**The table gains a state, exactly as A9 said it would**, and the old (c) becomes
(d). (c) is the new one: exports a promotion *dropped* because their entrypoint
was `kind`-refused. They are not certified, they carry no honest unknown a
consumer can read, and they are not in a refused document either — the document
verified without them, so a consumer importing one gets the fail-closed
pre-contract state.

**The denominator is unchanged at 8,696, which is the invariant that makes (a)
readable.** A9 required the dropped exports to stay inside it precisely so the
certified *share* could not rise by unobservable exports leaving the population.
It held: 929 + 1,751 + 334 + 5,682 = 8,696, and the arithmetic of the movement
closes exactly — **406 exports left (d)**, of which **334 became (c)** and **72
became (a) or (b)** (+39 and +33). So (a)'s rise from 10.23% to 10.68% is 39
exports newly certified against a fixed denominator, not a shrinking one.

(d) is every export of a contract that was generated and then refused, timed out,
or errored. Rows whose install or generation failed describe no exports and are in
none of the states. (a) and (b) together are now **30.82%**, from 29.99%, 27.00%,
20.91% and 9.76%.

**Read (c) as the cost this change set made visible.** 334 exports — 3.84% of the
corpus — sit in a state that did not exist before, and they are there because a
document was promoted without them rather than refused with them. Whether that is
better for a consumer than the previous state's whole-document refusal is the
consistency argument A9 makes, not something this measurement decides: the same
export is uncertifiable either way, and what changed is that the other 72 exports
of the same eight documents are now usable.

### What it costs

The table below is retained from the pre-Bun baseline; its `npm install` labels
and cache observations are historical. New benchmark runs use Bun and report
their install phase as `bun install`.

| Phase | Rows | Median | p90 | Max |
| --- | --- | --- | --- | --- |
| `npm install` | 416 | 351 ms | 615 ms | 14,336 ms |
| `contract generate` | 413 | 121 ms | 667 ms | 18,554 ms |
| `contract probe` | 396 | 663 ms | 3,242 ms | 202,807 ms |
| `contract verify` | 396 | 52 ms | 61 ms | 81 ms |
| generate + probe + verify | 413 | **879 ms** | **4,031 ms** | 221,421 ms |
| whole row, install included | 416 | 1,348 ms | 4,962 ms | 221,942 ms |

Under a second at the median for the checker's own three phases, unchanged across
all seven states. **`contract generate` now pays for two extra round trips and it
is barely measurable here**: median 105 ms → 121 ms, p90 582 ms → 667 ms. The
attributable figure is the generation benchmark's aggregate worker time, +2.4%
(197,685 ms → 202,504 ms over 416 probes), because it is a mean over the whole
corpus rather than a median of a distribution this run also shifted for host
reasons. `contract verify` is flat: median 47 ms → 52 ms, max 108 ms → 81 ms.

The maximum is the four wide-surface rows the scaled budget rescued four states
ago — `@kobalte/core@0.13.13`, `@tanstack/solid-table@9.1.2`,
`motion-solidjs@0.7.0-beta.4`, `@kobalte/utils` — still completing rather than
timing out, except that `@kobalte/utils` no longer generates a contract to probe
at all. The whole 416-row corpus took 6 m 24 s of wall clock at concurrency 6,
against 7 m 08 s, 6 m 54 s, 7 m 18 s, 7 m 10 s, 7 m 11 s and 6 m 30 s. **Do not
read the `npm install` row in either direction as something a change set caused**:
its median has now gone 346 ms → 677 ms → 351 ms across three states purely on how
warm the host's npm cache was. No change in any state touches installs, and
`pipelineWithoutInstall` is the row to compare. The run's tail is still one row
finishing alone after the other 415 — `@tanstack/solid-table@9.1.2` at 167 s,
whose client-mode session death is the artifact recorded in the measured state —
so the total is bounded by the longest row plus how late it is dispatched rather
than by the corpus's total cost. The single longest row is
`@kobalte/core@0.13.13` at 222 s, but it was dispatched early enough not to be the
tail.

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
- **A `verified` row may now describe less than its package's export map.** Since
  A9 stage 1, eight of the 275 verified rows promoted a document with an
  entrypoint omitted — 30 entrypoints and 334 exports in total — so "this row
  verified" no longer implies "every subpath the draft described was certified or
  honestly unknown". The dropped subpaths are named in each row's
  `<contract>.verify.json` and in its rewritten review plan's
  `refused-entrypoint` items, and counted as their own state in "The composite a
  consumer feels". Six of the eight promote fewer than nine exports.
- **A callback claim driven by a server *build* is not verifiable in this corpus,
  in either direction.** Both audited Solid releases resolve the `node` condition
  to a build that re-runs nothing, and every session that measured one answered
  so (383 of 383), so a callback observation made in it is withdrawn whichever way
  it came out — 49 claims outright, and more as the withdrawn half of a
  multi-mode claim. The withdrawal is per *runtime*, not per mode: a `server`
  session probing `solid-js/jsx-dev-runtime` drives `dist/solid.js`, which
  re-runs normally, and those observations still count — four of the eleven
  remaining execution-kind failures are exactly that. Reaching a server *build's*
  schedule needs a probe that drives its own scheduling, which does not exist.
- **This measurement executed package code.** Nothing here is a safety claim
  about any package.

## 2026-08-26 authority run: recursive leaves and contradiction repairs

A fresh, uninterrupted 416-row run used checker SHA-256
`c1b606862f4ea98ac719c0d53c1db51d19a0f62e60ffb9f2899bb8b85d0f6cf8`
and Type Facts SHA-256
`31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`.
The checked-in [verification report](../benchmarks/ecosystem/verification-report.md)
is the unmodified result of that run:

| Outcome | Previous checked-in run | Current run |
| --- | ---: | ---: |
| Verified | 284 | **297** |
| Refused | 109 | **97** |
| Generation failure | 15 | **17** |
| Install failure | 6 | **3** |
| No runtime | 2 | **2** |

Of 396 rows that generated contracts, 297 verified (75.00%); across the fixed
416-row denominator the rate is 71.39%. Solid 1 contributes 115 verified rows
and Solid 2 contributes 182. All 7,734 driven claims passed. The preceding eight
runtime contradictions — five callback-timing shapes, two additional instances
of those shapes, and one exported-kind mismatch — are zero; the report contains
no failed claim and no contradicted `kind` claim.

The gain is not a claim that every newly named leaf is now observable. Recursive
return traversal removed the coarse 270-claim `nested return leaf` bucket, but
most accessor leaves lack a callback in which to plant a reactive source:
`no plantable reactive source` increased from 235 to 859, and store-path leaves
from 23 to 51. They remain uncertifiable. Other broad gaps are 1,079 reactive
reads with no runtime probe form, 463 owner requirements, 98 async claims, 9
callback-argument descriptors, 601 entrypoint-import throws, 368 synthesized
throws, and 286 calls that never invoked the planted callback.

Raw generation and install classes are environment-sensitive. This run traded
three prior install failures for two additional generation failures and produced
one more contract overall; no no-contention substitution was made. The reports
therefore preserve 297/97/17/3/2 rather than presenting a corrected composite.

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
