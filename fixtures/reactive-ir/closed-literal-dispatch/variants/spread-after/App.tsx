import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
declare const extra: object;

export function SpreadAfter() {
  const t = { then(f: () => void) { f(); }, ...extra };
  (t as unknown as Promise<void>).then(() => { setW(3); });
  return <p />;
}
