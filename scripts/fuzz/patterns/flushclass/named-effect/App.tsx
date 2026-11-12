import { createRoot, createEffect, createSignal, flush } from "solid-js";
const [n] = createSignal(0);
export function App() {
  setTimeout(function tick(again?: boolean) {
    flush();
    if (!again)
      createRoot(() => {
        createEffect(
          () => n(),
          () => {
            tick(true);
          },
        );
      });
  });
  return <p>x</p>;
}
