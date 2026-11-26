# store-write-through-module-store-open

**Claim (ADR 1232).** `store-write-through-module-store`'s `Direct` in an open program (a manifest with `main`, not `private`): an
export may be imported by code the project does not hold, so `exported_store_contained` needs `program_closed` and the write stays
uncertifiable.
