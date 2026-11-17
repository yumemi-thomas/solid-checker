import { createMemo } from "solid-js";
import util from "./util";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { util.run(() => { slow(); }); return 1; })()}</p>;
}
