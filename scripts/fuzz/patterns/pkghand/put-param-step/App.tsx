// The neighbours of the review of #229, B2: A function stored through a parameter's member is stored into the object the callers pass; the holder is read by the caller's name (no receiver in the program). Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: { m(): void }) { t.m = function () { flush(); }; }
const o = { m(): void {} };
put(o);
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
