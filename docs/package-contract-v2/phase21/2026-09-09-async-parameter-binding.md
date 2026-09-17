# Async parameter binding identity

The unwritten-parameter source premise previously reused the returned-parameter
predicate's unconditional async exclusion. That exclusion is necessary for
return identity, because async completion wraps the result, but does not follow
for the lexical value held by a parameter.

The producer now separates the two predicates. An ordinary parameter in a plain
or async implementation can retain original-input identity when its exact
compiler symbol has one declaration and no writes, including nested writes.
Rest/default/destructured bindings, duplicate names, arguments/eval references,
and generators remain excluded. The selected signature must bind the exact
parameter declaration in the same source. No result identity, member shape,
schedule, reachability, or contract closure follows from this premise alone.

The Rust client and certification consumer admit async completion for this
existing binding fact only. Initial-prefix, positional, helper-transfer, and
returned-parameter premises retain their own restrictions. No receipt, schema,
trust, or wire field changes. Matching producer/client source-manifest pins are
rebuilt together; receipts from a prior build are not copied into the new run.

Producer regressions cover reads after await, ordinary and captured writes,
defaults, arguments, and eval, and assert that an async return still has no
unwrapped parameter identity. Native client and verifier tests bind the exact
signature slot and reject missing, duplicate, foreign, default/rest, or generator
premises. The authenticated package test includes positive and reassigned async
implementations with a read before suspension.

The bounded diagnostics Playwright transaction now passes the previously open
recursive-value demand. Final measurement and preservation evidence accompany
this note; no complete-row transition follows from this entrypoint alone.

The combined scoped publication selected `.`, `./browser`, `./protocol`, and
the new `./playwright` case. All three prior main documents are byte-identical.
Ordinary analysis authenticated the receipts and selected the exact cases.
The new runtime is `./dist/playwright.js`, SHA-256
`34ab4d0d84989c14df23b957dd978616d0550f4194e3b71cf4d790a84c8c605c`;
the declaration is `./dist/playwright.d.ts`, SHA-256
`41650ac6f5974881142047db9d1ca198cfe4c802558a6550984859d0f2170803`.
Resolution selects `/exports/.~1playwright/default` and
`/exports/.~1playwright/types`, with authenticated closure digest
`139a2dac089dcbf46e999a0c2584a842e62411cfadc384e26a21428e4fff5467`.
The measurement records the complete importer, manifest, archive integrity,
receipt bindings, binary digests, and exact before/after case sets in
[the scoped evidence](2026-09-09-async-binding-scoped-measurement.json).

The real-package transaction requested `./vitest` too; its effectful test-host
initialization remains unproved. `./package.json` remains an asset exclusion.
Thus this is one new executable entrypoint and artifact case, with a
partial-to-partial row transition, and no coverage-metric correction.

## Full validation and corpus measurement

Focused Go unwritten-parameter tests passed. The native client test and two
backend binding/receipt tests passed; the receipt test was rerun after adding
the async positive/negative pair and passed. Full `make verify` exited 0 with
`TOTAL 173.42` and no `FAILED during step` marker. This includes Go race tests,
armed Rust tests, coverage, ownership, CLI, conformance and performance checks.
Log: `/private/tmp/async-binding-verify.log`. No snapshots changed.

The full 418-probe corpus then exited 0 in 534.042 seconds, finishing September
9 at 10:47:52 JST. It used a fresh pinned release checker with 8 generation and
20 certification slots and the same scope and timeout as the preceding run.
`2026-09-09-async-binding-full.json` is the new measurement authority. Installed
package versions did not drift. Counts remain 327 complete, 64 partial, 18
refused, and 9 not advanced. Accepted entrypoint occurrences increased from
1,185 to 1,186; artifact cases increased from 1,549 to 1,550.

The only new case is diagnostics `./playwright`. Every prior artifact selection,
closure digest, and claim set was preserved across all 391 certified rows.
[Full measurement](2026-09-09-async-binding-full-measurement.json) records exact
before/after cases and bound consumer results for the changed package, plus
the preservation census for all prior certified rows.
[Claim comparison](2026-09-09-async-binding-full-all-claim-preservation.json)
independently compares existing claim sets and closure identities.

The [refreshed partial-row audit](2026-09-09-async-binding-partial-row-audit.json)
has 34 explicit executable gaps and 68 wildcard executable candidates: 102
remaining in-scope occurrences. The 121 intentionally unrequested candidates
and 28 unresolved wildcard selections remain separate. This is still an
installed-file audit, not a new authenticated denominator metric.

This closes the async binding defect, not every remaining proof limitation.
SSE and test-host initialization need effectful host authority. Shared defaults
and Kobalte's overwritten input require independently bound conditional or
per-operation origin proofs. UI's declaration resolution and framework virtual
modules retain their previously documented blockers. No unproved case was
silently cleared, and no complete-row gain is claimed.
