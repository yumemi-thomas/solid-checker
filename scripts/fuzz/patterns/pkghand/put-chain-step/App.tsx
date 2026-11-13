// The neighbours of the review of #229, B2: The same with the argument the parameter of an outer function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: { m(): void }) { t.m = function () { flush(); }; }
function putOuter(h: { m(): void }) { put(h); }
const o = { m(): void {} };
putOuter(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
