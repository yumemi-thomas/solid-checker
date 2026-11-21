import { createEffect, createRoot, createStore } from "solid-js";

// ADR 1167's counterexample: a `createRoot` that takes its `dispose` lets project code dispose the owner on a stack it chooses
// (here the body, inside its strict-read window), so the cleanup's read is placed nowhere (ADR 0695): uncertifiable.
export function DisposedInBody() {
  const [s] = createStore({ v: 0 });
  const dispose = createRoot((d) => {
    createEffect(
      () => 1,
      () => () => {
        void s.v;
      },
    );
    return d;
  });
  dispose();
  return <div />;
}
