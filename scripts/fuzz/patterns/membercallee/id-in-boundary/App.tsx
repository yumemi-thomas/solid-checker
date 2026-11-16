import { createMemo } from "solid-js";
import { exprArrow } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { exprArrow(() => { slow(); }); return 1; })()}</p>;
}
