import { createProjection, createSignal } from "solid-js";

// ADR 1053: @tanstack/solid-query 6.0.0-rc.5's metaProjection: createProjection hands its callback the projection's draft.
export function useMeta(source: () => { status: string }) {
  return createProjection(
    (draft: { status: string }) => {
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
