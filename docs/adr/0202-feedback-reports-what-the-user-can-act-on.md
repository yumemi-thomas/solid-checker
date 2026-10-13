# ADR 0202: Feedback reports what the user can act on

- Status: accepted (2026-10-05). Track A of
  `docs/2026-10-05-package-direction.md`: feedback quality.
- Owners:
  - `reactive_analysis::collect_project` (contract-generation obligations);
  - `interproc::interprocedural_result_reads_for_file` (dispatch obligations
    by role);
  - `snapshot_emission::render_default` (the default output's tiers).

## Context

On the rc.13 corpus, 6,801 of 7,385 findings were uncertifiable. Most of them
were not about code the user wrote:

- **1,880 contract-generation obligations.** These were "callback parameter N
  of current project:fn reaches a call whose execution timing is unknown":
  the analysis could not state, for a *package contract* of the project's own
  exports, when an export runs a callback parameter. Only a consumer of that
  contract needs it, and an application has none (ADR 0193).
- **2,685 unresolved member dispatches** ("F invokes .M on a caller-supplied
  value"). 1,450 distinct root causes sit behind them, about 400 of them at
  call sites in event handlers, deferred callbacks or tracked JSX, where no
  read rule reports anything.
- **The default output** rendered each of them as a full diagnostic, with
  code frame, help and docs link, interleaved with the proven violations.

## Decision

1. **A closed program does not report contract-generation obligations.**
   They stay in the analysis result for `contract generate`. Libraries (open
   programs) still report them.
2. **An unresolved dispatch is reported only where the reads it hides could
   be.** A call site whose execution role is an event callback, a deferred
   callback, a tracked JSX region or deleted code raises no dispatch
   obligation. The read rules fed by summaries never report reads in those
   roles: strict reads report only the untracked roles; the conditional-return
   rule asks about tests in a component body; result-access asks about
   predicates. Asynchronous reads come only from direct reads and are
   unaffected.
3. **The default output has three tiers:**
   - violations, in full;
   - "Needs review": uncertifiable findings about the user's code, in full;
   - "Analysis coverage": the uncertifiable findings of the coverage rules
     (`package-contract-incomplete`, `reactive-dispatch-unresolved`,
     `reactive-source-uncaptured`, `unaudited-solid-release`). These are
     grouped by root cause (same code and message), one line per group with
     its site count and first site, and the top 20 groups are listed.

   `--format full` keeps the previous one-diagnostic-per-finding rendering,
   and `--format json` and `text` are unchanged.

## Consequences

- An application's report is its violations, a review list, and one coverage
  summary.
- Nothing proven moves. 1 and 2 remove obligations that decide no claim, and
  3 changes presentation only.
- A library still sees its contract-generation obligations. They may move to
  `contract generate` output entirely later.

## Evidence

- rc.13 corpus, browser host, against the ADR 0201 sweep:
  - uncertifiable results go from 6,801 to 4,011 (-2,790, -41%), with none
    added: 1,794 contract-generation obligations and 813 dispatch
    obligations removed, plus 183 that shared their sites;
  - violations are unchanged at 285 (+0, -0).
- What a reader of the corpus now sees:
  - 759 findings to review, 693 of them `strict-read-untracked`;
  - 3,252 coverage sites, which group into 1,962 root causes. Messages name
    one helper or export each, so grouping by message alone compresses less
    than a per-package view would.
- Fixtures: `feedback-tiers` (closed) and `feedback-tiers-open` share one
  `App.tsx`. Only the open one reports the export's `SC9005`; both keep the
  body dispatch and drop the click-handler one. No other fixture moves (172
  projects, 907 findings).
- Process test:
  `diagnostics_process::default_output_groups_coverage_gaps_after_findings_to_review`.
