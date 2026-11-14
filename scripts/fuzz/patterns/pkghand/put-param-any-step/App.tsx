// The neighbours of the review of #229, B2: The same with an `any` parameter and holder. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: any) { t.m = function () { flush(); }; }
const o: any = {};
put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
