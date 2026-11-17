import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function NotConst() {
  let t = { then(f: () => void) { f(); } };
  (t as unknown as Promise<void>).then(() => { setW(9); });
  return <p />;
}
