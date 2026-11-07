import { createMemo } from "solid-js";
export function App() {
  const m1 = createMemo(async () => 1, { sync: true });
  return <i>{String(m1())}</i>;
}
