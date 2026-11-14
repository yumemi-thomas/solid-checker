// The review of #229, B2 (Q11): the member written by a project function (`put(o)`). Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = { get h(): number { runStep(() => { (this as any).m(); }); return 1; }, m: undefined as any };
function put(t: { m: any }) { t.m = function () { flush(); }; }
put(o);
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
