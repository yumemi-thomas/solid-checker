# Package direction, 2026-10-05

Owner decision of 2026-10-05: stop relying on certification as the source of
package knowledge. Contracts are authored by the project, probe-checked, keyed
by package version and shipped in the checker
([ADR 0189](adr/0189-package-contracts-are-authored-and-probe-checked.md)).
Most effort moves to the analysis of application code.

## Why

All measurements are over the 38-app corpus (`rust/target/app-import-metric`),
browser host, release binary.

- **Package misuse is rare in real apps.**
  - 10 package findings, all true positives, in two apps (`probus-hk`,
    `spotify-desk-thing`), after ADRs 0184 to 0188.
  - Of 34 hand-checked reads of router hook values in application code, every
    one outside `probus-hk` sits in a memo, an effect or a handler, which is
    correct.
- **The analysis of application code finds about twenty times more.** The
  sweep at ADR 0188 (`rust/target/defect-sweep/c0188-browser.json`) holds:

  | Rule | Violations |
  |---|---|
  | `strict-read-untracked` | 197 |
  | `reactive-handler-frozen` | 29 |
  | `missing-owner` | 24 |

  Package contracts deliver almost none of these.
- **Certification closes too little.** Signing raw proposals instead of
  certified contracts moved no violation. 76% of exported dependency callables
  propose nothing closed (2026-10-04).

## Baseline at 95d354d58

- **Sweep** (`c0188-browser.json`): 273 violations, as `compare.mjs` counts
  them. 10,396 uncertifiable results, of which:

  | Group | Results |
  |---|---|
  | `reactive-dispatch-unresolved` | 5,634 |
  | `package-contract-incomplete` | 2,764 |
  | `strict-read-untracked` | 1,224 |
  | `missing-owner` | 565 |
  | `reactive-handler-frozen` | 114 |
  | other | 95 |

- **Misuse ledger:** 79 of 123.
- **`make verify`:** green at 95d354d58 (611 s). Three stale checks were
  fixed on the way:
  - `incremental_contract_exports_refresh_changed_summaries` (f2a802428): the
    test's edit no longer moved the contract it asserted on;
  - the runtime-pins test (60a7ce52d): it assumed every `@solidjs/signals`
    release in the tier is a probe pin, which ADR 0186's consumer
    environments are not;
  - three tsc-oracle cases (95d354d58), which predated the Oct 4 precision
    changes (traced mounts, summary-attributed reads).

## The rc.13 measurement corpus (2026-10-05)

After ADRs 0194–0197 the audited release is rc.13, and no corpus app installed
it. The rc.13 corpus is a copy-on-write clone of the 38-app corpus
(`../solid-checker-rc13-corpus/apps`, outside the repository), made by
`rust/target/defect-sweep/upgrade-rc13.py`:

- every installed Solid 2 runtime copy (113, from rc.0 to rc.9, pnpm and Bun
  stores included) is replaced with the exact rc.13 tarball contents;
- lockfiles and every other package are unchanged;
- 48 of the 49 projects resolve the rc.13 triple (`en-passant` resolves no
  `solid-js`, as in the original).

Sweep at HEAD (`rc13-browser.json`) against the original corpus at HEAD
(`head-browser.json`, identical to ADR 0195's):

| | original | rc.13 |
|---|---|---|
| violations | 265 | 256 (+1, −10) |
| uncertifiable | 7,285 | 7,006 (+214, −488) |

- **−10 violations:** the package findings (TanStack router memos in
  `probus-hk`, the `useMutation` read in `spotify-desk-thing`). Their tier
  bundles were proven on the apps' old runtimes, so they no longer apply.
- **+1 violation:** the `combineProps` `overlays()` read in `app-game`, a
  true positive that ADR 0186's partial bundle had made uncertifiable.
- **−46 `SC9014` notices:** every app is on the audited release.
- **Package claims fall back** from "partial" to "no accepted contract", and
  64 reads at package boundaries become uncertifiable.

So on the latest release, package findings depend on Track B: authored
contracts, probe-checked on rc.13. The analysis of application code is
unchanged.

## Plan

### Track A: application-code analysis (main track)

Each step is one ADR with positive, negative and edge fixtures, a sweep, and
runtime confirmation of new violations.

| Step | Uncertifiable results | Change |
|---|---|---|
| A1 | ~900 | **Done (ADR 0190).** A primitive receiver dispatches to its built-in prototype. Uncertifiable 10,396 → 8,297, violations unchanged. |
| A2 | ~820 (738 findings on rc.13) | First slice done (ADR 0192): built-in calls in a leaf scope resolve, 813 → 766. The rest is methods of bodiless objects, optional calls (a Type Facts producer gap) and incomplete helpers. |
| A3 | ~620 | Callbacks passed to consumers not proven to invoke them synchronously. Done: ADR 0199 and its extension (callback props reaching only DOM events, 134 sites), authored `useQuery` options (33), ADR 0200 (wrappers running a literal as a tracked compute, 40). The remainder is mostly package option objects (Track B). |
| A4 | ~565 (14 uncertifiable on rc.13) | `missing-owner`: owner nullability (`runWithOwner(getOwner())`, unknown callers). On rc.13 all 14 are one `runWithOwner(getOwner())` and need a two-pass owner proof. |
| A5 | 2,672 findings on rc.13 | Members invoked on caller-supplied values (`.map`, `.preventDefault`, props methods): each caller's argument identity. Diagnosed 2026-10-05; see `docs/precision-backlog.md`. |

### Track B: authored, probe-checked contracts (ADR 0189)

- **B1.** ADR 0189: the trust rule.
- **B2.** Layout: `pkg/contracts/authored/<package>.json`, the existing claim
  vocabulary, an explicit version list, and probe cases beside it.
- **B3.** Convert the existing certified contracts into authored entries,
  spot-checked and probe-tested, so the ledger and the 10 package findings
  survive.
  - First slice done (ADR 0191): the compiled-in tier is admitted on its
    package bytes and its Solid runtime, not the whole dependency tree. It
    moved nothing on the corpus. The apps run Solid rc.3, rc.4 and rc.9, so
    the **Solid release**, not the rest of the tree, limits reach. Authored
    contracts therefore need a probe run per audited Solid release.
- **B4.** Pilot on TanStack router and query, `@solidjs/router` and
  `@solidjs/meta` hooks.
  - Exit: the 10 package findings come from authored contracts with the
    certified tier off, and there is no false positive.
- **Later:**
  - the probe drafts contracts for review;
  - a job checks new package releases against their contract.

### Track C: retire certification (after B4 passes)

Remove, in green slices:

- in-place certify;
- signing, receipts and trust configuration;
- environment admission;
- the certified tier (`pkg/contracts/accepted`, `embedded.rs`);
- the generator, census and veto as authorities.

Keep the contract vocabulary and reader, the probe harness, and every
application-code analysis. `SC9005` becomes "no contract for this version".
Update AGENTS.md and re-baseline the gates that depend on certification.

### Order

1. A1 and B2 to B4, interleaved.
2. C.
3. A2 to A4.

## Risks

- **A wrong authored claim is a false positive.** Every claim needs a probe
  case, and version lists are explicit.
- **Removing certification breaks gates.** Remove it in small slices, with
  `make verify` at the end of Track C.
- **Track A may yield few violations.** Fewer uncertifiable results is still
  less noise. Report both numbers.
