# optimistic-write-site-code

**Claim (batch 11, slice 4; ADR 0968).** A write site carries Chrome's OPTIMISTIC_WRITE_BEFORE_LOAD (SC2009) beside REACTIVE_WRITE_IN_OWNED_SCOPE (SC2001) when the setter's source is optimistic and may be async, or when no fact names the primitive that made it (fail closed). The code is ended by SC2009's own row, by a silence that names it, or by the code's uncertifiable row `optimistic-write-unaccounted`; SC2001's row of the site is a separate ending (`write-unaccounted` beside SC2009's violation, because SC2009's violation does not answer SC2001's code).

- `Async`: `createOptimistic(async …)`'s setter before the first answer: SC2009's violation (rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD, ahead of any owned-scope guard) and SC2001's `write-unaccounted` beside it.
- `Aliased`: the same through `const write = setValue` (the alias row's path): the same two rows at `write(2)`.
- `Sync`: a sync source has its answer: no OPTIMISTIC code, no SC2009 row, no `optimistic-write-unaccounted`.
- `SlotAlias`: a setter reached through an object slot of a `const` arrow's result (ADR 0480). SC2001's violation stands, and no fact names the alias's primitive for SC2009, so the OPTIMISTIC code is carried and its row, `optimistic-write-unaccounted`, stands beside the violation. Chrome raises only REACTIVE_WRITE_IN_OWNED_SCOPE for a signal's setter: the row is the cost of not knowing (slice 8 names SC2009's silences).

`solid-js.d.ts` is `local-alias-literal-slots`' (declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17, see there) with `createOptimistic` as `rc21-optimistic-before-load` declares it; `node_modules` holds the rc.21 package manifests.
