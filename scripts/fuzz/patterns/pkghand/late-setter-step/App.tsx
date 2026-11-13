// The review of #229, B2 (Q04): the same read by a setter (`o.h = 1`). Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = { set h(v: number) { runStep(() => { (this as any).m(); }); }, m: undefined as any };
o.m = function () { flush(); };
setTimeout(() => { o.h = 1; });

export function App() {
  return <p>x</p>;
}
