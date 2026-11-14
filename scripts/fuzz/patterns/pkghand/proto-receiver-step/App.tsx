// The review of #229, B2 (Q08): G1 mirrored: the receiver is written on the prototype, the flushing member on the derived literal, which writes no receiver. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const o = { __proto__: base, m() { flush(); } };
setTimeout(() => { void (o as any).h; });

export function App() {
  return <p>x</p>;
}
