import { createSignal, mapArray } from "solid-js";

const [list] = createSignal([1, 2]);
const [tag] = createSignal("a");

// A project helper forwards its map function: the literal below is handed to
// project code, so it is not placed at the call (ADR 0442's carrier rules).
function rowsOf<T, U>(items: () => T[], map: (item: T) => U) {
  return mapArray(items, map);
}

export function Wrapped() {
  const rows = rowsOf(list, (item) => `${item}${tag()}`);
  return <div>{rows()}</div>;
}
