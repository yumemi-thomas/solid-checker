# Exact fresh constructor population (draft)

The Set constructor iterates values; Map additionally spreads yielded entries.
The corresponding signature is copied from the real next.2 declarations.
Their callbacks census is deliberately open, including nested entry iteration.
ArraySetLeaf and ArrayEntryMapLeaf should still prove leaf-owner-forbidden-call
at the exact member invocation. LiteralTracked/NullPopulation add no prototype
finding. DynamicSet, DynamicMapEntry, AliasInput and SpreadInput must withhold
the instance proof as SC9012 obligations, never an alleged leaf violation.

SC9005 constructor/import obligations may remain: this fixture tests the member
proof, not complete iterable/result protocol closure. It carries its own exact
manifest bytes, authorization instruction and Solid 2 dialect stubs.
Expected findings are hand-stated, with no checker snapshot or runtime result.
