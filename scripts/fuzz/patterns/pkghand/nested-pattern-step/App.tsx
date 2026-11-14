// The review of #232, B1: the nested literal of a confined holder, bound by a destructuring, is another name for an object whose uses nobody asked. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const hh = { sub: { m() { flush(); } } };
const { sub }: any = hh;
sub.__proto__ = base;
setTimeout(() => { void sub.h; });

export function App() {
  return <p>x</p>;
}
