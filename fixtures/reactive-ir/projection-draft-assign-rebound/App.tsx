import { createProjection, createSignal } from "solid-js";

// The draft parameter rebound inside the callback: the write reaches what it was rebound to.
export function useMeta(source: () => { status: string }, target: { status: string }) {
  return createProjection(
    (draft: { status: string }) => {
      draft = target;
      Object.assign(draft, source());
    },
    { status: "pending" }
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
