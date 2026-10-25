# spread-declared-keys-open

Claim (ADR 0394): a spread onto an intrinsic element is proven handler-free only when the compiler resolves its declared type to a complete key set with no listener key. Each spread here lacks that proof, so each keeps its `missing-owner` obligation (SC4001, uncertifiable, `handler-spread-unresolved`): a declared `onClick` key, `omit` that leaves `onClick`, an index signature, `any`, a union with an open member, generic values (a type parameter and its `omit`), and `{}`, an empty interface and `object`, which admit any non-nullish value and declare no key.

The twin `spread-declared-keys-closed` holds the spreads that are proven. The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
