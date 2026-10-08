# ADR 0247: A contract read may state that strict reads are cleared

- Status: accepted and implemented (2026-10-08).
- Owners: `Operation.strict_read` (`solid-reactive-ir/src/contract_semantics.rs`)
  and its validation, digest and certification refusal; the wire field
  `strictRead` (`solid-facts-backend/src/contract_document.rs`,
  `schema/solid-reactivity.schema.json`); `project_reactive_reads`
  (`contracts.rs`); the package-read wording in `projection.rs`; authoring in
  `scripts/author-contracts.mjs`.
- Fixture: `fixtures/reactive-ir/package-strict-read-cleared-consumer`.
- Relation: completes ADR 0246, which stopped a package's own read from
  becoming a false violation through a wrapper. Brings back the
  `createScrollPosition` claim that ADR 0245 withdrew. Patch drafted
  read-only in `rust/target/research/strict-read-cleared/`.

## Context

rc.13's `untrack(fn, strictReadLabel)` clears tracking and sets the strict-read
window to the label, or to nothing
(`@solidjs/signals/dist/dev-shared.js:5863-5887`). An unlabelled `untrack`
therefore neither subscribes nor warns. `createDerivedStaticStore` reads its
own memo that way (`static-store/dist/index.js:98`), and `createScrollPosition`
is built on it. The contract could only call the read `untracked`. That word
covers a labelled `untrack`, which can warn, and the generator's unproven
`untracked`. So the consumer reported every such read as a package-internal
obligation, and its wording claimed that Solid warns.

## Decision

1. **A new optional field, `strictRead: "cleared"`, on a read operation.**
   - It is valid only on `kind: read` with `tracking: untracked`; elsewhere it
     is refused.
   - Authoring requires a citation for it.
   - Certification refuses to state it, because no census establishes the label.
   - An operation without it hashes exactly as before, so no existing contract
     changes digest.
2. **A cleared read is not a read at the call.** It stays a known item of
   `reads`, but it seeds no caller read and no package-internal notice, directly
   or through a wrapper. `untracked` alone still does.
3. **A package-internal read no longer claims Solid warns.** It says the
   strict-read context is not established, which is all the contract proves.
4. **`createScrollPosition`'s initial memo read states the clearing.** Its
   reads and creates closures from ADR 0245 return.

## Consequences

- Browser ledger: 73 of 114 report correctly (was 71). Both
  `createScrollPosition` cases now report correctly, and no correct twin has
  a violation.
- rc.13 sweep: no violation added or lost. Primitive import declarations
  still raising SC9005: 28 to 27 of 43. The six `solid-virtual` sites that
  produced false violations under ADR 0245 are clean.
- The new scroll claim keeps its host-free block. Its added operations also
  run on the server build (`scrollPosition.js:55`,
  `static-store/dist/index.js:98-105`). Probe results are bound to the claim
  by digest, so the scroll pairs were re-probed after the edit.
- The broader preservation of each read's execution context through summaries
  (`rust/target/research/read-tracking/DESIGN.md`) is still not implemented.
  A labelled `untrack`, or a read whose clearing the contract does not state,
  stays an obligation.
