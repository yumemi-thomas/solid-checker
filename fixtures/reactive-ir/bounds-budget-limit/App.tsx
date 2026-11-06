// ADR 0752: the member-invocation forwarding loop stops after 8 rounds; an owner still growing there, and every
// owner forwarding to it, gets an unmodeled use: the call from the apply is an obligation, never clean.
import { createEffect, createSignal } from "solid-js";
function fa3(o: { read: () => number }): void { o.read(); }
function fa2(o: { read: () => number }): void { fa3(o); }
function fa1(o: { read: () => number }): void { fa2(o); }
function fb12(o: { read: () => number }): void { o.read(); }
function fb11(o: { read: () => number }): void { fb12(o); }
function fb10(o: { read: () => number }): void { fb11(o); }
function fb9(o: { read: () => number }): void { fb10(o); }
function fb8(o: { read: () => number }): void { fb9(o); }
function fb7(o: { read: () => number }): void { fb8(o); }
function fb6(o: { read: () => number }): void { fb7(o); }
function fb5(o: { read: () => number }): void { fb6(o); }
function fb4(o: { read: () => number }): void { fb5(o); }
function fb3(o: { read: () => number }): void { fb4(o); }
function fb2(o: { read: () => number }): void { fb3(o); }
function fb1(o: { read: () => number }): void { fb2(o); }
export function Forwarded() {
  const [a] = createSignal(1);
  createEffect(() => 1, () => { fa1({ read: a }); });
  createEffect(() => 1, () => { fb1({ read: a }); });
  return <div>x</div>;
}
