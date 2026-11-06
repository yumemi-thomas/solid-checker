import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
const P = Promise.prototype;
P.then = function (this: any, f: any) { f(); return this; } as any;
export function App() {
  Promise.resolve().then(() => { setW(1); });
  return <p />;
}
