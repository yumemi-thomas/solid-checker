# store-link-alias-get

**Claim (ADR 1169).** A Get through a store link held whole in a `const` (`const held = s1.a; void held.v`) reads the store in the
component body, where rc.21 warns STRICT_READ_UNTRACKED. Main had no row of any rule there.
- Part 1: the value-use census walks the alias, so the Get has an SC9012 `value-held-in-binding` row.
- Part 2: the alias is proven the link's proxy, so the Get is an SC1001 violation, as its member twin `s1.a.v` (`MemberGet`).

Chrome: `fixtures/runtime-oracle/store-link-alias-get`. Stub: `store-write-through-prop`'s.
