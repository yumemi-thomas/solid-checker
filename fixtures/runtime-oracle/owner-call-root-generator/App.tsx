import { createEffect, createMemo, createRoot } from "solid-js";

// The review of #220 (F3): the callback of `createRoot` is a generator an effect's apply steps (the program of fixtures/reactive-ir/owner-call-root-generator). The body runs
// where the iterator is stepped, after the root's owner is gone: Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at the creation. The checker used to end it with
// `EffectCreationOwnerSetAround`.
export function RootGenerator() {
  const outer = createMemo(() =>
    createRoot(function* () {
      createMemo(() => 1);
      yield 1;
    }),
  );
  createEffect(
    () => outer(),
    (it) => {
      it.next();
    },
  );
  return <p>generator</p>;
}

// The control: a plain callback runs under the owner the root creates; Chrome is quiet.
export function RootPlain() {
  const outer = createMemo(() =>
    createRoot(() => {
      createMemo(() => 2);
      return 2;
    }),
  );
  return <p>{outer()}</p>;
}
