import { createMemo } from "solid-js";
import { visitBoundary } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { visitBoundary({ v: 1 }, () => { slow(); }); return 1; })()}</p>;
}
