// The review of #232, B2: the read spelled with an escape. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = { get h(): number { runStep(() => { this.\u006d(); }); return 1; } };
o.m = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
