# dialect-solid-1x

Half of a pair. `App.tsx` and `tsconfig.json` here are byte-identical to
`dialect-solid-2/`; the coverage runner asserts it. The package declaration
is deliberately dialect-specific and preserves the real 1.9.14
`createEffect` overloads. `node_modules/solid-js/package.json` selects 1.x.

Read the two snapshots side by side -- the diff between them is the whole
point, and it is the only automated evidence that the 1.x adapter does
anything.
