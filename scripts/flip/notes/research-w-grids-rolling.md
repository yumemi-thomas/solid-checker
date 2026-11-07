# Rolling recall grids: resumed at 72e9b2ff6

Tip: `72e9b2ff6b26c15b8c8f905a15f2c9a69958454b` (origin/claude/flip-census, fetched 2026-11-08 05:00 JST).
Wall time: in progress; resumed runner started 2026-11-08 05:02:56 JST.
Total programs: pending; this is a startup report, not a clean result.
New false positives: not yet measured.
New certified false cleans: not yet measured; ids and uncached Chrome confirmations follow when the grids finish.

The runner uses this exact tip's generators, judge (`rowProblem`), and row ledger. The runaway fix e40429f5e is an ancestor; `composite_callee_walk` contains `if !seen.insert(span)` at types.rs:2197. Release checker SHA-256: `c3c1c1c23856f77126dac21c626c39dc4956e4aeb8bdfab1e4d041d9e85ef423`. Producer SHA-256: `998ad2d92a37eb74af8ff266daca23555dc3c1760a818d1085288347c248d6c9`.

Serial units take the existing shared heavy lock, yield to other waiting work, and leave a 90-second gap. Processes inherit nice 10. Each unit uses 12 workers, 12 checker concurrency and 8 Chrome pages, with one retry at 6/6/4 after a memory trip. The guard is 20,480 MB of VM used memory; a checker or typefacts process above 12 GiB RSS is killed only within the run's process group. No VM configuration changes were made. Host load above 30 delays the next lock acquisition.

## Correction to the prior pass

The 77d80eefd runner reached its `tip done` marker, but its grids were incomplete: 23 of 71 requested units produced rows (7 mx codes, 6 lg codes; 518,225 programs). The remaining units stopped at disk preflight: one log records 477,597 of 9,830,400 inodes free. At this resume the shared VM data disk had only 154,674 free inodes (99% used). This is an infrastructure blocker, not a checker result. That pass is excluded from full-grid comparisons; the last complete baseline remains 4c81fcc00.

No shared cache or other agent's files were deleted. The new run's output and new cache are in this agent's host-backed directories (`~/EDF/tmp/flipw/rolling/vm-runs/72e9b2ff6` and `~/EDF/tmp/flipw/rolling/cache`), which have ample free space and inodes. Program scratch remains on this agent's existing tmpfs. The runner now rejects failed unit statuses and missing rows before declaring completion. It will confirm every off-ledger problem, plus all false positives and certified false cleans even if listed, using uncached Chrome.

Validation so far: shell syntax checks for the runner/build/publisher and Node syntax checks for the analysis, confirmation-manifest and report helpers passed. Product verification and full-grid verdicts are pending; no product code changed. Remaining uncertifiable cases will be counted by the tip's judge in the completed report.
