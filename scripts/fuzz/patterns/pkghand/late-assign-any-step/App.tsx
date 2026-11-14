// The review of #229, B2 (P26): the same on an `any` holder. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = { get h(): number { runStep(() => { this.m(); }); return 1; } };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
