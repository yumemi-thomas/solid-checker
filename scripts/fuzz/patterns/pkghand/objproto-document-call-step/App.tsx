// The re-check of #232, second round (found probing the first round's fix; Chrome: FLUSH_IN_ACTION, the fix of the first round certified it with no finding): the window through a call that returns `document`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o: any = {};
o.m = function () { flush(); };
function getDoc(): any { return document; }
Object.defineProperty(getDoc().defaultView.Object.prototype, "h", { get() { runStep(() => { this.m(); }); return 1; }, configurable: true });
setTimeout(() => { void o.h; });

export function App() {
  return <p>x</p>;
}
