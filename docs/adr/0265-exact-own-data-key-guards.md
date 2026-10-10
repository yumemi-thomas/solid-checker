# ADR 0265: owner guards on an argument's exact own data keys

- Status: accepted and implemented (2026-10-09).
- Owners: `GuardAtom::OwnDataKeys` (model, validation, canonical encoding,
  wire, schema `ownDataKeys`); the owner-guard consumer in `owners.rs`;
  `scripts/lib/own-data-key-contracts.mjs`; the
  `@solid-primitives/pointer@1.0.0-next.2` spec.

`createPointerListeners(config)` registers one owned effect per truthy
normalized handler. Its `listener-effects` row had to be `min: 0` for every
input, because a later alias overwrites an earlier one: `{ onDown: f, ondown:
undefined }` normalizes both keys to `down`, and the last wins
(`helpers.js:20-24`). So a module-scope call was only an uncertifiable
missing-owner, never a proven one.

A guard may now say that an argument is an object literal whose own data keys
are exactly a given set. With that premise the alias collision cannot happen.
The pointer spec adds a mandatory registration guarded by: exactly the keys
`target` and `onDown`, with `onDown` an own callable. That row states `min: 1`
on the browser; the unconditional row keeps `min: 0`, and the server branch
keeps `hostFree.minZero`.

The guard is true only for a complete object literal fact: data properties
with unique static keys, no spread, getter, setter, method, computed key or
`__proto__`. Any other key set makes it false. Nonliteral configs, spreads,
accessors, computed keys, `__proto__`, proxies, argument spreads and missing
facts leave it unknown, never true. Integer-like keys, which change
enumeration order, are not in the set, so they make it false.

Probe observations for these claims are now bound to the full claim, pair
metadata, runtime pins and both program bytes. A changed guard or cardinality
cannot reuse an earlier Chrome result.

## Review

An adversarial review found no input the guard accepts where no listener
registers. It found two weaker problems:

- **Major (fixed here):** stale probe observations could be reused for an
  edited claim; hence the digest binding above.
- **Minor (open):** normalization accepts impossible conjunctions such as an
  exact key set without `onDown` plus an `onDown` property demand. The owner
  adapter keeps them false or unknown, so this is a validation gap, not a
  false positive.

## Measured consequences

- Fixture `package-closed-map-owner-consumer`: module and namespace-wrapped
  calls are missing-owner violations; a leaf's first call is a
  leaf-owner-forbidden-call violation; the createRoot twin and a shadowed local
  are clean; every refused shape is uncertifiable.
- All twelve pointer pairs pass in Chrome on rc.13.
- Primitives ledger, browser: 98 of 111 report correctly (was 97).
  `createPointerListeners` at module scope. No correct twin has a violation.
- rc.13 corpus: no violation or uncertifiable site added or removed.
