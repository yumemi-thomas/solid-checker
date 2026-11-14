// The review of #232, B3: an array pattern runs the receiver's iterator. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function f([a]: any) {}
const base = { get h(): number { runStep(() => { f(this); }); return 1; } };
const o = { __proto__: base, *[Symbol.iterator]() { flush(); } };
setTimeout(() => { void (o as any).h; });

export function App() {
  return <p>x</p>;
}
