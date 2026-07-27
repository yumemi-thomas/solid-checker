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
