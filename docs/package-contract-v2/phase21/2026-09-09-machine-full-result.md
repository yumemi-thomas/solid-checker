# Full corpus with current machine settings

The full 418-probe run completed September 9 at 09:58 JST, exit 0, in
513.899 seconds (8 minutes 33.899 seconds). It used a fresh pinned release
checker, 8 generation slots, 20 certification slots, one inner artifact-analysis
slot per certification, the existing registry/install/materialized-source caches,
and the baseline's 900-second per-probe deadline. Projects and publication
artifacts were retained. No builds or tests competed with the benchmark.

The original recovery probe set was retained and the three scoped SolidStart
probes were added so their recent recoveries could be measured. The probe scope
itself stayed full and exactly matches the previous report's 418 identities.
Installed version records are unchanged across all rows.

| Certification outcome | Previous full run | This full run |
| --- | ---: | ---: |
| Complete entrypoint coverage | 327 | 327 |
| Partial entrypoint coverage | 64 | 64 |
| Certification refused | 18 | 18 |
| Did not advance | 9 | 9 |
| Accepted entrypoint occurrences | 1173 | 1185 |
| Accepted artifact cases | 1535 | 1549 |

There are no row transitions. Every previous artifact selection and closure
identity is preserved. Existing claim fields are also preserved: the only
non-identical prior claim set is Shared `./detect`, where `onSolidDevDetect`
and `onSolidDevtoolsDetect` gain closed-return claims. This is additive, not a
claim loss. Consumer authentication and exact-case selection checks pass for
the measured certified rows examined by the comparison.

| Probe | Artifact cases before → after | Newly covered entrypoints |
| --- | --- | --- |
| `@solid-devtools/shared@0.20.0 / solid1 / only` | 6 → 8 | `.`, `./index` |
| `@tanstack/solid-pacer@0.22.0 / solid1 / only` | 13 → 14 | `./types` |
| `@tanstack/solid-start@1.168.47 / solid1 / only` | 3 → 5 | `./client-only`, `./server-only` |
| `@tanstack/solid-start@2.0.0-rc.2 / solid2 / floor` | 3 → 5 | `./client-only`, `./server-only` |
| `@tanstack/solid-start@2.0.0-rc.2 / solid2 / head` | 3 → 5 | `./client-only`, `./server-only` |
| `solid-devtools@0.34.5 / solid1 / only` | 1 → 6 | `.`, `./babel`, `./setup`; root/setup include ordinary and browser import branches |

Devtools' browser/development root and setup are not established by the accepted
no-op browser branches. No assets/types were reclassified into certifications,
and no coverage metric or denominator changed.

No row reports a generation timeout or memory-limit termination, and no
certification attempt reports a memory-limit termination. The run used the
existing process-group cleanup and optimized-build configuration. A mid-run
sample observed roughly 618% aggregate process-tree CPU and 17.9 GiB RSS; a
later sample observed four native checkers, roughly 215% CPU and 10.4 GiB RSS.
These are samples, not peak measurements. They show the workload narrowing
to a small remaining set of checks, rather than being limited throughout to
two workers.

The prior full run took 1570.421 seconds. This run is 3.06 times faster, but
the ratio combines configuration, binary-profile, cache-state and code changes;
it does not isolate a concurrency-only speedup. No future runtime guarantee
is inferred from this run.

Evidence:

- Full report: `rust/target/ecosystem-investigations/2026-09-09-machine-full.json`
  and its adjacent Markdown report.
- [Exact before/after artifact selections, receipt roots and binary digests](2026-09-09-machine-full-measurement.json).
- [All prior claim sets and additive-change comparison](2026-09-09-machine-full-all-claim-preservation.json).
- Execution log: `/private/tmp/machine-full.log`; terminal exit status 0 was
  observed independently of the report's existence.
- Reproduction: `/private/tmp/run-machine-full.mjs`,
  `/private/tmp/measure-machine-full.mjs`,
  `/private/tmp/compare-machine-full-all-claims.mjs`, and
  `/private/tmp/check-machine-claim-additions.mjs`.

All comparison scripts completed successfully. No source changes, commits or
pushes were made for this measurement. Full `make verify` was not rerun: this
task ran the separate full ecosystem measurement with the already validated
source state and a fresh pinned release build.
