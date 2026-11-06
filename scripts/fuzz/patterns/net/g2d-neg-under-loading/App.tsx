import { createMemo, Loading } from "solid-js";
export function App() {
  const a = createMemo(async () => 1);
  const hf = () => a();
  return <Loading fallback={0}><p>{[1].map(hf)}</p></Loading>;
}
