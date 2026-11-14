// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): a package function the project calls writes `Object.prototype` (a write in a function of a reached file counts, called or not).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
import { setup } from "fz-proto";
setup();

setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
