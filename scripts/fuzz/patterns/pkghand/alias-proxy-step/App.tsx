// ADR 0980, probe a18: a proxy of the object. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
const a: any = new Proxy(o, {});
a.m = function () { flush(); };
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
