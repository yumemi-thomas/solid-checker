# prop-alias-link-get

**Claim (ADR 1172 part 1).** A store handed by prop and held in a `const` alias (`const held = props.s1`): the Get `held.a` inside a
deeper write (`held.a.v = 9`, `WriteChild`) had no row on main, though rc.21 warns STRICT_READ_UNTRACKED. The value-use census took
the write's SC2003 row for the whole chain; a write's row answers its own Set only, so the Get keeps an SC9012
`value-held-in-binding` row, as the Get alone does (`ReadChild`). `RootWriteChild` (`held.items = [1]`, a Set with no Get of a link)
is unchanged.

The Get stays uncertifiable even where ADR 1169 proves a local alias: its direct twin `props.s1.a` is SC1001 uncertifiable
(`read-backing-unproven`) too. Chrome: `fixtures/runtime-oracle/prop-alias-link-get`. Stub: `store-write-through-prop`'s.
