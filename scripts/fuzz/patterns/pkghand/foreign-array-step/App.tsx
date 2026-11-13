// The review of #229, B1 (P04): the method that names `this` reaches the literal through an array's element, and the member it is called through resolves to no one function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const fns = [function (this: any) { runStep(() => { this.m(); }); }];
const o = { m() { flush(); }, g: fns[0] };
setTimeout(() => { o.g(); });

export function App() {
  return <p>x</p>;
}
