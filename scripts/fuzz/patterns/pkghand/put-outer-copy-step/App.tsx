// ADR 0980, B6(a): the argument is a parameter of the caller that holds a copy. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: any) { t.m = function () { flush(); }; }
function outer(h: any) { const c = h; put(c); }
const o: any = {};
outer(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
