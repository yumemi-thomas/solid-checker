import { createSignal, resolve, until } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  setTimeout(() => {
    void resolve(() => { void resolve(p); return n(); });
  });
  return <i>x</i>;
}
