# ADR 0222: A returned source and a helper's prop read reach the caller

- Status: accepted and implemented (2026-10-07).
- Owners:
  - returned sources: `source_discovery` (project-function return
    resolution), and `completion_return_cover` beside the bounded call-cover
    walk in `solid-facts/src/ast/completion_call_cover.rs`;
  - helper prop reads: `local_access` (per-call rows of a component-local
    helper), with the zero-argument resolved-call demand in
    `solid-facts-backend/src/demand_plan.rs`.
- Relation: extends ADR 0201, 0204 and 0221 (reads proven to run during a
  call) and ADR 0216 (per-property prop backing). Each is queried, not
  replaced.

## Context

The second corpus-twin census (`fixtures/app-patterns-misuse/README.md`)
found two runtime-confirmed misuses with no finding at all. A silence there
reads as a clean result:

- `donegeon-returned-location-seed`: `const location = useLocation()`, where
  the hook returns a store it created. The checker did not carry the store
  through the `return`, so `location.search` in the body was not a store Get.
- `error-menu-apply-browse-helper-prop`: a prop read inside a component-local
  helper, `browse`, called in an effect's apply. Helper summaries carry
  signal and store reads to their calls, but never prop reads.

## Decision

1. **A project function every return of which yields one source binding
   returns that source.** The function must:
   - resolve exactly (same file, import, export alias or namespace member);
   - be current, synchronous, a non-generator, block-bodied, and not a
     method.

   Every normal completion must return the same unwritten `const` binding,
   which a dialect source primitive initialized in that frame. The
   completion cover refuses loops, `try`, `switch`, labels, fallthrough and
   implicit `undefined`. The root may not escape or alias. Each call creates
   a fresh source at its own binding, and identities never merge across
   calls.
2. **A component-local helper's prop read counts at each proven call.** The
   helper must be current and unwritten, and the call exact and plain. The
   read must run during the call (ADR 0201 and ADR 0221's
   `body_site_runs_during_call`, at every hop).
   - The row takes the call's execution role.
   - Its backing comes from the ADR 0216 index: reactive gives a violation,
     unknown gives uncertifiable, static gives nothing.
   - A call proven to be tracked or sampled adds nothing.
3. **A zero-argument call of a nested function declaration or `const`
   function is demanded a resolved-call fact.** The proof in 2 needs that
   fact, which was requested only for calls with arguments.
4. **An unproven use of a helper adds nothing new.** An escape, a handler, a
   JSX prop or an unproven call leaves the read as it was before this path:
   the definition-site row where the existing gate admitted it, as for a
   named control-flow child, and nothing where the gate refused it. The agent's draft
   added a definition-site obligation for every such helper: 1,351 new
   corpus obligations, nearly all derived getters
   (`const x = () => props.x`) and handler helpers. That is noise, not
   evidence, so it was removed.

5. **Only a file returning a bare identifier invalidates caches for
   returned sources.** The cross-file proof digest binds the bytes of every
   file that returns a bare identifier (`indexes::returns_bare_identifier`),
   and `interproc::call_role_syntax` counts that shape. The draft hashed
   every source file and counted every `return`, which disabled incremental
   reuse for nearly every edit. Three session tests caught it.

## Consequences

- Twins: of 38, 22 are proven, 16 uncertifiable and 0 silent (were 21, 15
  and 2). `donegeon` is proven. `error-menu` is uncertifiable because its
  caller passes a helper call whose backing is unknown. The base ledger is
  unchanged.
- rc.13 corpus: violations went from 222 to 231. Every added site was
  checked against its caller:
  - helper calls in an apply (`GroupRoute` `refresh`, `AccountLogin`
    `unavailable`);
  - a body call (`MessagesRoute` `streamUrl`);
  - hook-returned sources.

  Uncertifiable went from 3,447 to 3,479. The returned-store half alone
  removed 7 obligations, each a read in a hook's tracked `createMemo`
  compute, which is now known to be tracked.
- Not yet:
  - a helper used both at proven calls and through an unproven use reports
    only the proven calls;
  - a returned member path (`return state.items`) and conditional
    different-source returns stay unresolved;
  - transitive helper-in-helper prop reads are refused.

## Evidence

- Proposals by Codex agents: `rust/target/research/silent-returned-store/`
  and `rust/target/research/silent-helper-prop/round2/`. The helper
  proposal's round 1 lost an ADR 0204 proof and added noise. Round 2 found
  the missing demand. The fallback in decision 4 was removed during
  integration.
- Fixtures: `returned-store-source` and `helper-prop-read`. Both type-check
  against their byte-faithful rc.13 stub. Three delivered cases assigned to
  a function declaration (tsc TS2630) and were rewritten or removed.
