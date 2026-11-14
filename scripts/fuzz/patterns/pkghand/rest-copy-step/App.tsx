// The review of #232, B1: a rest copy of a holder is a holder: it holds the same functions under the same keys, and given the prototype it reads the getter that runs them through `this`. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const o = { m() { flush(); } };
const { ...rest }: any = o;
rest.__proto__ = base;
setTimeout(() => { void rest.h; });

export function App() {
  return <p>x</p>;
}
