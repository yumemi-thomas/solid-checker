# package-contract-incomplete

`SC9005` · **error** · uncertifiable

An external package's reactivity contract is absent, stale, unverified, or
missing facts for an imported export or callback. The checker refuses to guess
behavior across that package boundary.

## What it does

The rule reports three message variants:

- no usable contract exists for the exact installed package version;
- an accepted contract exists but omits the imported export or exact artifact
  selection/guard facts are unresolved;
- an exact external helper receives a callback, but the contract does not say
  whether that callback runs synchronously, deferred, tracked, or owned.

Package, version, integrity, runtime/declaration files, closure, entrypoint, and
export identity must all match. Unknown callbacks include a wire-independent
open-claim context naming the exact callback and independent semantic axes; the
diagnostic never emits an editable contract or treats an omitted field as
negative proof.

## Severity by gate

The rule's severity is **error**, and it stays error at every gate but one
([ADR 0119](../adr/0119-sc9005-severity-by-gate.md)):

| gate | severity |
| --- | --- |
| no receipt-accepted contract matches the import (acceptance gate) | error |
| a contract authorized only by obsolete proof policy 1 | error |
| an accepted contract omits the imported export | error |
| an accepted contract states a claim the call site cannot bind | error |
| **an accepted contract leaves claims open** (`unknown-contract-claims:…`) | **warning** |

Every one of them is `uncertifiable`, so a run with any of them still certifies
nothing; the warning only says the analysis ran over an accepted, partial
premise rather than a missing one.

## Why it matters

Reactive reads, writes, ownership, and timing can cross ordinary function and
package calls. Treating an unknown helper as transparent, synchronous, or
owner-preserving would turn missing evidence into false proof. This finding is
therefore an explicit certification gap rather than a guessed violation.

## How to fix

Generate a proposal for the exact installed package:

```sh
solid-checker contract generate \
  --package-root node_modules/example \
  --integrity 'sha512-…' \
  --output .solid-checker/contracts/example/solid-reactivity.json
```

Review the recursively open claim, supply complete proof-family transcripts,
run `solid-checker contract verify` to issue an accepted document and receipt,
then register those exact bytes with the full import resolution in
`.solid-checker/accepted-contracts.json`. A package-shipped proposal is not
accepted merely because it is on disk; probes may falsify closure but cannot
replace proof.

## Related

- [reactive-source-uncaptured](reactive-source-uncaptured.md) — a source handed to an external call without a complete behavioral summary
- [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) — unresolved project-level dispatch
- [Package contracts](../package-contracts.md) — schema, evidence, and review workflow
