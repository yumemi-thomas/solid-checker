import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function Assigned() {
  const t = { then(f: () => void) { f(); } };
  Object.assign(t, { then: (f: () => void) => { queueMicrotask(f); } });
  (t as unknown as Promise<void>).then(() => { setW(1); });
  return <p />;
}
