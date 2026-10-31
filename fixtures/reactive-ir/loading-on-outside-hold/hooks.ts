import { createMemo, createSignal } from "solid-js";

// A hook another module default-exports: its memo is async.
export default function useVals() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return { t, setT, a };
}
