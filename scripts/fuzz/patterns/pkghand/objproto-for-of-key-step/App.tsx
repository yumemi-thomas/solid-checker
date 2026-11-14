// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): a `for of` variable as the key (`for (const k of ["constructor"]) C = ({})[k]`).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
let C: any; for (const k of ["constructor"]) { C = ({} as any)[k]; }
Object.defineProperty(C.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
