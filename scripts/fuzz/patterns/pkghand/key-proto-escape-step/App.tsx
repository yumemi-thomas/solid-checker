// The review of #232, B2 (found probing the fix): `__proto__` spelled with an escape (`o.__\u0070roto__ = base`) is the prototype write, whatever its text. Chrome: FLUSH_IN_ACTION.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = { get h(): number { runStep(() => { (this as any).m(); }); return 1; } };
const o: any = { m() { flush(); } };
o.__\u0070roto__ = base;
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
