// The review of #232, B3: a rest parameter is an array that holds the receiver. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function f(...xs: any[]) { xs[0].m(); }
const o: any = { get h(): number { runStep(() => { f(this); }); return 1; } };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
