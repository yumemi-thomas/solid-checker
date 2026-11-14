// The review of #229, B2 (Q10): the same with a class: the receiver is read by the class's getter, the member is assigned to an instance. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
class B { m: any; get h(): number { runStep(() => { this.m(); }); return 1; } }
const o = new B();
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
