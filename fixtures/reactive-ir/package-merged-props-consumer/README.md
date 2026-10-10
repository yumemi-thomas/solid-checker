# A consumer of a `merged-props` return

Pins the one contract claim whose meaning depends on the **caller's argument**:
ADR 0109's `merged-props` return, and the `no-destructure` arm in
`binding_initializes_reactive_store` that reads it.

`App.tsx` calls two exports of the same shape four times. The contract is
identical across the four; the argument position is not, and that is the only
thing the verdicts may be attributed to:

| component | export | props at | verdict |
| --- | --- | --- | --- |
| `Card` | `withDefaults` (reaches argument 0) | 0 | SC1003 |
| `Static` | `withDefaults` | — (plain object) | clean |
| `Overridden` | `withOverrides` (reaches argument 1) | 1 | SC1003 |
| `Defaulted` | `withOverrides` | 0 | clean, and an under-report |

`Defaulted` is the honest half: a merge really does carry argument 0's
reactivity, and the census does not certify it. ADR 0109 § "what was dropped"
explains why no producer fact can, and the snapshot records the gap rather than
hiding it.

## Why this fixture needs an accepted contract

Every claim above is downstream of a contract the checker *accepted*. A
contract-consumer fixture that ships the usual `obsolete-policy1` catalog is
rejected before a claim is read, so all four components would report the same
`SC9005` and the fixture would prove nothing.

So this fixture ships `.solid-checker/authorize-contract.json` instead of a
catalog: the resolution it has always hand-written, and no authorization.
`scripts/coverage.mjs` copies the tree, has `solid-contract-authorize` mint a
policy-2 receipt over the document, and analyzes the copy with the trust
configuration supplied **out of band**. Un-authorized -- which is what a plain
`solid-checker-rust --project` over this directory does -- there is no accepted
contract at all, which is the truthful baseline for a package nobody certified.

Both halves of that are load-bearing and both are pinned in
`rust/crates/solid-facts-backend/src/fixture_authorization.rs`: the signing key
is fixed, and a fixed key is not a forgery because the signature is inert until
a verifier is separately told to trust it, which nothing in this directory can
do.

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-return-consumer`'s, so the closure digest and package integrity in the
import block are the ones that fixture already pins rather than numbers invented
here. `index.d.ts` is outside the closure and is this fixture's own.

`node_modules/solid-js` is required, not optional: an authorized fixture is
analyzed from a copy at a different depth, and a fixture without its own stub
would pick its dialect from whatever sits above the copy.
