# Keep an implementation subject in its own installation

An export's verified runtime binding includes its owning snapshot. When that
snapshot is the current certification plan's own package, implementation lookup
now uses the current plan's materialized package root. Previously it selected
the first plan with matching snapshot bytes. Two installations of the same
package may contain identical bytes while having different dependency
resolution contexts and different runtime modules rooted in the witness
program. Byte equality is not permission to substitute their implementation
subjects.

The change selects an existing authenticated subject. It adds no behavior
premise, receipt field or trust exception. Repeated artifact plans for one
installation remain valid. Dependency-owned runtime bindings still use the
existing planned-owner path; this bounded fix does not claim to resolve every
possible ambiguity among dependency installations.

The focused regression constructs two plans for identical package snapshots
at distinct installation roots, places the other installation first, and
requires the current package's implementation to remain in its own installation.
It fails before the patch and passes after it. The existing repeated-plan and
single-plan controls pass. The motivating Corvu Popover measurement stops at
`@corvu/utils@0.4.2`'s `controllableSignal` implementation with
`sourceUnavailable`. The [scoped measurement](../package-contract-v2/phase21/2026-09-07-implementation-owner-measurement.json)
confirms that this refusal clears with unchanged installed versions. The graph
then stops at Floating UI's reassigned `list.concat` value path. Corvu Popover
remains refused: zero accepted artifact cases before and after, no complete-row
transition, and no new certification. The benchmark exits 0 in 16.683 seconds.
The scoped result is followed by the aggregate measurement below.

Final full `make verify` passes with actual exit 0, `TOTAL 116.91s`, and no
`FAILED during step` marker (`/private/tmp/implementation-owner-verify.log`,
exit record `/private/tmp/implementation-owner-verify.exit`). Both focused
implementation-location tests pass. No fixture snapshot or bundled contract
changed in this slice. Full ecosystem preservation subsequently passes in
`2026-09-07-implementation-owner-full.json`: 418 probes, exit 0, 1,048.255 seconds,
finished 2026-09-07 23:46:37 JST, no timeout or memory-limit flags. The
[exact comparison](../package-contract-v2/phase21/2026-09-07-implementation-owner-full-measurement.json)
preserves all 1,489 selections across 387 certified rows. Counts remain
324 complete / 63 partial / 22 refused / 9 not advanced. This result precedes
the subsequent parameter-read origin correction in ADR 0064.
