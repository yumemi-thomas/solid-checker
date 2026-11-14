// The review of #232, B4: `Object.assign(this, …)` writes every key of its sources through `[[Set]]`, which runs a setter of the receiver. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { Object.assign(this, { k: 1 }); }); return 1; } };
const o = { __proto__: base, set k(v: number) { flush(); } };
setTimeout(() => { void (o as any).h; });

export function App() {
  return <p>x</p>;
}
