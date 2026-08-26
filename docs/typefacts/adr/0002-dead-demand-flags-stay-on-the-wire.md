---
status: superseded by ADR-0003
---

# Dead demand flags stay on the wire

Three of the nine demand flags in the v3 schema — `type`, `resolveAlias`, and
`declarations` — are honoured by no producer path and cannot be expressed by the
Rust client, whose demand type has no such fields. They stay on the wire,
accepted and ignored, because removing them is a breaking protocol change rather
than cleanup: the producer decodes with unknown fields treated as errors, and
both languages pin the schema digest in the startup handshake, so a producer and
a client that disagree about the schema refuse to talk to each other.

> **Superseded by [ADR-0003](0003-coordinated-schema-bump-removes-unhonoured-request-fields.md).**
> This ADR treated a coordinated release as an extra cost the removal would
> incur. That was wrong: the handshake already pins the build id as well as the
> schema digest, so producer and client have always shipped in lockstep. The
> removal cost nothing that the deployment contract was not already paying.

Alias targets and declarations are not withheld by their absence — they arrive
unconditionally through symbol closure. Nothing is missing from the answers.

## Considered options

- **Delete them now.** Rejected: changes the schema digest, so every producer
  and client must ship together, for no behavioural gain.
- **Keep them indefinitely.** Rejected: they mislead. Each one reads like a
  capability a caller could ask for.
- **Batch them into one coordinated schema release.** Chosen, together with
  `structuralSpans` and `compilerSpans`, which are dropped by the producer for
  the same reason and carry the same constraint. One digest bump, not two.

## Consequences

- The Go compact demand codec keeps flags 6–8, and the per-file demand hash
  digests three bytes per demand that are always zero.
- A future reader will find these flags dead on both sides and reach for the
  obvious deletion. This ADR is that deletion's precondition, not its objection:
  do it, but do it as a coordinated release.
