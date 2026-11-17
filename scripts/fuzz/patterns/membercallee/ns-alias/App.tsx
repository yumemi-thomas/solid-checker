import { createMemo } from "solid-js";
import * as H from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
const K = H;
export function App() {
  return <p>{(() => { K.run(() => { slow(); }); return 1; })()}</p>;
}
