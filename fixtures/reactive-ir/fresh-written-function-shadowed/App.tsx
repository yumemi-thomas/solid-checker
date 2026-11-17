import { createSignal } from "solid-js";

function query() {}

// The write goes through a `let` that shadows the declaration and holds the caller's object.
export function patch(target: unknown, key: string, value: unknown) {
  let query: any = target;
  query[key] = value;
  return query;
}

export function own() {
  return query;
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
