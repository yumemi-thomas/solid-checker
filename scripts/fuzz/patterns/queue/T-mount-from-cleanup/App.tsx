import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
function Child() {
  const [c] = createSignal(1);
  console.log(c());
  return <i />;
}
export function App() {
  const [c, setC] = createSignal(0);
  createEffect(c, () => {
    onCleanup(() => { render(() => <Child />, document.createElement("div")); });
  });
  return <button onClick={() => setC(1)}>go</button>;
}
export async function after() { document.querySelector("button")!.click(); }
