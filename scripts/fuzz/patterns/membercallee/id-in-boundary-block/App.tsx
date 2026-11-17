import { createMemo } from "solid-js";
import { inBoundary } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { inBoundary(() => { slow(); }); return 1; })()}</p>;
}
