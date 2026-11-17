import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function Alias() {
  const t = { then(f: () => void) { f(); } };
  const q = t as unknown as Promise<void>;
  q.then(() => { setW(11); });
  return <p />;
}
