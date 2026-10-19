# ADR 0328: Uncertifiable results are internal

- Status: accepted and implemented (2026-10-19). Owner decision.
- Owners: `user_facing`, `certify_fails` and `render_user`
  (`solid-facts-backend/src/snapshot_emission.rs`); the `--internal` flag
  (`main.rs`); the JSON `--certify` exit of the daemon client (`daemon.rs`);
  `userFacing` in the ESLint adapter (`packages/cli/eslint.cjs`).
- Supersedes: the user-facing tiers of ADR 0202 (violations, then "needs
  review", then coverage gaps) and ADR 0248's opt-in contract-gap reporting.
  Both remain, unchanged, behind `--internal`.

## Context

On the rc.13 corpus (49 projects) the checker reports 554 violations and 4,101
uncertifiable results. An uncertifiable result says the analysis could prove
the code neither correct nor wrong. A user cannot act on that, and seven times
as many of them as violations bury the violations. The owner decided
(2026-10-19) that uncertifiable is an internal measure: it drives coverage,
corpus sweeps, the runtime oracle and decided-share tracking, and users see
proven violations.

## Decision

1. **Users see** a finding when it is not uncertifiable, or when it is a
   project-level notice (`subjectKind: "project"`). The notices are SC9013
   (the installed Solid runtime is refused), SC9014 (an unaudited Solid or
   router release). They say the analysis did not apply as stated; hiding
   them would show a clean run over a project that was never analyzed as
   stated.
2. **Human formats** (`default`, `full`, `text`) show only those findings.
   `default` prints violations, then notices. `text`'s status line is
   `violation`, `notice` or `clean`.
3. **`--certify`** fails on a violation or a notice.
4. **Internal views keep everything.** `--format json` is the complete machine
   report: every finding, and the certification status
   (`certified`/`violation`/`uncertifiable`), unchanged, because sweeps,
   coverage snapshots and the oracle read it. `--internal` restores the ADR
   0202 tiers in the human formats and makes `--certify` fail on anything short
   of certified.
5. **ESLint** reports only user-facing findings, from the catch-all rule and
   from every per-rule rule alike.

## Consequences

- A weak proof now costs users nothing when it falls back to uncertifiable.
  That makes uncertifiable the safe default for any proof that is not cheap,
  and a proven violation the only claim a user sees.
- Silence covers both "proven correct" and "not decided". Recall is guarded
  only by internal measures: the runtime oracle (`scripts/runtime-oracle.mjs`),
  the corpus sweep's uncertifiable count, and seeded mutants.
- The WASM adapter returns the complete report, like JSON; a host that shows
  it to users applies the same filter.

## Proof obligations

| Place | Decision | Premise | Test |
| --- | --- | --- | --- |
| human formats | `user_facing` filter unless `--internal` | a user cannot act on "not decided" | `formats_snapshot_without_adapter_specific_logic` |
| project notices stay visible | `subjectKind == "project"` passes the filter | a refused or unaudited runtime must not look clean | `internal findings are never reported, project notices are` (adapter); `dialects_process` refusal tests with `--certify` |
| exit status | `certify_fails`: violation or notice; `--internal`: not certified | CI should fail only on what a user can act on | `text_and_json_share_certification_semantics` |
| JSON unchanged | no filter for `json` | internal consumers read every finding | coverage snapshots, corpus sweep |
