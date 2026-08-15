import { createEffect, createMemo, createRoot, createSignal, createTrackedEffect, flush, onCleanup, onSettled } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  createMemo(() => count());

  onSettled(() => {
    onCleanup(() => {});
    createSignal(1);
    createSignal(() => count());
    flush();
  });

  createTrackedEffect(() => {
    onCleanup(() => {});
    createMemo(() => count());
    createRoot(() => {});
    flush();
  });

  onSettled(() => {
    console.log("settled");
    onCleanup(() => console.log("disposed"));
  });

  onSettled(() => 42);
  createTrackedEffect(() => "invalid");
  createEffect(() => count(), () => ({ invalid: true }));
  onSettled(async () => {});
  createEffect(() => count(), async () => {});

  onSettled(() => undefined);
  createTrackedEffect(() => () => console.log("valid"));
  onSettled(() => {
    if (count()) return 99;
    return () => console.log("valid branch");
  });
  const cleanup = () => console.log("cleanup");
  onSettled(() => {
    return cleanup;
  });

  return <div>{count()}</div>;
}

// Out-of-band onSettled: called from an event handler, the callback is
// enqueued as a plain function, not a leaf owner (rc.0 dev.js:4855-4893) —
// onCleanup only warns no-owner-cleanup, primitives attach nowhere without
// throwing, and flush() is a silent no-op, so no SC3xxx fires here.
export function OutOfBand() {
  const [count] = createSignal(0);
  return (
    <button
      onClick={() =>
        onSettled(() => {
          onCleanup(() => {});
          createMemo(() => count());
          flush();
        })
      }
    >
      {count()}
    </button>
  );
}

// An exported helper's callers are unknowable: owner-backed the onCleanup
// throws, out-of-band it only warns — so the leaf-owner finding is an
// uncertifiable proof obligation, not a proven violation.
export function settleWithCleanup() {
  onSettled(() => {
    onCleanup(() => {});
  });
}
