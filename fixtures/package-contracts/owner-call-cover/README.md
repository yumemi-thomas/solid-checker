# owner-call-cover

ADR 0173: alternative calls of one owner role that together run on every
normal completion register on every call. The generator proposes the
`min: 1` bound from the host-folded emission source; the census proves the
same cover from the producer's facts or withdraws it by name.

The `solid-js` stub is byte-faithful to `@solidjs/signals@2.0.0-rc.9` for
every declaration it carries (see its header).

- `listen` is `createEventListener`'s shape: `createEffect` in one arm,
  `createRenderEffect` in the other. Its computation is published with
  `min: 1`.
- `maybeListen` has no `else`: `min: 0`.
- `effectOrCleanup` registers a computation in one arm and a cleanup in the
  other. Every call registers something, but neither role on every call:
  both items stay `min: 0`.
- `guardedListen` can return before either arm: `min: 0`.
