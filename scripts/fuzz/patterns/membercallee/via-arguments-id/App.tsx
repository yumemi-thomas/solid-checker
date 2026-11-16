import { createMemo } from "solid-js";
import { viaArguments } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { viaArguments(() => { slow(); }); return 1; })()}</p>;
}
