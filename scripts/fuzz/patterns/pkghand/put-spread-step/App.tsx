// ADR 0980, B6(c): a spread stands before the index. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(first: any, t: any) { void first; t.m = function () { flush(); }; }
const o: any = {};
const args: [any, any] = [{}, o];
put(...args);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
