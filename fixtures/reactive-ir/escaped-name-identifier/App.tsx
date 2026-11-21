import { createSignal } from "solid-js";

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  const al\u0069as = s;
  return <div>{alias()}</div>;
}
