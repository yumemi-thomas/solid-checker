# Original-input proof restores the seven regressed artifact cases

The nine-probe run finished at 2026-09-08 00:30:43 JST with actual exit 0.
Protocol 40's bounded opening-prefix proof restores all seven selections lost
by ADR 0064, while keeping its replacement counterexample refused. This is
recovery relative to the stricter read-origin baseline, not additional coverage
beyond the earlier implementation-owner baseline.

| Probe | Strict-baseline accepted set | Current accepted set | Transition |
| --- | --- | --- | --- |
| Motion 0.7.0-beta.4, Solid 2 floor | `./m` | `.`, `./m`, `./v2` | partial → complete |
| Motion 0.7.0-beta.4, Solid 2 head | `./m` | `.`, `./m`, `./v2` | partial → complete |
| Motion 0.6.0, Solid 1 | none | `.`, `./v1` | refused → complete |
| Solid Primitives Utils 6.4.1 | `./immutable` | `.`, `./immutable` | partial → complete |

Solid 2 Motion's `.` and `./v2` select `./dist/v2/index.mjs` with
`./dist/v2/index.d.mts`; `./m` retains `./dist/v2/m.mjs` and
`./dist/v2/m.d.mts`. Solid 1 Motion's `.` and `./v1` select
`./dist/v1/index.mjs` and `./dist/v1/index.d.mts`. Utils root selects
`./dist/index.js` and `./dist/index.d.ts`; its immutable case is unchanged.
These are distinct exact export branches even where their target files match.

Scoped totals change from 22 to 29 artifact selections and from
0 complete / 5 partial / 4 refused to 4 complete / 2 partial / 3 refused.
Pacer's 13 selections and Table's six selections remain unchanged and partial.
The three Corvu rows remain refused. Every one of the pre-regression baseline's
29 selections **and its complete exported claim set** is preserved; no claims
were dropped to recover these entrypoints.

The artifacts are measured from each report row's retained artifact root through
the published case-set pointer and its named catalogs, with matching document
digests. Each successful row reports ordinary consumer receipt authentication
and exact case selection. Evidence retains importer/specifier coordinates,
artifact and closure hashes, declaration hashes, exact resolution branches,
dependency receipt/trust roots, producer identities and signed receipt digests:

- [Recovery against the strict baseline](2026-09-08-initial-reads-recovery-measurement.json).
- [Exact selections against the earlier baseline](2026-09-08-initial-reads-scoped-measurement.json).
- [Exported claim-set equality](2026-09-08-initial-read-claim-preservation.json).

The new positive premise binds the two original Utils `handleDiffArray` inputs
at the initial `current.length` and `prev.length` reads, before assignments to
sliced results. It does not treat those results as the original inputs. The
native verifier binds exact parameter declarations and exact use census rows,
then records the additional witness in the recursive input proof. Each package
and importer context is recertified through the normal publication transaction;
neither matching bytes nor a receipt from another context grants permission.

Validation: focused producer, protocol-client and native/verifier tests pass;
full `make verify` exits 0 with `TOTAL 242.12s` and no failed-step marker in
`/private/tmp/initial-reads-verify-final.log`. The first run exposed stale schema
hash pins, corrected in both producer and client before the passing run. The
ordinary CLI still refuses the [unchanged replacement archive](2026-09-08-initial-read-negative-control.json)
and the [early-root/later-member boundary](2026-09-08-initial-read-member-boundary.json).
The latter has zero TypeScript diagnostics and runtime evidence of one original
root read but zero calls of the original method. No snapshots or bundled
contracts changed.

The [completed full 418-probe rerun](2026-09-08-initial-read-full-frontier.md)
preserves this control group but finds 30 other lost selections across ten rows:
318 complete / 61 partial / 30 refused / 9 not advanced, 1,459 selections.
Corpus-wide preservation therefore fails and further proof recovery is required.
Remaining scoped blockers are Floating UI `getOverflowAncestors`, Corvu Utils
0.4.2 `contains`, and Utils 0.3.2 `sortByDocumentPosition` lacking the currently
required original-input premise, plus Table's separate recursive member census
gap. Their exact causes still need investigation. There is no metric/denominator
correction, no new gain beyond the pre-regression scoped baseline, and no
measured coverage ceiling. Nothing was committed or pushed.
