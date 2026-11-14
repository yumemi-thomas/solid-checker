// The re-check of #232, S3 (P1): the way to `Object.prototype` the rule did not follow, `let O; O = Object`: a name assigned, not initialized. Chrome: FLUSH_IN_ACTION; the head the re-check read (a59025c35) certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
let O: any;
O = Object;
Object.defineProperty(O.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
