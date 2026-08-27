# Solid compiler semantic-facts port ledger

Date: 2026-08-27

This ledger covers the compiler half of Phase 0A. It is an inventory of the
semantic work carried by `yumemi-thomas/dom-expressions#next`, not a cherry-pick
list.

## Identities

| Role | Exact revision |
| --- | --- |
| Solid upstream base | `solidjs/solid@a10cf1a1` (`next`) |
| New fork branch | `yumemi-thomas/solid:solid-checker/compiler-facts-v2` |
| Current checker compiler pin | `yumemi-thomas/dom-expressions@26e744fb` |
| DOM Expressions inventory head | `yumemi-thomas/dom-expressions@46fe53df` |
| DOM Expressions inventory merge-base | `95612b4a` |

The Solid fork is maintained only in `yumemi-thomas/solid`. No upstream Solid
pull request will be opened. A missing compiler behavior remains an open fact
limitation until it appears in a later upstream base.

## Rulings

| DOM revision | Subject | Ruling in Solid fork |
| --- | --- | --- |
| `58cdcfd3` | total semantic trace | Ported: trace v2 model, census, recording hooks, validation, host-independent option/result. Old compiler-specific placements were re-derived against Solid. |
| `46e516bf` | nested fragment census | Ported: census follows the current fragment lowering contexts. |
| `6b2345de` | folded event withdrawal | Ported: the recorder resolves the folded event site at the current attribute-plan decision. |
| `f1386807` | folded special attributes | Ported: folded ref/event/special attributes are resolved by censused kind. |
| `d1f0ed16` | fragment verification | Ported as Rust census/regression tests; old changeset was not copied. |
| `e109692b` | Oxc 0.144 source-map alignment | Obsolete as a patch: Solid already uses the current host-independent interface. Its source-map assertion was retained and expanded to corpus-wide neutrality. |
| `2bed6963` | ownership regions | Ported: owner establishments, component render sites, deferred callback sites, wrapper identity, and shared group IDs. |
| `f4473dd2` | stage-0 reconciliation | Ported selectively: total reconciliation, output baseline, events, refs, conditions, fragments, SSR/universal refusal, and mutation/canary tests. DOM documentation and JavaScript probe edits were not copied. |
| `6db8559d` | void-child census | Ported as discarded-range reconciliation and focused tests, without changing void-element lowering. |
| `eabc563d` | nested `children` promotion | Already present in the Solid compiler's current behavior. Only observations/tests were ported; the historical emitted-code change was excluded. |
| `0ae49137` | native `children` trace semantics | Adapted to current Solid behavior. Template-root and nested promotion are both traced; obsolete parity exclusions and generated-output changes were excluded. |
| `b7de2cff` | native `children` exclusions | Obsolete: exclusions described the former compiler. Current Solid cases are positive reconciliation tests. |
| `0ce01d74` | preserve nested `textContent` children | Historical compiler behavior change, later reverted from the trace-only branch and not ported. Current Solid source children win; the losing attribute is traced as elided. |
| `7353a8a8` | preserve nested custom-element owners | Historical compiler behavior change, later reverted and not ported. The trace observes only owner-context operations the current Solid compiler emits. |
| `ba580ff4` | restore output-neutral tracing | Governing ruling, preserved. It excludes the three behavior-changing series above and requires facts to follow actual output. |
| `59c56671` | shadowed JSX retraction | Ported: nested sites inside a shadowed JSX attribute value are withdrawn while the outer discarded value remains explicit. |
| `c7e83a1b` | root shadowed decision | Ported at the current attribute-plan decision. |
| `26e744fb` | discarded void child ranges | Ported: discarded void-element child ranges are terminally reconciled without changing output. |

The intervening pull-request merge commits and upstream-sync commits contain no
additional semantic ruling beyond the rows above.

## Solid-specific adaptations

- The fixture census uses `packages/babel-plugin/test`, the corpus location in
  the Solid monorepo.
- The adversarial probe corpus currently contains 266 cases; the old 494-case
  DOM Expressions count is not treated as an invariant.
- Current Solid promotes nested native `children` attributes. Tests assert that
  behavior instead of preserving old DOM Expressions exclusions.
- Static native lowering may record facts before discovering that a descendant
  requires dynamic fallback. The trace recorder therefore checkpoints and
  rolls back speculative observations. This changes no lowering data or branch.
- Unsupported non-DOM and import-bypassed modes refuse semantic trace production
  rather than returning a partial trace.

## Open facts

No known current corpus site remains unreconciled. This does not certify runtime
scheduling, cleanup, server transport, or package behavior; those domains remain
outside the compiler trace. Experimental or newly added compiler paths must fail
reconciliation until they receive an explicit terminal decision.

## Reviewed checker delta

The checker migration has one finding delta against the frozen pre-bootstrap
baseline: `body()` in `jsx-census-gap-solid-2` changes from an uncertifiable
SC1001 to silence. The former DOM Expressions producer omitted the child beside
a dynamic `textContent` attribute; current Solid lowers it as a reactive child
insert and the new trace reports that positive fact. Retaining the refusal would
require a compatibility shim that discards authoritative compiler evidence, so
the fixture was updated. The independent SC8003 authoring violation remains.
