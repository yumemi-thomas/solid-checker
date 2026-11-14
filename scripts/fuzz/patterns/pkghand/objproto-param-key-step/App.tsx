// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): a key that is a parameter (`pick(k)` reads `({})[k]`, called with a key built from two strings).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
function pick(k: string): any { return ({} as any)[k]; }
Object.defineProperty(pick(["const", "ructor"].join("")).prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
