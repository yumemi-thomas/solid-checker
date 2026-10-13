# What holds an import open, after ADR 0116, read strictly

- Measured 2026-09-24 with
  [`2026-09-23-what-holds-an-import-open.mjs`](2026-09-23-what-holds-an-import-open.mjs)
  as made strict for [ways-to-improve § 3.1 (a) and (b)](2026-09-24-ways-to-improve.md),
  over the 0ec743c4 census run (finished 2026-09-24T03:13:24Z). Its retained
  `$TMPDIR/solid-checker-ecosystem-out-*` directories and `run.json` were copied
  before anything else ran, and the script read the copy (`--outputs`). The
  per-export evidence is
  [`2026-09-24-what-holds-an-import-open-after-0116-strict.json`](2026-09-24-what-holds-an-import-open-after-0116-strict.json).
  It supersedes the statuses, not the sites, of
  [`…-after-0116.json`](2026-09-24-what-holds-an-import-open-after-0116.json).
- Every number here is **measured** from those two files unless marked
  otherwise.
- What changed in the script: a status comes only from the artifact cases that
  gave the answer. On the plain lane that is the cases the proposal lists for
  the answering (nameable) entrypoint, with no `?? withheld[0]` fallback for
  withheld closures or withheld operations. On the graph lane
  (`lane: published-graph`) it is the audit records carrying the answering
  node's digest (or, for a case-set root, its artifact case id), never the
  plain lane's proposal, refusals or generated document, which describe the
  root before its dependency was accepted.

## The consumer view did not move

73 exports, 733 sites, 1,879 open domain-sites, and every export's view are
identical to the after-0116 file. Only the reason attached to an open domain
changed. No entrypoint with two nameable cases (`@kobalte/utils` `.` under
`import` and `solid`) had cases disagreeing on a status.

## Domain-sites by status

| status | `returns` | `callbacks` | `creates` | `reads` |
| --- | ---: | ---: | ---: | ---: |
| never proposed | 287 | 104 | 137 | 4 |
| never proposed on the graph lane | 50 | 50 | 16 | 1 |
| withheld: census refused | 0 | 511 | **97** (was 257) | 80 |
| withheld operation: census refused | **182** (was 67) | 0 | 0 | 0 |
| withheld: veto did not complete | **0** (was 112) | 0 | **0** (was 8) | 0 |
| withheld: no recipe in corpus | 0 | 0 | 0 | 25 |
| declined: unresolved-callee | 0 | 0 | **172** (was 10) | 0 |
| declined: unaccepted-external-dependency | **18** (was 71) | **18** (was 68) | **16** (was 25) | **18** (was 19) |
| declined: dialect-silent | 0 | 0 | 26 (was 33) | 0 |
| declined: refusing-callee-fixpoint | 0 | 0 | 8 (was 2) | 0 |
| proposed, not certified | 26 | 32 | 0 | 1 |

## The two predictions

**(a) `@kobalte/utils` `callHandler` (112 sites). Confirmed, with one
refinement.** Its `returns` is now a *withheld operation* in both dist cases
(`6d69dd5a`, `9dd8dc77`): "primitive returns census refuses an implementation
… whose completion the producer did not prove primitive". Its `creates` is not
proposed in either dist case, and the generator recorded why: a
`closure-proposal` decline `unresolved-callee`, shape `computed-member`,
spelling `handler`, at `dist/index.js:186:215` (`handler[0](handler[1],
event)`), under both conditions. The report predicted "never proposed"; the
more exact status is "declined at generation on the item-B callee", which is
the report's § 3.3 reading of why the walk does not propose. The 112-site
"veto did not complete" row and the 8-site `creates` one are gone: every one
came from the `./src/*.ts` cases no consumer names. `composeEventHandlers`
(48) moves the same way for `creates`; `snapValueToStep` (6) and
`getPrecision` (2) move from "veto did not complete" to
`refusing-callee-fixpoint` and `unresolved-callee`.

**(b) The 113 `unaccepted-external-dependency` domain-sites on `rootless`,
`trigger`, `memo` and `storage` (53 sites, 9 exports). Confirmed for
`callbacks` and `returns`; refuted for `creates`.** Every one of the nine
answers comes from a graph node, and on the graph lane:

| domain | domain-sites | status now |
| --- | ---: | --- |
| `callbacks` | 50 | never proposed on the graph lane |
| `returns` | 50 | never proposed on the graph lane |
| `returns` | 3 | withheld operation (`createSubRoot`'s `return`) |
| `creates` | 9 | never proposed on the graph lane (`TriggerCache` 6, `createSharedRoot` 2, `makePersisted` 1) |
| `reads` | 1 | never proposed on the graph lane (`makePersisted`) |

The report expected those nine `creates` domain-sites to be the dialect-callee
refusal. They are not: that refusal (`getOwner` or `onCleanup` declared in
`@solidjs/signals/dist/types/…`) is on `createHydratableSingletonRoot` (27),
`createSingletonRoot` (7), `createMicrotask` (20), `debounce` (4) and
`throttle` (1), **59 domain-sites, as § 3.4 sized it**, and the first two were
already labelled correctly in the after-0116 file. The graph lane proposed
`creates` on those two and the census refused it; it proposed nothing for the
nine.

**The graph lane shows no `callbacks` or `returns` candidate the plain lane
lacks** for these exports, so § 3.4 is not worth more than ranked. The ranking
of § 4 stands.

## What "never proposed on the graph lane" rests on

The graph lane regenerates each node's proposal in private scratch, and the
row retains neither its unresolved claims nor its declines. So the status is
an inference: no withheld closure and no withheld operation for that node and
export, and the domain is not closed. It is sound only if every graph-lane
candidate is accounted for. The audit's candidate list is truncated at 64
(`closureCandidatesFromNativeOutput`), but its count is not, and it
reconciles:

| row | candidates | closed + withheld + withheld operations |
| --- | ---: | --- |
| `memo` floor, head | 145 | 75 + 60 + 10 = 145 |
| `rootless` floor, head | 148 | 77 + 59 + 12 = 148 |
| `trigger` floor, head | 141 | 76 + 56 + 9 = 141 |
| `storage` floor, head | 139 | 78 + 52 + 10 = **140** |

`storage` over-counts by one, so one lost candidate there cannot be excluded
by counting; the JSON detail says so on `makePersisted`. "Never proposed on
the graph lane" also includes a graph-lane *decline*: the lane keeps none, so
a `dialect-silent` decline there would read the same. The plain lane's
`dialect-silent` on `createSubRoot`, `createLazyMemo` and `createWritableMemo`
`creates` is about a different artifact case and is no longer reported.

## Remaining measurement gaps

- The graph lane's declines and unresolved claims are not retained. Keeping
  the regenerated root's `.refusals.json` beside the plain one would make the
  status direct instead of inferred.
- The two `@kobalte/utils` `.` cases cannot be told apart from the document
  (their artifact descriptors are identical), so the script reports per
  entrypoint and lists both cases; they agree everywhere today.
- A withdrawn operation's domain on a graph node is read from the generator's
  operation-id scheme (`return…`, `callback-N`, `read-N`,
  `inferred_contract.rs`), since no generated document for a graph node is
  kept. All 82 graph-node withdrawal records in the run are `return`.
