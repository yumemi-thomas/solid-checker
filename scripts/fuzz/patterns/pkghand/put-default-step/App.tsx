// ADR 0980, B6(c): a default value receives the store. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const shared: any = {};
function put(t: any = shared) { t.m = function () { flush(); }; }
put();
runStep(() => { shared.m(); });

export function App() {
  return <p>x</p>;
}
