# package-contract-incomplete

`SC9005` · **error** · uncertifiable

An external package's reactivity contract is absent, stale, unverified, or
missing facts for an imported export or callback. The checker refuses to guess
behavior across that package boundary.

## What it does

The rule reports three message variants:

- no usable contract exists for the exact installed package version;
- a contract exists but omits the imported export or has environment-dependent
  implementations that cannot be summarized as one behavior;
- an exact external helper receives a callback, but the contract does not say
  whether that callback runs synchronously, deferred, tracked, or owned.

Version matching is exact. A project-owned contract can override a package or
bundled contract, but stale evidence never silently certifies a newer artifact.
Unknown callbacks include an editable schema-v1 JSON stub in the diagnostic so
the missing execution fact can be filled deliberately.

## Why it matters

Reactive reads, writes, ownership, and timing can cross ordinary function and
package calls. Treating an unknown helper as transparent, synchronous, or
owner-preserving would turn missing evidence into false proof. This finding is
therefore an explicit certification gap rather than a guessed violation.

## How to fix

Generate a local contract for the exact installed package and review it against
the runtime artifact:

```sh
solid-checker contract generate \
  --package-root node_modules/example \
  --output .solid-checker/contracts/example/solid-reactivity.json
```

Add the missing export summary or callback parameter execution mode, preserve
the generated package/version evidence, and validate the document. Package
maintainers can ship `solid-reactivity.json` in the package root so all
consumers receive the reviewed facts.

## Related

- [reactive-source-uncaptured](reactive-source-uncaptured.md) — a source handed to an external call without a complete behavioral summary
- [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) — unresolved project-level dispatch
- [Package contracts](../package-contracts.md) — schema, evidence, and review workflow
