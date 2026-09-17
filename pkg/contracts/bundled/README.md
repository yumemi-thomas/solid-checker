# Historical first-party contract artifacts

Both dialect bundle indexes are currently empty. The retained stable-v1 main
documents are proposals and conformance history, not active receipt-issued
inputs to ordinary analysis. The matching review location is
`rust/crates/solid-dialect/contracts/`.

The historical `solid-v2` material comes from the RC.3 conformance audit. It covers
exact `solid-js@2.0.0-rc.3`, `@solidjs/web@2.0.0-rc.3`, and
`@solidjs/signals@2.0.0-rc.3` cases. Environment selection is explicit; the
consumer never guesses a browser, node, development, or production branch.

The `solid-v1` directory is down to three files and is **not** an audit
inventory any more. Sixteen historical 1.x documents were deleted on 2026-09-17
to keep the repository free of Solid 1.x artifacts; what is left is
`bundle-index.json`, which the bundle walker requires of any `solid-v*`
directory, and two stable-v1 documents that `policy2_receipt`'s tests compile in
as fixtures. Those two are inputs to dialect-neutral receipt logic, not
statements about Solid 1.x, and retargeting them onto 2.0 documents needs that
test's closure-identity fixture rebuilt rather than substituted.

Ordinary analysis uses the built-in Solid dialect for `solid-js`,
`@solidjs/signals`, and `@solidjs/web`; core contracts cannot override that
model. External packages still require independently accepted contracts.
See ADR 0027 for the migration and the distinction between reviewed runtime
premises and independent package certification.

Bundle and conformance gates retain the historical inventories and artifact
pins. `runtime-lock.json` records the published closure used by the audit;
its existence alone is not evidence that an installed runtime was validated.
