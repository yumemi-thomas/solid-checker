# Solid 1.x first-party bundle identity control

The fixture installs deliberately reduced `solid-js` and
`@solid-primitives/debounce` package bytes. Their names and versions resemble
the published packages, but their runtime/declaration/closure digests do not
match the receipt-issued first-party artifact cases.

Phase 14 therefore refuses to borrow published bundle claims. Native v1 facts
and the inspectable reduced debounce body still prove the delayed reads in
`Scheduled` and `Debounced`, producing the two expected
`v1/strict-read-untracked` violations. This is a positive refusal test: exact
artifact identity prevents a name/version match from becoming proof.

The actual published `solid-js@1.9.14` and primitive bundles are regenerated
and receipt-validated by the contract conformance gate. The fixture declarations
retain the published signatures used here, and `tsc --noEmit` accepts the
project.
