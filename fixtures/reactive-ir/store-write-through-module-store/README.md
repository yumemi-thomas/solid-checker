# store-write-through-module-store

**Claim (ADR 1232).** ADR 0491's census of a module store refused any export. An exported store (`stores.ts`) is now followed into
every file that references it (`exported_store_contained`): each reference is carried there (ADR 1231's `carried_store_reads`) to
uses that are contained against the store's own literal (`contained_store_use_in`), and its setter has no reference in another
file. A direct write to the imported store is proved in its own file (`imported_links_wrappable`); one through a local alias takes
ADR 0919's handed route (`handed_links_wrappable`, cross-file since ADR 1232).

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-through-module-store`): `Direct`, `Alias` (an array
mutator through a `const` alias). On main they are uncertifiable.

Uncertifiable: `Kept` (the store is handed to a function in another file), `SetterElsewhere` (the setter is referenced in another
file).

Stub: `store-write-through-carrier`'s.
