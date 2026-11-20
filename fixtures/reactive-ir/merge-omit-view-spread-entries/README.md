# merge-omit-view-spread-entries

**Claim (ADR 1129, review F1).** The callers of a component whose `merge`/`omit` view of its props reaches an intrinsic spread are the references of every binding the function is entered through (ADR 1127, `write_roles::entry_bindings`): a named function expression's own name as well as the `const` holding it. A tag through the own name is a call site whose keys the spread may carry; any other reference (a call) leaves the spread unproved.

- Proved (no spread row): `Plain` (the own name is never referenced) and `Quiet` (the own name renders the component again with keys that carry no content key and no listener). The recursive tags keep their SC4001 `owner-callers-unenumerated` and SC6002 rows, which are not this proof's.
- Uncertifiable (SC8003 `spread-content-unproven`): `Row` (`<Inner nested innerHTML=… />` inside `const Row = function Inner`) and `Same` (`function Same` under `const Same`): the view is proved and its keys now hold the own-name site's `innerHTML`, beside the span's children; `Called` (`CalledInner({ … innerHTML })`, a plain call): no view is proved, so SC4001 `handler-spread-unresolved` stays too.
- Uncertifiable (SC4001 `handler-spread-unresolved`): `Act` (`<ActInner nested onClick={boom} />`: a listener that is no function literal).

On 379f2f79e (ADR 1129 round 0) the four refused spreads were clean: only the holding `const` was read. Stubs and pinned sources as in `merge-omit-view-spread`; `tsc --noEmit` accepts the fixture against the real rc.21 packages, and Chrome runs it (`scripts/runtime-oracle.mjs --cases fixtures/reactive-ir --only merge-omit-view-spread-entries`).
