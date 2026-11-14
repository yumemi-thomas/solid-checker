// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): a package exports `Object.prototype` and the project defines the getter on it.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
import { proto } from "fz-proto";
Object.defineProperty(proto, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
