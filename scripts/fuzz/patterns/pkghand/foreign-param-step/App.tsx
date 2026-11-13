// The review of #229, B1 (P01): the method that names `this` reaches the literal through a parameter, and the member it is called through resolves to no one function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function mk(f: any) { return { m() { flush(); }, g: f }; }
const o = mk(function (this: any) { runStep(() => { this.m(); }); });
setTimeout(() => { o.g(); });

export function App() {
  return <p>x</p>;
}
