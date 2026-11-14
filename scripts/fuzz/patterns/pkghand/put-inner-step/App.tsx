// ADR 0980, B6(b): a copy of the parameter inside the function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: any) { const s = t; s.m = function () { flush(); }; }
const o: any = {};
put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
