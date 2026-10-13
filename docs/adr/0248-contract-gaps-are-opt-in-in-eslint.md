# ADR 0248: Contract gaps are opt-in in ESLint

- Status: accepted and implemented (2026-10-08).
- Owner: `packages/cli/eslint.cjs` (the `certification` rule's filter and
  the dialect config generation).
- Relation: carries ADR 0202's split (violations, findings about the user's
  code, analysis-coverage gaps) into the ESLint adapter.

## Context

An import of a package without a complete reactivity contract raises
`SC9005` (`package-contract-incomplete`, uncertifiable). The CLI's default
output already summarizes these under "Analysis coverage", labelled as not
findings about the user's code. In ESLint:

- `configs.recommended`'s catch-all `certification` rule reported each one as
  a warning at the import;
- `configs.v2` enabled `package-contract-incomplete` at `error`, so every
  import of an uncovered package failed the lint.

## Decision

- `certification` leaves out uncertifiable `package-contract-incomplete`
  findings.
- No shipped config enables `package-contract-incomplete`.
- Enabling `solid-checker/package-contract-incomplete` reports them again, at
  the severity the user picks.
- JSON and CLI outputs, `--certify`, and the snapshot's status are unchanged:
  a project with gaps is still not certified. The other coverage rules
  (`reactive-dispatch-unresolved`, `reactive-source-uncaptured`,
  `unaudited-solid-release`) still report as before.

## Consequences

- Editors no longer show one warning per uncovered import. A project
  linted with `configs.v2` no longer fails on contract gaps alone.
- A user who relies on gaps being visible must enable the rule. The README
  says how.
