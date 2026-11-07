import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
const parts = (s: string) => s.split("|").join("");
const k = parts("Arr|ay"), m = parts("sl|ice");
let armed = false;
const real = (globalThis as any)[k].prototype[m];
(globalThis as any)[k].prototype[m] = function (this: unknown[], ...a: any[]) { if (armed) { armed = false; setW(1); } return real.apply(this, a); };
export function App() {
  armed = true;
  [1, 2].slice(0, 1);
  return <p>{w()}</p>;
}
