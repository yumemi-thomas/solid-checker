# dialect-solid-2

Half of a pair. `App.tsx` and `tsconfig.json` here are byte-identical to
`dialect-solid-1x/`; the coverage runner asserts it. The package declaration
is deliberately dialect-specific and preserves the real 2.0.0-rc.0
`createEffect` overloads. `node_modules/solid-js/package.json` selects 2.0.

Read the two snapshots side by side -- the diff between them is the whole
point, and it is the only automated evidence that the 1.x adapter does
anything.
