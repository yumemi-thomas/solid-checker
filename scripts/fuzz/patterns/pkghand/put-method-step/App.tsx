// ADR 0980, B6(d): the function is a method of an object literal. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const api = { put(t: any) { t.m = function () { flush(); }; } };
const o: any = {};
api.put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
