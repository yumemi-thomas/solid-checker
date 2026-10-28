# store-write-chain-links-loose

`store-write-chain-links` without `strict` (ADR 0465). `store.v = 1` stays a violation. The writes through an optional or nullable link are uncertifiable here, not silent: without strictNullChecks the links' types drop `undefined` and `null`, and the facts record neither the option nor a property's optionality, so nothing says strict `tsc` would report TS18048/TS18047 on them. The residual is internal only (no violation) and listed in docs/open-work.md.
