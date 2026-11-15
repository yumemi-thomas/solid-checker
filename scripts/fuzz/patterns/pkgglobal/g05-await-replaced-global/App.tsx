import { createMemo, createSignal, Loading } from "solid-js";
import { mock } from "fz-mock";

const [flag] = createSignal(1);
mock(flag);
const pause = () => new Promise<void>((r) => setTimeout(r, 5));

// The package replaced `structuredClone` and calls the accessor it was handed: the call after the await reads `flag` untracked.
export function App() {
  const m = createMemo(async () => { await pause(); return structuredClone(1); });
  return <Loading fallback="w"><p>{String(m())}</p></Loading>;
}
