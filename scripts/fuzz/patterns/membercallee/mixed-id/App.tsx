import { createMemo } from "solid-js";
import { mixed } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { mixed(() => { slow(); }); return 1; })()}</p>;
}
