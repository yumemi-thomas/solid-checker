# Integrated primitives checkpoint, 2026-09-30

Source: `6208b85f`, including the consumer accessor fix (`5381c7c8`), exact
dependency restatement (`3eb6417a`, ADR 0170), returned-accessor witness
(`ba751770`, ADR 0162) and pinned measurement-runtime selection. Nothing pushed.
The pinned corpus remains 97 published packages and 721 exports.

## Measured result

| Metric | Result |
| --- | ---: |
| Full package checkpoint | 1/97 |
| Criterion 1: certified per host and delivered for the required entrypoints/runtime | 83/97 |
| Distinct primitives package names in the tier | 89 |
| Criterion 2: every export accounted for | 6/97 |
| Criterion 3: every export's misuse covered | 4/97 |
| Clean exports, none / browser / node | 101 / 101 / 131 of 721 |
| Exports accounted for, including published defects | 134/721 |
| Misuse ledger cases passing in every named host | 2/123 |
| Twins with TypeScript errors against published typings | 0/246 |

The supplied clean-export baseline was 100 / 100 / 130. `sse:number` is the
one-export gain per host. The distinct-name tier count is not criterion 1:
the latter checks the required entrypoints and runtime. Nine certified package
runs still lack required tier entries: analytics's relay entrypoints,
storage's tauri entrypoint, and devices, filesystem, input-mask, keyed,
sensors, share and transition-group. Devices, input-mask, sensors and
transition-group have bundles only for another runtime.

Five published defects remain refused: animation omits its runtime target;
controlled-props, drag-drop, favicon and virtual import `solid-js/web`, which
the pinned runtime does not export. Their 33 measured exports are accounted
for as uncertifiable, never clean. Animation has no measurable runtime surface.

The two passing ledger cases are `utils.access(count)` (one SC1001
uncertifiable finding; possible invocation only) and `utils.createMicrotask`
(SC4001 violation). Their correct twins stay clean. Host-free verdicts for the
other cases are 30 correct twins not clean, 22 expected rules only
uncertifiable and 69 wrong findings. There are 358 exports with a misuse path
and no case, 81 with a case that does not report correctly and two reporting.

The returned-accessor witness is implemented, but no `owned-memo` export is
carried in this tier. The older branch's 19-export return-closure gain preceded
the exact archive, options and spread restrictions and is not delivered
coverage. `solid-js`'s actual hydration/server factory behavior needs a
separate audit; the signals factory's witness cannot authorize it.

## Reproduction and verification

The lead ran the required sequence without clearing retained trees:
`make contract-coverage-census`, `make consumer-environment-runs`,
`make primitives-checkpoint PRIMITIVES_CHECKPOINT_KEEP=1`, and
`make accepted-bundles PRIMITIVES_CHECKPOINT_KEEP=1`. Then the release checker
was rebuilt with the generated tier, the retained host runs were remeasured,
and the complete misuse ledger was rerun against that build. The reports are
under `rust/target/primitives-checkpoint/`; the runs took 214 / 280 / 370 s.

All 30 census probes certified per host and both census comparisons passed
without repinning. All nine consumer-environment runs completed with the
existing published/runtime-policy refusals preserved. Bundling produced 1,480
bundles and 2,009 objects, with no conflicting certifications or withdrawn
citations. `every_bundle_this_build_carries_authenticates` passed.

Full source `make verify` passed in an isolated worktree: TOTAL 847.77 s,
exit 0, no `FAILED during step` marker. It covered 141 fixture projects,
733 findings, 120 contract-corpus cases, 375 script tests, 102 TypeScript
oracle cases and 41 ownership cases/465 rows with none pending, plus armed
Rust tests, Go race tests, Clippy and formatting.

Post-tier `make test-rust`, Clippy/formatting, coverage (141 projects/733
findings), contract corpus (120 cases), ownership (41 cases/465 rows, none
pending), contract conformance and `git diff --check` passed. The retired
bundled-contract conformance paths have no active cases; accepted-tier
authentication and the contract corpus provide the receipt-tier checks.

The fresh app-import measurement is pending.
The previous owner-supplied app-import figure, 0/1,872, is not a fresh result.

## Remaining proof work

In the none host, 466 exports still have four independent causes. Closing one
return shape cannot certify them. Fifteen exports have only a missing or
refused return-value shape; two more have only `callable-path`. The next work
must preserve their other domains and the consumer's finding kind.

New claim forms or value shapes require the owner's decision. No new form,
declared-signature trust premise, hand-written reads recipe or runtime pin
was introduced to inflate these measurements.
