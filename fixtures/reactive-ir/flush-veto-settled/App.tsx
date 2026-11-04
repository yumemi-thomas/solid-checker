import { createEffect, createRoot, createSignal, flush, onSettled } from "solid-js";

// A handler's `onSettled` queues its callback, and a root's `flush()` drains it under the root:
// the effect there is owned (no NO_OWNER_EFFECT) and the write throws (REACTIVE_WRITE_IN_OWNED_SCOPE).
export function Settled() {
  const [n, setN] = createSignal(0);
  return (
    <button
      onClick={() => {
        onSettled(() => {
          createEffect(
            () => 0,
            () => {},
          );
          setN(1);
        });
        createRoot(() => {
          flush();
        });
      }}
    >
      {n()}
    </button>
  );
}

// The same through layers run in place in the queued callback.
export function SettledLayers() {
  const [n, setN] = createSignal(0);
  return (
    <button
      onClick={() => {
        onSettled(() => {
          (() => {
            setN(2);
          })();
          const later = () => {
            setN(3);
          };
          later();
        });
        createRoot(() => {
          flush();
        });
      }}
    >
      {n()}
    </button>
  );
}
