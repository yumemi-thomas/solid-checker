import { createMemo } from "solid-js";
import { returned } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { returned(() => { slow(); })(); return 1; })()}</p>;
}
