// The re-check of #232, S3 (P14): the way to `Object.prototype` the rule did not follow, `Object.prototype.valueOf()` returns the prototype itself: a built-in read is exempt only when it returns no object. Chrome: FLUSH_IN_ACTION; the head the re-check read (a59025c35) certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
Object.defineProperty(Object.prototype.valueOf(), "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
