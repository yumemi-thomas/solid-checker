import { createSignal } from "solid-js";

// The compiled shape of `@tanstack/solid-start-server` 2.0.0-rc.9 `StartServer.js` (ADR 1045).
var _m$ = Symbol();
function _P$(this: Record<symbol, unknown>, _p: unknown) {
  this[_m$] = _p;
}

export function hold(props: unknown) {
  return new (_P$ as unknown as new (p: unknown) => object)(props);
}

// A standard root goes on as a value.
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
