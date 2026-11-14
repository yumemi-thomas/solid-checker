// The re-check of #232, S3 (found probing the fix): a key a variable holds in a program that spells `__proto__`: `const k = "__proto__"; ({})[k]`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
const k: string = "__proto__";
Object.defineProperty(({} as any)[k], "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
