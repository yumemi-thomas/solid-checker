// The re-check of #232, S3 (found probing the fix): an assignment pattern reads the prototype: `({ __proto__: P } = {})`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
let P: any;
({ __proto__: P } = {} as any);
Object.defineProperty(P, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
