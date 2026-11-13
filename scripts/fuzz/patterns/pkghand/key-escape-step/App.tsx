// The review of #232, B2: a key spelled with a unicode escape is the key it decodes to; the store and the read meet. Chrome: FLUSH_IN_ACTION; the head the review read certified it with no finding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = { get h(): number { runStep(() => { this.m(); }); return 1; } };
o.\u006d = function () { flush(); };
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
