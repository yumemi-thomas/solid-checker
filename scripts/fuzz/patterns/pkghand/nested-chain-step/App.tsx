// The review of #232, B1: the nested literal given the prototype through a chain of members, with no local. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const hh: any = { sub: { m() { flush(); } } };
hh.sub.__proto__ = base;
setTimeout(() => { void hh.sub.h; });

export function App() {
  return <p>x</p>;
}
