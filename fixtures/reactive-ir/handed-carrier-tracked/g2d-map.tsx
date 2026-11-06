import { createMemo } from "solid-js";
export function App() {
  const a = createMemo(async () => 1);
  const hf = () => a();
  return <p>{[1].map(hf)}</p>;
}
