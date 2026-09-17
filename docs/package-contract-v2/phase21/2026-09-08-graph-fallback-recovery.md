# Graph-fallback independent certification: scoped and full measurement

[ADR 0068](../../adr/0068-independent-generated-graph-fallback.md) was
implemented with a focused routing test but never measured against a real
package or verified. Both are done here, and the full corpus is the rerun the
[coverage ceiling plan](2026-09-07-coverage-ceiling-plan.md) asked to batch
after this workflow correction.

## Scoped: the package that motivated the change

`@kobalte/utils@0.9.2|solid1|only`, one probe, exit 0. Before the change the
same probe
[refused with zero accepted cases](2026-09-08-kobalte-utils-independent-measurement.json):
graph recovery failed on `./src/scroll-into-view.ts:scrollIntoViewport`, and the
fallback then retried the whole unaccepted generated proposal, which that same
case blocked again.

It now certifies **19 of 20 artifact cases**, recorded in the audit's
`graphPreparation.independentCaseRecovery` as 20 expected, 19 published, one
case refusal. The row moves **refused → partial** with 19 certified entrypoints
and the root still uncertified — the root's own refusal is unchanged and
unrelated (`accepted dependency @solid-primitives/keyed has no exact runtime
binding for export Key`).

The 19 published specifiers are exactly the pre-regression baseline's 20 minus
`./src/scroll-into-view.ts`:

```
array assertion create-generate-id create-global-listeners dom enums events
focus-manager focus-without-scrolling get-scroll-parent is-virtual-event noop
number platform polygon props run-after-transition styles tabbable
```

The twentieth remains refused, with the exact record preserved:

> `recursive-value-shape (artifact-case:4428aaa5…:scrollIntoViewport):`
> `parameter-rooted read lacks positive original-input identity`

That is an honest proof gap, not a scheduling one. `scrollIntoViewport` reads
`targetElement` inside the guarded branch and reassigns it in the other branch's
`while` loop, so no opening-declaration prefix reaches those reads.
[ADR 0069](../../adr/0069-original-input-by-position.md) is the premise for it
and is measured separately; nothing here depends on that work.

## Full corpus

`2026-09-08-graph-fallback-full-t600.json`, 418 probes, `--timeout 600`,
finished 2026-09-07T23:46:52Z, 752.583 seconds, exit 0. Checker
`sha256:17b08405…`, producer `sha256:6a1aff3e…`. The recovery lane is the prior
report's reviewed 47 `--recover-probe` ids plus
`@kobalte/utils@0.9.2|solid1|only`, added because the scoped run above measured
it.

| Metric | Baseline `initial-reads-full` | This run |
| --- | ---: | ---: |
| Complete rows | 318 | **324** |
| Partial rows | 61 | 61 |
| Refused rows | 30 | **23** |
| Not advanced | 9 | 9 |

The [per-row comparison](2026-09-08-graph-fallback-full-measurement.json) holds
status, certified-entrypoint count, root status and refusal stage for every
probe. **407 of 418 rows are identical and there is no certification loss.**
Seven rows gain:

| Probe(s) | Transition | Entrypoints |
| --- | --- | ---: |
| `@solid-primitives/marker@0.2.2`, Solid 1 | refused → complete | 0 → 1 |
| `@solid-primitives/marker@2.0.0-next.2`, Solid 2 floor/head | refused → complete | 0 → 1 each |
| `@solid-primitives/i18n@2.2.1`, Solid 1 | refused → complete | 0 → 1 |
| `@solid-primitives/i18n@3.0.0-next.4`, Solid 2 floor/head | refused → complete | 0 → 1 each |
| `@kobalte/utils@0.9.2`, Solid 1 | refused → partial | 0 → 19 |

Six of those are ADRs 0066 and 0067, whose recoveries had only ever been shown
in scoped runs; this is their first corpus-wide confirmation, and it closes six
of the thirty historical losses the previous full baseline recorded. The
seventh is this ADR.

Three further rows differ only in the digest naming their published dependency
graph — `@solid-primitives/intersection-observer@3.0.0-next.3` floor and head,
and `@tanstack/solid-db@0.2.40`. All three refuse before and after, at the same
stage, in the same family; the graph identity moves because its closure now
contains the recovered Marker and I18n catalogs. No coverage changed there.

## The timeout boundary, which is not a semantic result

`@kobalte/core@0.13.13|solid1|only` reports `infrastructure-failure` at stage
`timeout` in this run: its certification took **600.014 s** against the 600 s
budget. The same row in the baseline took **595.496 s** — 0.75 % of margin. The
run above was executed while this session was compiling the producer.

The scoped control settles it: the same probe alone, 1,200 s budget, quiet host,
exit 0, certifies in **378.394 s** with **508 certified entrypoints and the root**
— identical to the baseline. The full-run timeout was host contention, not a
certification change, and this row is the one to watch on any future full run.

Two operator notes belong with that. `run.mjs`'s own default is
`DEFAULT_TIMEOUT_SECONDS = 300`, while every Makefile target passes
`--timeout 600`; a full run invoked directly without the flag reports four rows
as timeouts and reads like a 509-entrypoint regression that never happened.
And the corpus's recovery lane is an explicit reviewed id list, so a newly
measured target does not enter it by itself.

## Validation

`bun test packages/cli/test/contract-workflow.test.mjs`: 84 pass, 0 fail,
including ADR 0068's routing test — the graph creates a directory and refuses,
independent trials retain exact claims, the selected union is freshly published,
and a previously existing publication is preserved without attempting a smaller
selection. Full `make verify` passes with actual exit 0, `TOTAL 116.44s`, and no
`FAILED during step` marker. No snapshots, bundled contracts, receipts or
protocol changed for this slice; no commit or push was made.
