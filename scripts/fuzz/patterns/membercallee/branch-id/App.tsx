import { createMemo } from "solid-js";
import { branch } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { branch(() => { slow(); }, false); return 1; })()}</p>;
}
