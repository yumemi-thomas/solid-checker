import { createRoot, onCleanup, onSettled } from "solid-js";

// The forbidden call runs inside IIFEs the leaf callback runs in place: rc.21
// raises the code from the call's own line, so the violation is placed there.
export function Settled() {
  onSettled(() => {
    (() => {
      createRoot(() => 1);
    })();
  });
  return <i />;
}
export function Nested() {
  onSettled(() => {
    (() => {
      (() => {
        onCleanup(() => {});
      })();
    })();
  });
  return <i />;
}
