// The review of #229, B2 (P10): a prototype written through a computed key no file spells (`"__pro" + "to__"`; ADR 0643's premise), a getter of the literal reads it through `this`. Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get g(): number { flush(); return 1; } };
const o: any = { get h(): number { runStep(() => { void this.g; }); return 1; } };
const k = "__pro" + "to__";
o[k] = base;
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
