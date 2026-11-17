import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
declare const key: string;

export function UnnamedAfter() {
  const t = { then(f: () => void) { f(); }, [key]: () => {} };
  (t as unknown as Promise<void>).then(() => { setW(4); });
  return <p />;
}
