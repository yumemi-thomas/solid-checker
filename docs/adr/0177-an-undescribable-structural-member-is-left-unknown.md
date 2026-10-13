# ADR 0177: An undescribable structural member is left unknown

- Status: accepted and implemented (2026-10-04). The owner chose "describe the
  parts we can prove" as the first package-misuse lever on 2026-10-04.
- Owners: the structural return census (`structural_returns::return_sites`,
  `member`, `covers`, `weakening_refusal`), the operation withholding
  (`withheld_operation_weakening`,
  `ExportSemantics::weaken_return_members`), the synthesized structural veto
  (`any` members), and the consumer projection (`project_return_shape`).
- Relation: amends ADR 0172 ("a failed construction proof withdraws its
  entire return operation"). It takes the census and veto halves of the parked
  ADR 0157 work (`wip/adr0157-cross-package-returns`), without premises. No new
  wire field: `"unknown"` was already a value in the stable language.

## Context

The misuse ledger had 29 runtime-detected strict reads the checker did not
report, each one a read of something a package returned. For none of them did
the accepted contract state a returned accessor. In 19 the generator proposed
the return and the census refused it, and ADR 0172 made that refusal
all-or-nothing:

- `createRAF` returns `[running, start, stop]`. `running` is an owned signal
  accessor. `start` reads `running` but is a plain closure, so the proposed
  "accessor" member had no owned-accessor evidence, and `stop` was already
  `unknown`. The census has no `unknown` case at all ("unsupported structural
  member claim"), so the whole return was withdrawn.
- `createDate`, `createPagination`, `makePersisted` and `createClipboard`
  each had one leaf with "no exact primitive, caller parameter or owned
  accessor evidence".

A strict-read finding needs one positive fact about the member the user reads,
not a description of every member. Your apps import `createRAF` 38 times.

## Decision

1. **`unknown` is a structural member claim.** It asserts nothing about the
   member, so any construction covers it (`covers`). It still counts towards
   the container's exact length or key set, in the census and in the veto
   (`{"kind": "any"}`). It is never the root of a structural claim.
2. **The census names the members it cannot describe.** A leaf whose own
   proof fails becomes `unknown` in the produced tree, with its path and
   reason (`member`). A container that contradicts itself still refuses.
3. **Weakening is decided over every completion at once**
   (`weakening_refusal`). It applies only when there is exactly one claimed
   structure, and only when the claim differs from the produced trees solely
   at members the census left unknown. The united weakened claim must cover
   every live completion and must still describe at least one member.
   Otherwise the plain refusal stands and the return is withdrawn as before,
   so a weakening never trades one refusal for a later one.
4. **The withholding rewrites those members in place.** The refusal carries
   the member paths after `STRUCTURAL_MEMBERS_UNKNOWN_MARKER`, and its record
   takes `WITHHELD_OPERATION_MEMBERS_UNKNOWN_PREFIX`. An operation whose every
   record is such a weakening keeps its return, with those members set to
   `unknown`. One other refusal withdraws it. The census then re-confirms the
   weakened claim member for member. Claims at a path below a weakened member
   disappear with it, because they are read off the shape. The graph lane
   re-derives the same weakening through the same function.
5. **A listed member of an open enumeration is projected.** The consumer
   projection reads a tuple or object whose `items`/`properties` enumeration
   is not closed, member by member. Each listed member is proven at its
   position or key. "Partial" only says more members may exist. An unlisted
   position names no leaf. The projection keeps reactive leaves only, so this
   can surface a proven accessor and never invents one.

## Consequences

- A proposed member the census cannot prove no longer costs its siblings.
  `start` in `createRAF` is left `unknown` rather than claimed an accessor.
- A return whose every member would be unknown is still withdrawn: an
  all-unknown container states nothing a consumer can use.
- `unknown` members name no reactive leaf for consumers, exactly as plain
  members did. Calling one reports nothing; it can hide a finding, never add
  one.

## Evidence

- Unit tests:
  - structural census: `covers`, `unknown_members`, the all-completions rule
    and the member-path round trip;
  - certification: `an_undescribable_structural_member_is_weakened_and_its_siblings_kept`,
    where `[value, other]` certifies as `[parameter 0, unknown]`;
  - veto: `the_structural_return_veto_matches_an_undescribed_member_by_anything`;
  - model: `a_structural_return_member_is_weakened_in_place`.
- Fixture `fixtures/reactive-ir/partial-structural-return-consumer`. A read of
  the listed accessor at a component's top level is a violation, for a tuple
  and for an object. The same read in JSX, a call of the `unknown` member, and
  an unlisted index all stay clean.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 60 to 63 static violations of 123:
    `createDate`, `createRAF` and its `default` alias;
  - each new violation is runtime-detected (`STRICT_READ_UNTRACKED`);
  - no correct twin is flagged;
  - no checkpoint row changes certification status.

  A first run without the all-completions rule refused
  `@solid-primitives/storage` in the graph lane: a second pass met a plain
  mismatch at a stage that does not withhold.
- Coverage: 161 fixtures, 831 findings. Only the new fixture's two
  violations are added.
- Contract corpus: 122 fixtures pass unchanged.
