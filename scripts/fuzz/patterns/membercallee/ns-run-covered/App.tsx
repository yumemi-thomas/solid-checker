import { Loading, createMemo } from "solid-js";
import * as H from "./helpers";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
function Inner() {
  return <p>{(() => { H.run(() => { slow(); }); return 1; })()}</p>;
}
export function App() {
  return <Loading fallback="..."><Inner /></Loading>;
}
