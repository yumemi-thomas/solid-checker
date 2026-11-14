// The review of #232, B2: `1.0` is the key `1`, which `this["1"]` reads. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any)["1"](); }); return 1; } };
const o = { __proto__: base, 1.0() { flush(); } };
setTimeout(() => { void (o as any).h; });

export function App() {
  return <p>x</p>;
}
