import { createMemo } from "solid-js";
import * as H from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { H.forward(() => { slow(); }); return 1; })()}</p>;
}
