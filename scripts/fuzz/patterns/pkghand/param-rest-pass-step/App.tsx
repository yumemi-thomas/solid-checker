// The review of #232, B3: a rest parameter that passes the receiver on. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function f(p: any) { p.m(); }
function g2(...xs: any[]) { f(xs[0]); }
const o: any = { get h(): number { runStep(() => { g2(this); }); return 1; } };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
