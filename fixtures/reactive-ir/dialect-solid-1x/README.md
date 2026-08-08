# dialect-solid-1x

Half of a pair. `App.tsx` and `solid-js.d.ts` here are byte-identical to
`dialect-solid-2/`; the coverage runner asserts it. The only
difference is `node_modules/solid-js/package.json`, which says `1.9.14`,
and that is what makes the checker pick a dialect.

Read the two snapshots side by side -- the diff between them is the whole
point, and it is the only automated evidence that the 1.x adapter does
anything.
