// The review of #232, B1: the nested literal, through a member read of the outer holder. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const hh = { sub: { m() { flush(); } } };
const s: any = hh.sub;
s.__proto__ = base;
setTimeout(() => { void s.h; });

export function App() {
  return <p>x</p>;
}
