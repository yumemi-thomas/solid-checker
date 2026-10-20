# rc9-heldout-execution-shapes

Five held-out-sweep shapes do not run in the component body, so they are not proven strict reads or owned-scope writes: a primitive inline callback (`flush`) in a callback prop, `latest()` in a helper running after an `await`, a default-parameter initializer, a function source of `merge()` (rc.9 wraps it in `createMemo`), a write in a listener handed to an unknown subscriber inside a memo compute; `latest()` and helper defaults called in the body stay proven. Declarations copy `@solidjs/signals@2.0.0-rc.9` `dist/types` verbatim.
