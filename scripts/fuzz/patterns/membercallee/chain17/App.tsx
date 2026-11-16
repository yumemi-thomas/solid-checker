import { createMemo } from "solid-js";
import { chain0 } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { chain0(() => { slow(); }); return 1; })()}</p>;
}
