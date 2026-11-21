import { createEffect, createRoot, createStore } from "solid-js";

// ADR 1167's premise: a `createRoot` that takes its `dispose` lets project code dispose an owner on a stack it chooses, so the
// cleanup's read is placed nowhere (ADR 0695): uncertifiable. (Chrome rc.21 is quiet here: the body's call does not warn.)
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
