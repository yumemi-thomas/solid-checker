# Composed provenance names its target by identity, not by declaration name

Two modules each declare a function named `readSignal`, and each reads one
accessor of its own. `index.js` exports its own as `readSignal`; `other.js`'s
is exported under the alias `otherReadSignal`. Two exports then compose one of
them each:

- `composesTheLocalTarget` calls the `readSignal` declared in `index.js`, and
  its `read-0` carries
  `composedFrom: {"export": "readSignal", "operation": "read-0"}`.
- `composesTheImportedTarget` calls the *other* `readSignal`, and its `read-0`
  carries `composedFrom: {"export": "otherReadSignal", "operation": "read-0"}`.

A generator that resolved the composed read's owner by the name the two
declarations share would name `readSignal` for both, and a consumer would then
discharge the second row against the first export's evidence. The provenance is
resolved from the discovering node's *symbol* instead, which is why the two
rows disagree.

The second row is also the must-not-clear case at the other end of the wire.
`otherReadSignal` is exported by `other.js` but is **not** an export of the
package: `exports` is `./index.js` alone, so the artifact case's export surface
is `readSignal`, `composesTheLocalTarget` and `composesTheImportedTarget`. A
consumer therefore cannot resolve `otherReadSignal` in the artifact case at all
and refuses the demand rather than reaching outside the case for it. Publishing
the nomination anyway is deliberate and safe in exactly that way: it is a
target to prove against, and an unresolvable target is a refusal, never a
silently believed fact.

The `node_modules/solid-js` stub is 1.x, so `createSignal` resolves through the
v1 catalog.
