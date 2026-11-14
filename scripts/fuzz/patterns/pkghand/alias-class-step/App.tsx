// ADR 0980, probe a12: an instance of a project class, through an alias. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
class H { m: any; }
const o = new H();
const a = o;
a.m = function () { flush(); };
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
