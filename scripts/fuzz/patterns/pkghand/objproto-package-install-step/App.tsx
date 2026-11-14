// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): the project hands the prototype (`__lookupGetter__("__proto__")`) to a package function that defines the getter: the read of `this.m` is the package's.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
import { install } from "fz-proto";
install(({} as any).__lookupGetter__("__proto__").call({}), runStep);

setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
