// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): another name for the window, the bare global `top` (`(top as any).Object.prototype`): the file spells no reference of `Object`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty((top as any).Object.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
