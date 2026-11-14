// The review of #229, B2 (Q05): an object assigned after the literal (`o.sub = { m() { … } }`), the getter runs `this.sub.m()`. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = { get h(): number { runStep(() => { (this as any).sub.m(); }); return 1; }, sub: undefined as any };
o.sub = { m() { flush(); } };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
