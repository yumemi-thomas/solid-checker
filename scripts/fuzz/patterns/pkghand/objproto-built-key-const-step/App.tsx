// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): the key built at run time kept in a `const`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
const k = "const" + "ructor";
Object.defineProperty(({} as any)[k].prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
