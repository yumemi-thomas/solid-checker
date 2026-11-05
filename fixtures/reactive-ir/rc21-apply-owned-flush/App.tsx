import { createEffect, createSignal, flush } from "solid-js";
import { render } from "@solidjs/web";

export function App() {
  const [go, setGo] = createSignal(0, { ownedWrite: true });
  const [count, setCount] = createSignal(0);
  let button!: HTMLButtonElement;
  createEffect(go, (value) => {
    if (value) button.click();
  });
  function Later() {
    setGo(1);
    flush();
    return <i />;
  }
  setTimeout(() => render(() => <Later />, document.createElement("div")), 0);
  return <button ref={button} onClick={() => setCount(count() + 1)}>go</button>;
}
