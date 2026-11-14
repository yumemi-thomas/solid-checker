// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): the prototype read through a key built at run time (`({})["__pro" + "to__"]`).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty(({} as any)["__pro" + "to__"], "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
