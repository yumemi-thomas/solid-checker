// The re-check of #232, S3 (found probing the fix): a pattern binds the prototype: `const { __proto__: P } = {}`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
const { __proto__: P }: any = {};
Object.defineProperty(P, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
