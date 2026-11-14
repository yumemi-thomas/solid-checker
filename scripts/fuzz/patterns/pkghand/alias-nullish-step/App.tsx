// ADR 0980, probe a16: a nullish expression. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
const a: any = o ?? null;
a.m = function () { flush(); };
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
