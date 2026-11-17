import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function WrittenAfter() {
  const t = { then(f: () => void) { f(); } };
  t.then = (f: () => void) => { queueMicrotask(f); };
  (t as unknown as Promise<void>).then(() => { setW(5); });
  return <p />;
}
