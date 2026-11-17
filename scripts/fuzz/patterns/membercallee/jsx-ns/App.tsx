import { createMemo } from "solid-js";
import * as J from "./jsxhelpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { J.jsxBoundary(() => { slow(); return "x"; }); return 1; })()}</p>;
}
