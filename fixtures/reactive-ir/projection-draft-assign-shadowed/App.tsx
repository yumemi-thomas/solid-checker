import { createSignal } from "solid-js";

// A local function of the same name hands its callback whatever it was given.
function createProjection<T extends object>(fn: (draft: T) => void, seed: T): T {
  fn(seed);
  return seed;
}

export function useMeta(source: () => { status: string }, target: { status: string }) {
  return createProjection(
    (draft: { status: string }) => {
      Object.assign(draft, source());
    },
    target
  );
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
