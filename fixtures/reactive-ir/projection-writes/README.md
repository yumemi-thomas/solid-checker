# SC2003: a write to a `createProjection` result

A projection (and the store half of a function-form `createStore(fn, seed)`) is derived: a write through its result outside the compute function is dropped (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:5690-5700`, the same `set` trap as any store proxy outside a write scope). The finding names the projection and says to derive the value in its compute function.

Silent, probed on rc.17: a write through the draft parameter, and a write through the result symbol lexically inside the compute function (`runProjectionComputed` runs it inside `storeSetter(draft, …)`, `dist/dev.js:6675-6678`, `:5847-5866`), which commits after `flush()`.
