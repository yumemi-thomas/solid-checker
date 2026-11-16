import { createMemo } from "solid-js";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  const bus = { run(cb: () => unknown) { cb(); return 1; } };
  return <p>{(() => { bus.run(() => { slow(); }); return 1; })()}</p>;
}
