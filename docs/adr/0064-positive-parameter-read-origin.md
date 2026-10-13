# Positive original-input identity for parameter reads

The [accepted counterexample](../package-contract-v2/phase21/2026-09-07-parameter-read-origin-investigation.md)
shows why an exact parameter binding and a type-correct member path are not
enough: the implementation may have replaced that binding with a local value.

Every recursive input proof for a parameter-rooted read now requires the
existing affirmative `unwrittenParameters` premise before direct, typed or
composed read evidence can discharge the demand. Validation binds one unique
parameter slot, its exact implementation declaration, the selected signature,
and a plain implementation. Defaulted and rest parameters remain excluded.
The witness names that original-input binding. A missing row is a refusal,
never permission to fall through to the declaration's member shape.

This proves the root's input identity only. Member existence, callability,
actual read evidence and execution still need their existing proofs. The
change uses protocol 39's existing field and introduces no protocol, receipt
format, trust exception or TypeScript diagnostic.

The native packed-package regression covers both generic whole-parameter and
typed member inputs. Each has an unchanged-input positive control and an
unconditional replacement negative control. The typed replacement was wrongly
accepted before the fix and now refuses for missing original-input identity.
The ordinary CLI counterexample also now refuses at witness acquisition.
The [after evidence](../package-contract-v2/phase21/2026-09-07-parameter-read-origin-correction.json)
verifies unchanged source/archive bytes and that no new catalog was published.
Both focused tests pass. Full `make verify` passes with actual exit 0,
`TOTAL 171.95s`, no failed-step marker, log
`/private/tmp/read-origin-verify-final.log`. The first verification attempt
stopped at Clippy's `question_mark` check; that refactoring issue was fixed
before the successful full run. No fixture snapshots or bundled contracts
were changed for this correction.

This conservative correction is a prerequisite to the Floating UI coverage
extension. Mixed caller/local-default origins, writes that preserve an input,
and reads before a later write need positive flow-sensitive evidence; the
current unwritten premise cannot establish them. Their refusals must not be
removed merely because the declaration has the expected member type. Aggregate
coverage after this correction must be measured independently of the preceding
implementation-owner run. The [nine-probe measurement](../package-contract-v2/phase21/2026-09-08-read-origin-coverage-regression.md)
records seven lost artifact selections and four regressing rows, including both
Solid 2 Motion probes. Their `./m` cases survive, but `.` and `./v2` now refuse
at Utils `handleDiffArray`. This is an unresolved coverage regression, not a
new certification or a demonstrated coverage ceiling.
