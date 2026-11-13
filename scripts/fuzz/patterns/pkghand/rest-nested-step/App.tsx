// The review of #232, B1: a rest copy that carries a nested holder; the getter runs `this.sub.m()`. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base2 = { get h(): number { runStep(() => { (this as any).sub.m(); }); return 1; } };
const hh = { sub: { m() { flush(); } } };
const { ...rest }: any = hh;
rest.__proto__ = base2;
setTimeout(() => { void rest.h; });

export function App() {
  return <p>x</p>;
}
