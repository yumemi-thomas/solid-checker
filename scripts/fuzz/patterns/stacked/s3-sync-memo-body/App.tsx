import { createMemo } from "solid-js";
export function App() {
  createMemo(async () => 1, { sync: true });
  return <i>app</i>;
}
