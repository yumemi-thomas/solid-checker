// The review of #232, B1: the nested literal in an argument of a function the build reads. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
function f(p: any) { p.sub.__proto__ = base; setTimeout(() => { void p.sub.h; }); }
f({ sub: { m() { flush(); } } });

export function App() {
  return <p>x</p>;
}
