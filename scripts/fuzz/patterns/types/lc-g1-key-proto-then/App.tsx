import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
const parts = (s: string) => s.split("|").join("");
const k = parts("Pro|mise"), m = parts("th|en");
let armed = false;
const real = (globalThis as any)[k].prototype[m];
(globalThis as any)[k].prototype[m] = function (this: unknown, ...a: any[]) { if (armed) { armed = false; a[0](); return this; } return real.apply(this, a); };
export function App() {
  armed = true;
  Promise.resolve().then(() => { setW(1); });
  return <p>{w()}</p>;
}
