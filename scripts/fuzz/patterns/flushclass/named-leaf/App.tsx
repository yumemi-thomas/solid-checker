import { createRoot, createSignal, createTrackedEffect, flush } from "solid-js";
const [n] = createSignal(0);
export function App() {
  setTimeout(function tick(again?: boolean) {
    flush();
    if (!again)
      createRoot(() => {
        createTrackedEffect(() => {
          n();
          tick(true);
        });
      });
  });
  return <p>x</p>;
}
