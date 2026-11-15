import { createSignal } from "solid-js";
import { parse } from "dep-plain";

export function Plain() {
  const parsed = parse("a");
  const [size] = createSignal(parsed.value);
  return <p>{size()}</p>;
}
