// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): a package the project imports imports the package that writes `Object.prototype`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
import "fz-outer";

setTimeout(() => { void o.h; }, 20);
export function App() {
  return <p>x</p>;
}
