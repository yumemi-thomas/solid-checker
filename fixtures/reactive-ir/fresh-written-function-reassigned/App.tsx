import { createSignal } from "solid-js";

// A `let` bound to a function expression, then assigned the caller's object before the write.
export function toAction(target: unknown, key: string, value: unknown) {
  let fn: any = function () {};
  fn = target;
  fn[key] = value;
  return fn;
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
