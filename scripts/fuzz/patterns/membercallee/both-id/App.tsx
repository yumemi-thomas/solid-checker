import { createMemo } from "solid-js";
import * as H from "./helpers";
import { both } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { both(() => { slow(); }); return 1; })()}</p>;
}
