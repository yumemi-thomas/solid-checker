# leaf-identity-const-namespace

**Claim (ADR 1007).** SC3001 attributes a leaf operation to a primitive only where the binder confirms the callee is that primitive (ADR 0711: a named import, or a plain member of a namespace import of a module the dialect owns). A `const` the file never rebinds confirms it as well: a `const` object pattern over such a namespace whose slot names the primitive by a plain identifier key (`const { createEffect: make } = S`, the shorthand, a pattern with a rest, an exported one), the namespace itself or a `const` that holds it (`const N = S; const { createEffect: make } = N`). A module namespace object is frozen and a `const` is never rebound, so the slot holds that export for the life of the module.

Violations: `Destructured` (a creation), `Shorthand`, `WithRest` (a cleanup), `ExportedSlot` (a flush in a tracked effect), `HeldDestructured`.

Obligations: `LetSlot`, `DefaultSlot` (where the export is `undefined` the slot is the default's value), `NestedSlot` (`leaf-operation-identity-unproven`); `ComputedSlot`, `LetHolder`, `HeldNamespace` and `OwnObject` have no primitive attributed at the call and keep SC9012 `leaf-callback-opaque`.

No violation: `OwnedBody`, the same slot called in a component's body (the snapshot holds what the other rules say of an exported component).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/leaf-identity-const-namespace`. Premises: the closed program is the `private` manifest.
