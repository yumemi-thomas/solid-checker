# 0012 — Rebind receipt-owned targets across entrypoints of the same package

Status: accepted; implemented
Date: 2026-09-04

## Decision before code

After ADR 0011, Kobalte 0.9.2's graph passes the Aliases kind refusal and stops
while loading `solid-js/web` as a proposal dependency of
`@solid-primitives/utils@6.4.1`. ErrorBoundary is re-exported from the separate
`solid-js` root entrypoint. Its file is inside the same package directory, but
belongs to a semantic dependency edge rather than the web entrypoint's local
file closure. The catalog rebinder recognizes targets outside the directory
and nested installed packages, but not this self-package edge.

Recognize a target outside the selected axis's root/local closure as a
dependency target when the resolution carries a self-package semantic edge
(both package name and bare/subpath specifier name this package). Preserve
targets already covered by the local closure. A missing self edge or an edge
for a different package must retain the existing refusal.

This helper is used only in unauthenticated proposal projection and after
ordinary policy-2 discovery has authenticated the receipt and matched the
entire resolved-import root. It is not the native planning proof. Native
planning continues to derive external targets from independently planned
dependency bindings, replay exports from archive bytes, and verify every
closure edge. The helper cannot issue a receipt. An attacker changing a
target or edge in discovery changes the signed import root and is rejected
before this rebinding. No synthetic closure entry or artifact substitution is
introduced, and no dependency's executable bytes are dropped.

## Evidence required

Pin classification with and without the self edge, a foreign edge, unchanged
local bindings, and the inability of ordinary binding without planned targets
to accept a foreign entrypoint. Retain existing signed-root mutation and native
export-replay tests. Measure the exact real graph again; do not infer that
passing this rebinding also proves dependency behavior or completes a probe.

No public field, policy digest scheme, harness pin rule, transformer, or
Type Facts protocol changes. The fix reconciles private projection/discovery
with the authority-bearing native graph planner's existing target ownership.

## Measured result

The paired classification and native archive tests pass. A self-package forward
entrypoint binds with the independently planned root dependency; omission of
that plan and a forged runtime target digest both refuse. Foreign and lookalike
package edges do not activate the rule, and local root targets stay local.

The retained Kobalte 0.9.2 graph now prepares all 20 canonical nodes across 13
published artifacts (25 acquisition units, 20 proposals) and reaches one native
certification transaction and Type Facts case-set batch, with zero cache misses.
It then refuses `@solid-primitives/keyed@1.5.3`'s `SetValues` read demand:
`parameter-rooted read has no exact implementation call or use`. The exact
demand is `sha256:71692cbcd0429cdf24d41397ef43268cac1c99e21208f322eb5763e88dd9c18c`.
That positive-operation proof remains required. Neither this fix nor ADR 0011
waives it, and this graph still produces no accepted Kobalte row.
