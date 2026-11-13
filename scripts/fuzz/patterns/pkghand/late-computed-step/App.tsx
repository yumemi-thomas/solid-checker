// The review of #229, B2 (a neighbour of Q01): the receiver read through a key the program does not name (`this[key]`). Chrome: FLUSH_IN_ACTION; main certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const key = "m";
const o = { get h(): number { runStep(() => { (this as any)[key](); }); return 1; }, m: undefined as any };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
