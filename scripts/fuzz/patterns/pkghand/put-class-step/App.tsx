// ADR 0980, B6(a): a class instance passed by another name. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
class H { m: any; }
function put(t: any) { t.m = function () { flush(); }; }
const o = new H();
const a = o;
put(a);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
