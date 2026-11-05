import { createEffect, createSignal, createStore } from "solid-js";
import { render } from "@solidjs/web";


export function Effects() {
  const [store] = createStore({ v: 1 });
  const [count, setCount] = createSignal(0);
  createEffect(count, () => {
    setCount(5);
  });
  setTimeout(() => {
    render(() => <span />, document.createElement("div"));
  }, 0);
  void (async () => {
    await null;
    render(() => <span />, document.createElement("div"));
  })();
  return <button onClick={() => render(() => <span />, document.createElement("div"))}>{store.v}</button>;
}

render(() => <Effects />, document.body);
