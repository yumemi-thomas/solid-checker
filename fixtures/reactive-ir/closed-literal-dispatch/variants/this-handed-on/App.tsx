import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
declare function keep(value: unknown): void;

export function ThisHandedOn() {
  const t = { then(f: () => void) { keep(this); f(); } };
  (t as unknown as Promise<void>).then(() => { setW(7); });
  return <p />;
}
