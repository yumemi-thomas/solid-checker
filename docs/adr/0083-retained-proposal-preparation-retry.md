# Retain generated roots after graph preparation fails their floor

SolidStart 2.0.3 has ten accepted artifact cases. Preparing semantic graphs for
all of them refuses an uninstalled `crossws` declaration dependency of `h3`,
before the existing native retained-floor strategy can run. Separately,
`./serialization` certifies through an exact two-node graph in 3.067 seconds,
with no cache misses. Neither outcome permits dropping an accepted case or
copying a receipt between contexts.

When opt-in entrypoint recovery loses a retained case during preparation,
retry once using the existing retained-proposal-root strategy previously used
for large case sets. A dedicated preparation-error type selects this retry;
matching text, generic acquisition failures, native proof refusals and requests
without entrypoint recovery do not. The retry has private scratch storage and
retains the first preparation refusal in the audit. Failure records both
reasons and falls back to ordinary certification.

The exact retained prefix and missing frontier still obey the 1,024-case
publication and 32-case graph bounds. Retained proposals carry their original
compiler-source requests. They are not accepted evidence: the same native
case-set transaction re-authenticates artifacts, resolutions, dependency
receipts, trust and all semantic demands. Every positively established
retained case remains mandatory. No public receipt or Type Facts interface
changes.

Focused tests cover exact retained/frontier selection, successful retry,
no opt-in, generic-error non-retry, and failed retry with both refusal traces.
Both affected CLI suites pass: 105 tests. The first supported SolidStart
attempt preserves all ten cases but exposes the historical registry record
blocker in ADR 0084. Its private serialization certificate is not yet a
combined coverage gain.

After the selected-registry repair, the supported CLI run at
`/private/tmp/start-supported-retained-v7JNwc` finishes in 12.774 seconds with
actual exit 0 and zero cache misses. Publication follows its case-set pointer;
ordinary analysis reports receipt authentication and exact case selection.
The [measurement](../package-contract-v2/phase21/2026-09-08-start-retained-preparation-measurement.json)
records **10 → 11 accepted cases**, preserving every old selection and exported
claim and adding only `./serialization`. Its runtime is
`./dist/fns/plugins.js`, declarations `./dist/fns/plugins.d.ts`, and artifact
case is `artifact-case:2bd327b9174326fd569e49043c319c2401fbf4471de9055c05b91f79e2c56e19`.
The JSON contains full artifact/closure hashes and consumer-bound receipt,
importer, dependency and trust evidence. No trial receipt was reused.

The row stays partial: `./client` still needs the virtual `solid-start:app`
module, and `./env` remains outside the accepted set. This adds one executable
entrypoint, zero complete rows, and makes no denominator correction. The
previous all-ten-only result is retained as evidence of the next blocker,
not counted as recovery.

Full `make verify` exits 0 with TOTAL 125.07 seconds and no failed-step marker
(`/private/tmp/retained-preparation-registry-verify.log`). No snapshots changed,
and nothing was committed or pushed. Full-corpus preservation remains to be
measured in `2026-09-08-retained-preparation-full.json` against the completed
callback-retry baseline in the full audit below.

The full 418-row run finished at 21:53:41 JST on September 8 in 1,489.158
seconds with actual exit 0. The unchanged metric remains **327 complete /
63 partial / 19 refused / 9 not advanced**. There are **1,533 → 1,534 accepted
artifact cases**: only SolidStart `./serialization` is added. Every prior
selection and exported claim is preserved, with zero closure identity
transitions. Motion Solid 2 floor/head retain `.`, `./m`, and `./v2`.

The [full measurement](../package-contract-v2/phase21/2026-09-08-retained-preparation-full-measurement.json),
[claim audit](../package-contract-v2/phase21/2026-09-08-retained-preparation-all-claim-preservation.json),
and [closure audit](../package-contract-v2/phase21/2026-09-08-retained-preparation-closure-transitions.json)
follow the published pointers and exact package/version catalogs. Matching
binaries and the producer stamp are archived under
`rust/target/ecosystem-investigations/2026-09-08-retained-preparation-binaries/`.
This is one new executable certificate, zero complete-row transitions, and
zero metric corrections. ADR 0085 subsequently narrows a false positional
read premise discovered during this run; the archived result describes the
earlier build and does not establish preservation after that narrowing.
