// The review of #229, B2 (Q09): the same with `Object.create(base)` and a member assigned after. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const o = Object.create(base);
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
