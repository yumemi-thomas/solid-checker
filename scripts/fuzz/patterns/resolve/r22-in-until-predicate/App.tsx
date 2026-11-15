import { createSignal, resolve, until } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  setTimeout(() => {
    void until(() => { void resolve(p); return n() > 0; });
  });
  return <i>x</i>;
}
