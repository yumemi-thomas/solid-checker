import { createSignal } from "solid-js";

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  const options = { "ke\u0079": 1 };
  void options;
  return <div>{s()}</div>;
}
