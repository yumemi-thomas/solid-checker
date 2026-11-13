// The review of #229, B1 (P03): the method that names `this` reaches the literal through an alias a destructuring makes, and the member it is called through resolves to no one function. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const t = { a(this: any) { runStep(() => { this.m(); }); } };
const { a: h } = t;
const o = { m() { flush(); }, h };
setTimeout(() => { o.h(); });

export function App() {
  return <p>x</p>;
}
