// The re-check of #232, S3 (P2): the way to `Object.prototype` the rule did not follow, `Object` passed to a parameter. Chrome: FLUSH_IN_ACTION; the head the re-check read (a59025c35) certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
function g(O: any) { Object.defineProperty(O.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true }); }
g(Object);
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
