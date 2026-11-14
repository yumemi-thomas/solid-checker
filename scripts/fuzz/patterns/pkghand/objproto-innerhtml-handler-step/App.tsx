// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): `innerHTML` markup with an `onerror` handler defines the getter.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;
const host = document.createElement("div");
host.innerHTML = `<img src="data:," onerror="Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })">`;
document.body.append(host);

setTimeout(() => { void o.h; }, 20);
export function App() {
  return <p>x</p>;
}
