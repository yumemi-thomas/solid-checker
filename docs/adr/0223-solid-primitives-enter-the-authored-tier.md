# ADR 0223: Solid Primitives enter the authored tier, and a guard is evaluated at the call

- Status: accepted and implemented (2026-10-07).
- Owners:
  - authored specs: `pkg/contracts/authored/specs/@solid-primitives+*`;
  - the probe guard: `scripts/author-contracts.mjs`
    (`patchedDependenciesBlock`);
  - guarded owner requirements: `ContractOwnerRequirement::guard`, which
    `contracts::project_owner_requirements` carries and
    `owners::owner_requirement_at_call` evaluates.
- Relation: extends ADR 0198 (the authored tier) and ADR 0161 (the lower
  bound decides the finding kind).

## Context

Every `@solid-primitives/*` row in the rc.13 corpus sweep was `SC9005`: 43
rows over 39 packages. The 2026-09 certification still matches the installed
bytes, but it was probed on Solid runtimes older than rc.13, and ADR 0198
admits a claim only on its exact probed runtime. A research pass
(`rust/target/research/contracts/solid-primitives/`) found that only
`@solid-primitives/timer` has real consumer defects behind these rows. The
others carry knowledge that turns obligations into proven-clean results.

## Decision

1. **Nine specs ship, each claim probed in Chrome on rc.13:**
   - `timer` 1.4.5-next.1: a numeric delay registers `onCleanup` on the
     caller's owner, and an accessor delay creates an effect;
   - `event-listener` 3.0.0-next.3 and next.5: `createEventListener` creates
     an effect, for plain and accessor targets;
   - `resize-observer` 4.0.0-next.3: cleanup and effect registrations of
     three exports;
   - `media`, `scroll`, `memo`, `refs` and `raf`: a returned accessor or
     store.

   `utils.access` did not pass (its warning was not attributed to the case
   site), and `upload` hit a harness error. Neither ships.
2. **The probe guard reads only the `patchedDependencies` block.** It
   flagged any package named anywhere in a lockfile that has the block, which
   refused every `app-game` probe although only `drag-drop` is patched there.
3. **A guarded owner requirement keeps its guard and is evaluated at each
   call.**
   - An argument-kind atom is settled by the argument's normalized runtime
     kind: a primitive or nullish value is plain, and a function literal is
     callable.
   - A guard that is false at the call drops the requirement there.
   - An unsettled atom, or any other guard atom, leaves the registration
     possible, never guaranteed.

   Before this, a guarded operation counted as guaranteed on every call. The
   first sweep with these specs then reported, at a numeric `createTimer`
   after an `await`, that an effect was created with no owner. That was false:
   only an accessor delay creates one.

4. **The leaf-forbidden projection carries guards too.** A guarded
   registration is a forbidden operation inside a leaf owner only where its
   guard holds at the call. The message says the export registers "at this
   call", which is true whether or not a guard selects it.

## Consequences

- rc.13 corpus: four new proven violations, all `readingroom` timers with a
  numeric delay:
  - `settings/integrations.tsx:30`: `createTimer(…, 1500, setTimeout)` runs
    after an `await`, so its cleanup has no owner (`missing-owner`);
  - `queue.tsx:45`, `settings/metadata.tsx:174` and `books/[...id].tsx:469`:
    `createTimer` runs inside a component's `onSettled`, a leaf owner that
    forbids the cleanup (`leaf-owner-forbidden-call`).

  Each matches its Chrome-probed pair. Three `SC1001` obligations become
  proven clean.
- More `SC9005` rows (675 to 741). A package with no contract was one
  grouped notice; with a contract, each export and call site whose claims stay
  open is listed. That is more rows, not more uncertainty.
- Not yet:
  - no repository fixture pins the guarded projection, because the authored
    tier admits only an install whose files reproduce the package's
    identity;
  - 28 installed primitive versions cannot load on rc.13 (for example
    `virtual` imports the removed `solid-js/web`).
