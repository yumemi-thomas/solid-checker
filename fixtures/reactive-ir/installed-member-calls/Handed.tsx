import { createSignal } from "solid-js";

// The operation handed on: any object may be the one it lands on.
export function HandedOn() {
  const [, setW] = createSignal(0);
  const t: any = {};
  const install = (op: typeof Object.defineProperty) => op(t, "a", { set: setW });
  install(Object.defineProperty);
  t.a = 7;
  return <i />;
}
