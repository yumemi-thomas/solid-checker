// ADR 0750: static_api runs_unplaced_through_callers, four helpers: a flush fifteen helpers below an action body.
import { action, createEffect, createSignal, flush } from "solid-js";
function s15(): void { flush(); }
function s14(): void { s15(); }
function s13(): void { s14(); }
function s12(): void { s13(); }
function s11(): void { s12(); }
function s10(): void { s11(); }
function s9(): void { s10(); }
function s8(): void { s9(); }
function s7(): void { s8(); }
function s6(): void { s7(); }
function s5(): void { s6(); }
function s4(): void { s5(); }
function s3(): void { s4(); }
function s2(): void { s3(); }
function s1(): void { s2(); }
const act = action(function* () { s1(); yield; });
export function Stepped() {
  const [a] = createSignal(1);
  createEffect(a, () => {});
  return <button onClick={() => act()}>{a()}</button>;
}
