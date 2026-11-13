// The review of #232, B5: a name an object pattern assignment rewrites (`({ s } = …)`) is no unique symbol. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
let s: any = Symbol();
({ s } = { s: "m" });
const o: any = { get h(): number { runStep(() => { this.m(); }); return 1; } };
o[s] = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
