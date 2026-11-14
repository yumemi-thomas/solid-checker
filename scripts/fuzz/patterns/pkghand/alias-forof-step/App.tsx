// ADR 0980, a store through the variable of a loop over an array that holds the object. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
for (const x of [o]) { x.m = function () { flush(); }; }
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
