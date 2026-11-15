import { createSignal, resolve, until, untrack } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  setTimeout(() => {
    void resolve(() => untrack(() => { void until(p); return n(); }));
  });
  return <i>x</i>;
}
