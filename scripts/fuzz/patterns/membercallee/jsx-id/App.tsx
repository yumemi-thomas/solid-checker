import { createMemo } from "solid-js";
import { jsxBoundary } from "./jsxhelpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { jsxBoundary(() => { slow(); return "x"; }); return 1; })()}</p>;
}
