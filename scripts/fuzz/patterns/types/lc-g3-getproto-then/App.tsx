import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
let armed = false;
const proto = Object.getPrototypeOf(Promise.resolve());
const real = proto.then;
proto.then = function (this: unknown, ...a: any[]) { if (armed) { armed = false; a[0](); return this; } return real.apply(this, a); };
export function App() {
  armed = true;
  Promise.resolve().then(() => { setW(1); });
  return <p>{w()}</p>;
}
