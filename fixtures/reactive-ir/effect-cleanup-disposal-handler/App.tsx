import { createEffect, createRoot, createStore } from "solid-js";

// ADR 1230: the root's `dispose` is handed back only as `return d;` and called only from a button's inline `onClick`, so every
// disposal the project can make runs from an event handler, where no strict-read window is open: the never-rerunning effect's
// returned cleanup reads outside any window (ADR 1167): SC1001 is clean.
export function HandlerDisposed() {
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
  return (
    <button
      onClick={() => {
        dispose();
      }}
    />
  );
}
