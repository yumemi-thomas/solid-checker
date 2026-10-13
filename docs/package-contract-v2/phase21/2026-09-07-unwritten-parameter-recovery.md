# Unwritten parameter recovery measurement

The bounded protocol 39 source-identity rule in [ADR 0056](../../adr/0056-unwritten-parameter-input-identity.md)
removes the measured `store.shallow` input-identity blocker. It does not clear
Table's absent generic member or add module-evaluation/virtual-module authority.

## Scoped measurement

`rust/target/ecosystem-investigations/2026-09-07-unwritten-parameter-recovery.json`
uses the fresh pinned checker from full verification. Both native certification
and ordinary analysis report authenticated receipts and exact case selection.
Compared with the independent-recovery full checkpoint:

- Pacer retains `./provider` and `./utils`, adding `.`, `./async-batcher`,
  `./async-debouncer`, `./async-queuer`, `./async-rate-limiter`,
  `./async-throttler`, `./batcher`, `./debouncer`, `./queuer`, `./rate-limiter`
  and `./throttler`. All use `[import]`: 2 to 13 artifact cases and entrypoints.
- Table retains `./flex-render` under `[import]` and `[import, solid]`, adding
  `.` and `./experimental-worker-plugin` under each condition set: 2 to 6
  artifact cases, 1 to 3 entrypoints.

This is 15 additional artifact cases and 13 additional entrypoint names across
the two rows. Both remain partial under the unchanged metric. Pacer's declared
denominator is 15 (missing `./types` and `./package.json`); Table's is 5
(missing `./static-functions` and `./package.json`). No inapplicability
reclassification is credited as certification.

[Exact scoped evidence](2026-09-07-unwritten-parameter-scoped-measurement.json)
follows each published case-set pointer and digest-bound catalog reference.
It records every before/after artifact and declaration hash, branch, condition,
importer, receipt payload, producer and checker identity. Every previous
selection is present in the new set. The receipt binds artifact provenance,
resolution, producer sessions, positive witnesses, dependency receipts and
dependency trust; trial receipts are not substituted into the final context.

The newly satisfied leaf premise names an affirmative unwritten parameter
declaration and a reachable direct property use in authenticated `shallow`
runtime bytes. The verifier's recursive-value witness commits those locations;
the final graph's accepted-dependency composition and dependency receipt roots
bind the separately certified leaf into each dependent case. Other operation
and behavior proofs remain required.

## Remaining proof obligations

Table's two `./static-functions` cases still refuse `cell_getIsAggregated` at
`column.table.atoms.grouping.get`, with exact demand IDs retained in the evidence.
The generic declaration does not establish this callable member. Pacer's
`./types` preparation explicitly refuses a dependency module with no runtime
ESM exports. Neither is silently dropped from the coverage denominator.

Router's graph cycle/runtime policy and export-shape blockers, SSE's worker
module-evaluation subject, and SolidStart's authenticated application virtual
input remain as described in the independent recovery investigation. No broad
proof models or speculative coverage estimates are reported as implemented.

## Validation

The producer's 11-case source-identity regression passed. Native tests cover
exact slot/declaration agreement, absent or duplicate premises, rest/default,
capture/alias, reachability, foreign locations, async, nested paths and asserted
callability. A packed published-archive test certifies an unwritten generic
read input and refuses the same proposed input after reassignment.

Full `make verify` exited 0 with `TOTAL 159.59s` and no `FAILED during step`
marker. It includes producer race tests, protocol validation, the live native
regression and 183 CLI tests. Initial verification caught a test-only dependency
mistake and stale schema digest constants; both were corrected before this pass.
No snapshots were updated for this proof slice. No commits or pushes were made.

The first full protocol 39 corpus (`2026-09-07-unwritten-parameter-full.json`,
458.091 seconds) is diagnostic: it exposed new binding rows whose selected
signature/demand referred to a different source declaration. The consumer
correctly rejected these rows, regressing accepted packages including Motion.
This run does not establish preservation. The producer was corrected to emit
only exact selected-signature, same-file bindings; a cross-file implementation
test proves the optional premise is withheld without invalidating the existing
transcript. The corrected source passed full `make verify`, exit 0,
`TOTAL 127.08s`, with no `FAILED during step` marker; log:
`/private/tmp/unwritten-parameter-bound-verify.log`. Final corpus results must
supersede the diagnostic run before preservation is claimed for the corpus.

## Final full-corpus result

`2026-09-07-unwritten-parameter-bound-full.json` finished at
2026-09-07 18:49:42 JST, exit 0, in 635.072 seconds. Its published catalogs and
ordinary consumer verification establish **321 complete, 51 partial, 26 refused,
20 not advanced** across the same 418 probes and installed versions.

Relative to the independent-recovery checkpoint, all **746 accepted artifact
selections across 368 certified rows** are preserved, including multiplicity,
runtime/declaration hashes and resolution branches. Motion floor/head retain
all three entrypoints. The final run adds **19 artifact cases**: the 15 scoped
Table/Pacer cases and four newly certified roots:

| Probe | Before | After | Binding that unlocks its dependency |
| --- | --- | --- | --- |
| `@solidjs/element@2.0.0-rc.3` Solid 2 | refused | complete, `{.}` | `component-register@0.8.8 hot` root read input |
| `@tanstack/solid-form@2.0.0-alpha.2` | refused | partial, `{.}` | `@tanstack/store@0.11.1 shallow` root read input |
| `@tanstack/solid-hotkeys@0.10.0` | refused | partial, `{.}` | same exact Store export |
| `@tanstack/solid-store@0.11.1` | refused | partial, `{.}` | same exact Store export |

Each dependent case is freshly certified in its own resolution/trust context;
“same export” does not mean copied receipts. Element's manifest has no exports
map and its certified legacy root satisfies the **existing** completeness rule.
No new denominator rule was introduced.

[Final slice evidence](2026-09-07-unwritten-parameter-bound-full-measurement.json)
records exact case selections, published pointers, receipts, binary identities,
row transitions and the preservation check for every previously certified row.

Across the complete bounded recovery effort, compared with the reconciled
`2026-09-07-union-full.json` baseline, counts move **317/51/30/20 →
321/51/26/20** (complete/partial/refused/not advanced). All **737** baseline
artifact cases remain accepted; **28 artifact cases / 26 entrypoint occurrences**
are newly certified. The four complete-row gains are Motion floor/head,
`@solid-primitives/utils@6.4.1`, and Element. The three refused-to-partial gains
are Form, Hotkeys and Store. SSE, Pacer and Table gain cases while remaining
partial. [Cumulative exact evidence](2026-09-07-entrypoint-recovery-final-measurement.json)
contains each before/after case set and importer-bound receipt evidence.

The scope distinguishes new certification from metric correction: all gains
above are accepted cases; metric corrections are zero. Conditional Table
members, SSE module evaluation, Router's graph/runtime/shape obligations and
SolidStart's missing application context remain unresolved, with no estimated
gain counted. The diagnostic regressing corpus is superseded, not merged into
this result.
