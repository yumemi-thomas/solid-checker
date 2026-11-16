import { createMemo } from "solid-js";
import { forwardBoundary } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { forwardBoundary(() => { slow(); }); return 1; })()}</p>;
}
