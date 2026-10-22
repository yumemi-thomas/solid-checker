# rc13-flush-in-effect-two-copies

Two installed copies of `@solidjs/signals` (a nested one under `solid-js`, which its `createEffect` runs on, and a top-level one the project imports `flush` from) have two queues; a flush of the other copy's queue is not running while this effect's apply runs, so nothing here is claimed. SC2008 requires the `flush` callee and the `createEffect` call to resolve to the same package instance.

Today the top-level `flush` import is also not resolved to a declaration by Type Facts, so the silence has two guards; the package-directory comparison (`effect_phase::package_root`, unit-tested) is the one that holds when it is.
