# dialect-solid-2

Half of a pair. `App.tsx` and `tsconfig.json` here are byte-identical to
`dialect-solid-1x/`; the coverage runner asserts it. The package declaration
is deliberately dialect-specific and preserves the real 2.0.0-rc.0
`createEffect` overloads. `node_modules/solid-js/package.json` selects 2.0.

Read the two snapshots side by side -- the diff between them is the whole
point, and it is the only automated evidence that the 1.x adapter does
anything.

ADR 0027 also uses this pair in `contracts_process`: native findings must be
identical with no catalog and with obsolete core catalog entries whose document
files do not exist, including aliased references to all three core packages.
`--check-contracts` must report `solid-js` as `builtin`, never as independently
certified and never as requiring a core receipt. The declaration stub selects
the reviewed dialect for semantic regression tests; it does not authenticate
installed runtime bytes.
