# ADR 0119: SC9005's severity follows the gate that raised it

- Status: accepted and implemented (2026-09-25)
- Date: 2026-09-25
- Owners: the shared finding projection (`solid-reactive-ir/src/projection.rs`)
  and the rule page `docs/rules/package-contract-incomplete.md`
- Relation: ways-to-improve § 3.6, step 5 of § 4. Changes no finding kind, no
  run status, no rule option and no rule metadata.

## Context

`SC9005` is `error, uncertifiable` (`rules.rs`), and rule options take
`enabled` only, so a native user's only escape from it is to disable the rule
and lose every fail-closed answer with it. The same rule reports two very
different situations. At the **acceptance gate** no receipt-accepted contract
matches the import: the analysis runs with a premise missing altogether, the
situation `SC9013` reports as an error for the runtime itself. At the
**open-claims gate** a contract *was* accepted and says, claim by claim, which
domains it leaves open: the analysis runs over a stated, partial premise. The
A/B on `kobalte/packages/core` showed the tier turning 45 acceptance-gate
findings into 139, 97 of which name open domains, so shipping more contracts
made more errors.

## Decision

**A `SC9005` finding raised at the open-claims gate
(`analysis_context: unknown-contract-claims:<claims>`, at an import or at a
call argument) has severity `warning`. Every other gate keeps the rule's
`error`: no accepted contract, a policy-1-only contract, an accepted contract
that omits the export, and a claim the site cannot bind.** The override is
applied per finding in the shared projection, so the per-package and per-claim
collapse carries it.

The finding kind stays `uncertifiable`. The run status is derived from kinds
(`diagnostics.rs`), not severities, so a run with only open-claims warnings is
still `uncertifiable` and certifies nothing: fail-closed is kept. What the
severity says is how far the analysis got, not whether it is proven.

## This inverts the one per-finding precedent

The only existing per-finding severity override,
`Finding::for_owner_requirement`, **raises** an uncertain owner requirement to
`error`. This one **lowers** a severity. The two are consistent in what they
measure: an uncertain owner requirement is a possible runtime defect the
analysis could not rule out, and the error says "this may be broken"; an open
claim of an accepted contract is a limit of what the package's author has
certified, and the warning says "this could not be proven". Neither changes
the kind, and neither lets a run certify.

## Consequences

- Three fixture snapshots move by one line each (`severity`), the
  open-claims findings of `package-argument-container-consumer`,
  `package-plain-return-consumer` and `package-protocol-callbacks-consumer`.
- Adapters that map severity (the ESLint adapter) now report those findings
  as warnings.
- Not done here, and still open in § 3.6: moving the import-level `creates`
  obligation to each call, which needs a call-site defect variant and a
  per-call owner-context measurement.
