// Research B's probe of #232 (second round; Chrome: FLUSH_IN_ACTION, the build before the fix certified it with no finding): `import()` of a `data:` URL built at run time defines the getter.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
(globalThis as any).rs = runStep;
const code = "Object.defineProperty(Object.prototype, 'h', { get() { rs(() => { this.m(); }); return 1; }, configurable: true })";
await import(/* @vite-ignore */ "data:text/javascript," + encodeURIComponent(code));

setTimeout(() => { void o.h; });
export function App() {
  return <p>x</p>;
}
