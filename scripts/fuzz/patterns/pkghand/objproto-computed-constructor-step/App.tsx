// The re-check of #232, S3 (found probing the fix): a computed literal key spells the same route: `({})["constructor"]["prototype"]`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty(({} as any)["constructor"]["prototype"], "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
