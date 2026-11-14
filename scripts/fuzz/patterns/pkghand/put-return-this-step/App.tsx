// The neighbours of the review of #229, B2: A class method that returns `this`, an instance given a member by a project function, the member read through the call's result. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
class B { add() { return this; } m: any; }
function put(t: { m: any }) { t.m = function () { flush(); }; }
const o = new B();
put(o);
runStep(() => { o.add().m(); });

export function App() {
  return <p>x</p>;
}
