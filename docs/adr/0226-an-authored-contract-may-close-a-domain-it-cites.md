# ADR 0226: An authored contract may close a domain it cites

- Status: accepted (2026-10-07). The consumer and tooling changes are
  implemented; specs that use them follow in later batches.
- Owners:
  - the closure policy: `scripts/author-contracts.mjs` (`validateClosures`);
  - timing-aware package reads: `contracts::project_reactive_reads` and
    `operation_runs_after_the_call`;
  - optional-owner leaf rules: `contracts::project_leaf_forbidden_operations`
    and `leaf_forbids`.
- Relation: extends ADR 0198 (the authored tier, positive claims only) and
  ADR 0223.

## Context

The owner wants every Solid Primitives call site the corpus has to be a
proven violation or certified clean, never a package-contract obligation.
That needs closed claim domains, for example "this export reads nothing
else" and "creates no other owner". A runtime probe shows what happens, not
that nothing else does. A first research batch
(`rust/target/research/primitives-batch1/`) found that, with the format and
consumer as they were, only 2 of 35 exports could close all four domains
`SC9005` demands (reads, returns, creates and callbacks). The blockers were
systematic:
- package reads that happen later were attributed to the call;
- operations that tolerate no owner but need a present one to accept
  children could not be forbidden in a leaf;
- callbacks dispatched by host APIs;
- adversarial edge cases.

## Decision

1. **A closure stands on a citation, with probes for the positives.** An
   authored spec may list domains in `call.closed` only if each one has a
   `closures[domain]` entry citing the installed source (file and lines)
   that shows the export's complete behavior in that domain. Its positive
   claims must still pass their probes (ADR 0198). The tool refuses a closed
   domain without a citation, and a citation for a domain that is not closed.
   `closures` is spec metadata and is not written into the document.
   - Admission pins the cited bytes by identity, so the citation stays true
     for every install that is admitted.
2. **Premise:** built-in function properties (`length`, `name`) and the
   coercion of a primitive value are not reactive reads. A contract may close
   `reads` without listing them.
3. **Only reads at the call are attributed to it.** A read that a contract
   states runs later is not a read during the call: queued or external, at
   an event other than the call, or triggered by a resource. Examples are a
   native event handler, a queued callback or result access. It stays a known
   item, so a closed `reads` still means "nothing else". A read whose timing
   is not stated is attributed to the call, as before.
4. **A leaf forbids what it cannot accept.** A leaf owner accepts neither
   child owners nor cleanups. An operation at the call that needs a present
   owner to accept either is forbidden in a leaf, even when it tolerates no
   owner at all. Examples are `createMemo` with an optional owner and
   `tryOnCleanup`. The missing-owner rule is unchanged: such an operation
   requires no owner.

## Consequences

- No fixture snapshot moved. The bundled and authored contracts state no
  operation that decisions 3 or 4 change.
- Not yet:
  - callbacks a host API dispatches (`addEventListener`, timers) stay an open
    domain until host dispatch is modelled;
  - lazy getters that create a signal only on first observed read
    (`createElementSize`, `createWindowSize`) keep `returns` open.
