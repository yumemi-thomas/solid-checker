import { createEffect, createSignal } from "solid-js";

export function App() {
  const [go] = createSignal(0);
  const [count, setCount] = createSignal(0);
  let button!: HTMLButtonElement;
  createEffect(go, (value) => {
    if (value) button.click();
  });
  return <button ref={button} onClick={() => setCount(count() + 1)}>go</button>;
}
