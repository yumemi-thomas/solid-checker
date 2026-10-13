# The corpus wall, decomposed and moved (2026-09-14)

The pinned full-corpus run (`benchmarks/ecosystem/report.json`, 629d2c52) is
1,316 s of wall for 418 rows whose median row takes 1.8 s. Four rows take
about 1,200 s each — `solid-js@1.9.14`, `corvu@0.7.2`, `@kobalte/utils@0.9.2`,
`@solidjs/web@2.0.0-rc.3` — and `witnessAcquisition` is over 98% of each. The
runner already schedules heaviest-first, so the wall is the serial chain
inside one row, not the queue. This note records where that chain's time went,
what moved it, and what did not move.

## Method

`SOLID_CHECKER_TIMINGS=1` on a single-row run of the ecosystem runner
(`--package solid-js --solid solid1 --timeout 1800 --attempt-certification`,
release binary, host otherwise idle). The runner's default `--timeout 300`
also bounds certification, so a single-row measurement of a recovery row needs
the Makefile's 1800. An isolated row runs about four times faster than the same
row under the loaded corpus (308 s here against the pin's 1,245 s); the
structure is what transfers, not the wall.

## Where the solid-js row's 308 s went

| Unit | Count | Time |
| --- | ---: | ---: |
| serial native transactions (`--execute-contract-certification`) | 36 | the whole row |
| Type Facts acquisitions (producer launches) | 292 | 81 s + 18 s launch |
| probe-gate batches / sessions | 292 / 1,606 | 221 s |
| watched-input censuses | 2,190 | 123 s of the 221 |
| census CPU on the pinned Node executable alone | 2,190 | 122 s |
| census CPU on the verifier and Type Facts images | 2,190 | 59 s |
| census CPU on the copied recipe corpus (`recipe-modules`) | 2,190 | 31 s |

Two multipliers, one on top of the other:

1. **The recovery search was a growing prefix.** The verified-retained-floor
   path and independent recovery for sets of 32 cases and under certified
   `[...selected, cases[index]]` once per case (`certifyIndependentCaseSelection`,
   `certifyRetainedProposalSelection`). Each of those is a whole native
   transaction that rebuilds pin checks, Type Facts sessions for every case in
   the prefix and every probe gate. solid-js: 33 floor transactions plus 3
   graph ones; `@solidjs/web`: 26 plus 3; `@kobalte/utils`: 14;
   `@tanstack/solid-table`: 9. That is why a 24-candidate `@solidjs/web` row
   cost as much as the 3,252-candidate `corvu` row.
2. **Every census re-hashed 170 MB of immutable images.** Between every
   session `watch_digests` re-read the pinned Node executable (117 MB), the
   verifier image and the Type Facts image; `shared_from_pin` re-hashed the
   producer image before each of the 292 launches. 80% of census CPU, and the
   census was 55% of batch time.

## What moved

- Subdivision replaces the prefix on all three selection paths, starting at
  the two halves of the set that just refused, with trials of one selection
  running side by side up to `SOLID_CHECKER_RECOVERY_TRIAL_CONCURRENCY`
  (default 1; the runner sets 4 for certification children). ADR 0059,
  amended.
- Pinned images are hashed once per process and re-asserted by inode
  fingerprint (device, inode, size, mtime, ctime) on every later census and
  pin check. ADR 0102 states what that gives up.
- A workspace copies only the recipes its batch schedules, not the whole
  276-module corpus; the corpus is still loaded and identified whole.
- The `private-dependency:*` census digests derive from the single
  `private-node-modules` walk (the derived digest is byte-for-byte `hash_tree`
  of the copy) instead of reading each dependency tree twice.
- Certification children get `SOLID_CHECKER_CERTIFICATION_PARALLELISM`, their
  share of the cores, so twenty children no longer each fan census labels and
  graph batches out to all fourteen.
- Each native transaction writes its own request file, which concurrent trials
  of one scratch directory need.

## Measured, same row, same host, release binary

| | before | after |
| --- | ---: | ---: |
| row wall | 308 s | 46 s |
| `witnessAcquisition` | 302 s | 39 s |
| native transactions in the retained floor | 33 | 14 |
| Type Facts acquisitions | 292 | 44 |
| probe-gate batches / sessions | 292 / 1,606 | 63 / 510 |
| censuses / census time | 2,190 / 123 s | 636 / 31 s |
| census CPU on the three pinned images | 181 s | 0 s (fingerprint hits) |

Outcome identical: 129 certified closures across 61 cases with the same
per-domain split (returns 65, callbacks 64, creates 16, reads 56), the same 39
published recovery cases, the same two refusals (`./dist/dev.js`,
`./dist/solid.js`), the same 64 `censusRefused` withholdings with reason texts
identical modulo the private-project temp path.

## Measured, full corpus (`make ecosystem-regression` against the 1,316 s pin)

| | pinned run | this binary |
| --- | ---: | ---: |
| wall | 1,316 s | 829 s |
| summed row time | 15,462 s | 9,359 s |
| complete / partial / certified rows | 349 / 32 / 381 | 349 / 32 / 381 |
| heaviest row | `solid-js@1.9.14`, 1,246 s | `corvu@0.7.2`, 829 s |
| `solid-js@1.9.14` / `@solidjs/web@2.0.0-rc.3` | 1,246 s / 1,192 s | 322 s / 341 s |

Every one of the 418 rows agrees with the pin on class, certification status,
certified-closure count, withheld count, published recovery cases and
coverage. Host on mains (`pmset -g` powermode 0), warm registry cache.

The first regression run did **not** agree: `@tanstack/solid-table@9.1.2` lost
`./experimental-worker-plugin [import, solid]` and `solid-devtools@0.34.5` lost
two `./setup` variants, each refused for a graph node the lost case does not
depend on. The graph-lane request file was named by its first case, every
trial of a retained-floor selection starts with the same base case, and the
trials now run side by side — so one verifier read another's planning. Both
native lanes now number their request files per execution; the second run is
the table above. A refusal that names another case's node is the signature to
look for if this ever recurs.

## Second pass: the graph row

With the recovery rows gone the wall was `corvu@0.7.2` (829 s in the corpus,
276 s alone): 616 graph nodes, 343 gate batches in one pass, 2,556 sessions,
3,050 censuses. Three things were wrong in it, found in order.

1. **The new parallelism cap serialized the tail.**
   `SOLID_CHECKER_CERTIFICATION_PARALLELISM` was set to the child's core share,
   which is 1 with fourteen children on fourteen cores, so pass 10 ran its 343
   batches on one worker: 165 s. The runner no longer sets it (the knob stays
   for hosts that want it). Uncapped, the gate wall fell to 107 s — but summed
   batch time rose from 163 s to 1,332 s, because fourteen walkers re-reading
   their private trees between every session are bound by the kernel's
   metadata path, not by cores.
2. **Every gating pass re-loaded the recipe corpus once per node**, reading
   and hashing all 276 modules each time (616 × 12 loads), sequentially. The
   corpus manifest and modules now go through the fingerprint memo and the
   per-node re-gating runs on the bounded pool.
3. **The private-tree census read every byte between every session.** ADR
   0102's argument holds for the private copies too (an unprivileged writer
   cannot move a file's ctime back or keep its inode across a replacement), so
   `collect_tree` now takes each file's digest by fingerprint from the walk's
   own `lstat`, hashing only what moved. Summed census time 901 s → 401 s.

| corvu@0.7.2 alone | capped (as shipped in pass one) | uncapped | + corpus memo, fingerprint census |
| --- | ---: | ---: | ---: |
| row wall | 276 s | 220 s | 220 s |
| pass-10 gate wall | 165 s (1 worker) | 107 s | 95 s |
| summed census time | 131 s | 801 s | 401 s |
| summed workspace materialization | 3 s | 236 s | 320 s |

Same 1,342 certified closures and 964 withheld in every run. Full corpus
(`make ecosystem-regression`): wall 829 s → 647 s, summed row time 9,359 s →
8,171 s, all 418 rows again identical to the pin on class, status, certified
closures, withheld count, published recovery cases and coverage; the heaviest
rows are now `corvu@0.7.2` (647 s) and `@kobalte/utils@0.9.2` (512 s). What is
left of the gate wall is metadata churn: each batch still writes the package and its
dependency closure into a fresh private tree and removes it (247 trees of
thousands of files, 320 s summed under fourteen-way contention). Reusing a
census-verified workspace across the batches that share a snapshot closure is
the next lever, and the invasive one.

## What did not move, and why

- **The wall is now the `corvu@0.7.2` graph row (829 s), and the graph rows
  moved only about a third.** Their cost is passes × batches × (workspace copy
  + sessions + census of the private tree), and the two items below are what
  is left of it.
- **Graph rows re-certify shared dependency nodes per row.** Each row owns its
  `.accepted-catalog`, so every corvu-family row certifies `solid-js@1.9.14`'s
  61 cases and `@corvu/utils`'s nodes again; `corvu@0.7.2` alone certifies 616
  cases. A run-shared dependency catalog consumed through
  `--accepted-contracts` would remove most of that. It changes which receipts a
  row's publication depends on and therefore belongs in an ADR, not here.
- **Workspace materialization** writes the package and its dependency closure
  from the in-memory snapshot per batch and removes it again. Reusing a
  census-verified copy across the batches that share a closure is the obvious
  next step; `clonefile` does not apply because the source is not a directory.
