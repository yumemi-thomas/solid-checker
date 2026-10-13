# Package-directed pilot: bounds, hosts and returned members

The extension completes eight offline trials over `event-listener`, `raf` and
`memo`, using the same pinned rc.9 runtime and published package installations
as the [initial pilot](2026-10-01-package-directed-pilot.md). All **16 consumer
files pass TypeScript 5.9.3**; **32 consumer analyses** compare the compiled tier
with independently authenticated authored proposals. The evidence is retained
under `rust/target/package-directed-extended/`. No accepted-tier artifact,
analyzer implementation, schema, dialect, snapshot or misuse-ledger entry changes.

## Results

| Trial | Accepted ownership/return claim | Consumer result |
| --- | --- | --- |
| Listener, browser, owner `min: 0` | Possible effect registration survives | SC4001 **uncertifiable** outside owner; none inside root; SC9005 remains in both |
| Listener, node, owner `min: 0` | Owner operation withheld | No owner finding; SC9005 remains |
| Listener, host-free, owner `min: 0` | Possible effect registration survives | SC4001 **uncertifiable** outside owner; none inside root; SC9005 remains in both |
| RAF, browser, cleanup `min: 1`, ownership alone | Guaranteed cleanup survives | SC4001 **violation** outside owner; none inside root; SC9005 remains in both |
| RAF, node, cleanup `min: 1` | Owner operation withheld | No owner finding; SC9005 remains |
| RAF, host-free, cleanup `min: 1` | Owner operation withheld | No owner finding; SC9005 remains |
| RAF, browser, tuple return `min: 0` | Tuple withheld; cleanup survives | Accessor-read pair unchanged and uncertifiable |
| `memo.createPureReaction`, browser, cleanup `min: 1` | Guaranteed cleanup survives | SC4001 **violation** outside owner, as in baseline; none inside root; SC9005 remains in both |

The memo replication adds a fourth package to the overall experiment, at
`@solid-primitives/memo@2.0.0-next.2`. It does **not** add a new finding: this
browser environment already has the proven owner finding in the compiled tier.
The ledger's host-free expected kind does not determine the result of a
browser-specific proof. The RAF improvement remains the single additional
proven misuse finding recovered by these proposals.

No new complete package or clean misuse/correct-use pair is demonstrated by
this extension. Partial receipts must not be promoted to complete package
certification, and SC9005 on correct use remains a material product limitation.

## Listener diagnosis

Changing the listener owner operation from `min: 1` in the initial pilot to
`min: 0` makes the same exact primitive calls adequate evidence in browser and
host-free mode. This rules out a general inability to resolve these dialect
calls as the reason for the guaranteed claim's failure.

The verifier's `operation_reachability_floor` requires positive lower bounds
to be witnessed by a call that is both reachable and independently
`unconditional`. `require_owner_operation_call` applies that floor to each
individual call. The published listener has mutually exclusive branches:

```js
if (typeof targets === "function") createEffect(compute, apply);
else createRenderEffect(compute, apply);
```

Neither individual branch call proves registration on every normal completion.
The current proof does not join their owner requirements into a guaranteed
registration claim. The weaker proposal proves possible registration and
therefore correctly reports an **uncertifiable** owner finding. Its acceptance
does not authorize silently changing the finding to a violation.

On node the early `if (isServer) return` makes both registrations unreachable.
Even the `min: 0` positive operation is withheld: "may happen" cannot describe
an operation proved never to happen in the selected artifact environment.

## RAF correction and actual blocker

The initial RAF tuple trial used the generic operation constructor's
`count.min: 1`. That was an authoring mistake: the already-approved structural
proof's `is_bare_return` requires the exact conditional `0..many` return form.
Its original `not a bare return` refusal therefore did not identify the
returned-member proof limit. The initial result remains historical evidence,
with this correction linked from it.

The extension changes only that return bound to `min: 0`. The native verifier
now proceeds to the actual refusal: **`unsupported structural member claim`**.
The proposed tuple contains a reactive accessor and two ordinary callables.
The current ADR 0172 structural candidate supports primitive, whole original
parameter and owned-accessor leaves; it does not support ordinary callable
leaves. Those two members prevent acceptance of the complete tuple. No member
was replaced by `plain` or hidden to get a receipt.

The ownership-only RAF trials isolate its cleanup claim from this unsupported
return. Browser proves `onCleanup(stop)` registration and the missing-owner
violation. Node executes the early server return; host-free certification
cannot prove a positive lower bound through that condition. Both withhold the
guaranteed cleanup rather than leaking browser authority into another host.

## What this establishes

Package-directed proposals are useful for separating three problems:

1. A supported claim not offered strongly enough by generation: browser RAF
   cleanup proves and produces a new violation.
2. A missing complete-path proof: guaranteed listener registration still needs
   evidence covering mutually exclusive branches. The existing operation form
   already expresses the desired claim; weakening its proof floor would be
   unsound.
3. A deliberately unsupported structural member: RAF's ordinary returned
   callables need a reviewed extension to the ADR 0172 member proof. Existing
   schema syntax alone is not proof authority. Owner approval remains required
   before adding a new supported value shape to that family.

The next bounded implementation candidate is the complete-path owner proof,
with missing-else, early-return, throw, unrelated-symbol and node controls,
followed by the exact published listener pair. It should establish every-path
registration from authenticated source facts, not infer it from names or
passing runtime samples. The returned-callable extension is a separate decision.

## Reproduction and validation

Use the environment in the pilot
[README](../../../benchmarks/package-directed-pilot/README.md), a fresh output
directory, and the additional `extended` argument. The runner preserves the
original browser pilot as its default and performs the eight-trial matrix only
when explicitly selected. `check-extended.mjs` also verifies a saved observation
record without repeating certification or consumer analysis. It asserts host
refusals, retained operation bounds, finding kinds, the unchanged memo control,
the corrected tuple refusal, and SC9005 on every incomplete consumer.

All eight certifications reached exact-case authenticated admission of their
weakened documents; this says nothing stronger than their retained claims.
The saved-result assertion pass reports 8 trials, 16 TypeScript checks and 32
consumer analyses. Formatting, workspace Clippy, schema/manifest validation and
diff checks pass. Full verification, coverage/ownership reruns, tier regeneration
and checkpoint/app sweeps are deferred because product semantics and accepted
artifacts did not change. Generated experimental receipts remain outside the tier.
