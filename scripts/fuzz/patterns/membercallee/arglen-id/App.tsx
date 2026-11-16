import { createMemo } from "solid-js";
import { argLen } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { argLen(() => { slow(); }); return 1; })()}</p>;
}
