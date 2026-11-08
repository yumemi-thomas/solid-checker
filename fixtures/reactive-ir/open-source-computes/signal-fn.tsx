import { createMemo, createSignal } from "solid-js";
export function SignalFn() {
  const a = createMemo(async () => { await 0; return 1; });
  const [s] = createSignal(() => a());
  return <time>{s()}</time>;
}
