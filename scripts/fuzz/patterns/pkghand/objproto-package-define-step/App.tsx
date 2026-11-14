// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): a package the project only imports with `import()` writes `Object.prototype` (a getter that calls `this.m()`): the project spells no route.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
import("fz-proto");

setTimeout(() => { void o.h; }, 20);
export function App() {
  return <p>x</p>;
}
