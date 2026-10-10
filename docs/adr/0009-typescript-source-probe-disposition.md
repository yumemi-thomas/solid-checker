# 0009 — Preserve the veto's artifact identity for TypeScript source cases

Status: accepted — retain the refusal; no transformed certification lane
Date: 2026-09-04

## Decision, before implementation

Keep ADR 0006's scheme-version 6 unchanged. A scheduled gate over a TypeScript
source artifact case must still run against that exact authenticated case. If
the pinned Node cannot load it, the gate remains `IncompleteGate` and closure
refuses. Do not substitute a published JavaScript sibling, transpile a private
copy, or waive the scheduled veto. Add regression fixtures for this disposition
and the ordinary published JavaScript path; change no production semantics.

The reason fits in one paragraph: the census establishes a claim about one
artifact case, and the veto is an independent opportunity to contradict that
claim in that case. Pinning a transformer proves which transformation ran, not
that it preserves every behavior relevant to the contradiction. A sibling's
published bytes are authentic but belong to a different case. Neither is the
missing equivalence proof. Keeping the refusal preserves the current meaning
of every accepted closed claim, at the measured cost of leaving 40 candidates
unprobeable. Investigating certification of the JavaScript cases themselves is
defensible future work, with their own census, dependencies, recipes and gates.

## Measurement that informed the decision

Starting point: `codex/phase19a-authenticated-proof-policy`,
`80d2a81e97fcdf38fe329f7896ebeb6cc9cd5501`. Before any edit, run the three-row
command in the task with the supplied recipe corpus, the debug checker and
local Type Facts producer, adding only `--keep-temp` for inspection. Outputs:
`before.json`, `before.md`, `artifact-measurement.json`, and `measure.cjs` under
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/`.
The checked-in ecosystem baseline has no recipe corpus and is **not** this
comparison's baseline. No benchmark artifact or phase20/21 ledger is repinned.

Enumerate `call.proposedClosures` in each freshly generated document, by
entrypoint, artifact case and export. Inspect the installed exact package's
published `exports` map and parse its `dist/index.js` export clauses with the
local TypeScript parser. This is a measurement of same-named public exports,
**not symbol equivalence or authority to transfer a closure**.

| row | candidates | source cases | same-named public JS exports | JS candidates in that row |
| --- | ---: | ---: | ---: | ---: |
| `@kobalte/utils@0.9.2\|solid1\|only` | 33 | 13 | 33 | 0 |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | 7 | 3 | 7 | 0 |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | 3 | 0 | already JS | 3 |

Thus option (b) has **40/40 (100%) same-named published JS siblings**, and
**0/40 (0%) replacements admissible for the existing gates**. This run counts
13 source cases for 0.9.2, not the historical document's 12; the candidate total
is still 33. In both Kobalte manifests `.` selects `dist/index.js` for import
and `dist/index.cjs` for require, while `./src/*` selects the source itself.
The measured JS SHA-256 values are:

- 0.9.2: `2117628a30cfb91851cf8751e3a35147776e3671eee52bd790dc74607d5a3eb8`.
- 2.0.0-alpha.0: `13414950b5a42bc1266d7b11668b5c8e9b2a5a1f8390db57d0dee83680d5245e`.

The same artifact acquisition reports integrity verified for all three rows.
Neither Kobalte JavaScript root currently offers a closure candidate:

- 0.9.2's `.` cases are omitted during generation: `accepted dependency
  @solid-primitives/keyed has no exact runtime binding for export Key`.
- Alpha's two `.` cases survive, but each carries
  `unaccepted-external-dependency` at `./dist/index.d.ts:@solidjs/web`, affecting
  `creates` and the other behavioral domains. This is the current observation,
  rather than the older document's `solid-js` shorthand.

## Options and their exact costs

### (a) Retain refusal, or replace the tier with census-only closure

Retaining refusal is the decision above. The veto still observes authenticated
published bytes of the selected case when it completes; it proves nothing
about a case it cannot load. Every scheduled failure blocks closure.

Census-only closure with the veto withheld is a different policy. It retains
the static census's conclusion but loses the independent runtime opportunity
to expose a declaration/runtime discrepancy, a census modelling gap, or a
package-initialization effect. Calling that the current certification would be
misleading. A consumer would need a distinct authenticated proof-policy identity
and an explicit supported acceptance mode, with receipts recording that no veto
was required. Current consumers must reject that identity until they opt into
its semantics; an empty `probe_gate_root` under the current policy is not such
a discriminator. Do not overload `withheldClosures`: today it means the domain
is **open** in the receipt-bound main. This slice adds no census-only policy.

### (b) Use a published JavaScript artifact case

Probing `dist/index.js` preserves the publisher's JS bytes and can detect a
contradiction in that JS case. It stops saying anything about the source case's
initialization, imports, implementation or export conditions. Bundling can
remove imports and unused declarations, inline values, or select a different
dependency path; a shared name and even a shared source map do not prove
equivalence. `verify_reported_resolution` correctly refuses that substitution.

The sound version is to **certify the JS case itself**, acquiring its own
census and authenticated dependency closure and deriving its own claim id and
gate. That leaves all 40 source claims open/refused. Availability is 40/40 by
export name, but actual new JS closure candidates in this measurement are 0.
Clearing the root-case dependency blockers is outside this workspace slice.

The subsequent user-requested investigation is recorded in
[`2026-09-04-published-js-binding-investigation.md`](../2026-09-04-published-js-binding-investigation.md).
It confirms that `Key` exists and binds; the actual dependency-graph certifier
advances to `solid-js/web`'s unknown `Aliases` runtime kind. Alpha's declaration
edge keeps its JS creates domains open despite authenticated compiler-source
support. Neither observation changes this ADR's exact-source-case decision.

### (c) A pinned, digest-bound transformer

A defensible weaker statement is: the veto ran the output of transformer T,
with exact options O, over authenticated inputs S, and observed no contradiction
in those **derived bytes**. The receipt must bind T's executable and dependency
identity, O, the input and output manifests, and the sandbox policy version;
the derived artifact must be named as derived, not passed off as published.
Missing or mismatching pins must refuse at the gate. Scheme 6 does not describe
this and would have to change, together with consumer acceptance semantics.

That binding still does not prove preservation. Import elision can remove an
initialization side effect; const-enum inlining can remove a runtime read;
decorator or field lowering can change the timing of calls; target lowering
can change iterator/coercion behavior; a transformation defect can erase the
very operation a recipe would detect. Even erasure-only processing can alter
source reflection (`Function.prototype.toString`) and stack/location-sensitive
behavior. A no-contradiction result applies only to the derived execution. A
contradiction found there is useful, but absence there cannot satisfy the
current gate for the published source. No reviewed preservation premise or
consumer-visible derived-case policy exists here, so this option does not ship.

### (d) Relocate unchanged source, or narrowly interpret JS-compatible `.ts`

Moving a package outside `node_modules` can let Node's own stripping run while
keeping the file's stored bytes unchanged. It also changes package scopes,
self-reference and bare-specifier ancestry, file URLs, and the resolution echo's
subject. A symlink also changes realpathing and violates the watched-tree rule.
Node's stripping is itself an interpretation of TypeScript; byte equality alone
is not a proof of equivalent module initialization/resolution after relocation.
This would need a fresh disposition table, workspace census and adversarial
resolver fixtures, as well as an explicit interpretation claim.

For JS-compatible source such as `noop.ts`, a trusted loader could require a
successful JS parse and hand the unchanged text to Node as a JS module. That
avoids general transpilation but still overrides Node's load policy and changes
what a `.ts` module means in this environment. It does not cover arbitrary TS
dependencies, and accepting by suffix or regex is not a parse proof. A new
loader plus a recursively enforced input restriction is a separate design,
not a reason to waive this gate. No measured claim of population coverage is
made for either relocation or this narrower loader.

## Invariants and receipt meaning

`PrivateProbeWorkspace::create`, `safe_package_directory`, authenticated
dependency closure, `verify_reported_resolution` and `SANDBOX_POLICY_FIELDS`
stay unchanged. So do compiled-in harness/Node pins, the 0700 workspace,
`env_clear` and its allowlist, process groups and `killpg`, one startup plus
one run frame, primordial capture/prototype freezing before recipe import,
and detect-and-refuse write isolation. There is no scheme bump because no
disposition changes. No existing receipt changes meaning.

The veto remains a bounded contradiction detector, never evidence of absence.
Its existing recipe coverage limitations, same-realm limitations and lack of
OS-level denial remain exactly ADR 0006's. This decision adds no new guarantee
and removes none. The accessor census over untyped JavaScript receivers remains
out of scope and continues refusing.

## Before/after and regression verification

The baseline produced exactly:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

The supplied corpus addresses four candidates, not 43: Kobalte 0.9.2's `noop`
and i18n's three. Alpha has seven candidates withheld for missing recipes;
0.9.2 has another 32 candidates not addressed by the corpus. Its failed audit
reports `withheldClosures: 0` because finalization did not return its accumulated
withholdings; that field must not be read as evidence that all 33 ran.

The same command after the fixture patch produced `after.json` and `after.md`
in the same scratch directory. Row outcome, class, certification status, demand
counts and `exportsProven` match; refusal text matches after replacing only the
ephemeral Type Facts project directory. Report SHA-256:

- Before: `1ae364bc20978b7c6ee2f90a4140fd518ccb0a71b4361797aecc0718d300a04c`.
- After: `bb905b9fd27fe805c8cbba16fac1e8ff423d487cbaf5261c33c7077579538ab4`.
- Artifact inventory: `57e0286144dcc09a79cc9a0b8b4403721ed3358ea938c6d4b8fb8e9439038351`.

| row | before | after | completed gates | contradictions |
| --- | --- | --- | ---: | ---: |
| Kobalte 0.9.2 | `noop`: census passes, gate incomplete; row refused | identical | 0 → 0 | 0 → 0 |
| Kobalte alpha | seven candidates withheld; certified with those domains open; no gate scheduled | identical | 0 → 0 | 0 → 0 |
| i18n | three closure demands; row refuses on `chainedTranslator`'s object spread at `dist/index.js:3471..3483`; no probe launched | identical | 0 → 0 | 0 → 0 |

Every scheduled gate, including those never reached, follows. IDs are replayed
from the audit's snapshot/demand-graph roots and the corpus's exact claim ids
with `probe_gate_id`'s length-framed SHA-256; `noop` also matches the emitted
refusal verbatim. These are schedule identities, not invented observations.

| export | gate id | before and after |
| --- | --- | --- |
| `noop` | `sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc` | `IncompleteGate` |
| `chainedTranslator` | `sha256:00b9ed80e49fba15d38aab0b1e9f24f12ee2611633dc03eb510839c53ce14b75` | not run: census acquisition refuses |
| `flatten` | `sha256:db644a8718275f606e5ecf0661e4c7666df8fec8521c71a352fbb22ccb91a45f` | not run: same transaction refuses before probing |
| `scopedTranslator` | `sha256:b56c77a2d09debd3875a393c453cdc715c07cd4003767bca41108c12c17ff2c4` | not run: same transaction refuses before probing |

**Newly probeable: 0/43. Total structurally probeable: 3/43 before and after.**
Four gates are scheduled, one reaches runtime evaluation and errors, zero
complete and zero contradict. No real-package contradiction was found to quote.
A contradiction would be a successful veto, not a regression; the existing
`the_probe_gate_tracer_refuses_a_contradicted_closure` and realm-tampering tracer
both still pass. `exportsProven` remains 0 for all three rows (0/50, 0/40, 0/9).
That report field counts fully proven exports in the generated document; it is
not a count of independently discharged `creates` demands.

`fixtures/package-contracts/probe-source-disposition` pins both outcomes. Its
only-TS package passes the real census, produces a worker error (not timeout or
missing frame), and refuses the exact scheduled gate as `IncompleteGate` both
at gate inspection and full certification. Its published-JS sibling certifies
`creates: []` with a nonempty authenticated receipt binding. The test uses the
build's own pins. No main document is checked in for this tracer, so phase19's
`stableMainDocuments` stays 179; the fixture directory was staged before that
gate. No generated snapshots, receipts, contracts or benchmark ledgers move.

Verification (one Cargo process at a time, through the Makefile; the additional
target file `/private/tmp/probe-ts-checks.mk` uses `CERTIFICATION_ENV` for the
requested narrow tests and Clippy):

| check | result |
| --- | --- |
| `make build-checker-debug` | passed; local Type Facts source stamp matched |
| `make test-probe-harness` | 95 passed, including the new pair and both existing contradiction tracers |
| backend `--lib` | 361 passed |
| IR `--lib` | 233 passed |
| armed `contracts_process` / `diagnostics_process` / `dialects_process` | 7 / 15 / 37 passed |
| non-updating contract corpus, fresh debug binary | 88 fixtures matched; no update needed |
| coverage, fresh debug binary | 94 projects, 546 findings matched |
| ownership gate, `--require-retained --require-complete` | 289 cases passed; 465 ledger rows, 0 pending |
| Vitest `scripts/*.test.mjs` | 153 tests in 25 files passed, including phase19 at 179 stable mains |
| `bun run --cwd packages/cli test` | 171 tests in 5 files passed |
| Cargo fmt, then fmt `--check` | passed |
| workspace/all-targets Clippy, `-D warnings` | passed with certification pins supplied |
| schema JSON and dialect manifest validation | passed |

The first harness build caught a test-only `unwrap_err` requiring an unavailable
`Debug` implementation; changed the assertion to `let Err(error)`, then the
complete harness and backend suites above passed. No production fix was needed.
Clippy was followed by a successful `make build-checker-debug`, leaving the
pinned binary in place. Logs are `/private/tmp/probe-ts-{build,harness,backend,rust-checks,corpus,coverage,ownership,scripts,cli,final-build}.log`.
`git diff --check` and `git diff --cached --check` passed. `make verify` is deliberately
deferred to the lead, as requested; no benchmark-wide rerun, snapshot update,
commit or push was performed.
