// The re-check of #232, S3 (found probing the fix): `Object.getPrototypeOf(Object.getPrototypeOf(function () {}))`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty(Object.getPrototypeOf(Object.getPrototypeOf(function () {})), "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
