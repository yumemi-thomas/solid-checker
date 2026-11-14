// The review of #232, B3: a parameter that is a pattern is no name for the receiver: `f({ m })` reads the key `m` where `f` runs. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function f({ m }: any) { m.call(null); }
const o: any = { get h(): number { runStep(() => { f(this); }); return 1; } };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
