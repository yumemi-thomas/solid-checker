# store-copy-alias-write

ADR 1104 part 2: `const c = st.items.slice(); c[0].v = 5` writes the store through an element of a copy. The zero-argument
`slice()` has no call fact, so the store flow read the `const` alias as nothing and the write was clean, where Chrome rc.21 warns
STORE_WRITE_OUTSIDE_SETTER. A call no row describes is now read at its alias as the same call written at the sink is: the write is
uncertifiable (`unproven-container-root`), and the row at the `slice()` call stays.
Twins: a plain array's copy stays silent; the direct `st.items[0].v` and the inline `st.items.slice()[0].v` keep their rows.
