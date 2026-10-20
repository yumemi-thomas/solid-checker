import { createMemo, createRoot, createSignal, onCleanup } from "solid-js";
export function App() {
  const [n] = createSignal(1);
  return (
    <div
      ref={el => {
        createRoot(dispose => {
          const label = createMemo(() => `n${n()}`);
          onCleanup(() => undefined);
          el.id = "w2";
          void label;
          dispose();
        });
      }}
    >
      w2
    </div>
  );
}
