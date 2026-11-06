import { createMemo } from "solid-js";
const a = createMemo(() => 7);
export function App() {
  setTimeout(() => { a(); }, 0);
  return <i>app</i>;
}
