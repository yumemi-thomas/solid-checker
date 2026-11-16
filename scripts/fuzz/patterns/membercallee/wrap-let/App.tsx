import { createMemo } from "solid-js";
import * as H from "./helpers";
import { createLoadingBoundary } from "solid-js/internal";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
let wrapLet = (cb: () => unknown) => {
  createLoadingBoundary(() => cb(), () => 0);
  return 1;
};
wrapLet = (cb: () => unknown) => {
  cb();
  return 2;
};
export function App() {
  return <p>{(() => { wrapLet(() => { slow(); }); return 1; })()}</p>;
}
