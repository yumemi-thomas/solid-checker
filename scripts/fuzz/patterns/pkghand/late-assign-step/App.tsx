// The review of #229, B2 (Q01): a member assigned after the literal, read by the literal's getter through `this` in a step a package runs. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = { get h(): number { runStep(() => { (this as any).m(); }); return 1; }, m: undefined as any };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
