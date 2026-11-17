import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
declare function keep(value: unknown): void;

export function Escaped() {
  const t = { then(f: () => void) { f(); } };
  keep(t);
  (t as unknown as Promise<void>).then(() => { setW(8); });
  return <p />;
}
