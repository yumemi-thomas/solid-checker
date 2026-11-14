// The review of #232, B1 (found probing the fix): a spread copy of a holder shares the nested literal, so the copy's uses are the nested literal's uses. Chrome: FLUSH_IN_ACTION.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const hh = { sub: { m() { flush(); } } };
const N: any = { ...hh };
N.sub.__proto__ = base;
setTimeout(() => { void N.sub.h; });

export function App() {
  return <p>x</p>;
}
