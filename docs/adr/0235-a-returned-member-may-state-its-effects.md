# ADR 0235: A returned member may state what calling it does

- Status: accepted and implemented (2026-10-08). Pilot: `createRAF`'s `start`
  and `stop`.
- Owners: `ValueShape::EffectfulCallable` (`contract_semantics.rs`), its
  validation (`validate.rs`: `lift_effectful_members`, `normalize_call_in`),
  digest tag 24 (`canonical.rs`), the wire form (`contract_document.rs`,
  `schema/solid-reactivity.schema.json` `effectfulCallableValue`), the
  certification refusal (`certification.rs` `inspect_candidates`), the
  projection (`contracts.rs` `project_returned_member_effects`,
  `bind_returned_member_effects`), `contract_declared_state` (`lib.rs`), and
  `scripts/author-contracts.mjs` (`memberClosures`).
- Relation: the format extension ADR 0234 was the prerequisite for (E1 in
  `rust/target/research/format-extension/DESIGN.md`, first slice). Additive
  to `schemaVersion: 1`: no document before it carries the shape, and every
  old document keeps its bytes, meaning and digest.

## Context

ADR 0234 made a call of an opaque returned member a proof obligation. The
contract had no way to say what that call does, so `createRAF`'s `start()`
in a component body, which Chrome flags (`STRICT_READ_UNTRACKED`), could
never be proven.

## Decision

1. **A new value, `effectful-callable`, is a member's own call graph.**
   `{"kind": "effectful-callable", "call": { ... }}`, with the ordinary call
   vocabulary. Its operations share the export's id namespace and must not
   reuse an export operation id. It may name the export's resources, which is
   how `start` says it reads the signal `createRAF` created.
2. **Placement.** It is valid only as a direct member of a `return`
   operation's tuple or object output. As a whole output, an input, inside
   another member, or returned by another effectful callable, it is refused.
3. **Certification refuses it.** No census proves what a member's call does,
   so only an authored, probed contract states one. An authored spec cites
   each closed domain of a member in `memberClosures[<returnOp>.<member>]`,
   and every positive claim needs a probe pair, as for the export.
4. **The consumer binds it.** A `const` destructuring of a contracted call
   makes each effectful member a callee with its own summary. Every rule then
   reads a call of it as a call of an export. Its reads are the call site's,
   not the package's (`contract_declared_state`): when a member runs is the
   caller's choice. A `let` binding, an escape, or access through the
   undestructured value keeps ADR 0234's obligations.

## Consequences

- `createRAF` and `default` state `start` (one read of `running`, at the
  call, in the caller's tracking context) and `stop` (no read). The
  `start-read` pair passes in Chrome on rc.13 for both.
- Primitives ledger, browser: 21 report correctly (was 20).
  `raf-createRAF-start-top-level` is now a proven `strict-read-untracked`
  violation. No correct twin on any host has a violation.
- Coverage: the new `package-effectful-member-consumer` fixture pins every
  branch. No other fixture moves.
- Not in this slice: captured constructor parameters, a member's writes with
  a write policy, members of members, and the date setters' later effects.
  `writes` stays open on every member.
