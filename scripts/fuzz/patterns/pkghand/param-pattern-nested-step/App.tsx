// The review of #232, B1 (found probing the fix): a parameter that is a pattern binds the nested literal of the object it is given. Chrome: FLUSH_IN_ACTION; the head the review read already handed it (uncertifiable), so it stays fine.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
function f({ sub }: any) { sub.__proto__ = base; setTimeout(() => { void sub.h; }); }
f({ sub: { m() { flush(); } } });

export function App() {
  return <p>x</p>;
}
