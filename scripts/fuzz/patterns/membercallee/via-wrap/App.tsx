import { createMemo } from "solid-js";
import * as H from "./helpers";
import { createLoadingBoundary } from "solid-js/internal";
import { ignore } from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
const wrapped = ignore((cb: () => unknown) => {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
});
export function App() {
  return <p>{(() => { wrapped(() => { slow(); }); return 1; })()}</p>;
}
