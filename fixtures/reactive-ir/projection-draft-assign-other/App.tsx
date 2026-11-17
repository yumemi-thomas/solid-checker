import { createMemo, createSignal } from "solid-js";

// Another primitive's callback parameter: createMemo hands its compute the previous value, which may be any object.
export function useMeta(source: () => { status: string }) {
  return createMemo((previous?: { status: string }) => {
    if (previous) Object.assign(previous, source());
    return source();
  });
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
