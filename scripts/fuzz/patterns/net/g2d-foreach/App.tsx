import { createMemo } from "solid-js";
export function App() {
  const a = createMemo(async () => 1);
  const hf = () => { a(); };
  return <p>{(() => { [1].forEach(hf); return 1; })()}</p>;
}
