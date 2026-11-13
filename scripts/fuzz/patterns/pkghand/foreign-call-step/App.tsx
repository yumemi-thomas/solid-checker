// The review of #229, B1 (P02): the method that names `this` reaches the literal through a call's result, and the member it is called through resolves to no one function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function makeG() { return function (this: any) { runStep(() => { this.m(); }); }; }
const o = { m() { flush(); }, g: makeG() };
setTimeout(() => { o.g(); });

export function App() {
  return <p>x</p>;
}
