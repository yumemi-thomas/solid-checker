// ADR 0980, B6(a) of the review of #232: the argument is another name of the object. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: any) { t.m = function () { flush(); }; }
const o: any = {};
const a = o;
put(a);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
