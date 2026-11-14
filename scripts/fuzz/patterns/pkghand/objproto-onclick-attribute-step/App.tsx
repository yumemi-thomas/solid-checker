// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): an `onclick` attribute set with `setAttribute` and a `.click()` define the getter.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;
const b = document.createElement("div");
b.setAttribute("onclick", "Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })");
b.click();

setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
