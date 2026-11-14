// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): code a string timer runs defines the getter (`setTimeout("…", 0)`).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;

setTimeout("Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })", 0);
setTimeout(() => { void o.h; }, 20);
export function App() {
  return <p>x</p>;
}
