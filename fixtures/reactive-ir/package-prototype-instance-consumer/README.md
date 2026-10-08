# Exact constructor instance prototypes (draft)

Hand-stated contract consumer for a new `prototype-instance` recipe. The
authorize-contract.json shape and package manifest bytes come from
package-own-tracked-read-consumer; only export identities and semantics change.
The fictional package models zero-argument constructors so its empty constructor
callbacks census is sound. Real package constructors retain iterable obligations
in the separate replacement specs. Read method/getter/iterator signatures are
copied from the installed next.2 typings; the constructor is deliberately narrower.
The Solid effect callback return signature is copied from rc.13.

expected-cases.json states semantic expectations, not observed snapshots.
LeafHas/LeafSize/LeafResume must prove the leaf-owner violation under a present
observer. Tracked JSX reads are clean. A discarded generator is not resumed;
no-observer has/size calls return plain values even if the cache had been primed.
NestedMemo tests that the outer leaf does not own the nested has read (creating
that memo still independently violates the existing leaf rule).

Arrays, shorthand, aliases, casts, passing, returning, exports, JSX values,
mutations, computed members, stored iterators and wrapped callees keep obligations.
ShadowedConstructor is native Set behavior and must never pick the import's recipe.
prototype-mutation is a separate project, to keep its global constructor mutation
from invalidating the main project's exact constructor premise.

No checker or runtime probe was executed. A landing agent must authorize the
fixture copy out of band, compare findings before updating its focused snapshot,
and add coverage/ownership checks with a fresh pinned binary.
