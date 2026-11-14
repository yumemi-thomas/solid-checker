// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): an `onerror` attribute set with `setAttribute` defines the getter when the image fails.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;
const img = document.createElement("img");
img.setAttribute("onerror", "Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })");
img.src = "data:,x";
document.body.append(img);

setTimeout(() => { void o.h; }, 20);
export function App() {
  return <p>x</p>;
}
