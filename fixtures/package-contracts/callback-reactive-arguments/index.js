import { createSignal } from "solid-js";

export function mapValue(mapFn) {
  const [getItem] = createSignal(1);
  return mapFn(1, getItem);
}
