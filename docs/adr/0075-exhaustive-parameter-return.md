# Exhaustive whole-parameter return identity

Factory export composition cannot use an open returns domain as an exhaustive
description of a dependency's possible outputs. ADR 0074 recovered Seroval's
positive identity operation, but the accepted contract still allowed additional
return operations. This change supplies the next prerequisite without changing
export-root composition itself.

The returns census may close a nonempty enumeration consisting of exactly one
return operation whose output is a whole parameter. It requires an explicit
plain completion form, an authenticated control-flow census, no unclassified
or unaccounted flow, a positively reachable value-carrying return, and the exact
original parameter identity at every possible return. Missing values, another
parameter, member paths, unknown identity, async/generator completion and absent
censuses refuse. The existing empty-return rule is unchanged. Structured values
and multiple return operations remain unsupported.

The generator may propose this closure for an already inferred parameter
identity. The proposal grants no authority: native verification independently
binds its runtime implementation and return sites before issuing the receipt.
Operation cardinality remains separately verified; closing the returns domain
does not raise the operation's zero lower bound or establish termination.

No new protocol, transcript field, receipt or trust interface is introduced.
The existing producer's positive original-parameter facts are the premise.
Factory export-root composition and the Solid raw-entrypoint tuple census remain
separate required work; this closure alone is not an entrypoint gain.

The empty-return synthesized veto is explicitly restricted to empty return
enumerations. Its observation of any returned value would falsely contradict
an identity return. The new nonempty closure requires a claim-addressed recipe;
the Seroval production and development recipes pass two valid plugin objects
and report only an identity mismatch. They never expose transcript interfaces
to the package. Separate module files follow the harness's per-entry private
copy requirement.

Fresh ordinary certification now publishes `createPlugin` with `returns`
closed for both Seroval artifacts. The [catalog and receipt evidence](../package-contract-v2/phase21/2026-09-08-seroval-closed-identity-certification.json)
follows the published pointer, checks exact named catalog/document/receipt
digests, and records importer, resolution, positive-proof, closed-claim and
probe-gate roots. Ordinary consumer authentication and exact case selection
both succeed. The transaction exited 0 in 4.008 seconds without network or
cache misses. These two stronger dependency contracts are not new corpus
entrypoints, and their receipts cannot be transplanted into graph contexts.

Earlier trials certified only partial contracts because planning withheld the
closures for lack of recipes. The plan recorded those refusals even though the
final CLI audit's withholding list was empty; that audit limitation is not
evidence of closure. The first hand-recipe trial also refused duplicate module
copying, corrected by the separate module files.

Focused census and authenticated identity tests pass, including wrong/mutated
parameter refusals. Three corpus fixtures intentionally gain a returns closure:
`entrypoint-condition-isolation`, `exported-class`, and `mutated-parameter-return`.
Only their contract and proposal-plan snapshots changed. Full verification
after the final recipe correction passed with actual exit 0, TOTAL 78.19
seconds, and no `FAILED during step` marker in
`/private/tmp/parameter-returns-verify-final.log`. Full-corpus entrypoint
measurement is deferred until the factory composition consumer exists; this
slice claims no row transition. No commit or push was made.
