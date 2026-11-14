// ADR 0980, probe a14: a comma expression. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
const a = (0, o);
a.m = function () { flush(); };
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
