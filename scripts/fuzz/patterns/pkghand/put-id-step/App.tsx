// ADR 0980, B6(a): the argument is a call's result. Chrome: FLUSH_IN_ACTION; main already reports it uncertifiable (a call's result is no name); kept as a ratchet.
import { flush } from "solid-js";
import { runStep } from "fz-step";
function put(t: any) { t.m = function () { flush(); }; }
function same(x: any) { return x; }
const o: any = {};
put(same(o));
runStep(() => { o.m(); });

export function App() {
  return <p>x</p>;
}
