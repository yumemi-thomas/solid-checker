import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function Getter() {
  const t = { get then() { return (f: () => void) => { f(); }; } };
  (t as unknown as Promise<void>).then(() => { setW(2); });
  return <p />;
}
