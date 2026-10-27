import { createSignal } from "solid-js";

const [tag] = createSignal("a");

// Negative: a local function of the same name is not Solid's `mapArray`.
function mapArray<T, U>(items: () => T[], map: (item: T) => U): () => U[] {
  return () => items().map(map);
}

export function Local() {
  const rows = mapArray(() => [1, 2], (item) => `${item}${tag()}`);
  return <div>{rows()}</div>;
}
