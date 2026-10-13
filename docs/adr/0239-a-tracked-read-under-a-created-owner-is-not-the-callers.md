# ADR 0239: A tracked read under a created owner is not the caller's

- Status: accepted and implemented (2026-10-08).
- Owner: `project_reactive_reads` and
  `operation_reads_under_its_own_computation`
  (`solid-reactive-ir/src/contracts.rs`).
- Fixture: `fixtures/reactive-ir/package-own-tracked-read-consumer`.
- Investigation: `rust/target/research/open-domain-census/CENSUS.md`.

## Context

`project_reactive_reads` turns each read a contract states into a read at
the call. It skipped reads stated to run after the call (ADR 0226), but kept
every same-stack read, whatever its `tracking` and `owner`. An export such
as `createPermission` reads its own signal inside an effect it creates. The
contract states that read `tracked` under a `created` owner, and Chrome on
rc.13 is silent for it. The checker still reported it as the package's own
untracked read (`SC1001`, uncertifiable) at every call, which dirtied the
correct twins of `createPermission`, `createSelection` and `createUserTheme`.

## Decision

A read stated `tracking: tracked` under an owner whose `source` is
`created` is not projected as a read at the call. It stays a known item of
`reads`, so a closed `reads` still means nothing else is read. Any other read
is projected as before, including one stated `tracked` under an ambient owner.

## Consequences

- No call is reported for a package's own read that its own computation
  observes.
- The rule trusts the contract's `tracking` and `owner` fields. Those are
  authored claims, so each one needs a citation (ADR 0226) and passing Chrome
  probe pairs before it ships.
