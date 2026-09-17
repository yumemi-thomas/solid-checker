# Rootless callback premise for Until

The existing native verifier certifies a positive callback claim for both
`createSubRoot` and `createBranch` in `@solid-primitives/rootless@1.5.4`.
The claim names parameter 0, an invocation with explicit minimum 0, same-stack
schedule and untracked execution. It leaves the callback domain open and
makes no exhaustive callback or creates claim.

The retained runtime defines `createBranch = createSubRoot`. `createSubRoot`
passes a closure to `createRoot`, and that closure invokes `fn(dispose)`.
The diagnostic adds the proposed operations only to these two exports, then
uses native normalization, planning and ordinary certification. No proof rule,
receipt interface, trust policy, dependency pin or package byte is changed.

The [measurement](2026-09-08-rootless-callback-premise.json) contains the exact
artifact selection, importer, resolution closure, accepted summaries and
receipt. Ordinary analysis authenticates the receipt and selects the exact
case. The successful transaction exits 0 in 385 ms with zero archive/metadata
cache misses. The same proposal with parameter 1 instead refuses at the exact
callback-flow demand in 334 ms, exit 2, with zero cache misses. All unrelated
exported claims are preserved.

An initial attempt at a new catalog destination regenerated the original
proposal because its acquisition sidecar named the old synthetic importer.
That attempt did not certify the callback claims and is excluded. The
corrected attempt rebinds the untrusted acquisition request to the destination
importer; native planning and certification establish its new resolution and
receipt. No receipt is copied across contexts. This private diagnostic receipt
is not installed into Until's context or counted as an Until certification.

## Why ordinary generation omitted the claims

A bounded observation of fresh Rootless generation captured native stderr at
`/private/tmp/rootless-native-call-NBKKQo/stderr.log`. The generator reports
`PackageContractExportMissing`, with context `no receipt-accepted contract
matches this exact import`, for its Utils import at bytes 151–241. Its
`fallback-all` attribution opens all claim domains of every export, including
both target exports. The separate `UnknownCallbackExecution` records name
`createCallback` and `createRootPool`, not `createSubRoot`.

This identifies a dependency-acquisition and generation path to test before
adding any new semantic rule. The dependency graph can certify Utils before
generating Rootless; the missing callback premise may then survive normal
generation. Until currently generates a proposal without a dependency-case
refusal, so the existing graph switch does not automatically select this path
after its later callback-flow certification refusal.

The next bounded experiment should request Until's exact existing artifact
case through graph preparation, retain explicit acquisition provenance, and
inspect the next native result. Do not manufacture a generation refusal to
route it. Any supported production routing must preserve previously certified
cases and their claims and must not treat a graph lane as a superset of a
reused proposal.

This result adds two certified behavioral claims to an already covered
Rootless entrypoint: **zero new executable entrypoints and zero complete-row
transitions**. Until's possible one-row gain remains unmeasured. The previous
full corpus baseline remains 326 complete rows. The diagnostic used the fresh
build from the prior successful full verification (exit 0, TOTAL 140.59s);
no production source changed in this investigation, so that unchanged check
was not repeated. Nothing was committed or pushed.
