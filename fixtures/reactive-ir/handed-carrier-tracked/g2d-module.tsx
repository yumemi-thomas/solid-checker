import { createMemo } from "solid-js";
const m = createMemo(async () => 1);
const hf = () => m();
export function App() {
  return <p>{[1].map(hf)}</p>;
}
