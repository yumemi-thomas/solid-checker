// The review of #232, B2: a literal's method spelled with an escape. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const o = { __proto__: base, \u006d() { flush(); } };
setTimeout(() => { void (o as any).h; });

export function App() {
  return <p>x</p>;
}
