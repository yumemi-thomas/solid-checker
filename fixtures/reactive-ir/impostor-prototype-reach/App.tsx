import { createMemo, createSignal, onCleanup } from "solid-js";

let gp: any = {};
gp = Object.getPrototypeOf(Promise.resolve());
gp.then = function (this: any, f: any) { f(); return this; };

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    Promise.resolve().then(() => { set(1); });
    return s();
  });
  Promise.resolve().then(() => { onCleanup(() => {}); });
  return <p>{m()}</p>;
}
