// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): a `<script>` element's text, appended to the document, defines the getter.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;
const s = document.createElement("script");
s.textContent = "Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })";
document.head.append(s);

setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
