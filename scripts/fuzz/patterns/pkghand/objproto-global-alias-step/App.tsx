// The re-check of #232, S3 (P4): the way to `Object.prototype` the rule did not follow, another name for `globalThis` (`const g = globalThis; g.Object.prototype`). Chrome: FLUSH_IN_ACTION; the head the re-check read (a59025c35) certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
const g: any = globalThis;
Object.defineProperty(g.Object.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
