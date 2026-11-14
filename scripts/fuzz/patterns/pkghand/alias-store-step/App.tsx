// ADR 0980, S2 of the review of #232: the store goes through a second name of the object. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
const a = o;
a.m = function () { flush(); };
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
