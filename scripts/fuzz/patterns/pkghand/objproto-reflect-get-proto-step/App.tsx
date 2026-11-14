// The re-check of #232, S3 (found probing the fix): a key a string spells: `Reflect.get({}, "__proto__")`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty(Reflect.get({}, "__proto__"), "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
