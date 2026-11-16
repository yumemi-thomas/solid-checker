import { createMemo } from "solid-js";
import * as H from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { H.maybe(() => { slow(); }, true); return 1; })()}</p>;
}
