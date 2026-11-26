import { createEffect, createRoot, createStore } from "solid-js";

// ADR 1230's control: the root's result is also handed to a helper, which may call it anywhere: the cleanup's read stays
// uncertifiable (ADR 0695's premise stands down).
function keep(f: () => void) {
  f();
}

export function HandlerEscaped() {
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
  keep(dispose);
  return (
    <button
      onClick={() => {
        dispose();
      }}
    />
  );
}
